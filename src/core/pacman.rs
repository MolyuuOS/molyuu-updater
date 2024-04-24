use std::cell::RefCell;
use std::collections::HashMap;
use std::error::Error;
use std::rc::Rc;

use alpm::{DownloadEvent, Progress, TransFlag};
use crossbeam_utils::atomic::AtomicCell;
use r2d2::Pool;
use r2d2_alpm::AlpmManager;

pub static ALPM_HELD_LOCK: AtomicCell<bool> = AtomicCell::new(false);

struct ProgressTracker {
    download_map: HashMap<String, (usize, usize)>,
    install_map: HashMap<String, usize>,
    download_finished: bool,
    total_num: usize,
}

impl ProgressTracker {
    pub fn new(total_num: usize) -> Self {
        Self {
            download_map: HashMap::new(),
            install_map: HashMap::new(),
            download_finished: false,
            total_num,
        }
    }

    pub fn update_download_progress(&mut self, filename: &str, downloaded: usize, total: usize) {
        if self.download_map.contains_key(filename) {
            let map = self.download_map.get_mut(filename).unwrap();
            *map = (downloaded, total);
        } else {
            self.download_map.insert(filename.to_string(), (downloaded, total));
        }
    }

    pub fn update_install_progress(&mut self, package_name: &str, percent: usize) {
        self.download_finished = true;
        if self.install_map.contains_key(package_name) {
            let map = self.install_map.get_mut(package_name).unwrap();
            *map = percent;
        } else {
            self.install_map.insert(package_name.to_string(), percent);
        }
    }

    pub fn get_current_progress(&self) -> usize {
        let mut current = 0.0;
        for (_, (downloaded, total)) in self.download_map.iter() {
            current += ((*downloaded as f64 / *total as f64) * 100.0) / (self.total_num as f64 * 2.0);
        }

        if self.download_finished {
            if current < 50.0 {
                current = 50.0
            }
            for (_, percent) in self.install_map.iter() {
                current += *percent as f64 / (self.total_num as f64 * 2.0);
            }
        }
        let progress = current as usize;

        if progress > 100 {
            100
        } else {
            progress
        }
    }
}

pub struct Pacman {
    pool: Pool<AlpmManager>,
    steam_progress: bool,
}

impl Pacman {
    pub fn new(steam_progress: bool) -> Result<Self, Box<dyn Error>> {
        // Check if we have root permission
        if unsafe { libc::geteuid() } != 0 {
            Err(Box::from("Permission Denied"))
        } else {
            Ok(Self {
                pool: Pool::builder().max_size(4).build(AlpmManager::from_file("/etc/pacman.conf")?)?,
                steam_progress,
            })
        }
    }

    pub fn check_updates(&mut self) -> Result<usize, Box<dyn Error>> {
        let mut handle = self.pool.get()?;
        handle.alpm.syncdbs_mut().update(false)?;
        handle.alpm.trans_init(TransFlag::DB_ONLY)?;
        ALPM_HELD_LOCK.store(true);
        handle.alpm.sync_sysupgrade(false)?;
        handle.alpm.trans_prepare().unwrap();
        let package_changes = handle.alpm.trans_add().len();
        handle.alpm.trans_release()?;
        ALPM_HELD_LOCK.store(false);
        Ok(package_changes)
    }

    pub fn update_system(&mut self) -> Result<(), Box<dyn Error>> {
        let mut handle = self.pool.get()?;
        handle.alpm.syncdbs_mut().update(false)?;
        handle.alpm.trans_init(TransFlag::NONE)?;
        ALPM_HELD_LOCK.store(true);
        handle.alpm.sync_sysupgrade(false)?;
        handle.alpm.trans_prepare().unwrap();
        let to_install = handle.alpm.trans_add();
        let progress_tracker = Rc::new(RefCell::new(ProgressTracker::new(to_install.len())));
        handle.alpm.set_dl_cb((progress_tracker.clone(), self.steam_progress), |filename, event, data| {
            let progress_tracker: &RefCell<ProgressTracker> = &*data.0;
            let mut progress_tracker = progress_tracker.borrow_mut();
            if filename.ends_with(".tar.zst") {
                if let DownloadEvent::Progress(progress) = event.event() {
                    progress_tracker.update_download_progress(filename, progress.downloaded as usize, progress.total as usize);
                    if data.1 {
                        println!("{}%", progress_tracker.get_current_progress());
                    } else {
                        println!("Downloading {}: {}%", filename, progress_tracker.get_current_progress());
                    }
                }
            }
        });

        handle.alpm.set_progress_cb((progress_tracker.clone(), self.steam_progress), |progress, package_name, percent, howmany, current, data| {
            let progress_tracker: &RefCell<ProgressTracker> = &*data.0;
            let mut progress_tracker = progress_tracker.borrow_mut();
            if progress == Progress::AddStart || progress == Progress::ReinstallStart || progress == Progress::RemoveStart || progress == Progress::UpgradeStart {
                progress_tracker.update_install_progress(package_name, percent as usize);
                if data.1 {
                    println!("{}%", progress_tracker.get_current_progress());
                } else {
                    println!("Installing {}: {}%, Total Progress: {}%", package_name, percent, progress_tracker.get_current_progress());
                }
            }
        });


        match handle.alpm.trans_commit() {
            Ok(_) => {
                handle.alpm.trans_release()?;
            }
            Err(err) => {
                println!("{:#?}", err);
                handle.alpm.trans_release()?;
            }
        }
        ALPM_HELD_LOCK.store(false);

        Ok(())
    }
}
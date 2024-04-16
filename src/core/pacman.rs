use std::process::Command;

use crate::core::pacman::PacmanError::{DatabaseSyncFailed, PermissionDenied, SystemUpgradeFailed};

#[derive(Debug)]
pub enum PacmanError {
    PermissionDenied,
    DatabaseSyncFailed,
    SystemUpgradeFailed,
}

#[derive(Debug)]
pub struct PackageUpdate {
    package_name: String,
    current_version: String,
    new_version: String,
}

#[derive(Debug)]
pub struct Pacman {
    update_list: Vec<PackageUpdate>,
    steam_progress: bool,
}

impl Pacman {
    pub fn new(steam_progress: bool) -> Result<Self, PacmanError> {
        // Check if we have root permission
        if unsafe { libc::geteuid() } != 0 {
            Err(PermissionDenied)
        } else {
            Ok(Self {
                update_list: Vec::new(),
                steam_progress,
            })
        }
    }

    pub fn sync_database(&self) -> Result<(), PacmanError> {
        let output = Command::new("pacman")
            .arg("-Syy")
            .arg("--noconfirm")
            .output().unwrap();

        if !output.status.success() {
            Err(DatabaseSyncFailed)
        } else {
            Ok(())
        }
    }

    pub fn check_updates(&mut self) -> Result<usize, PacmanError> {
        self.sync_database()?;

        let output = Command::new("checkupdates")
            .arg("--nocolor")
            .output().unwrap();

        if output.status.success() {
            let output = String::from_utf8(output.stdout).unwrap();
            for line in output.lines() {
                let parts: Vec<&str> = line.split(" ").collect();
                self.update_list.push(PackageUpdate {
                    package_name: parts[0].to_string(),
                    current_version: parts[1].to_string(),
                    new_version: parts[3].to_string(),
                })
            }
        }
        Ok(self.update_list.len())
    }

    pub fn update_system(&mut self) -> Result<(), PacmanError> {
        for idx in 0..self.update_list.len() {
            if self.steam_progress {
                println!("{}%", ((idx + 1) * 100) / self.update_list.len());
            } else {
                println!("Updating {} [{}/{}]: ", self.update_list[idx].package_name, idx + 1, self.update_list.len());
            }
            let output = Command::new("pacman")
                .args(["-S", "--noconfirm", self.update_list[idx].package_name.as_str()])
                .output().unwrap();

            if !output.status.success() {
                return Err(SystemUpgradeFailed);
            }
        }

        Ok(())
    }

    pub fn get_update_list(&self) -> &Vec<PackageUpdate> {
        &self.update_list
    }
}
use std::process::exit;

use clap::{Arg, ArgAction, Command};

use crate::core::pacman::Pacman;

mod core;

fn cli() -> Command {
    Command::new("MolyuuOS Updater")
        .arg(Arg::new("supports-duplicate-detection")
            .long("supports-duplicate-detection")
            .action(ArgAction::SetTrue)
            .help("Dummy argument for Steam compatability, just return 1 before exit."))
        .arg(Arg::new("enable-duplicate-detection")
            .long("enable-duplicate-detection")
            .action(ArgAction::SetTrue)
            .help("Dummy argument for Steam compatability."))
        .arg(Arg::new("verbose-progress")
            .long("verbose-progress")
            .action(ArgAction::SetTrue)
            .help("Disable Steam Compatability Progress"))
        .subcommand(Command::new("check")
            .about("Check for update"))
}

fn main() {
    let matches = cli().get_matches();
    let supports_duplicate_detection = matches.get_one::<bool>("supports-duplicate-detection").unwrap_or(&false);
    let enable_duplicate_detection = matches.get_one::<bool>("enable-duplicate-detection").unwrap_or(&false);
    let verbose_progress = matches.get_one::<bool>("verbose-progress").unwrap_or(&false);
    let mut exit_code: i32 = 7;
    let mut pacman = Pacman::new(!verbose_progress).unwrap();

    match matches.subcommand() {
        Some(("check", _)) => {
            pacman.sync_database().unwrap();
            let num = pacman.check_updates().unwrap();
            if num > 0 {
                exit_code = 0;
            }
        }
        _ => {
            if *supports_duplicate_detection {
                exit_code = 1;
            } else {
                pacman.sync_database().unwrap();
                let num = pacman.check_updates().unwrap();
                if num == 0 {
                    exit_code = 7;
                } else {
                    pacman.update_system().unwrap();
                    exit_code = 0;
                }
            }
        }
    }

    exit(exit_code)
}

/*
    This is a small utility for organizing my photo library, acting as a wrapper around 'exiftool'.

    Copyright 2023 Seth Pendergrass. See LICENSE.
*/
use clap::{ArgAction, Parser, Subcommand};
use env_logger::Builder;
use log::LevelFilter;
use std::fs;
use std::io::Write;
use std::path::PathBuf;

mod config;
mod file;
mod import;
mod metadata;
mod process;
mod scan;

#[derive(Parser)]
struct Args {
    /// Directory of photo library. Updates default in XDG_CONFIG_HOME.
    #[arg(long, short)]
    library: Option<PathBuf>,

    /// Whether to apply changes. "Deleted" files will be moved to `trash` under the library
    /// directory.
    #[arg(long, short)]
    apply_changes: bool,

    /// Enable Debug and Trace logs.
    #[arg(long, short, action = ArgAction::Count)]
    verbose: u8,

    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Validate the entire photo library. This optionally applies fixes as possible.
    Scan,
    /// Validate and organize the photos in `imports` under the library directory.
    Import,
}

// Sets up env_logger, with the formatting "ERROR_LEVEL message" (e.g. "WARN something went wrong").
fn enable_logging(verbose: u8) {
    let level = match verbose {
        0 => LevelFilter::Warn,
        1 => LevelFilter::Info,
        2 => LevelFilter::Debug,
        _ => LevelFilter::Trace,
    };

    Builder::new()
        .filter_level(level)
        .format(|buf, record| {
            writeln!(
                buf,
                "{} {}",
                buf.default_level_style(record.level())
                    .value(record.level()),
                record.args()
            )
        })
        .init();
}

// Get library root from provided arg, if present, and write to ~/.config/photo_manager.
// Else, read library root from ~/.config/photo_manager.
fn get_library_root(library_arg: Option<PathBuf>) -> Option<PathBuf> {
    let Ok(xdg_dirs) = xdg::BaseDirectories::new() else {
        log::error!("Failed to get XDG directories.");
        return None;
    };

    let config_path = xdg_dirs.get_config_file("photo_manager");

    // New library path specified, so update config file.
    if let Some(path) = library_arg {
        let Some(path_str) = path.to_str() else {
            log::error!("Non-UTF-8 library path.");
            return None;
        };

        if !path.is_dir() {
            log::error!("Invalid library path specified.");
            return None;
        }

        if fs::write(config_path, path_str).is_err() {
            log::error!("Failed to write library path.");
            return None;
        }

        Some(path)

    // Read from existing config file as no path provided.
    } else {
        let Ok(path_str) = fs::read_to_string(config_path) else {
            log::error!("Failed to read library path from config file.");
            return None;
        };

        Some(PathBuf::from(path_str))
    }
}

fn main() {
    let args = Args::parse();
    enable_logging(args.verbose);

    if let Some(library_root) = get_library_root(args.library) {
        match args.command {
            Commands::Scan => scan::scan(&library_root, args.apply_changes),
            Commands::Import => import::import(&library_root, args.apply_changes),
        }
    }
}

//! Program setup functions.
//!
//! Copyright 2023-4 Seth Pendergrass. See LICENSE.

use env_logger::Builder;
use log::LevelFilter;
use std::fs;
use std::io::Write;
use std::path::PathBuf;

/// Sets up env_logger with the format "ERROR_LEVEL message" (e.g. "WARN something went wrong").
///
/// Log levels:
/// Error: Program errors.
/// Warn: File removal.
/// Info: General program flow and non-removal file operations.
/// Debug: Detailed file operations.
/// Trace: Exiftool output.
pub fn configure_logging(verbosity: u8) {
    let level = match verbosity {
        0 => LevelFilter::Info,
        1 => LevelFilter::Debug,
        _ => LevelFilter::Trace,
    };

    Builder::new()
        .filter_level(level)
        .format(|buf, record| {
            writeln!(
                buf,
                "{}\t{}",
                buf.default_level_style(record.level())
                    .value(record.level()),
                record.args()
            )
        })
        .init();
}

/// Get library root from provided arg, if present, and write to XDG_CONFIG_HOME/imlib.
/// Else, read library root from XDG_CONFIG_HOME/imlib.
pub fn get_or_update_library(path: Option<PathBuf>) -> Result<PathBuf, &'static str> {
    let xdg_dirs = xdg::BaseDirectories::new().map_err(|_| "Failed to get XDG directories.")?;
    let config_path = xdg_dirs.get_config_file("imlib");

    match path {
        Some(path) => {
            if !path.is_dir() {
                return Err("Library path is not a directory.");
            }
            fs::write(config_path, path.to_str().ok_or("Invalid library path.")?)
                .map_err(|_| "Failed to write library path.")?;
            Ok(path)
        }
        None => Ok(PathBuf::from(
            fs::read_to_string(config_path).map_err(|_| "Failed to read library path.")?,
        )),
    }
}

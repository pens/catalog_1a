// Copyright 2023-5 Seth Pendergrass. See LICENSE.

//! Program setup functions.

use std::{fs, io::Write, path::PathBuf};

use env_logger::Builder;
use log::LevelFilter;

/// Sets up `env_logger` with the format "`ERROR_LEVEL` message" (e.g. "WARN
/// something went wrong").
///
/// Log levels:
/// Error: Program errors.
/// Warn:  File removal and issues preventing work.
/// Info:  General program flow.
/// Debug: Per-file operations.
/// Trace: Detailed per-file operations.
pub fn configure_logging(verbosity: u8) {
  let level = match verbosity {
    0 => LevelFilter::Info,
    1 => LevelFilter::Debug,
    _ => LevelFilter::Trace,
  };

  Builder::new()
    .filter_level(level)
    .format(|f, r| writeln!(f, "{}\t{}", f.default_level_style(r.level()), r.args()))
    .init();
}

/// Get catalog root from `path`, if present, and write to
/// `XDG_CONFIG_HOME/catalog_1a`. Else, read catalog root path from
/// `XDG_CONFIG_HOME/catalog_1a`.
pub fn get_or_update_catalog_path(path: Option<PathBuf>) -> Result<PathBuf, String> {
  let xdg_dirs = xdg::BaseDirectories::new();
  let config_path = xdg_dirs
    .get_config_file(env!("CARGO_PKG_NAME"))
    .ok_or("Failed to get XDG directories.")?;

  match path {
    Some(path) => {
      if !path.is_dir() {
        return Err("Library path is not a directory.".to_string());
      }
      fs::write(config_path, path.to_str().ok_or("Invalid catalog path.")?)
        .map_err(|_| "Failed to write catalog path.")?;
      Ok(path)
    }
    None => Ok(PathBuf::from(
      fs::read_to_string(config_path)
        .map_err(|_| "Failed to read catalog path.")?
        .trim(),
    )),
  }
}

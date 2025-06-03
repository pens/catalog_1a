// Copyright 2023-5 Seth Pendergrass. See LICENSE.

//! This is a program for organizing my photo catalog, acting as a wrapper
//! around `ExifTool`.

#![feature(path_add_extension)]

mod commands;
mod io;
mod org;
mod prim;
mod setup;
#[cfg(test)]
mod testing;

use std::path::PathBuf;

use clap::{ArgAction, Parser, Subcommand};

/// Command-line arguments.
#[derive(Parser)]
struct Args {
  /// Directory of multimedia catalog. Updates default in `XDG_CONFIG_HOME`.
  #[arg(short, global = true)]
  catalog: Option<PathBuf>,

  /// Verbosity level. Max: 2.
  #[arg(short, action = ArgAction::Count, global = true)]
  verbose: u8,

  /// Function to run.
  #[command(subcommand)]
  command: Commands,
}

/// Main functions.
#[derive(Subcommand)]
enum Commands {
  /// Clean catalog.
  Org,
  /// Import photos from path into the catalog.
  Import { path: PathBuf },
}

fn run() -> Result<(), String> {
  commands::exiftool_check()?;

  let args = Args::parse();

  setup::configure_logging(args.verbose);

  let catalog = setup::get_or_update_catalog_path(args.catalog)?;

  match args.command {
    Commands::Org => commands::org(&catalog),
    Commands::Import { path } => commands::import(&catalog, &path),
  }
}

fn main() {
  if let Err(e) = run() {
    log::error!("{e}");
    std::process::exit(1);
  };
}

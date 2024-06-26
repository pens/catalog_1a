//! Program subcommands for managing photo/video catalog.
//!
//! Copyright 2023-4 Seth Pendergrass. See LICENSE.

use std::path::Path;

use crate::org::Organizer;

/// Scans all files under library, performing various cleanup tasks. This will move files that
/// are to be deleted to library/.trash.
pub fn org(library: &Path) {
    log::info!("Cleaning {}.", library.display());

    let mut organizer = Organizer::load_library(library, &library.to_path_buf().join(".trash"));
    organizer.remove_live_photo_duplicates();
    organizer.remove_leftover_live_photo_videos();
    organizer.remove_leftover_sidecars();
    organizer.synchronize_live_photo_metadata();
    organizer.validate_tags();
    organizer.create_missing_sidecars();
    organizer.move_and_rename_files(library);
}

/// Performs cleanup on import` and then moves all *good* files to `library. Other files will
/// remain in place.
pub fn import(library: &Path, import: &Path) {
    log::info!("Importing {} into {}.", import.display(), library.display());

    let mut organizer = Organizer::import(import);
    organizer.remove_live_photo_duplicates();
    organizer.remove_leftover_live_photo_videos();
    organizer.remove_leftover_sidecars();
    organizer.synchronize_live_photo_metadata();
    organizer.validate_tags();
    organizer.create_missing_sidecars();
    organizer.move_and_rename_files(library);
}

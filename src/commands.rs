//! Program subcommands for managing photo/video catalog.
//!
//! Copyright 2023-4 Seth Pendergrass. See LICENSE.

use std::path::Path;

use crate::catalog::CatalogManager;

/// Scans all files under `library`, performing various cleanup tasks. This will move files that
/// are to be deleted to `library/.trash`.
pub fn clean(library: &Path) {
    log::info!("Cleaning {}.", library.display());

    let mut catalog = CatalogManager::new(library, &library.to_path_buf().join(".trash"), true);
    catalog.remove_duplicates_from_live_photos();
    catalog.remove_videos_from_deleted_live_photos();
    catalog.copy_metadata_from_live_photo_image_to_video();
    catalog.remove_sidecars_without_references();
    catalog.create_xmp_sidecars_if_missing();
    // catalog.move_files_and_rename_empties_catalog(library);
}

/// Performs cleanup on `import` and then moves all *good* files to `library`. Other files will
/// remain in place.
pub fn import(library: &Path, import: &Path) {
    log::info!("Importing {} into {}.", import.display(), library.display());

    // TODO should set trash path equal to source.
    let mut catalog = CatalogManager::new(import, &library.to_path_buf().join(".trash"), false);
    catalog.remove_duplicates_from_live_photos();
    // Don't bother removing videos from deleted Live Photos, since we're importing.
    catalog.copy_metadata_from_live_photo_image_to_video();
    catalog.remove_sidecars_without_references();
    catalog.create_xmp_sidecars_if_missing();
    // catalog.move_files_and_rename_empties_catalog(library);
}

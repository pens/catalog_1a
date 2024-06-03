//! Core catalog management type and functionality.
//!
//! Copyright 2023-4 Seth Pendergrass. See LICENSE.

use std::fs;
use std::path::{Path, PathBuf};

use super::catalog::Catalog;
use super::exiftool;
use super::live_photos::LivePhotoMapping;
use super::metadata::Metadata;
use super::sidecar::Sidecar;

pub struct CatalogManager {
    trash: Option<PathBuf>,
    catalog: Catalog,
    live_photo_mapping: LivePhotoMapping,
}

impl CatalogManager {
    //
    // Constructors.
    //

    /// Scans `import` for files to import into a catalog.
    pub fn import(import: &Path) -> Self {
        Self::new(import, None)
    }

    /// Loads an existing library for maintenance. Removed files will be moved to `trash`.
    /// Note: If `trash` lies within `library`, files within will not be scanned.
    pub fn load_library(library: &Path, trash: &Path) -> Self {
        Self::new(library, Some(trash))
    }

    //
    // Public.
    //

    /// Remove duplicate images or videos based on Live Photo `ContentIdentifier`. Most often, this
    /// is because a photo exists as both a JPG and HEIC.
    /// This will keep the newest file and remove the rest, preferring HEIC over JPG for images.
    pub fn remove_live_photo_duplicates(&mut self) {
        log::info!("Removing duplicates from Live Photos.");

        for (keep, duplicates) in self.live_photo_mapping.remove_duplicates(&self.catalog) {
            log::warn!(
                "{}: Live Photo has the following duplicates, deleting:",
                keep.display()
            );
            for path in duplicates {
                log::warn!(
                    "\t{}: Duplicate Live Photo image, removing.",
                    path.display()
                );
                self.remove_from_catalog(&path);
            }
        }
    }

    /// Removes any Live Photo videos without corresponding images. This is based on the
    /// presence and value of the `ContentIdentifier` tag.
    pub fn remove_leftover_live_photo_videos(&mut self) {
        log::info!("Removing videos from deleted Live Photos.");

        for path in self.live_photo_mapping.remove_leftover_videos() {
            log::warn!(
                "{}: Video remaining from presumably deleted Live Photo image.",
                path.display()
            );
            self.remove_from_catalog(&path);
        }
    }

    /// Remove sidecar files for which the expected source file does not exist.
    pub fn remove_leftover_sidecars(&mut self) {
        log::info!("Removing XMP sidecars without corresponding files.");

        for path in self.catalog.remove_leftover_sidecars() {
            log::warn!(
                "{}: XMP sidecar without corresponding media file.",
                path.display()
            );
            // TODO this needs to be live photo mapping
            self.remove_from_catalog(&path);
        }
    }

    /// Copy metadata from Live Photo images to videos.
    /// This keeps datetime, geotags, etc. consistent.
    pub fn synchronize_live_photo_metadata(&mut self) {
        log::info!("Copying metadata from Live Photo images to videos.");

        for (photos, videos) in self.live_photo_mapping.iter() {
            // If there are multiple images or videos, warn and skip.
            if photos.len() > 1 || videos.len() > 1 {
                log::warn!(
                    "{}: Live Photo can't synchronize metadata due to duplicates:",
                    photos[0].display()
                );
                for path in photos.iter().skip(1) {
                    log::warn!("\t{}: Duplicate Live Photo image", path.display());
                }
                for path in videos.iter() {
                    log::warn!("\t{}: Duplicate Live Photo video", path.display());
                }
                continue;
            }

            // Select metadata source.
            let source = self.catalog.get_primary_metadata_source(&photos[0]);

            // Collect metadata sinks.
            let sinks = self.catalog.get_metadata_sinks(&videos[0]);

            // Copy metadata.
            for sink in sinks {
                log::debug!(
                    "{} -> {}: Synchronizing metadata from Live Photo image.",
                    source.display(),
                    sink.display()
                );
                // TODO unify stdout
                let stdout = exiftool::copy_metadata(&source, &sink);
                let metadata = serde_json::from_slice::<Metadata>(&stdout[..]).unwrap();
                // TODO make this not require a path
                self.catalog.update(&metadata.source_file.clone(), metadata);
            }
        }
    }

    /// Check that all media files have expected metadata tags.
    /// If there are associated XMP files, they will be checked as well, however XMP files without
    /// referenced media files will *not* be checked.
    pub fn validate_tags(&self) {
        log::info!("Checking that all files have required tags.");

        self.catalog.validate_tags();
    }

    /// Ensures every file has an associated XMP sidecar, creating one if not already present.
    pub fn create_missing_sidecars(&mut self) {
        log::info!("Ensuring all media files have associated XMP sidecar.");

        for path in self.catalog.get_missing_sidecars() {
            log::debug!("{}: Creating XMP sidecar.", path.display());
            let path_new = exiftool::create_xmp(&path);
            assert_eq!(path, path_new);

            let stdout = exiftool::get_metadata(&path_new);
            let metadata = serde_json::from_slice::<Metadata>(&stdout[..]).unwrap();
            let sidecar = Sidecar::new(metadata);

            self.catalog.insert_sidecar(sidecar);
        }
    }

    /// Moves files into their final home in `destination`, based on their DateTimeOriginal tag, and
    /// changes their file extensions to match their format. This unifies extensions per file type
    /// (e.g. jpeg vs jpg) and fixes incorrect renaming of mov to mp4.
    ///
    /// TODO: until updates implemented, catalog dropped after return.
    pub fn move_and_rename_files(&mut self, destination: &Path) {
        log::info!("Moving and renaming files.");

        let mut updates = Vec::new();

        for media in self.catalog.iter_media() {
            let media_path = &media.metadata.source_file;
            log::debug!("{}: Moving & renaming.", media_path.display());

            // Prefer XMP metadata, if present.
            let source = self.catalog.get_primary_metadata_source(media_path);

            // Get DateTimeOriginal tag
            let metadata = self.catalog.get(media_path);
            if metadata.date_time_original.is_none() {
                log::warn!(
                    "{}: DateTimeOriginal tag not found. Skipping move & rename.",
                    media_path.display()
                );
                continue;
            }

            let media_file_ext = &metadata.file_type_extension;
            let media_file_rename_format = format!(
                "-FileName<{}/${{DateTimeOriginal}}.{}",
                destination.to_str().unwrap(),
                media_file_ext
            );
            let new_path = exiftool::rename_file(&media_file_rename_format, media_path, &source);
            log::debug!("{}: Moved to {}.", media_path.display(), new_path.display());

            // TODO maybe update function should just take new_path
            let stdout = exiftool::get_metadata(&new_path);
            let metadata = serde_json::from_slice::<Metadata>(&stdout[..]).unwrap();
            updates.push((media_path.clone(), metadata));

            for sidecar_path in self.catalog.get_sidecar_paths(media_path) {
                // Move XMP as well, keeping "file.ext.xmp" format.
                let xmp_rename_format = format!(
                    "-FileName<{}/${{DateTimeOriginal}}.{}.xmp",
                    destination.to_str().unwrap(),
                    media_file_ext
                );
                let new_sidecar_path =
                    exiftool::rename_file(&xmp_rename_format, &sidecar_path, &source);
                log::debug!(
                    "\tMoved XMP sidecar {} -> {}.",
                    sidecar_path.display(),
                    new_sidecar_path.display()
                );

                let stdout = exiftool::get_metadata(&new_path);
                let metadata = serde_json::from_slice::<Metadata>(&stdout[..]).unwrap();
                updates.push((sidecar_path.clone(), metadata));
            }
        }

        // TODO update live photo map first
        for (path_old, metadata) in updates {
            self.catalog.update(&path_old, metadata);
        }
    }

    //
    // Private.
    //

    /// Create a new catalog of library, with trash as the destination for removed files.
    fn new(directory: &Path, trash: Option<&Path>) -> Self {
        log::info!("Building catalog.");
        let stdout = exiftool::collect_metadata(directory, trash);
        let catalog = Catalog::new(stdout);

        log::info!("Building Live Photo image <-> video mapping.");
        let live_photo_mapping = LivePhotoMapping::new(&catalog);

        Self {
            trash: trash.map(|p| p.to_path_buf()),
            catalog,
            live_photo_mapping,
        }
    }

    /// Remove path from catalog, and if a media file, any dependent sidecars.
    /// If self.trash is Some(), moves files to trash.
    /// Note: This does *not* remove Live Photo mappings.
    fn remove_from_catalog(&mut self, path: &Path) {
        for path in self.catalog.remove(path) {
            if let Some(trash) = &self.trash {
                log::debug!("{}: Moving to trash.", path.display());

                let path_trash = trash.join(path.file_name().unwrap());
                assert!(
                    !path_trash.exists(),
                    "Cannot safely delete {} due to name collision in {}.",
                    path.display(),
                    trash.display()
                );
                fs::rename(path, path_trash).unwrap();
            }
        }
    }
}

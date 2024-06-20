//! Core catalog management type and functionality.
//!
//! Copyright 2023-4 Seth Pendergrass. See LICENSE.

use std::fs;
use std::path::{Path, PathBuf};

use super::catalog::Catalog;
use super::exiftool;
use super::file::FileHandle;
use super::live_photos::LivePhotoMapping;
use super::metadata::Metadata;
use super::sidecar::Sidecar;

pub struct CatalogManager {
    trash: Option<PathBuf>,
    catalog: Catalog,
    live_photo_mapping: LivePhotoMapping,
}

// TODO rename module & organize project structure
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
                "{}: Live Photo has the following duplicates, removing:",
                self.catalog.get_metadata(&keep).source_file.display()
            );
            for path in duplicates {
                log::warn!(
                    "\t{}",
                    self.catalog.get_metadata(&path).source_file.display()
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
                "{}: Video remaining from presumably deleted Live Photo image. Removing.",
                self.catalog.get_metadata(&path).source_file.display()
            );
            self.remove_from_catalog(&path);
        }
    }

    /// Remove sidecar files for which the expected source file does not exist.
    pub fn remove_leftover_sidecars(&mut self) {
        log::info!("Removing XMP sidecars without corresponding files.");

        for sidecar in self.catalog.remove_leftover_sidecars() {
            let path = sidecar.metadata.source_file;
            log::warn!(
                "{}: XMP sidecar without corresponding media file.",
                path.display()
            );
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
                    self.catalog.get_metadata(&photos[0]).source_file.display()
                );
                for path in photos.iter().skip(1) {
                    log::warn!(
                        "\t{}: Duplicate Live Photo image",
                        self.catalog.get_metadata(path).source_file.display()
                    );
                }
                for path in videos.iter() {
                    log::warn!(
                        "\t{}: Duplicate Live Photo video",
                        self.catalog.get_metadata(path).source_file.display()
                    );
                }
                continue;
            }

            // Select metadata source.
            let source = self.catalog.get_metadata_source_path(&photos[0]);

            // Collect metadata sinks.
            let sinks = self.catalog.get_media_sinks(&videos[0]);

            // Copy metadata.
            for (handle, sink) in sinks {
                log::debug!(
                    "{} -> {}: Synchronizing metadata from Live Photo image.",
                    source.display(),
                    sink.display()
                );
                // TODO: Make this one function call:
                let stdout = exiftool::copy_metadata(&source, &sink);
                let metadata = serde_json::from_slice::<Metadata>(&stdout[..]).unwrap();
                // end

                self.catalog.update(&handle, metadata);
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

            // TODO: This block should be one function call:
            let path_new = exiftool::create_xmp(&path);
            assert_eq!(path, path_new);
            let stdout = exiftool::get_metadata(&path_new);
            let metadata = serde_json::from_slice::<Metadata>(&stdout[..]).unwrap();
            // End

            let sidecar = Sidecar::new(metadata);

            self.catalog.insert_sidecar(sidecar);
        }
    }

    /// Moves files into their final home in `destination`, based on their DateTimeOriginal tag, and
    /// changes their file extensions to match their format. This unifies extensions per file type
    /// (e.g. jpeg vs jpg) and fixes incorrect renaming of mov to mp4.
    pub fn move_and_rename_files(&mut self, destination: &Path) {
        log::info!("Moving and renaming files.");

        let mut updates = Vec::new();

        for (handle, media) in self.catalog.iter_media() {
            let media_path = &media.metadata.source_file;
            log::debug!("{}: Moving & renaming.", media_path.display());

            // Prefer XMP metadata, if present.
            let source = self.catalog.get_metadata_source_path(handle);

            // Get DateTimeOriginal tag
            if media.metadata.date_time_original.is_none() {
                log::warn!(
                    "{}: DateTimeOriginal tag not found. Skipping move & rename.",
                    media_path.display()
                );
                continue;
            }

            let media_file_ext = &media.metadata.file_type_extension;
            let media_file_rename_format = format!(
                "-FileName<{}/${{DateTimeOriginal}}.{}",
                destination.to_str().unwrap(),
                media_file_ext
            );
            let new_path = exiftool::rename_file(&media_file_rename_format, media_path, &source);
            log::debug!("{}: Moved to {}.", media_path.display(), new_path.display());

            // TODO make single fn
            let stdout = exiftool::get_metadata(&new_path);
            let metadata = serde_json::from_slice::<Metadata>(&stdout[..]).unwrap();
            // end

            updates.push((*handle, metadata));

            for (sidecar_handle, sidecar_path) in self.catalog.get_media_sinks(handle) {
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

                // TODO: make single fn
                let stdout = exiftool::get_metadata(&new_path);
                let metadata = serde_json::from_slice::<Metadata>(&stdout[..]).unwrap();
                // end

                updates.push((sidecar_handle, metadata));
            }
        }

        for (handle, metadata) in updates {
            self.catalog.update(&handle, metadata);
        }
    }

    //
    // Private.
    //

    /// Create a new catalog of library, with trash as the destination for removed files.
    fn new(directory: &Path, trash: Option<&Path>) -> Self {
        log::info!("Building catalog.");

        // TODO: merge stdout into serde_json parsing within new()
        let stdout = exiftool::collect_metadata(directory, trash);
        let metadata = serde_json::from_slice::<Vec<Metadata>>(&stdout[..]).unwrap();
        let catalog = Catalog::new(metadata);
        // end

        log::info!("Building Live Photo image <-> video mapping.");
        let live_photo_mapping = LivePhotoMapping::new(&catalog);

        Self {
            trash: trash.map(|p| p.to_path_buf()),
            catalog,
            live_photo_mapping,
        }
    }

    /// Remove file_handle from catalog, and if a media file, any dependent sidecars.
    /// If self.trash is Some(), moves files to trash.
    /// Note: This does *not* remove Live Photo mappings, as this should only be used on files that
    /// the live photo mapping has removed.
    fn remove_from_catalog(&mut self, file_handle: &FileHandle) {
        for path in self.catalog.remove(file_handle) {
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

#[cfg(test)]
mod test {
    use super::*;
}

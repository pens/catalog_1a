//! Type for organizing media files and their associated sidecars.
//!
//! Copyright 2023-4 Seth Pendergrass. See LICENSE.

use super::media::Media;
use super::metadata::Metadata;
use super::sidecar::Sidecar;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

pub struct Catalog {
    media_files: HashMap<PathBuf, Media>,
    sidecar_files: HashMap<PathBuf, Sidecar>,
}

impl Catalog {
    //
    // Constructor.
    //

    /// Creates a new `Catalog` out of the output from `exiftool`.
    pub fn new(exiftool_stdout: Vec<u8>) -> Self {
        // Parse exiftool output.
        let Ok(metadata) = serde_json::from_slice::<Vec<Metadata>>(&exiftool_stdout[..]) else {
            panic!("Failed to parse exiftool output.");
        };

        // Split media from xmp based on exiftool FileType.
        let (file_metadata, sidecar_metadata): (Vec<_>, Vec<_>) =
            metadata.into_iter().partition(|m| m.file_type != "XMP");

        // Add media files to catalog with empty XMP refs.
        let mut media_files = file_metadata
            .into_iter()
            .map(|m| {
                let media = Media::new(m);
                media.validate_extension();

                (media.metadata.source_file.clone(), media)
            })
            .collect::<HashMap<PathBuf, Media>>();

        // Add XMP files to catalog, linking XMP and media file references.
        let sidecar_files = sidecar_metadata
            .into_iter()
            .map(|xmp_metadata| {
                let mut xmp = Sidecar::new(xmp_metadata);
                xmp.validate_extension();
                Self::link_source_to_sidecar(&mut media_files, &mut xmp);

                (xmp.metadata.source_file.clone(), xmp)
            })
            .collect::<HashMap<PathBuf, Sidecar>>();

        Self {
            media_files,
            sidecar_files,
        }
    }

    //
    // Public.
    //

    /// Gets the metadata for the given file.
    pub fn get(&self, path: &Path) -> Metadata {
        self.sidecar_files.get(path).map_or_else(
            || self.media_files.get(path).unwrap().metadata.clone(),
            |f| f.metadata.clone(),
        )
    }

    /// Gets all files that should be written to when synchronizing metdata.
    /// For example, path points to a media file with multiple sidecars, this will return the path
    /// to each.
    pub fn get_metadata_sinks(&self, path: &Path) -> Vec<PathBuf> {
        let media = self.media_files.get(path).unwrap();

        if media.sidecars.is_empty() {
            vec![media.metadata.source_file.clone()]
        } else {
            media.sidecars.iter().cloned().collect()
        }
    }

    /// Gets the paths to all sidecars that should exist, but don't.
    pub fn get_missing_sidecars(&self) -> Vec<PathBuf> {
        self.media_files
            .iter()
            .filter(|(_, media)| media.sidecars.is_empty())
            .map(|(_, media)| media.get_base_sidecar_path())
            .collect()
    }

    /// Gets the path to the file that should be used as the source when updating metadata.
    /// For example, if path points to a media file with a sidecar, this will return the path to
    /// the sidecar.
    pub fn get_primary_metadata_source(&self, path: &Path) -> PathBuf {
        let media = self.media_files.get(path).unwrap();

        if media.sidecars.is_empty() {
            media.metadata.source_file.clone()
        } else {
            media
                .sidecars
                .get(&media.get_base_sidecar_path())
                .unwrap()
                .clone()
        }
    }

    /// Given a path to a media file, get all associated sidecars.
    pub fn get_sidecar_paths(&self, path: &Path) -> Vec<PathBuf> {
        self.media_files
            .get(path)
            .unwrap()
            .sidecars
            .iter()
            .cloned()
            .collect()
    }

    /// Adds a sidecar file to the catalog.
    pub fn insert_sidecar(&mut self, mut sidecar: Sidecar) {
        Self::link_source_to_sidecar(&mut self.media_files, &mut sidecar);
        assert!(self
            .sidecar_files
            .insert(sidecar.metadata.source_file.clone(), sidecar)
            .is_none());
    }

    /// Returns an iterator over all media files in the catalog.
    pub fn iter_media(&self) -> impl Iterator<Item = &Media> {
        self.media_files.values()
    }

    /// Removes `path` from the catalog, alongside any sidecars refencing it. Paths for all removed
    /// files are returned.
    pub fn remove(&mut self, path: &Path) -> Vec<PathBuf> {
        let mut extracted = Vec::new();

        // `path` is a sidecar file.
        if let Some(sidecar) = self.sidecar_files.remove(path) {
            // Sidecars can only reference one media file, which is not removed.
            if let Some(media_path) = sidecar.media {
                assert!(self
                    .media_files
                    .get_mut(&media_path)
                    .unwrap()
                    .sidecars
                    .remove(path));
            }
            extracted.push(sidecar.metadata.source_file);

        // `path` is a media file.
        } else if let Some(media) = self.media_files.remove(path) {
            // Media files can have multiple sidecars, all of which should be removed.
            for sidecar_path in media.sidecars {
                assert!(self.sidecar_files.remove(&sidecar_path).is_some());
                extracted.push(sidecar_path);
            }
            extracted.push(media.metadata.source_file);

        // `path` is not in the catalog.
        } else {
            panic!("{}: File not found in catalog.", path.display());
        }

        extracted
    }

    /// Removes all sidecars that do not have associated media files, and returns them.
    pub fn remove_leftover_sidecars(&mut self) -> Vec<PathBuf> {
        let (keep, remove): (Vec<_>, Vec<_>) = self
            .sidecar_files
            .drain()
            .partition(|(_, sidecar)| sidecar.media.is_some());
        self.sidecar_files = keep.into_iter().collect();

        remove.into_iter().map(|(path, _)| path).collect()
    }

    /// Updates the metadata for file formerly at `path_old` to `metadata`.
    /// If this is a media file, this will update the associated sidecars to point to the new path,
    /// without updating the sidecar's metadata or path.
    /// If this is a sidecar, it will only update the metadata.
    pub fn update(&mut self, path_old: &Path, metadata: Metadata) {
        // Media file.
        if let Some(mut media) = self.media_files.remove(path_old) {
            media.metadata = metadata;

            // Update sidecar references.
            for sidecar in media.sidecars.iter() {
                self.sidecar_files.get_mut(sidecar).unwrap().media =
                    Some(media.metadata.source_file.clone());
            }

            assert!(self
                .media_files
                .insert(media.metadata.source_file.clone(), media)
                .is_none());

        // Sidecar file.
        } else if let Some(mut sidecar) = self.sidecar_files.remove(path_old) {
            sidecar.metadata = metadata;
            assert!(self
                .sidecar_files
                .insert(sidecar.metadata.source_file.clone(), sidecar)
                .is_none());

        // File not found.
        } else {
            panic!(
                "{}: File not found in catalog. Cannot update.",
                path_old.display()
            );
        }
    }

    /// Validates tag presence and values for all media files in the catalog. Also checks sidecars
    /// associated with these media files.
    /// Does *not* check sidecars that are not associated with media files.
    pub fn validate_tags(&self) {
        for media in self.media_files.values() {
            media.metadata.validate_tags();

            for xmp in &media.sidecars {
                self.sidecar_files
                    .get(xmp)
                    .unwrap()
                    .metadata
                    .validate_tags();
            }
        }
    }

    //
    // Private.
    //

    /// Links a sidecar to its target media file, if it exists.
    fn link_source_to_sidecar(media_files: &mut HashMap<PathBuf, Media>, sidecar: &mut Sidecar) {
        let exp_media_path = sidecar.get_source_file();

        // If the referenced media file exists, link it to this XMP file.
        if let Some(media) = media_files.get_mut(&exp_media_path) {
            media.sidecars.insert(sidecar.metadata.source_file.clone());
            sidecar.media = Some(exp_media_path);
        }
    }
}

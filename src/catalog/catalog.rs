//! Type for organizing media files and their associated sidecars.
//!
//! Copyright 2023-4 Seth Pendergrass. See LICENSE.

use super::file::FileHandle;
use super::media::Media;
use super::metadata::Metadata;
use super::sidecar::{self, Sidecar};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

pub struct Catalog {
    media_files: HashMap<FileHandle, Media>,
    sidecar_files: HashMap<FileHandle, Sidecar>,
    handle_map: HashMap<PathBuf, FileHandle>,
    next_handle: FileHandle,
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

        let mut media_files = HashMap::new();
        let mut sidecar_files = HashMap::new();
        let mut handle_map = HashMap::new();
        let mut next_handle = 0;

        // Builds FileHandle -> Media & Sidecar maps.
        for m in metadata {
            // Insert path -> FileHandle mapping.
            handle_map.insert(m.source_file.clone(), next_handle);

            // Insert Media or Sidecar into catalog.
            if m.file_type == "XMP" {
                sidecar_files.insert(next_handle, Sidecar::new(m));
            } else {
                media_files.insert(next_handle, Media::new(m));
            }

            next_handle += 1;
        }

        // Link media files to sidecars.
        for (handle, sidecar) in sidecar_files.iter_mut() {
            Self::link_source_to_sidecar(&mut media_files, &handle_map, handle, sidecar);
        }

        Self {
            media_files,
            sidecar_files,
            handle_map,
            next_handle,
        }
    }

    //
    // Public.
    //

    /// Gets the metadata for the given file.
    pub fn get_metadata(&self, file_handle: &FileHandle) -> Metadata {
        self.sidecar_files.get(file_handle).map_or_else(
            || self.media_files.get(file_handle).unwrap().metadata.clone(),
            |f| f.metadata.clone(),
        )
    }

    /// Gets all files that should be written to when synchronizing metdata.
    /// For example, if file_handle points to a media file with multiple sidecars, this will return the path
    /// to each.
    pub fn get_metadata_sink_paths(&self, file_handle: &FileHandle) -> Vec<(FileHandle, PathBuf)> {
        let media = self.media_files.get(file_handle).unwrap();

        if media.sidecars.is_empty() {
            vec![(*file_handle, media.metadata.source_file.clone())]
        } else {
            media.sidecars.iter().map(|fh| (*fh, self.sidecar_files.get(fh).unwrap().metadata.source_file.clone())).collect()
        }
    }

    /// Gets the paths to all sidecars that should exist, but don't.
    pub fn get_missing_sidecars(&self) -> Vec<PathBuf> {
        self.media_files
            .values()
            .filter(|media| media.sidecars.is_empty())
            .map(|media| media.get_base_sidecar_path())
            .collect()
    }

    /// Gets the path to the file that should be used as the source when updating metadata.
    /// For example, if file_handle points to a media file with a sidecar, this will return the path to
    /// the "primary" (non-duplicate) sidecar.
    pub fn get_metadata_source_path(&self, file_handle: &FileHandle) -> PathBuf {
        let media = self.media_files.get(file_handle).unwrap();

        if media.sidecars.is_empty() {
            media.metadata.source_file.clone()
        } else {
            let base_handle = self.handle_map.get(&media.get_base_sidecar_path()).unwrap();
            self.sidecar_files.get(base_handle).unwrap().metadata.source_file.clone()
        }
    }

    /// Given a handle to a media file, get all associated sidecars.
    pub fn get_sidecar_paths(&self, file_handle: &FileHandle) -> Vec<(FileHandle, PathBuf)> {
        self.media_files
            .get(file_handle)
            .unwrap()
            .sidecars
            .iter()
            .map(|fh| (*fh, self.sidecar_files.get(fh).unwrap().metadata.source_file.clone())) // TODO make fn
            .collect()
    }

    /// Adds a sidecar file to the catalog.
    pub fn insert_sidecar(&mut self, mut sidecar: Sidecar) {
        // TODO maybe reorder
        Self::link_source_to_sidecar(&mut self.media_files, &self.handle_map, &self.next_handle, &mut sidecar);
        assert!(self
            .sidecar_files
            .insert(self.next_handle, sidecar)
            .is_none());
        self.next_handle += 1; // TODO make fn
    }

    /// Returns an iterator over all media files in the catalog.
    pub fn iter_media(&self) -> impl Iterator<Item = (&FileHandle, &Media)> {
        self.media_files.iter()
    }

    /// Removes file_handle from the catalog, alongside any sidecars refencing it. Paths for all removed
    /// files are returned.
    pub fn remove(&mut self, file_handle: &FileHandle) -> Vec<PathBuf> {
        let mut extracted = Vec::new();

        // `path` is a sidecar file.
        if let Some(sidecar) = self.sidecar_files.remove(file_handle) {
            // Sidecars can only reference one media file, which is not removed.
            if let Some(media_path) = sidecar.media {
                assert!(self
                    .media_files
                    .get_mut(&media_path)
                    .unwrap()
                    .sidecars
                    .remove(file_handle));
            }
            extracted.push(sidecar.metadata.source_file);

        // `path` is a media file.
        } else if let Some(media) = self.media_files.remove(file_handle) {
            // Media files can have multiple sidecars, all of which should be removed.
            for sidecar_handle in media.sidecars {
                let sidecar = self.sidecar_files.remove(&sidecar_handle).unwrap();
                extracted.push(sidecar.metadata.source_file);
            }
            extracted.push(media.metadata.source_file);

        // `path` is not in the catalog.
        } else {
            panic!("File handle {} not found in catalog.", file_handle);
        }

        extracted
    }

    /// Removes all sidecars that do not have associated media files, and returns them.
    pub fn remove_leftover_sidecars(&mut self) -> Vec<FileHandle> {
        // TODO decide on explicit types or not
        let (keep, remove): (HashMap<_, _>, HashMap<_, _>) = self
            .sidecar_files
            .drain()
            .partition(|(_, sidecar)| sidecar.media.is_some());
        self.sidecar_files = keep.into_iter().collect();

        remove.into_keys().collect()
    }

    /// Updates the metadata for the file `file_handle` to `metadata`.
    /// If this is a media file, this will update the associated sidecars to point to the new path,
    /// without updating the sidecar's metadata or path.
    /// If this is a sidecar, it will only update its metadata.
    /// TODO this is now very wrong, sidecars wont need to update paths
    pub fn update(&mut self, file_handle: &FileHandle, metadata: Metadata) {
        // Media file.
        if let Some(mut media) = self.media_files.remove(file_handle) {
            media.metadata = metadata;

            // Update sidecar references.
            for sidecar in media.sidecars.iter() {
                self.sidecar_files.get_mut(sidecar).unwrap().media =
                    Some(*file_handle);
            }

            assert!(self
                .media_files
                .insert(*file_handle, media)
                .is_none());

        // Sidecar file.
        } else if let Some(mut sidecar) = self.sidecar_files.remove(file_handle) {
            sidecar.metadata = metadata;
            assert!(self
                .sidecar_files
                .insert(*file_handle, sidecar)
                .is_none());

        // File not found.
        } else {
            panic!(
                "{}: File not found in catalog. Cannot update.",
                metadata.source_file.display()
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
    fn link_source_to_sidecar(media_files: &mut HashMap<FileHandle, Media>, handle_map: &HashMap<PathBuf, FileHandle>, sidecar_handle: &FileHandle, sidecar: &mut Sidecar) {
        let exp_media_path = sidecar.get_source_file();

        // If the referenced media file exists, link it to this XMP file.
        if let Some(media_handle) = handle_map.get(&exp_media_path) {
            media_files.get_mut(media_handle).unwrap().sidecars.insert(*sidecar_handle);
            sidecar.media = Some(*media_handle);
        }
    }
}

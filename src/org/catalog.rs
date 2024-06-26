//! Type for organizing media files and their associated sidecars.
//!
//! Copyright 2023-4 Seth Pendergrass. See LICENSE.

use super::gbl::FileHandle;
use super::prim::{Media, Metadata, Sidecar};
use std::collections::HashMap;
use std::path::PathBuf;

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

    /// Creates a new Catalog out of the output from exiftool.
    pub fn new(metadata: Vec<Metadata>) -> Self {
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
        for (handle, sidecar) in &mut sidecar_files {
            Self::link_source_to_sidecar(&mut media_files, &handle_map, *handle, sidecar);
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

    /// Gets all files that should be written to when synchronizing metadata.
    /// For example, if `file_handle` points to a media file with multiple sidecars, this will return the path
    /// to each.
    pub fn get_sidecars(&self, file_handle: FileHandle) -> Vec<(FileHandle, PathBuf)> {
        self.media_files
            .get(&file_handle)
            .unwrap()
            .sidecars
            .iter()
            .map(|fh| {
                (
                    *fh,
                    self.sidecar_files
                        .get(fh)
                        .unwrap()
                        .metadata
                        .source_file
                        .clone(),
                )
            })
            .collect()
    }

    /// Gets the metadata for the given file.
    pub fn get_metadata(&self, file_handle: FileHandle) -> Metadata {
        self.sidecar_files.get(&file_handle).map_or_else(
            || self.media_files.get(&file_handle).unwrap().metadata.clone(),
            |f| f.metadata.clone(),
        )
    }

    /// Gets the path to the file that should be used as the source when updating metadata.
    /// For example, if `file_handle` points to a media file with a sidecar, this will return the path to
    /// the "primary" (non-duplicate) sidecar.
    pub fn get_metadata_source_path(&self, file_handle: FileHandle) -> PathBuf {
        let media = self.media_files.get(&file_handle).unwrap();

        if media.sidecars.is_empty() {
            media.metadata.source_file.clone()
        } else {
            let base_handle = self.handle_map.get(&media.get_base_sidecar_path()).unwrap();
            self.sidecar_files
                .get(base_handle)
                .unwrap()
                .metadata
                .source_file
                .clone()
        }
    }

    /// Gets the paths to all sidecars that should exist, but don't.
    pub fn get_missing_sidecars(&self) -> Vec<PathBuf> {
        self.media_files
            .values()
            .filter(|media| media.sidecars.is_empty())
            .map(Media::get_base_sidecar_path)
            .collect()
    }

    /// Adds a sidecar file to the catalog.
    pub fn insert_sidecar(&mut self, metadata: Metadata) {
        let mut sidecar = Sidecar::new(metadata);
        Self::link_source_to_sidecar(
            &mut self.media_files,
            &self.handle_map,
            self.next_handle,
            &mut sidecar,
        );
        self.handle_map
            .insert(sidecar.metadata.source_file.clone(), self.next_handle);
        assert!(self
            .sidecar_files
            .insert(self.next_handle, sidecar)
            .is_none());
        self.next_handle += 1;
    }

    /// Returns an iterator over all media files in the catalog.
    pub fn iter_media(&self) -> impl Iterator<Item = (FileHandle, &Media)> {
        self.media_files.iter().map(|(k, v)| (*k, v))
    }

    /// Removes `file_handle` from the catalog, alongside any sidecars refencing it. Paths for all removed
    /// files are returned.
    pub fn remove(&mut self, file_handle: FileHandle) -> Vec<PathBuf> {
        let mut extracted = Vec::new();

        // path is a sidecar file.
        if let Some(sidecar) = self.sidecar_files.remove(&file_handle) {
            // Sidecars can only reference one media file, which is not removed.
            if let Some(media_path) = sidecar.media {
                assert!(self
                    .media_files
                    .get_mut(&media_path)
                    .unwrap()
                    .sidecars
                    .remove(&file_handle));
            }
            extracted.push(sidecar.metadata.source_file);

        // path is a media file.
        } else if let Some(media) = self.media_files.remove(&file_handle) {
            // Media files can have multiple sidecars, all of which should be removed.
            for sidecar_handle in media.sidecars {
                let sidecar = self.sidecar_files.remove(&sidecar_handle).unwrap();
                extracted.push(sidecar.metadata.source_file);
            }
            extracted.push(media.metadata.source_file);

        // path is not in the catalog.
        } else {
            panic!("File handle {file_handle} not found in catalog.");
        }

        extracted
    }

    /// Removes all sidecars that do not have associated media files, and returns them.
    pub fn remove_leftover_sidecars(&mut self) -> Vec<Sidecar> {
        let (keep, remove): (HashMap<_, _>, HashMap<_, _>) = self
            .sidecar_files
            .drain()
            .partition(|(_, sidecar)| sidecar.media.is_some());
        self.sidecar_files = keep.into_iter().collect();

        remove.into_values().collect()
    }

    /// Updates the metadata for the file `file_handle` to metadata.
    /// This does not affect linked media files or sidecars. This **must** be handled by the
    /// caller.
    pub fn update(&mut self, file_handle: FileHandle, metadata: Metadata) {
        // Media file.
        if let Some(mut media) = self.media_files.remove(&file_handle) {
            media.metadata = metadata;

            // Update sidecar references.
            for sidecar in &media.sidecars {
                self.sidecar_files.get_mut(sidecar).unwrap().media = Some(file_handle);
            }

            assert!(self.media_files.insert(file_handle, media).is_none());

        // Sidecar file.
        } else if let Some(mut sidecar) = self.sidecar_files.remove(&file_handle) {
            sidecar.metadata = metadata;
            assert!(self.sidecar_files.insert(file_handle, sidecar).is_none());

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
    fn link_source_to_sidecar(
        media_files: &mut HashMap<FileHandle, Media>,
        handle_map: &HashMap<PathBuf, FileHandle>,
        sidecar_handle: FileHandle,
        sidecar: &mut Sidecar,
    ) {
        let exp_media_path = sidecar.get_source_file();

        // If the referenced media file exists, link it to this XMP file.
        if let Some(media_handle) = handle_map.get(&exp_media_path) {
            media_files
                .get_mut(media_handle)
                .unwrap()
                .sidecars
                .insert(sidecar_handle);
            sidecar.media = Some(*media_handle);
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;

    fn new_metadata(path: &str, file_type: &str) -> Metadata {
        Metadata {
            source_file: PathBuf::from(path),
            file_type: file_type.to_string(),
            ..Default::default()
        }
    }

    fn get_handle(c: &Catalog, path: &str) -> FileHandle {
        *c.handle_map.get(&PathBuf::from(path)).unwrap()
    }

    /// Should get correct metadata.
    #[test]
    fn test_get_metadata() {
        let c = Catalog::new(vec![new_metadata("with_xmps.jpg", "JPEG")]);

        let handle = get_handle(&c, "with_xmps.jpg");
        let metadata = c.get_metadata(handle);
        assert_eq!(metadata.source_file, PathBuf::from("with_xmps.jpg"));
    }

    /// Should get all associated sidecar paths.
    #[test]
    fn test_get_metadata_sink_paths() {
        let c = Catalog::new(vec![
            new_metadata("with_xmps.jpg", "JPEG"),
            new_metadata("with_xmps.jpg.xmp", "XMP"),
            new_metadata("with_xmps_01.jpg.xmp", "XMP"),
        ]);

        let handle = get_handle(&c, "with_xmps.jpg");
        let sinks = c.get_sidecars(handle);
        assert_eq!(sinks.len(), 2, "Sinks: {:?}", sinks);
        assert!(sinks
            .iter()
            .any(|(_, p)| *p == PathBuf::from("with_xmps.jpg.xmp")));
        assert!(sinks
            .iter()
            .any(|(_, p)| *p == PathBuf::from("with_xmps_01.jpg.xmp")));
    }

    /// Should get the base sidecar path: basename.ext -> basename.ext.xmp.
    #[test]
    fn test_get_metadata_source_path() {
        let c = Catalog::new(vec![
            new_metadata("with_xmps.jpg", "JPEG"),
            new_metadata("with_xmps.jpg.xmp", "XMP"),
            new_metadata("with_xmps_01.jpg.xmp", "XMP"),
        ]);

        let handle = get_handle(&c, "with_xmps.jpg");
        let source = c.get_metadata_source_path(handle);
        assert_eq!(source, PathBuf::from("with_xmps.jpg.xmp"));
    }

    /// Should get sidecar paths for media files without.
    #[test]
    fn test_get_missing_sidecars() {
        let c = Catalog::new(vec![
            new_metadata("with_xmps.jpg", "JPEG"),
            new_metadata("with_xmps.jpg.xmp", "XMP"),
            new_metadata("no_xmp.mp4", "MP4"),
        ]);

        let missing = c.get_missing_sidecars();
        assert_eq!(missing.len(), 1, "Missing: {:?}", missing);
        assert_eq!(missing[0], PathBuf::from("no_xmp.mp4.xmp"));
    }

    /// Inserting sidecar for existing media file.
    #[test]
    fn test_insert_sidecar() {
        let mut c = Catalog::new(vec![
            new_metadata("with_xmps.jpg", "JPEG"),
            new_metadata("with_xmps.jpg.xmp", "XMP"),
            new_metadata("with_xmps_01.jpg.xmp", "XMP"),
        ]);

        let metadata = new_metadata("with_xmps_02.jpg.xmp", "XMP");
        c.insert_sidecar(metadata);

        // Sidecar in catalog.
        let sidecar_handle = get_handle(&c, "with_xmps_02.jpg.xmp");
        let metadata_catalog = c.get_metadata(sidecar_handle);
        assert_eq!(
            metadata_catalog.source_file,
            PathBuf::from("with_xmps_02.jpg.xmp")
        );

        // Media and sidecar linked.
        let media_handle = get_handle(&c, "with_xmps.jpg");
        let sinks = c.get_sidecars(media_handle);
        assert!(sinks
            .iter()
            .any(|(_, p)| *p == PathBuf::from("with_xmps_02.jpg.xmp")));
    }

    /// All sidecars without associated media files should be returned.
    #[test]
    fn test_remove_leftover_sidecars() {
        let mut c = Catalog::new(vec![
            new_metadata("with_xmps.jpg", "JPEG"),
            new_metadata("with_xmps.jpg.xmp", "XMP"),
            new_metadata("with_xmps_01.jpg.xmp", "XMP"),
            new_metadata("lonely.jpg.xmp", "XMP"),
        ]);

        let leftovers = c.remove_leftover_sidecars();
        assert_eq!(
            leftovers.len(),
            1,
            "Leftovers: {:?}",
            leftovers
                .into_iter()
                .map(|s| s.metadata.source_file)
                .collect::<Vec<PathBuf>>()
        );
        assert_eq!(
            leftovers[0].metadata.source_file,
            PathBuf::from("lonely.jpg.xmp")
        );
    }

    /// Should remove only the specified media file.
    #[test]
    fn test_remove_media_no_sidecars() {
        let mut c = Catalog::new(vec![
            new_metadata("with_xmps.jpg", "JPEG"),
            new_metadata("with_xmps.jpg.xmp", "XMP"),
            new_metadata("with_xmps_01.jpg.xmp", "XMP"),
            new_metadata("no_xmp.mp4", "MP4"),
        ]);

        let handle = get_handle(&c, "no_xmp.mp4");
        let removed = c.remove(handle);
        assert_eq!(removed.len(), 1, "Removed: {:?}", removed);
        assert_eq!(removed[0], PathBuf::from("no_xmp.mp4"));
    }

    /// All sidecars should be removed alongside a media file.
    #[test]
    fn test_remove_media_includes_sidecars() {
        let mut c = Catalog::new(vec![
            new_metadata("with_xmps.jpg", "JPEG"),
            new_metadata("with_xmps.jpg.xmp", "XMP"),
            new_metadata("with_xmps_01.jpg.xmp", "XMP"),
        ]);

        let handle = get_handle(&c, "with_xmps.jpg");
        let removed = c.remove(handle);
        assert_eq!(removed.len(), 3, "Removed: {:?}", removed);
        assert!(removed.contains(&PathBuf::from("with_xmps.jpg")));
        assert!(removed.contains(&PathBuf::from("with_xmps.jpg.xmp")));
        assert!(removed.contains(&PathBuf::from("with_xmps_01.jpg.xmp")));
    }

    /// Crash if file not found.
    #[test]
    #[should_panic(expected = "File handle 4294967295 not found in catalog.")]
    fn test_remove_missing_panics() {
        let mut c = Catalog::new(vec![]);
        c.remove(u32::MAX);
    }

    /// With sidecars, only the chosen sidecar should be removed.
    #[test]
    fn test_remove_sidecar_only() {
        let mut c = Catalog::new(vec![
            new_metadata("with_xmps.jpg", "JPEG"),
            new_metadata("with_xmps.jpg.xmp", "XMP"),
            new_metadata("with_xmps_01.jpg.xmp", "XMP"),
        ]);

        let handle = get_handle(&c, "with_xmps.jpg.xmp");
        let removed = c.remove(handle);
        assert_eq!(removed.len(), 1);
        assert_eq!(removed[0], PathBuf::from("with_xmps.jpg.xmp"));
    }

    /// Only the specified file should have its metadata updated, even with linked media or
    /// sidecars. This is to enable updates per-file-move.
    #[test]
    fn test_update() {
        let mut c = Catalog::new(vec![
            new_metadata("with_xmps.jpg", "JPEG"),
            new_metadata("with_xmps.jpg.xmp", "XMP"),
        ]);

        // Validate ID wasn't set.
        let metadata_before = c.get_metadata(get_handle(&c, "with_xmps.jpg"));
        assert_eq!(metadata_before.content_identifier, None);

        // Update.
        let mut metadata = new_metadata("with_xmps.jpg", "JPEG");
        metadata.content_identifier = Some("1".to_string());
        c.update(get_handle(&c, "with_xmps.jpg"), metadata);

        // Validate ID was set & metadata updated correctly.
        let metadata_after = c.get_metadata(get_handle(&c, "with_xmps.jpg"));
        assert_eq!(metadata_after.content_identifier, Some("1".to_string()));

        // Validate that the sidecar was not updated.
        let sidecar_metadata = c.get_metadata(get_handle(&c, "with_xmps.jpg.xmp"));
        assert_eq!(sidecar_metadata.content_identifier, None);
    }
}

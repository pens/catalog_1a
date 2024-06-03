//! Media file handling.
//!
//! Copyright 2023-4 Seth Pendergrass. See LICENSE.

use super::metadata::Metadata;
use std::{collections::HashSet, path::PathBuf};

lazy_static! {
    static ref LIVE_PHOTO_IMAGE_EXTS: HashSet<&'static str> = HashSet::from(["JPEG", "HEIC"]);
    static ref LIVE_PHOTO_VIDEO_EXTS: HashSet<&'static str> = HashSet::from(["MOV"]);
}

pub struct Media {
    pub metadata: Metadata,
    pub sidecars: HashSet<PathBuf>, // TODO set
}

impl Media {
    //
    // Constructor.
    //

    /// Creates a new `Media` object with `Metadata` and no referenced sidecars.
    pub fn new(metadata: Metadata) -> Self {
        Self {
            metadata,
            sidecars: HashSet::new(),
        }
    }

    //
    // Public.
    //

    /// Returns the path to the base sidecar (i.e. not representing a duplicate). This is of the
    /// format `basename.ext.xmp` (not `basename_nn.ext.xmp`).
    /// This does *not* guaranee the sidecar exists.
    pub fn get_base_sidecar_path(&self) -> PathBuf {
        let mut ext = self
            .metadata
            .source_file
            .extension()
            .unwrap()
            .to_os_string();
        ext.push(".xmp");

        self.metadata.source_file.with_extension(ext)
    }

    /// Returns whether this file is a Live Photo image.
    pub fn is_live_photo_image(&self) -> bool {
        self.is_live_photo() && LIVE_PHOTO_IMAGE_EXTS.contains(&self.metadata.file_type.as_str())
    }

    /// Returns whether this file is a Live Photo video.
    pub fn is_live_photo_video(&self) -> bool {
        self.is_live_photo() && LIVE_PHOTO_VIDEO_EXTS.contains(&self.metadata.file_type.as_str())
    }

    /// Checks that the file has an extension.
    pub fn validate_extension(&self) {
        let path = &self.metadata.source_file;

        assert!(
            path.extension().is_some(),
            "{}: Media file without extension.",
            path.display()
        );
    }

    //
    // Private.
    //

    fn is_live_photo(&self) -> bool {
        self.metadata.content_identifier.is_some()
    }
}

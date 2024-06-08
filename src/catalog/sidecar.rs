//! Sidecar file handling.
//!
//! Copyright 2023-4 Seth Pendergrass. See LICENSE.

use super::{file::FileHandle, metadata::Metadata};
use std::path::PathBuf;

pub struct Sidecar {
    pub metadata: Metadata,
    pub media: Option<FileHandle>,
}

impl Sidecar {
    //
    // Constructor.
    //

    /// Creates a new sidecar object with `metadata` but no linked media file.
    pub fn new(metadata: Metadata) -> Self {
        Self::validate_extension(&metadata);

        Self {
            metadata,
            media: None,
        }
    }

    //
    // Public.
    //

    /// Gets the path to the source file for this sidecar.
    /// This does *not* guarantee the file exists.
    pub fn get_source_file(&self) -> PathBuf {
        self.metadata.source_file.with_extension("")
    }

    //
    // Private.
    //

    /// Checks that the file is of the format `basename[_nn].ext.xmp`.
    fn validate_extension(metadata: &Metadata) {
        assert!(
            metadata.source_file.extension().unwrap() == "xmp",
            "{}: XMP file without .xmp extension.",
            metadata.source_file.display()
        );

        assert!(
            metadata.source_file.with_extension("").extension().is_some(),
            "{}: XMP file without \".ext.xmp\" extension.",
            metadata.source_file.display()
        );
    }
}

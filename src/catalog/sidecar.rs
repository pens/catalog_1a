//! Sidecar file handling.
//!
//! Copyright 2023-4 Seth Pendergrass. See LICENSE.

use super::metadata::Metadata;
use std::path::PathBuf;

pub struct Sidecar {
    pub metadata: Metadata,
    pub media: Option<PathBuf>,
}

impl Sidecar {
    //
    // Constructor.
    //

    /// Creates a new sidecar object with `metadata` but no linked media file.
    pub fn new(metadata: Metadata) -> Self {
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

    /// Checks that the file is of the format `basename[_nn].ext.xmp`.
    pub fn validate_extension(&self) {
        //<basename>[_nn].<ext>.xmp
        let path = &self.metadata.source_file;

        assert!(
            path.extension().unwrap() == "xmp",
            "{}: XMP file without .xmp extension.",
            path.display()
        );

        assert!(
            path.with_extension("").extension().is_some(),
            "{}: XMP file without \".ext.xmp\" extension.",
            path.display()
        );
    }
}

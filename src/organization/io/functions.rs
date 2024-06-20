//! Copyright 2023-4 Seth Pendergrass. See LICENSE.

use super::super::primitives::Metadata;
use super::exiftool;
use std::fs;
use std::path::{Path, PathBuf};

pub fn copy_metadata(from: &Path, to: &Path) -> Metadata {
    serde_json::from_slice::<Metadata>(exiftool::copy_metadata(from, to).as_slice()).unwrap()
}

// Doesn't check that new path is as expected.
pub fn create_xmp(path: &Path) -> Metadata {
    serde_json::from_slice::<Metadata>(exiftool::create_xmp(path).as_slice()).unwrap()
}

pub fn move_file(fmt: &str, path: &Path, tag_src: &Path) -> PathBuf {
    exiftool::rename_file(fmt, path, tag_src)
}

pub fn read_metadata(path: &Path) -> Metadata {
    serde_json::from_slice::<Metadata>(exiftool::get_metadata(path).as_slice()).unwrap()
}

pub fn scan_directory(path: &Path, exclude: Option<&Path>) -> Vec<Metadata> {
    serde_json::from_slice::<Vec<Metadata>>(
        exiftool::get_metadata_recursive(path, exclude).as_slice(),
    )
    .unwrap()
}

pub fn trash(path: &Path, trash: &Path) {
    let path_trash = trash.join(path.file_name().unwrap());
    assert!(
        !path_trash.exists(),
        "Cannot safely delete {} due to name collision in {}.",
        path.display(),
        trash.display()
    );
    fs::rename(path, path_trash).unwrap();
}

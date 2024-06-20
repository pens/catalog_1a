//! Copyright 2023-4 Seth Pendergrass. See LICENSE.

use std::path::Path;
use super::{assets::{Metadata}, exiftool};

pub fn copy_metadata(from: &Path, to: &Path) -> Metadata {
    serde_json::from_slice::<Metadata>(exiftool::copy_metadata(from, to).as_slice()).unwrap()
}

pub fn create_xmp(path: &Path) -> Metadata {
    // TODO check path is correct?
    serde_json::from_slice::<Metadata>(exiftool::create_xmp(path).as_slice()).unwrap()
}

pub fn read_metadata(path: &Path) -> Metadata {
    serde_json::from_slice::<Metadata>(exiftool::get_metadata(path).as_slice()).unwrap()
}

pub fn scan_directory(path: &Path, exclude: Option<&Path>) -> Vec<Metadata> {
    serde_json::from_slice::<Vec<Metadata>>(exiftool::get_metadata_recursive(path, exclude).as_slice()).unwrap()
}
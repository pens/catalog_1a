//! Helper functions for CatalogManager.
//!
//! Copyright 2023-4 Seth Pendergrass. See LICENSE.

use std::{
    collections::HashMap,
    ffi::{OsStr, OsString},
    fs,
    path::{Path, PathBuf},
    process::Command,
};

// TODO helper function for src -> [dest] metadata copy

/// Finds the largest file in `paths, returning it alongside the remainder.
pub fn filter_out_largest(paths: &[PathBuf]) -> (PathBuf, Vec<PathBuf>) {
    let mut paths = paths.to_vec();
    paths.sort_by(|a, b| {
        let a_size = fs::metadata(a).unwrap().len();
        let b_size = fs::metadata(b).unwrap().len();
        b_size.cmp(&a_size)
    });
    let largest = paths.pop().unwrap();

    (largest, paths)
}

/// Moves `path` to `trash`.
pub fn move_to_trash(path: &Path, trash: &Path) {
    let path_trash = trash.join(path.file_name().unwrap());
    // If this trips, instead just switch to `exiftool`.
    assert!(
        !path_trash.exists(),
        "Cannot safely delete {} due to name collision in {}.",
        path.display(),
        trash.display()
    );
    fs::rename(path, path_trash).unwrap();
}

///  To ensure this tool doesn't cause problems if I ever switch to Adobe-style (e.g. .xmp vs
/// .ext.xmp) XMP file naming, panic if any are detected in the catalog.
/// HACK: T here just so I don't have to import XMP.
pub fn sanity_check_xmp_filenames<T>(xmps: &HashMap<PathBuf, T>) {
    log::debug!("Sanity checking XMP filename formats.");
    for xmp in xmps.keys() {
        let stem = xmp.file_stem().unwrap();
        let stem_path = PathBuf::from(stem);
        assert!(stem_path.extension().is_some(),
                "\n\nWARNING: XMP File in Adobe format (x.jpg -> x.xmp) detected. Program not able to continue.\n{}\n\n",
                xmp.display()
            );
    }
}

/// Given a file path, return the path with ".xmp" appended.
pub fn xmp_path_from_file_path(path: &Path) -> PathBuf {
    let mut ext = OsString::new();
    if let Some(ext_curr) = path.extension() {
        assert!(ext_curr != "xmp", "File already has an XMP extension.");
        ext = ext_curr.to_os_string();
    }
    ext.push(".xmp");

    path.with_extension(ext)
}

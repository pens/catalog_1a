/*
    Per-directory checks.

    Copyright 2023 Seth Pendergrass. See LICENSE.
*/
use std::{
    fs::{self, DirEntry},
    io,
    path::Path,
};

pub fn validate(path: &Path) {
    //log::trace!("{}: Validating directory.", path.display());
    assert!(path.is_dir());

    validate_children(path);
}

// This confirms that children are all files xor all directories.
fn validate_children(path: &Path) {
    log::trace!("Validating children");

    let get_is_dir = |next: io::Result<DirEntry>| -> io::Result<bool> {
        let Ok(entry) = next else {
            log::trace!("\tFailed to read entry data.");
            return Err(next.unwrap_err());
        };
        let file_type_result = entry.file_type();
        let Ok(file_type) = file_type_result else {
            log::trace!("\tFailed to get file type for {}", entry.path().display());
            return Err(file_type_result.unwrap_err());
        };

        Ok(file_type.is_dir())
    };

    if let Ok(iter) = &mut fs::read_dir(path) {
        let Some(next) = iter.next() else {
            log::trace!("\tEmpty directory.");
            return;
        };

        if let Ok(is_dir) = get_is_dir(next) {
            if !iter.all(|entry| get_is_dir(entry).map_or(false, |d| d == is_dir)) {
                log::warn!("{}: Mixing child files and directories.", path.display());
            }
        }
    }
}

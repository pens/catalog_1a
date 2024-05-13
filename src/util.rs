use std::{ffi::{OsString}, path::{Path, PathBuf}};

pub fn xmp_path_from_file_path(path: &Path) -> PathBuf {
    let mut ext = OsString::new();
    if let Some(ext_curr) = path.extension() {
        ext = ext_curr.to_os_string();
    }
    ext.push(".xmp");

    path.with_extension(ext)
}
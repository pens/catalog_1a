use std::{path::Path, fs, ffi::OsString};

pub fn safe_delete(library_root: &Path, path: &Path) {
    let Some(filename) = path.file_name() else {
        return;
    };

    let mut new_path = library_root.join("trash").join(filename);

    while new_path.exists() {
        let ext_file = new_path.extension().unwrap().to_owned();
        new_path = new_path.with_extension("");

        let mut new_count = 1;

        // Get next unused count `x` for `path.x.ext`.
        if let Some(ext_count) = new_path.extension().and_then(|e| e.to_str()) {
            let Ok(count) = ext_count.parse::<u32>() else {
                // If we do have another extension, it needs to be a number else something is wrong.
                return;
            };

            // Remove `.x` and save new x.
            new_path = new_path.with_extension("");
            new_count = count + 1;
        };

        let Some(file_name) = new_path.file_name() else {
            return;
        };

        let mut new_file_name = OsString::from(file_name);
        new_file_name.push(".");
        new_file_name.push(new_count.to_string());
        new_file_name.push(".");
        new_file_name.push(ext_file);

        new_path = new_path.with_file_name(new_file_name);
    }

    if fs::rename(path, new_path).is_err() {
        log::error!("{}: Failed to move to trash/.", path.display());
    }
}

pub fn rename_media() {
    // rename media file
    // rename xmp file
    // update db?
}

pub fn delete_media() {
    // safe_delete media
    // safe_delete xmp
    // update db?
}
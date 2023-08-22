/*
    Utilities for (hopefully) safely moving and renaming files, as well as
    manipulating metadata via `exiftool`.

    Copyright 2023 Seth Pendergrass. See LICENSE.
*/
use std::{fs, path::Path};

use crate::{metadata::{DateTimeOriginal, Metadata}, PostProcessingInfo};

/*
    File Moving & Renaming
*/

pub fn move_file(src_file: &Path, dst_dir: &Path) {
    if !src_file.is_file() {
        log::debug!("Attempted to move non-file: {}", src_file.display());
        return;
    }

    if !dst_dir.is_dir() {
        log::debug!("Attempted to move to non-directory: {}", dst_dir.display());
        return;
    }

    if !dst_dir.is_absolute() {
        log::debug!(
            "Attempted to move to non-absolute path: {}",
            dst_dir.display()
        );
        return;
    }

    let mut dst_file = dst_dir.join(src_file.file_name().unwrap());

    if dst_file.is_file() {
        let mut i = 1;
        loop {
            dst_file.set_file_name(format!(
                "{}.{}",
                dst_file.file_stem().unwrap().to_str().unwrap(),
                i
            ));
            if !dst_file.is_file() {
                break;
            }
            i += 1;
        }
    }

    log::debug!("Moving {} to {}", src_file.display(), dst_file.display());

    // Final sanity check
    assert!(
        !dst_file.is_file(),
        "Error: {} already exists.",
        dst_file.display()
    );

    fs::rename(src_file, dst_file).unwrap();
}

pub fn update_path_based_on_date_time(src_file: &Path, date_time: &DateTimeOriginal) {
}

pub fn update_extension(src_file: &Path, new_extension: &str) {
    if !src_file.is_file() {
        log::debug!(
            "Attempted to change extension of non-file: {}",
            src_file.display()
        );
        return;
    }

    if !src_file.is_absolute() {
        log::debug!(
            "Attempted to change extension of non-absolute path: {}",
            src_file.display()
        );
        return;
    }

    if src_file.extension().unwrap().to_str().unwrap() == new_extension {
        log::debug!(
            "Attempted to change extension to same extension: {}",
            src_file.display()
        );
        return;
    }

    let dst_file = src_file.with_extension(new_extension);
    if dst_file.is_file() {
        log::debug!(
            "Attempted to overwrite file via new extension: {}",
            src_file.display()
        );
        return;
    }

    log::debug!(
        "Changing extension of {} to {}",
        src_file.display(),
        new_extension
    );

    // Final sanity check
    assert!(
        !dst_file.is_file(),
        "Error: {} already exists.",
        dst_file.display()
    );

    fs::rename(src_file, dst_file).unwrap();
}

/*
    Metadata
*/

fn update_copyright(src_file: &Path, metadata: &Metadata) {
}

fn update_video_metadata_to_match_live_photo(src_file: &Path, metadata: &Metadata, post_processing_info: &PostProcessingInfo) {
}
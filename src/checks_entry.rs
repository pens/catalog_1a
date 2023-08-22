/*
    Checks for multimedia files to ensure proper file structure, metadata
    formatting, etc.

    Copyright 2023 Seth Pendergrass. See LICENSE.
*/
use crate::metadata::Metadata;
use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::Path;

/*
    Directories
*/

// Check that children of path are all files, or all directories.
pub fn all_entries_same_type(path: &Path) {
    let mut entries = fs::read_dir(path).unwrap();

    let all_dirs = entries.all(|e| e.unwrap().file_type().unwrap().is_dir());
    let all_files = entries.all(|e| e.unwrap().file_type().unwrap().is_file());

    if !all_dirs && !all_files {
        log::warn!("{}: Contains both files and directories.", path.display());
    }
}

/*
    XMP Sidecars
*/

// Is extension '.xmp'?
pub fn is_xmp_sidecar(path: &Path) -> bool {
    path.extension().unwrap().to_str().unwrap().to_lowercase() == "xmp"
}

// Check that `X.ext.xmp` file has corresponding `X.ext` file.
pub fn xmp_has_referenced_file(path: &Path) {
    let stem = path.file_stem().unwrap();

    if !Path::new(stem).is_file() {
        log::error!("{}: Does not reference a file.", path.display());
        // TODO move to removal directory
    }
}

// Does non-'.xmp' file have corresponding '.xmp' file?
pub fn has_xmp_sidecar(path: &Path) {
    if !path.with_extension(".xmp").is_file() {
        log::info!("{}: Does not have an XMP sidecar.", path.display());
    }
}

/*
    Media Files
*/

// Check that Date Time Original tag is filled out.
pub fn has_date_time_original(metadata: &Metadata) {
    if metadata.date_time_original.is_none() {
        log::warn!(
            "{}: Does not have a DateTimeOriginal tag.",
            metadata.path.display()
        );
    }
}

// Check that Artist matches Copyright, and that Copyright date matches Date Time Original.
pub fn artist_copyright(metadata: &Metadata) {
    if metadata.artist.is_none() {
        log::warn!("{}: Does not have an Artist tag.", metadata.path.display());
    } else if let Some(copyright) = &metadata.copyright {
        let expected_copyright = format!(
            "Copyright {} {}",
            metadata.date_time_original.as_ref().unwrap().year,
            metadata.artist.as_ref().unwrap()
        );
        if *copyright != expected_copyright {
            log::warn!(
                "{}: Unexpected Copyright format {}.",
                metadata.path.display(),
                expected_copyright
            );
            // TODO update copyright
        }
    } else {
        log::warn!(
            "{}: Does not have a Copyright tag.",
            metadata.path.display()
        );
    }
}

// Check that file path is formatted by Date Time Original.
pub fn path_format(metadata: &Metadata) {
    if let Some(date_time_original) = &metadata.date_time_original {
        if !metadata
            .path
            .to_str()
            .unwrap()
            .contains(date_time_original.get_path_format().as_str())
        {
            log::warn!(
                "{}: Naming does not match DateTimeOriginal.",
                metadata.path.display()
            );
            // TODO rename
        }
    } else {
        log::debug!(
            "{}: DateTimeOriginal unavailable. Skipping path format check.",
            metadata.path.display()
        );
    }
}

// Check that video file extension matches format, to correct `.mov` files formerly renamed to `.mp4`.
fn extension_video_format(metadata: &Metadata, ext: &str) {
    let check_exts = HashSet::from(["mp4"]);
    let expected_exts = HashMap::from([("qt", "mov")]);

    if check_exts.contains(ext) {
        if let Some(major_brand) = &metadata.major_brand {
            if expected_exts.contains_key(major_brand.as_str()) {
                log::warn!(
                    "{}: Extension should be {} for format {}.",
                    metadata.path.display(),
                    expected_exts.get(major_brand.as_str()).unwrap(),
                    major_brand
                );
                // TODO rename
            }
        }
    }
}

// Helper function to get corrected file extension.
// Checks both capitalization and desired spelling (e.g. 'jpg' instead of 'jpeg').
fn maybe_fix_extension(ext: &str) -> Option<String> {
    let good_exts = HashSet::from(["CR2", "CR3", "HEIC", "jpg", "mp4", "mov"]);
    let rename_exts = HashMap::from([("jpeg", "jpg")]);

    if good_exts.contains(ext) {
        return Some(ext.to_owned());
    } else if rename_exts.contains_key(ext) {
        return Some(rename_exts.get(ext).unwrap().to_owned().to_owned());
    }

    let ext_lower = ext.to_lowercase();
    if good_exts.contains(ext_lower.as_str()) {
        return Some(ext_lower);
    } else if rename_exts.contains_key(ext_lower.as_str()) {
        return Some(
            rename_exts
                .get(ext_lower.as_str())
                .unwrap()
                .to_owned()
                .to_owned(),
        );
    }

    let ext_upper = ext.to_uppercase();
    if good_exts.contains(ext_upper.as_str()) {
        return Some(ext_upper);
    } else if rename_exts.contains_key(ext_upper.as_str()) {
        return Some(
            rename_exts
                .get(ext_upper.as_str())
                .unwrap()
                .to_owned()
                .to_owned(),
        );
    }

    None
}

// Check file extension case, spelling and type.
pub fn extension_format(metadata: &Metadata) {
    let ext = metadata.path.extension().unwrap().to_str().unwrap();

    if let Some(ext_correct) = maybe_fix_extension(ext) {
        if ext != ext_correct {
            log::warn!(
                "{}: Extension should be {}.",
                metadata.path.display(),
                ext_correct
            );
            // TODO rename file
        }
    } else {
        log::debug!("{}: Unrecognized extension.", metadata.path.display());
    }

    extension_video_format(metadata, ext);
}
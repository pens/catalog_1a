/*
    Per-media-file checks.

    Copyright 2023 Seth Pendergrass. See LICENSE.
*/
use super::Results;
use crate::config::Config;
use crate::file::File;
use crate::file::FileType;
use std::path::PathBuf;

pub fn validate(file: &File, config: &Config, results: &mut Results) {
    log::trace!("{}: Validating media file.", file.path.display());
    assert!(file.is_media());

    validate_metadata(file, config, results);
    validate_path(file, config, results);
    validate_extension(file, config, results);

    link_live_photos(file, results);
}

// Checks that metadata fields are present and formatted correctly.
fn validate_metadata(file: &File, config: &Config, results: &mut Results) {
    log::trace!("Validating metadata.");
    let FileType::Media { metadata } = &file.file_type else {
        return;
    };

    // 1. Check for expected tags

    let mut missing = Vec::new();
    for e in &config.tags {
        if e.1.required && metadata.tags.get(*e.0).is_none() {
            missing.push(*e.0);
        }
    }
    if !missing.is_empty() {
        let missing_string = missing.join(", ");
        log::info!(
            "{}: Missing {} tag(s).",
            file.path.display(),
            missing_string
        );
    }

    // 2. Check formatting of Copyright tag

    if !(metadata.tags.contains_key("Artist")
        && metadata.tags.contains_key("Copyright")
        && metadata.tags.contains_key("DateTimeOriginal"))
    {
        log::debug!(
            "{}: Missing metadata needed for copyright check. Skipping.",
            file.path.display()
        );
        return;
    }

    let artist = metadata.tags.get("Artist").unwrap();
    if artist != "Seth Pendergrass" {
        log::debug!(
            "{}: Artist must be Seth Pendergrass for copyright check. Skipping.",
            file.path.display()
        );
        return;
    }

    let copyright = metadata.tags.get("Copyright").unwrap();
    let year = metadata.get_year().unwrap();
    let expected = format!("Copyright {} Seth Pendergrass", year);
    if *copyright != expected {
        results.bad_copyright.insert(file.path.clone(), expected);
        log::error!(
            "{}: Incorrect copyright format: {}",
            file.path.display(),
            copyright
        );
    }
}

// Checks that path matches datetime.
fn validate_path(file: &File, config: &Config, results: &mut Results) {
    log::trace!("Validating path.");
    let FileType::Media { metadata } = &file.file_type else {
        return;
    };

    // 3. Path follows datetime format

    // e.g. /media/nas/media/2023/08/230824_123456
    let Some(path_exp) = metadata.get_datetime_path().map(|p| config.library_root.join(p)) else {
        log::debug!("{}: No expected path format. Skipping.", file.path.display());
        return;
    };

    // Matches if no copy ID.
    // e.g. /path/230824_123456.jpg -> /path/230824_123456
    let path = file.path.with_extension("");
    if path == path_exp {
        return;
    }

    // Matches with copy ID.
    // e.g. /path/230824_123456_1.jpg -> /path/230824_123456, 1
    let Some(path_str) = path.to_str() else {
        log::debug!("{}: Error getting file name for path validation. Skipping.", file.path.display());
        return;
    };
    if let Some(split) = path_str.rsplit_once('_') {
        if PathBuf::from(split.0) == path_exp && split.1.chars().all(|c| c.is_ascii_digit()) {
            return;
        }
    }
    results.bad_path.insert(file.path.clone(), path_exp.clone());
    log::error!(
        "{}: Incorrect path format. Expected \"{}\"",
        file.path.display(),
        path_exp.display()
    );
}

// Checks that extension is spelled correctly and matches media format.
fn validate_extension(file: &File, config: &Config, results: &mut Results) {
    log::trace!("Validating extension.");
    let FileType::Media { metadata } = &file.file_type else {
        return;
    };

    // 4. Extension formatted correctly (.jpeg -> .jpg)

    let Some(ext_conf) = config.get_extension(&file.extension) else {
        log::warn!("{}: Unknown extension.", file.path.display());
        return;
    };
    if let Some(ext) = ext_conf.rename {
        results.bad_extension.insert(file.path.clone(), ext.to_owned());
        log::error!("{}: Extension should be \"{}\".", file.path.display(), ext);
    }

    // 5. Extension matches video format (e.g. .mov for QuickTime)

    if let Some(major_brand) = metadata.tags.get("MajorBrand") {
        if let Some(fmt_conf) = config.formats.get(major_brand.as_str()) {
            if file.extension != fmt_conf.extension {
                results.wrong_format.insert(file.path.clone(), fmt_conf.extension.to_owned());
                log::error!(
                    "{}: Wrong extension for format. Expected \"{}\".",
                    file.path.display(),
                    fmt_conf.extension
                );
            }
        }
    }
}

// Collects live photo groups into results by ContentIdentifier and MediaGroupUUID.
fn link_live_photos(file: &File, results: &mut Results) {
    log::trace!("Linking live photos.");
    let FileType::Media { metadata } = &file.file_type else {
        return;
    };

    // 6. Add live photos to map by ContentIdentifier or MediaGroupUUID

    let content_id = metadata.tags.get("ContentIdentifier");
    if let Some(id) = content_id {
        results.add_live_photo_image(id, &file.path);
    }

    let media_group_uuid = metadata.tags.get("MediaGroupUUID");
    assert!(content_id.and(media_group_uuid).is_none());
    if let Some(id) = media_group_uuid {
        results.add_live_photo_video(id, &file.path);
    }
}

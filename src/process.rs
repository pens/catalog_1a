use std::process::Command;

/*
    Warnings and automatic fixes for scan results.

    Copyright 2023 Seth Pendergrass. See LICENSE.
*/
use crate::config::Config;
use crate::scan::Results;

mod util;

pub fn process(config: &Config, results: &Results) {
    log::info!("Processing");

    process_sidecars(config, results);
    process_live_photos(config, results);

    fix_copyrights(config, results);
    fix_extensions(config, results);
    fix_paths(config, results);
}

fn process_sidecars(config: &Config, results: &Results) {
    // Delete sidecars that reference deleted files.
    for sidecar in &results.sidecars_without_targets {
        println!("{}: No target. Moving to trash/.", sidecar.display());
        if config.apply_changes {
            util::safe_delete(&config.library_root, sidecar);
        }
    }

    // Warn about sidecars referencing multiple files.
    for (sidecar, targets) in &results.sidecar_to_targets {
        if targets.len() > 1 {
            log::warn!("{}: Multiple targets.", sidecar.display());
        }
    }
}

fn process_live_photos(config: &Config, results: &Results) {
    for (content_id, videos) in &results.content_id_to_media {
        assert!(!videos.is_empty());

        if !results.media_group_uuid_to_media.contains_key(content_id) {
            println!("{}: No image for live photo. Moving to trash/.", content_id);
            if config.apply_changes {
                for video in videos {
                    util::safe_delete(&config.library_root, video);
                    // TODO make delete_media file, which does xmp as well
                }
            }
        } else if videos.len() > 1 {
            log::warn!("{}: Multiple videos for live photo.", content_id);
            // TODO deduplicate videos
        }
    }

    for (media_group_uuid, images) in &results.media_group_uuid_to_media {
        assert!(!images.is_empty());

        if !results.content_id_to_media.contains_key(media_group_uuid) {
            log::debug!("{}: No video for live photo.", media_group_uuid);
        }

        if images.len() > 1 {
            log::warn!("{}: Multiple images for live photo.", media_group_uuid);
            // TODO deduplicate images
        }
    }
}

fn fix_copyrights(config: &Config, results: &Results) {
    for (path, new_copyright) in &results.bad_copyright {
        println!("{}: Updating Copyright to `{}`.", path.display(), new_copyright);
        if config.apply_changes {
            let mut command = Command::new("exiftool");
            command.arg("-d").arg("%Y").arg("-overwrite_original").arg("-Copyright<Copyright $DateTimeOriginal Seth Pendergrass").arg(path);
            if let Ok(output) = command.output() {
                if !output.status.success() {
                    log::warn!("{}: Failed to update Copyright.", path.display());
                }
            }
        }
    }
}

fn fix_extensions(config: &Config, results: &Results) {
    for (path, new_path) in &results.bad_extension {
        // If format is bad, handle extension update there instead.
        if !results.wrong_format.contains_key(path) {
            println!("{}: Renaming to {}.", path.display(), new_path);
            // TODO rename
            // exiftool -testname=%f.ext file
        }
    }
    for (path, new_path) in &results.wrong_format {
        println!("{}: Renaming to {}.", path.display(), new_path);
        // TODO check if bad extension too
        // TODO rename
        //exiftool -ee -r -d '%Y/%m/%y%m%d_%H%M%S%%+c.%%e' '-filename<$filemodifydate' '-filename<$createdate' '-filename<$datetimeoriginal' staging
    }
}

fn fix_paths(config: &Config, results: &Results) {
    for (path, new_path) in &results.bad_path {
        println!("{}: Renaming to {}.", path.display(), new_path.display());
        // TODO rename
    }
}

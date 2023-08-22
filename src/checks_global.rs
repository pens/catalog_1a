/*
    Global checks for multimedia files, looking for things like duplicates and
    Live Photo image/video pairs.

    Copyright 2023 Seth Pendergrass. See LICENSE.
*/
use crate::PostProcessingInfo;
use crate::metadata::Metadata;
use std::collections::HashSet;

// Given multiple images with the same Media Group UUID, if one and only one is HEIC, pick it.
fn pick_image(image_metadatas: &HashSet<Metadata>) -> Option<Metadata> {
    let mut heic_metadata = None;

    for image_metadata in image_metadatas {
        if image_metadata.path.extension().unwrap().to_str().unwrap() == "HEIC" {
            if heic_metadata.is_none() {
                heic_metadata = Some(image_metadata.clone());
            } else {
                log::info!(
                    "{}: Multiple HEIC images. Skipping pick.",
                    image_metadata.path.display()
                );
                return None;
            }
        }
    }

    heic_metadata
}

// Simple check for duplicate images.
pub fn duplicate_images_based_on_live_photos(post_processing_info: &PostProcessingInfo) {
    for (content_id, video_metadatas) in &post_processing_info.content_id_map {
        if video_metadatas.len() > 1 {
            log::error!(
                "Videos with same Content ID {}. Skipping duplicate check.",
                content_id
            );
            for video_metadata in video_metadatas {
                log::warn!("\t{}", video_metadata.path.display());
            }
            return;
        }
    }

    for (media_group_uuid, image_metadatas) in &post_processing_info.media_group_uuid_map {
        if image_metadatas.len() > 1 {
            log::error!("Photos with same Media Group UUID {}", media_group_uuid);
            for image_metadata in image_metadatas {
                log::warn!("\t{}", image_metadata.path.display());
            }
            let pick = pick_image(image_metadatas);
            if pick.is_none() {
                log::warn!("{}: No HEIC image to pick.", media_group_uuid);
                continue;
            } else {
                log::info!(
                    "{}: Picked for UUID {}.",
                    pick.unwrap().path.display(),
                    media_group_uuid
                );
            }
        }
    }
}

// Finds Live Photo image & video pairs, assuming that all exist under the scan directory.
// If a pair is found, checks that location, datetime and copyright match.
pub fn correlate_live_photos(post_processing_info: &PostProcessingInfo) {
    let mut videos_without_photos = HashSet::new();

    for (content_id, video_metadatas) in &post_processing_info.content_id_map {
        let video_metadata = video_metadatas.iter().next().unwrap();
        if video_metadatas.len() > 1 {
            log::error!(
                "{}: Multiple videos with same Content ID. Skipping correlation.",
                video_metadata.path.display()
            );
            continue;
        }

        if !post_processing_info
            .media_group_uuid_map
            .contains_key(content_id)
        {
            log::error!(
                "{}: Corresponding Live Photo missing.",
                video_metadata.path.display()
            );
            videos_without_photos.insert(video_metadata);
        } else if let Some(image_metadatas) =
            post_processing_info.media_group_uuid_map.get(content_id)
        {
            let image_metadata = image_metadatas.iter().next().unwrap();
            if image_metadatas.len() > 1 {
                log::error!(
                    "{}: Multiple corresponding Live Photos. Skipping correlation.",
                    image_metadata.path.display()
                );
                continue;
            }

            if image_metadata.gps_latitude != video_metadata.gps_latitude
                || image_metadata.gps_latitude_ref != video_metadata.gps_latitude_ref
                || image_metadata.gps_longitude != video_metadata.gps_longitude
                || image_metadata.gps_longitude_ref != video_metadata.gps_longitude_ref
                || image_metadata.gps_altitude != video_metadata.gps_altitude
                || image_metadata.gps_altitude_ref != video_metadata.gps_altitude_ref
            {
                log::warn!(
                    "{}: GPS coordinates do not match Live Photo {}.",
                    video_metadata.path.display(),
                    image_metadata.path.display()
                );
                // TODO update
            }

            if image_metadata.date_time_original != video_metadata.date_time_original {
                log::warn!(
                    "{}: DateTimeOriginal does not match Live Photo {}.",
                    video_metadata.path.display(),
                    image_metadata.path.display()
                );
                // TODO update
            }

            if image_metadata.artist != video_metadata.artist
                || image_metadata.copyright != video_metadata.copyright
            {
                log::warn!(
                    "{}: Artist or copyright does not match Live Photo {}.",
                    video_metadata.path.display(),
                    image_metadata.path.display()
                );
                // TODO update
            }
        }
    }

    for (media_group_uuid, image_metadatas) in &post_processing_info.media_group_uuid_map {
        if image_metadatas.len() > 1 {
            log::error!(
                "{}: Multiple photos with same Media Group UUID.",
                image_metadatas.iter().next().unwrap().path.display()
            );
        }

        if !post_processing_info
            .content_id_map
            .contains_key(media_group_uuid)
        {
            for metadata in image_metadatas {
                log::warn!("{}: Live Photo's video deleted.", metadata.path.display());
            }
        }
    }
}
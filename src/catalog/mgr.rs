//! Core catalog management type and functionality.
//!
//! Copyright 2023-4 Seth Pendergrass. See LICENSE.

use serde::Deserialize;
use std::{
    collections::HashMap,
    path::{Path, PathBuf},
};

use super::util;

#[derive(Clone, Deserialize)]
#[serde(rename_all = "PascalCase")]
struct Metadata {
    source_file: PathBuf,
    file_type: String,
    file_type_extension: String,

    // Live Photos
    content_identifier: Option<String>,

    create_date: Option<String>,
    date_time_original: Option<String>,
}

struct Media {
    xmp: Option<PathBuf>,
    metadata: Metadata,
}

struct Xmp {
    media: Option<PathBuf>,
    metadata: Metadata,
}

pub struct CatalogManager {
    trash: PathBuf,
    media_files: HashMap<PathBuf, Media>,
    xmps: HashMap<PathBuf, Xmp>,
    // Key is a Vec in case of duplicate items (e.g. jpg & HEIC).
    live_photo_images: HashMap<String, Vec<PathBuf>>,
    live_photo_videos: HashMap<String, Vec<PathBuf>>,
}

impl CatalogManager {
    /// Move file to trash.
    fn remove(&mut self, path: &Path) {
        if let Some(media) = self.media_files.remove(path) {
            log::debug!("{}: Removing media file.", path.display());
            util::move_to_trash(path, &self.trash);

            // Remove references to file in Live Photo mappings.
            if let Some(id) = &media.metadata.content_identifier {
                if self.live_photo_images.contains_key(id) {
                    self.live_photo_images
                        .get_mut(id)
                        .unwrap()
                        .retain(|p| p != path);
                }
                if self.live_photo_videos.contains_key(id) {
                    self.live_photo_videos
                        .get_mut(id)
                        .unwrap()
                        .retain(|p| p != path);
                }
            }

            // Delete associated XMP file (if present).
            if let Some(xmp) = media.xmp {
                log::debug!("{}: Also removing associated XMP file.", xmp.display());
                util::move_to_trash(&xmp, &self.trash);
            }
        } else if self.xmps.remove(path).is_some() {
            log::debug!("{}: Removing sidecar file.", path.display());
            util::move_to_trash(path, &self.trash);
        } else {
            panic!("{}: File not found. Cannot remove.", path.display());
        }
    }

    /// Rather than trying to synchronize the catalog post-renaming, just clear it.
    fn hack_clear_catalog(&mut self) {
        self.media_files.clear();
        self.xmps.clear();
        self.live_photo_images.clear();
        self.live_photo_videos.clear();
    }

    /// Create a new catalog of library, with trash as the destination for removed files.
    pub fn new(directory: &Path, trash: &Path, move_to_trash: bool) -> Self {
        log::info!("Building catalog.");

        let stdout = util::run_exiftool([
            "-FileType",
            "-FileTypeExtension",
            "-ContentIdentifier",
            "-MediaGroupUUID",
            "-CreateDate",
            "-DateTimeOriginal",
            "-json", // exiftool prefers JSON or XML over CSV.
            "-r",
            directory.to_str().unwrap(),
            "-i",
            trash.to_str().unwrap(),
        ]);

        // Parse exiftool output.
        let Ok(metadata) = serde_json::from_slice::<Vec<Metadata>>(&stdout[..]) else {
            panic!("Failed to parse exiftool output.");
        };
        let (file_metadata, xmp_metadata): (Vec<_>, Vec<_>) =
            metadata.into_iter().partition(|m| m.file_type != "XMP");

        // XMP setup.
        let mut media_files = file_metadata
            .into_iter()
            .map(|m| {
                (
                    m.source_file.clone(),
                    Media {
                        xmp: None,
                        metadata: m,
                    },
                )
            })
            .collect::<HashMap<PathBuf, Media>>();
        let mut xmps = xmp_metadata
            .into_iter()
            .map(|m| {
                (
                    m.source_file.clone(),
                    Xmp {
                        media: None,
                        metadata: m,
                    },
                )
            })
            .collect::<HashMap<PathBuf, Xmp>>();
        for (path, metadata) in media_files.iter_mut() {
            // .ext -> .ext.xmp.
            assert!(
                path.extension().is_some(),
                "{}: Media file without extension.",
                path.display()
            );

            let expected_xmp = util::xmp_path_from_file_path(path);

            if xmps.contains_key(&expected_xmp) {
                metadata.xmp = Some(expected_xmp.clone());
                xmps.get_mut(&expected_xmp).unwrap().media = Some(path.clone());
            }
        }

        // Sanity check: In case I switch to Adobe at some point (which uses file.xmp instead of
        // file.jpg.xmp).
        util::sanity_check_xmp_filenames(&xmps);

        let image_file_types = ["HEIC", "JPEG"];
        let video_file_types = ["MOV", "MP4"];

        // Live Photo setup.
        // Go through all non-XMP files to build Live Photo ID to file mappings.
        let mut live_photo_images = HashMap::new();
        let mut live_photo_videos = HashMap::new();
        for media in media_files.values() {
            if let Some(id) = &media.metadata.content_identifier {
                if image_file_types.contains(&media.metadata.file_type.as_str()) {
                    log::debug!(
                        "{}: Live Photo image with ID {}.",
                        &media.metadata.source_file.display(),
                        id
                    );
                    live_photo_images
                        .entry(id.clone())
                        .or_insert_with(Vec::new)
                        .push(media.metadata.source_file.clone());
                } else if video_file_types.contains(&media.metadata.file_type.as_str()) {
                    log::debug!(
                        "{}: Live Photo video with ID {}.",
                        &media.metadata.source_file.display(),
                        id
                    );
                    live_photo_videos
                        .entry(id.clone())
                        .or_insert_with(Vec::new)
                        .push(media.metadata.source_file.clone());
                } else {
                    panic!(
                        "{}: Live Photo file with extension {} can't be identified as a photo or a video.",
                        &media.metadata.source_file.display(),
                        media.metadata.file_type
                    );
                }
            }
        }

        Self {
            trash: trash.to_path_buf(),
            media_files,
            xmps,
            live_photo_images,
            live_photo_videos,
        }
    }

    /// If there are multiple Live Photo images or videos sharing the same ID, assume that there has
    /// been duplication. Most often, this is because a photo exists as both a JPG and HEIC. Images
    /// prefer HEIC over JPG, while videos are purely based on size.
    pub fn remove_duplicates_from_live_photos(&mut self) {
        log::info!("Removing duplicates from Live Photos.");

        // Delete duplicate images.
        let mut images_to_remove = HashMap::new();
        for paths in self.live_photo_images.values() {
            if paths.len() > 1 {
                let (heic_paths, mut jpg_paths): (Vec<_>, Vec<_>) = paths
                    .clone()
                    .into_iter()
                    .partition(|p| self.media_files.get(p).unwrap().metadata.file_type == "HEIC");
                match heic_paths.len() {
                    0 => {
                        // No HEICs, so just keep the largest JPG.
                        let (largest, others) = util::filter_out_largest(&jpg_paths);
                        images_to_remove.insert(largest, others);
                    }
                    1 => {
                        // One HEIC, so keep it and delete the rest.
                        images_to_remove.insert(heic_paths[0].clone(), jpg_paths);
                    }
                    _ => {
                        // Multiple HEICs, so keep the largest and delete the remainding HEIC & JPG.
                        let (largest, mut others) = util::filter_out_largest(&heic_paths);
                        others.append(&mut jpg_paths);
                        images_to_remove.insert(largest, others);
                    }
                }
            }
        }
        for (path_keep, paths_delete) in &images_to_remove {
            log::warn!(
                "{}: Duplicated Live Photo image. Removing:",
                path_keep.display()
            );
            for path in paths_delete.iter() {
                log::warn!("\t- {}", path.display());
                self.remove(path);
            }
        }

        // Delete duplicate videos.
        let mut videos_to_remove = HashMap::new();
        for paths in self.live_photo_videos.values() {
            if paths.len() > 1 {
                let (largest, others) = util::filter_out_largest(paths);
                videos_to_remove.insert(largest, others);
            }
        }
        for (path_keep, paths_delete) in &videos_to_remove {
            log::warn!(
                "{}: Duplicated Live Photo video. Removing:",
                path_keep.display()
            );
            for path in paths_delete.iter() {
                log::warn!("\t- {}", path.display());
                self.remove(path);
            }
        }
    }

    /// I'm too lazy to delete the videos from Live Photos when I delete the images. If we find a
    /// remaining video, based on its ID missing a corresponding image, remove it.
    pub fn remove_videos_from_deleted_live_photos(&mut self) {
        log::info!("Removing videos from deleted Live Photos.");

        let mut to_remove = Vec::new();
        for (id, paths) in &self.live_photo_videos {
            if !self.live_photo_images.contains_key(id) {
                to_remove.extend_from_slice(paths);
            }
        }
        for path in to_remove {
            log::warn!(
                "{}: Video remaining from presumably deleted Live Photo image. Removing.",
                path.display()
            );
            self.remove(&path);
        }
    }

    /// Copy specific metadata from Live Photo images to videos. This saves having to duplicate work
    /// tagging, geotagging, etc.
    /// TODO: This does not update the metadata in the catalog for modified Live Photo videos.
    pub fn copy_metadata_from_live_photo_image_to_video(&self) {
        log::info!("Copying metadata from Live Photo images to videos.");

        for (id, video_paths) in &self.live_photo_videos {
            // Safety checks.
            let Some(image_paths) = self.live_photo_images.get(id) else {
                for path in video_paths {
                    log::error!("{}: Live Photo video without corresponding image. Cannot retrieve metadata to copy. Skipping.", path.display());
                }
                continue;
            };
            assert!(
                !image_paths.is_empty(),
                "Unexpectedly found Live Photo ID {} with no image target(s).",
                id
            );
            assert!(
                !video_paths.is_empty(),
                "Unexpectedly found Live Photo ID {} with no video target(s).",
                id
            );
            if image_paths.len() > 1 {
                log::error!("{}: Multiple Live Photo images with ID {}. Cannot copy metadata to video. Skipping.", image_paths[0].display(), id);
                continue;
            } else if video_paths.len() > 1 {
                log::error!("{}: Multiple Live Photo images with ID {}. Cannot copy metadata to video. Skipping.", video_paths[0].display(), id);
                continue;
            }

            // Copy metadata.
            let image_path = &image_paths[0];
            let video_path = &video_paths[0];
            log::debug!(
                "{}: Copying metadata to {}.",
                image_path.display(),
                video_path.display()
            );
            assert!(
                self.media_files.get(image_path).unwrap().xmp.is_none()
                    && self.media_files.get(video_path).unwrap().xmp.is_none(),
                "Live Photo metadata copying not able to handle XMPs."
            );
            util::run_exiftool([
                "-tagsFromFile",
                image_path.to_str().unwrap(),
                "-CreateDate",
                "-DateTimeOriginal",
                "-Artist",
                "-Copyright",
                // https://exiftool.org/TagNames/GPS.html recommends all of the below
                "-GPSLatitude",
                "-GPSLatitudeRef",
                "-GPSLongitude",
                "-GPSLongitudeRef",
                "-GPSAltitude",
                "-GPSAltitudeRef",
                video_path.to_str().unwrap(),
            ]);
        }
    }

    /// If a sidecar XMP is around with no possible target file, remove it.
    pub fn remove_sidecars_without_references(&mut self) {
        log::info!("Removing XMP sidecars without corresponding files.");

        let mut to_remove = Vec::new();
        for (path, metadata) in &self.xmps {
            if metadata.media.is_none() {
                to_remove.push(path.clone());
            }
        }
        for path in to_remove {
            log::warn!(
                "{}: XMP sidecar without corresponding file. Removing.",
                path.display()
            );
            self.remove(&path);
        }
    }

    // Ensures every file has an associated XMP sidecar, creating one if not already present.
    pub fn create_xmp_sidecars_if_missing(&mut self) {
        log::info!("Ensuring all media files have associated XMP sidecar.");

        for (path, media) in &mut self.media_files {
            if media.xmp.is_none() {
                log::debug!("{}: Creating XMP sidecar.", path.display());

                util::run_exiftool(["-o", "%d%f.%e.xmp", path.to_str().unwrap()]);

                // TODO split out exiftool read from new() into separate function, and use here for
                // consistency.
                media.xmp = Some(util::xmp_path_from_file_path(path));
                self.xmps.insert(
                    media.xmp.as_ref().unwrap().clone(),
                    Xmp {
                        media: Some(path.clone()),
                        metadata: media.metadata.clone(),
                    },
                );
            }
        }
    }

    /// Moves files into their final home in `destination`, based on their DateTimeOriginal tag, and
    /// changes their file extensions to match their format. This unifies extensions per file type
    /// (e.g. jpeg vs jpg) and fixes incorrect renaming of mov to mp4.
    /// TODO: This renames based on the DateTimeOriginal of the target file, not of the XMP.
    pub fn move_files_and_rename_empties_catalog(&mut self, destination: &Path) {
        log::info!("Moving and renaming files.");

        for (path, media) in &self.media_files {
            log::debug!("{}: Moving & renaming.", path.display());

            if media.metadata.date_time_original.is_none() {
                if media.metadata.create_date.is_some() {
                    if media.xmp.is_none() {
                        log::warn!("{}: No DateTimeOriginal tag in media file, but CreateDate is present. Falling back to CreateDate.", path.display());
                    } else {
                        log::error!("{}: Trying to fall back to CreateDate, but XMP is present. Cannot move & rename. Skipping.", path.display());
                        continue;
                    }
                } else if media.xmp.is_none() {
                    log::error!("{}: No DateTimeOriginal tag in media file, and no XMP present. Cannot move & rename. Skipping.", path.display());
                    continue;
                } else if self
                    .xmps
                    .get(media.xmp.as_ref().unwrap())
                    .unwrap()
                    .metadata
                    .date_time_original
                    .is_none()
                {
                    log::error!("{}: No DateTimeOriginal tag in media file or XMP. Cannot move & rename. Skipping.", path.display());
                    continue;
                }
            }

            // TODO: FileModifyDate in certain cases?
            // TODO: check CreateDate is valid.
            let datetime_tag = if media.metadata.date_time_original.is_some() {
                "DateTimeOriginal"
            } else {
                "CreateDate"
            };
            let media_file_ext = &media.metadata.file_type_extension;
            let media_file_rename_format = format!(
                "-FileName<{}/${{{}}}.{}",
                destination.to_str().unwrap(),
                datetime_tag,
                media_file_ext
            );

            if let Some(xmp) = &media.xmp {
                log::warn!("{:?}", xmp);
                // Sanity check for CreateDate case (which should never come through).
                assert!(
                    media.metadata.date_time_original.is_some(),
                    "XMP present, but no DateTimeOriginal tag in media file. Cannot move & rename."
                );
                log::debug!(
                    "{}: Moving XMP alongside {}.",
                    xmp.display(),
                    path.display()
                );

                // If XMP exists, prefer its tags over those in the media file.
                // Note: this will write the XMP tags to the image.
                util::run_exiftool([
                    "-tagsFromFile",
                    xmp.to_str().unwrap(),
                    "-d",
                    "%Y/%m/%y%m%d_%H%M%S%%+c",
                    &media_file_rename_format,
                    path.to_str().unwrap(),
                ]);

                // TODO fix formatting
                // Move XMP as well, keeping "file.ext.xmp" format.
                let xmp_rename_format = format!(
                    "-FileName<{}/${{DateTimeOriginal}}.{}.xmp",
                    destination.to_str().unwrap(),
                    media_file_ext
                );
                util::run_exiftool([
                    "-d",
                    "%Y/%m/%y%m%d_%H%M%S%%+c",
                    &xmp_rename_format,
                    xmp.to_str().unwrap(),
                ]);
            } else {
                // No XMP, so use tags in file's metadata only.
                util::run_exiftool([
                    "-d",
                    "%Y/%m/%y%m%d_%H%M%S%%+c",
                    &media_file_rename_format,
                    path.to_str().unwrap(),
                ]);
            }
        }

        self.hack_clear_catalog();
    }
}

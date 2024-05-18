//! Core catalog management type and functionality.
//!
//! Copyright 2023-4 Seth Pendergrass. See LICENSE.

use serde::Deserialize;
use std::{
    collections::HashMap,
    path::{Path, PathBuf},
};

use super::exiftool;
use super::util;

#[derive(Clone, Deserialize)]
#[serde(rename_all = "PascalCase")]
struct Metadata {
    source_file: PathBuf,
    file_type: String,
    file_type_extension: String,
    content_identifier: Option<String>, // Live Photo images & videos.
    create_date: Option<String>,        // Time of image write or photo scan.
    date_time_original: Option<String>, // Time of shutter actuation.
                                        // TODO read artist & copyright
                                        // TODO read camera make & model
                                        // TODO read GPS
}

struct Media {
    // TODO: darktable duplicates
    xmp: Option<PathBuf>,
    metadata: Metadata,
}

struct Xmp {
    media: Option<PathBuf>,
    metadata: Metadata,
}

pub struct CatalogManager {
    trash: Option<PathBuf>,
    media_files: HashMap<PathBuf, Media>,
    xmps: HashMap<PathBuf, Xmp>,
    // Vec in case of duplicate items (e.g. jpg & HEIC).
    // Note: Only HEIC and JPEG types.
    live_photo_images: HashMap<String, Vec<PathBuf>>,
    // Note: Only MP4 and MOV types.
    live_photo_videos: HashMap<String, Vec<PathBuf>>,
}

// TODO add fn that artist/copyright, GPS filled out
// TODO make list of my cameras
impl CatalogManager {
    /// Move file to trash, if trash is Some().
    fn remove(&mut self, path: &Path) {
        // Delete media file and get XMP file path, if present.

        let xmp_path = if let Some(media) = self.media_files.remove(path) {
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

            // Move file to trash, if one is specified.
            if let Some(trash) = &self.trash {
                log::debug!("{}: Moving to trash.", path.display());
                util::move_to_trash(path, trash);
            }

            media.xmp
        } else {
            Some(path.to_path_buf())
        };

        // Delete XMP file.

        if let Some(xmp_path) = xmp_path {
            if self.xmps.remove(&xmp_path).is_some() {
                if let Some(trash) = &self.trash {
                    log::debug!("{}: Moving sidecar file to trash.", path.display());
                    util::move_to_trash(path, trash);
                }
            } else {
                panic!(
                    "{}: Unable to remove file as it does not exist in the catalog.",
                    path.display()
                );
            }
        }
    }

    /// Create a new catalog of library, with trash as the destination for removed files.
    fn new(directory: &Path, trash: Option<&Path>) -> Self {
        log::info!("Building catalog.");

        // Gather metadata.
        let stdout = exiftool::collect_metadata(directory, trash);

        // Parse exiftool output.
        let Ok(metadata) = serde_json::from_slice::<Vec<Metadata>>(&stdout[..]) else {
            panic!("Failed to parse exiftool output.");
        };

        // Media file <-> XMP file mapping.

        // Split media from xmp based on exiftool FileType.
        let (file_metadata, xmp_metadata): (Vec<_>, Vec<_>) =
            metadata.into_iter().partition(|m| m.file_type != "XMP");

        // Add media files to catalog with empty XMP refs.
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

        // Add XMP files to catalog with empty media refs.
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

        // Match media files with XMP files.
        for (path, metadata) in media_files.iter_mut() {
            // .ext -> .ext.xmp.
            assert!(
                path.extension().is_some(),
                "{}: Media file without extension.",
                path.display()
            );

            // TODO: collect *all* XMPs to enable darktable duplicates.
            let expected_xmp = util::xmp_path_from_file_path(path);

            if xmps.contains_key(&expected_xmp) {
                metadata.xmp = Some(expected_xmp.clone());
                xmps.get_mut(&expected_xmp).unwrap().media = Some(path.clone());
            }
        }

        // Sanity check: In case I switch to Adobe at some point (which uses file.xmp instead of
        // file.jpg.xmp).
        util::sanity_check_xmp_filenames(&xmps);

        // Live Photo setup.
        // Note: Live Photos images and videos are expected to be stored side-by-side. Any scanned
        // directory with one but not the other indicates a deletion.

        // TODO pull out to top.
        let image_file_types = ["HEIC", "JPEG"];
        let video_file_types = ["MOV", "MP4"];

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
            trash: trash.map(|p| p.to_path_buf()),
            media_files,
            xmps,
            live_photo_images,
            live_photo_videos,
        }
    }

    /// Loads an existing library for maintenance. Removed files will be moved to `trash`.
    /// Note: If `trash` lies within `library`, files within will not be scanned.
    pub fn load_library(library: &Path, trash: &Path) -> Self {
        Self::new(library, Some(trash))
    }

    /// Scans `import` for files to import into a catalog.
    pub fn import(import: &Path) -> Self {
        Self::new(import, None)
    }

    /// If there are multiple Live Photo images or videos sharing the same ID, assume that there has
    /// been duplication. Most often, this is because a photo exists as both a JPG and HEIC. Images
    /// prefer HEIC over JPG, while videos are purely based on size.
    pub fn remove_duplicates_from_live_photos(&mut self) {
        log::info!("Removing duplicates from Live Photos.");

        // Delete duplicate images.

        // TODO assert in types

        // Prefer HEIC over JPG, and prefer the largest image.
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
                "{}: Live Photo image has the following duplicates:",
                path_keep.display()
            );
            for path in paths_delete.iter() {
                log::warn!("\t- {}", path.display());
                self.remove(path);
            }
        }

        // Delete duplicate videos.

        // Prefer the largest video.
        let mut videos_to_remove = HashMap::new();
        for paths in self.live_photo_videos.values() {
            if paths.len() > 1 {
                let (largest, others) = util::filter_out_largest(paths);
                videos_to_remove.insert(largest, others);
            }
        }
        for (path_keep, paths_delete) in &videos_to_remove {
            log::warn!(
                "{}: Live Photo video has the following duplicates:",
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

        // Collect videos without corresponding images.

        let mut to_remove = Vec::new();
        for (id, paths) in &self.live_photo_videos {
            if !self.live_photo_images.contains_key(id) {
                to_remove.extend_from_slice(paths);
            }
        }

        // Remove the videos.

        for path in to_remove {
            log::warn!(
                "{}: Video remaining from presumably deleted Live Photo image.",
                path.display()
            );
            self.remove(&path);
        }
    }

    /// Copy specific metadata from Live Photo images to videos. This saves having to duplicate work
    /// tagging, geotagging, etc.
    pub fn copy_metadata_from_live_photo_image_to_video(&mut self) {
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
            }
            if video_paths.len() > 1 {
                log::error!("{}: Multiple Live Photo images with ID {}. Cannot copy metadata to video. Skipping.", video_paths[0].display(), id);
                continue;
            }

            // Select source metadata.

            let image_path = &image_paths[0];
            let video_path = &video_paths[0];
            log::debug!(
                "{}: Copying metadata to {}.",
                image_path.display(),
                video_path.display()
            );
            // Prefer image XMP as copy source, if present.
            // HACK: Using owned copies to avoid borrowing media_files mutably twice for both the
            // image and video.
            // TODO: handle multiple XMPs.
            let image = self.media_files.get(image_path).unwrap();
            let (copy_src_path, metadata) = match &image.xmp {
                Some(xmp_path) => {
                    log::debug!(
                        "{}: Using {} as metadata source.",
                        image_path.display(),
                        xmp_path.display()
                    );
                    (
                        xmp_path.clone(),
                        self.xmps.get(xmp_path).unwrap().metadata.clone(),
                    )
                }
                None => (image_path.clone(), image.metadata.clone()),
            };

            // Copy image metadata to the video.

            exiftool::copy_metadata(&copy_src_path, video_path);
            // Update catalog to reflect new metadata.
            self.media_files.get_mut(video_path).unwrap().metadata = metadata.clone();

            // If the video XMP exists, copy metadata to it as well.

            let video = self.media_files.get(video_path).unwrap();
            if let Some(video_xmp_path) = &video.xmp {
                exiftool::copy_metadata(&copy_src_path, video_xmp_path);
                // Update catalog to reflect new metadata.
                self.xmps.get_mut(video_xmp_path).unwrap().metadata = metadata.clone();
            }
        }
    }

    /// If a sidecar XMP is around with no possible target file, remove it.
    /// TODO: Not safe for darktable duplicates.
    pub fn remove_sidecars_without_references(&mut self) {
        log::info!("Removing XMP sidecars without corresponding files.");

        // Collect all XMPs where we did not find a corresponding media file.
        // Note: XMPs are assumed to be stored side-by-side with their media files, such that if we
        // did not find the referenced media file then we can assume it no longer exists.

        let mut to_remove = Vec::new();
        for (path, metadata) in &self.xmps {
            if metadata.media.is_none() {
                to_remove.push(path.clone());
            }
        }

        // Remove the XMPs.

        for path in to_remove {
            log::warn!(
                "{}: XMP sidecar without corresponding media file.",
                path.display()
            );
            self.remove(&path);
        }
    }

    /// Ensures every file has an associated XMP sidecar, creating one if not already present.
    pub fn create_xmp_sidecars_if_missing(&mut self) {
        log::info!("Ensuring all media files have associated XMP sidecar.");

        for (path, media) in &mut self.media_files {
            if media.xmp.is_none() {
                log::debug!("{}: Creating XMP sidecar.", path.display());

                exiftool::create_xmp(path);

                // Update catalog to reflect new XMP.

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
    /// Note: This consumes the catalog, as its metadata will no longer be valid.
    pub fn finalize_move_and_rename_files(mut self, destination: &Path) {
        log::info!("Moving and renaming files.");

        for (media_path, media) in &self.media_files {
            log::debug!("{}: Moving & renaming.", media_path.display());

            // Prefer XMP metadata, if present.

            let (tag_src_path, metadata) = match &media.xmp {
                Some(xmp_path) => {
                    log::debug!(
                        "{}: Using {} as metadata source.",
                        media_path.display(),
                        xmp_path.display()
                    );
                    (xmp_path, &self.xmps.get(xmp_path).unwrap().metadata)
                }
                None => (media_path, &media.metadata),
            };

            // Select tag used for renaming.

            // TODO do not use createdate
            let datetime_tag = if metadata.date_time_original.is_some() {
                "DateTimeOriginal"
            } else if metadata.create_date.is_some() {
                "CreateDate"
            } else {
                log::error!(
                    "{}: No suitable datetime tag found for rename. Skipping.",
                    media_path.display()
                );
                continue;
            };

            // Move and rename the media file.

            let media_file_ext = &metadata.file_type_extension;
            let media_file_rename_format = format!(
                "-FileName<{}/${{{}}}.{}",
                destination.to_str().unwrap(),
                datetime_tag,
                media_file_ext
            );
            exiftool::rename_file(&media_file_rename_format, media_path, Some(tag_src_path));

            // If present, move and rename the XMP file.

            if let Some(xmp_path) = &media.xmp {
                log::debug!(
                    "{}: Moving XMP with {}.",
                    xmp_path.display(),
                    media_path.display()
                );

                // Move XMP as well, keeping "file.ext.xmp" format.
                let xmp_rename_format = format!(
                    "-FileName<{}/${{{}}}.{}.xmp",
                    destination.to_str().unwrap(),
                    datetime_tag,
                    media_file_ext
                );
                exiftool::rename_file(&xmp_rename_format, xmp_path, None);
            }
        }

        // Redundant safety drop to *really* be sure this catalog is no longer valid.
        drop(self);
    }
}

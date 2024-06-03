//! Type for managing linkage between Live Photo images and videos.
//!
//! Copyright 2023-4 Seth Pendergrass. See LICENSE.

use super::catalog::Catalog;
use chrono::DateTime;
use std::collections::HashMap;
use std::path::PathBuf;

pub struct LivePhotoMapping {
    // Vec in case of duplicate items (e.g. jpg & HEIC).
    live_photo_images: HashMap<String, Vec<PathBuf>>,
    live_photo_videos: HashMap<String, Vec<PathBuf>>,
}

impl LivePhotoMapping {
    //
    // Constructor.
    //

    /// Creates a new `LivePhotoMapping` linking Live Photo images to videos based on the value of
    /// the `ContentIdentifier` tag.
    /// TODO: This does not use or check associated sidecar files.
    pub fn new(catalog: &Catalog) -> Self {
        let mut live_photo_images = HashMap::new();
        let mut live_photo_videos = HashMap::new();

        for media in catalog.iter_media() {
            if let Some(id) = &media.metadata.content_identifier {
                if media.is_live_photo_image() {
                    log::debug!(
                        "{}: Live Photo image with ID {}.",
                        &media.metadata.source_file.display(),
                        id
                    );
                    live_photo_images
                        .entry(id.clone())
                        .or_insert_with(Vec::new)
                        .push(media.metadata.source_file.clone());
                } else if media.is_live_photo_video() {
                    log::debug!(
                        "{}: Live Photo video with ID {}.",
                        &media.metadata.source_file.display(),
                        id
                    );
                    live_photo_videos
                        .entry(id.clone())
                        .or_insert_with(Vec::new)
                        .push(media.metadata.source_file.clone());
                }
            }
        }

        Self {
            live_photo_images,
            live_photo_videos,
        }
    }

    // pub fn contains(&self, media: &Media) -> bool {
    //     if let Some(id) = &media.metadata.content_identifier {
    //         self.live_photo_images.contains_key(id) || self.live_photo_videos.contains_key(id)
    //     } else {
    //         false
    //     }
    // }

    // pub fn remove(&mut self, media: &Media) {
    //     let id = media.metadata.content_identifier.as_ref().unwrap();

    //     if self.live_photo_images.contains_key(id) {
    //         self.live_photo_images
    //             .get_mut(id)
    //             .unwrap()
    //             .retain(|p| p != &media.metadata.source_file);
    //     } else if self.live_photo_videos.contains_key(id) {
    //         self.live_photo_videos
    //             .get_mut(id)
    //             .unwrap()
    //             .retain(|p| p != &media.metadata.source_file);
    //     } else {
    //         panic!(
    //             "{}: Live Photo not found in mapping during removal.",
    //             &media.metadata.source_file.display()
    //         );
    //     }
    // }

    /// Removes all duplicate images and videos from the Live Photo map. This will keep the newest
    /// image and video, preferring HEIC over JPG for images.
    /// TODO: catalog only used to check file type. Maybe this should be broken out somehow?
    pub fn remove_duplicates(&mut self, catalog: &Catalog) -> Vec<(PathBuf, Vec<PathBuf>)> {
        let mut remove = Vec::new();

        // Images.
        for paths in self
            .live_photo_images
            .values_mut()
            .filter(|paths| paths.len() > 1)
        {
            let (heic_paths, jpg_paths): (Vec<PathBuf>, Vec<PathBuf>) = paths
                .drain(..)
                .partition(|p| catalog.get(p).file_type == "HEIC");

            match heic_paths.len() {
                // No HEICs, so just keep the newest JPG.
                0 => {
                    let (keep, remove_images) = Self::split_out_newest(catalog, jpg_paths);
                    *paths = vec![keep.clone()];
                    remove.push((keep, remove_images));
                }
                // One HEIC, so keep it and delete the rest.
                1 => {
                    *paths = heic_paths.clone();
                    remove.push((heic_paths[0].clone(), jpg_paths));
                }
                // Multiple HEICs, so keep the newest HEIC.
                _ => {
                    let (keep, remove_images) = Self::split_out_newest(catalog, heic_paths);
                    *paths = vec![keep.clone()];
                    remove.push((keep, remove_images));
                }
            }
        }

        // Videos.
        for paths in self
            .live_photo_videos
            .values_mut()
            .filter(|paths| paths.len() > 1)
        {
            let (keep, remove_images) = Self::split_out_newest(catalog, paths.drain(..).collect());
            *paths = vec![keep.clone()];
            remove.push((keep, remove_images));
        }

        remove
    }

    /// Removes any Live Photo videos that do not have a corresponding image.
    /// This assumes that the image(s) were purposely deleted, and as such so should the videos.
    pub fn remove_leftover_videos(&mut self) -> Vec<PathBuf> {
        let (keep, remove) = self
            .live_photo_videos
            .drain()
            .partition(|(id, _)| self.live_photo_images.contains_key(id));
        self.live_photo_videos = keep;

        remove.into_values().flatten().collect()
    }

    /// Creates an iterator over all paired Live Photo images and videos, returning all media files
    /// sharing the same `ContentIdentifier` as a pair of (images, videos).
    /// In cases where images exist without videos, they will be returned. However, videos without
    /// imaages will *not*.
    pub fn iter(&self) -> LivePhotoIterator {
        LivePhotoIterator::new(self)
    }

    //
    // Private.
    //

    /// Given a vector of paths, this splits out the most recently modify file (based on
    /// `FileModifyDate`) and returns it separated from all other paths.
    fn split_out_newest(catalog: &Catalog, vec: Vec<PathBuf>) -> (PathBuf, Vec<PathBuf>) {
        let to_datetime =
            |p: &PathBuf| DateTime::parse_from_rfc3339(&catalog.get(p).file_modify_date).unwrap();
        let max = vec.iter().map(|p| to_datetime(p)).max().unwrap();
        let (newest, remaining): (Vec<PathBuf>, Vec<PathBuf>) =
            vec.into_iter().partition(|p| to_datetime(p) == max);
        (newest[0].clone(), remaining)
    }
}

pub struct LivePhotoIterator<'a> {
    live_photo_mapping: &'a LivePhotoMapping,
    photo_iterator: std::collections::hash_map::Iter<'a, String, Vec<PathBuf>>,
}

impl<'a> LivePhotoIterator<'a> {
    fn new(live_photo_mapping: &'a LivePhotoMapping) -> Self {
        Self {
            live_photo_mapping,
            photo_iterator: live_photo_mapping.live_photo_images.iter(),
        }
    }
}

impl<'a> Iterator for LivePhotoIterator<'a> {
    type Item = (Vec<PathBuf>, Vec<PathBuf>);

    fn next(&mut self) -> Option<Self::Item> {
        for (id, image_paths) in self.photo_iterator.by_ref() {
            if let Some(video_paths) = self.live_photo_mapping.live_photo_videos.get(id) {
                return Some((image_paths.clone(), video_paths.clone()));
            }
        }

        None
    }
}

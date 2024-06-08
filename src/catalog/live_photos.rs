//! Type for managing linkage between Live Photo images and videos.
//!
//! Copyright 2023-4 Seth Pendergrass. See LICENSE.

use super::catalog::Catalog;
use super::file::FileHandle;
use chrono::{DateTime, FixedOffset};
use std::collections::HashMap;
use std::path::PathBuf;

pub struct LivePhotoMapping {
    // Vec in case of duplicate items (e.g. jpg & HEIC).
    live_photo_images: HashMap<String, Vec<FileHandle>>,
    live_photo_videos: HashMap<String, Vec<FileHandle>>,
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

        for (file_handle, media) in catalog.iter_media() {
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
                        .push(*file_handle);
                } else if media.is_live_photo_video() {
                    log::debug!(
                        "{}: Live Photo video with ID {}.",
                        &media.metadata.source_file.display(),
                        id
                    );
                    live_photo_videos
                        .entry(id.clone())
                        .or_insert_with(Vec::new)
                        .push(*file_handle);
                }
            }
        }

        Self {
            live_photo_images,
            live_photo_videos,
        }
    }

    /// Removes all duplicate images and videos from the Live Photo map. This will keep the newest
    /// image and video, preferring HEIC over JPG for images.
    pub fn remove_duplicates(&mut self, catalog: &Catalog) -> Vec<(FileHandle, Vec<FileHandle>)> {
        let mut remove = Vec::new();

        // Images.
        // For each content identifier with multiple images:
        for handles in self
            .live_photo_images
            .values_mut()
            .filter(|paths| paths.len() > 1)
        {
            // Remove all handles to these duplicate images from live_photo_images and partition into
            // HEIC and JPG.
            // TODO line length
            let (heic, jpg): (Vec<FileHandle>, Vec<FileHandle>) = handles
                .drain(..)
                .partition(|p| catalog.get_metadata(p).file_type == "HEIC");

            match heic.len() {
                // No HEICs, so just keep the newest JPG.
                0 => {
                    let (keep, remove_images) = Self::split_out_newest(catalog, jpg);
                    *handles = vec![keep.clone()];
                    remove.push((keep, remove_images));
                }
                // One HEIC, so keep it and delete the rest.
                1 => {
                    *handles = heic.clone();
                    remove.push((heic[0].clone(), jpg));
                }
                // Multiple HEICs, so keep the newest HEIC.
                _ => {
                    let (keep, remove_images) = Self::split_out_newest(catalog, heic);
                    *handles = vec![keep.clone()];
                    remove.push((keep, remove_images));
                }
            }
        }

        // Videos.
        for handles in self
            .live_photo_videos
            .values_mut()
            .filter(|paths| paths.len() > 1)
        {
            let (keep, remove_images) = Self::split_out_newest(catalog, handles.drain(..).collect());
            *handles = vec![keep.clone()];
            remove.push((keep, remove_images));
        }

        remove
    }

    /// Removes any Live Photo videos that do not have a corresponding image.
    /// This assumes that the image(s) were purposely deleted, and as such so should the videos.
    pub fn remove_leftover_videos(&mut self) -> Vec<FileHandle> {
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

    /// Given a vector of FileHandles, this splits out the most recently modify file (based on
    /// `FileModifyDate`) and returns it separated from all other paths.
    /// TODO: this is a confusing function
    fn split_out_newest(catalog: &Catalog, vec: Vec<FileHandle>) -> (FileHandle, Vec<FileHandle>) {
        let max = vec.iter().map(|fh| Self::to_datetime(catalog, fh)).max().unwrap();
        let (newest, remaining): (Vec<FileHandle>, Vec<FileHandle>) =
            vec.into_iter().partition(|fh| Self::to_datetime(catalog, fh) == max);
        (newest[0].clone(), remaining)
    }

    fn to_datetime(catalog: &Catalog, file_handle: &FileHandle) -> DateTime<FixedOffset> {
        DateTime::parse_from_rfc3339(catalog.get_metadata(file_handle).file_modify_date.as_str()).unwrap()
    }
}

pub struct LivePhotoIterator<'a> {
    live_photo_mapping: &'a LivePhotoMapping,
    photo_iterator: std::collections::hash_map::Iter<'a, String, Vec<FileHandle>>, // TODO style
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
    type Item = (Vec<FileHandle>, Vec<FileHandle>);

    fn next(&mut self) -> Option<Self::Item> {
        for (id, image_paths) in self.photo_iterator.by_ref() {
            if let Some(video_paths) = self.live_photo_mapping.live_photo_videos.get(id) {
                return Some((image_paths.clone(), video_paths.clone()));
            }
        }

        None
    }
}

//! Type for managing linkage between Live Photo images and videos.
//!
//! Copyright 2023-4 Seth Pendergrass. See LICENSE.

use super::catalog::Catalog;
use super::file::FileHandle;
use chrono::{DateTime, FixedOffset};
use std::collections::hash_map;
use std::collections::HashMap;

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
    /// TODO: This does not use or check associated sidecar files. Assert if content identifier mismatch.
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

    //
    // Public.
    //

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
            let (heic, jpg): (Vec<_>, Vec<_>) = handles
                .drain(..)
                .partition(|p| catalog.get_metadata(p).file_type == "HEIC");

            match heic.len() {
                // No HEICs, so just keep the newest JPG.
                0 => {
                    let (keep, remove_images) = Self::split_out_newest(catalog, jpg);
                    *handles = vec![keep];
                    remove.push((keep, remove_images));
                }
                // One HEIC, so keep it and delete the rest.
                1 => {
                    *handles = heic.clone();
                    remove.push((heic[0], jpg));
                }
                // Multiple HEICs, so keep the newest HEIC.
                _ => {
                    let (keep, mut remove_images) = Self::split_out_newest(catalog, heic);
                    *handles = vec![keep];
                    remove_images.extend(jpg);
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
            let (keep, remove_images) =
                Self::split_out_newest(catalog, handles.drain(..).collect());
            *handles = vec![keep];
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
    /// images will *not*.
    pub fn iter(&self) -> LivePhotoIterator {
        LivePhotoIterator::new(self)
    }

    //
    // Private.
    //

    /// Given a vector of FileHandles, this splits out the most recently modify file (based on
    /// `FileModifyDate`) and returns it separated from all other paths.
    fn split_out_newest(catalog: &Catalog, vec: Vec<FileHandle>) -> (FileHandle, Vec<FileHandle>) {
        let max = vec
            .iter()
            .map(|fh| catalog.get_metadata(fh).get_file_modify_date())
            .max()
            .unwrap();
        let (newest, remaining): (Vec<_>, Vec<_>) = vec
            .into_iter()
            .partition(|fh| catalog.get_metadata(fh).get_file_modify_date() == max);
        (newest[0], remaining)
    }
}

pub struct LivePhotoIterator<'a> {
    live_photo_mapping: &'a LivePhotoMapping,
    photo_iterator: hash_map::Iter<'a, String, Vec<FileHandle>>,
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

#[cfg(test)]
mod test {
    use super::super::metadata::Metadata;
    use super::*;
    use std::path::PathBuf;

    fn new_metadata(path: &str, date: &str, file_type: &str, id: Option<&str>) -> Metadata {
        Metadata {
            source_file: PathBuf::from(path),
            file_modify_date: date.to_string(),
            file_type: file_type.to_string(),
            content_identifier: id.map(|s| s.to_string()),
            ..Default::default()
        }
    }

    /// Checks that iter traverses all Live Photo image/video pairs, and not any other files.
    #[test]
    fn test_iter() {
        let c = Catalog::new(vec![
            new_metadata("img_no_id.jpg", "", "JPEG", None),
            new_metadata("vid_no_id.mov", "", "MOV", None),
            new_metadata("img_live.jpg", "", "JPEG", Some("1")),
            new_metadata("vid_live.mov", "", "MOV", Some("1")),
        ]);
        let m = LivePhotoMapping::new(&c);

        let mut iter = m.iter();

        // Should only find files with Content Identifier.
        let (images, videos) = iter.next().unwrap();
        assert_eq!(images.len(), 1);
        assert_eq!(videos.len(), 1);
        assert_eq!(
            c.get_metadata(&images[0]).source_file,
            PathBuf::from("img_live.jpg")
        );
        assert_eq!(
            c.get_metadata(&videos[0]).source_file,
            PathBuf::from("vid_live.mov")
        );

        // No more files in mapping.
        assert!(iter.next().is_none());
    }

    /// Regardless of file modify date, should keep HEIC over JPG. Of HEIC images, the newest should
    /// be kept.
    #[test]
    fn test_remove_duplicates_heic_over_jpg() {
        let c = Catalog::new(vec![
            new_metadata("img.jpg", "2024-01-01 00:00:00 +0000", "JPEG", Some("1")),
            new_metadata("img.heic", "1970-01-01 00:00:00 +0000", "HEIC", Some("1")),
            new_metadata("img-1.heic", "2000-01-01 00:00:00 +0000", "HEIC", Some("1")),
        ]);
        let mut m = LivePhotoMapping::new(&c);

        let dupes = m.remove_duplicates(&c);

        // Even with more recently modified jpeg, should keep HEIC (first item).
        assert_eq!(dupes.len(), 1);
        assert_eq!(
            c.get_metadata(&dupes[0].0).source_file,
            PathBuf::from("img-1.heic")
        );

        // Second item should be other images.
        let files = dupes[0]
            .1
            .iter()
            .map(|fh| c.get_metadata(fh).source_file)
            .collect::<Vec<_>>();
        assert!(files.contains(&PathBuf::from("img.jpg")));
        assert!(files.contains(&PathBuf::from("img.heic")));
    }

    /// Newest heic image duplicate, based on content identifier, should be kept.
    #[test]
    fn test_remove_duplicates_keep_newest_heic() {
        let c = Catalog::new(vec![
            new_metadata("img2.heic", "2024-01-01 00:00:00 +0000", "HEIC", Some("2")),
            new_metadata(
                "img2-1.heic",
                "1970-01-01 00:00:00 +0000",
                "HEIC",
                Some("2"),
            ),
        ]);
        let mut m = LivePhotoMapping::new(&c);

        let dupes = m.remove_duplicates(&c);

        assert_eq!(dupes.len(), 1);
        assert_eq!(
            c.get_metadata(&dupes[0].0).source_file,
            PathBuf::from("img2.heic")
        );
        assert_eq!(dupes[0].1.len(), 1);
        assert_eq!(
            c.get_metadata(&dupes[0].1[0]).source_file,
            PathBuf::from("img2-1.heic")
        );
    }

    /// Newest jpeg image duplicate, based on content identifier, should be kept.
    #[test]
    fn test_remove_duplicates_keep_newest_jpeg() {
        let c = Catalog::new(vec![
            new_metadata("img1.jpg", "1970-01-01 00:00:00 +0000", "JPEG", Some("1")),
            new_metadata("img1-1.jpg", "2024-01-01 00:00:00 +0000", "JPEG", Some("1")),
        ]);
        let mut m = LivePhotoMapping::new(&c);

        let dupes = m.remove_duplicates(&c);

        assert_eq!(dupes.len(), 1);
        assert_eq!(
            c.get_metadata(&dupes[0].0).source_file,
            PathBuf::from("img1-1.jpg")
        );
        assert_eq!(dupes[0].1.len(), 1);
        assert_eq!(
            c.get_metadata(&dupes[0].1[0]).source_file,
            PathBuf::from("img1.jpg")
        );
    }

    /// Newest video duplicate, based on content identifier, should be kept.
    #[test]
    fn test_remove_duplicates_keep_newest_videos() {
        let c = Catalog::new(vec![
            new_metadata("vid.mov", "2024-01-01 00:00:00 +0000", "MOV", Some("1")),
            new_metadata("vid1.mov", "1970-01-01 00:00:00 +0000", "MOV", Some("1")),
        ]);
        let mut m = LivePhotoMapping::new(&c);

        let dupes = m.remove_duplicates(&c);

        // Should keep vid.mov.
        assert_eq!(dupes.len(), 1);
        assert_eq!(
            c.get_metadata(&dupes[0].0).source_file,
            PathBuf::from("vid.mov")
        );
        assert_eq!(dupes[0].1.len(), 1);
        assert_eq!(
            c.get_metadata(&dupes[0].1[0]).source_file,
            PathBuf::from("vid1.mov")
        );
    }

    /// Checks that timezones are read correctly.
    #[test]
    fn test_remove_duplicates_with_timezone() {
        let c = Catalog::new(vec![
            new_metadata("img.heic", "2000-01-01 00:00:00 -0700", "HEIC", Some("1")),
            new_metadata("img-1.heic", "2000-01-01 06:00:00 +0000", "HEIC", Some("1")),
        ]);
        let mut m = LivePhotoMapping::new(&c);

        let dupes = m.remove_duplicates(&c);

        // img.heic is newer when timezone taken into account.
        assert_eq!(dupes.len(), 1);
        assert_eq!(
            c.get_metadata(&dupes[0].0).source_file,
            PathBuf::from("img.heic")
        );
        assert_eq!(dupes[0].1.len(), 1);
        assert_eq!(
            c.get_metadata(&dupes[0].1[0]).source_file,
            PathBuf::from("img-1.heic")
        );
    }

    /// Tests that videos with content identifiers, but not an associated image, are removed. No
    /// other files should be removed.
    #[test]
    fn test_remove_leftover_videos() {
        let c = Catalog::new(vec![
            new_metadata("img_live.jpg", "", "JPEG", Some("1")),
            new_metadata("vid_live.mov", "", "MOV", Some("1")),
            new_metadata("img_live_deleted_vid.jpg", "", "JPEG", Some("2")),
            new_metadata("vid_live_deleted_img.mov", "", "MOV", Some("3")),
            new_metadata("vid_not_live.mp4", "", "MP4", None),
        ]);
        let mut m = LivePhotoMapping::new(&c);

        let l = m.remove_leftover_videos();

        // Should only remove vid_live_deleted_img.mp4.
        assert_eq!(l.len(), 1);
        assert_eq!(
            c.get_metadata(&l[0]).source_file,
            PathBuf::from("vid_live_deleted_img.mov")
        );
    }
}

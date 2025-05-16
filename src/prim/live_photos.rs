// Copyright 2023-5 Seth Pendergrass. See LICENSE.

//! Types for managing Live Photos, which consist of multiple media files.

use std::{cmp::Ordering, collections::BinaryHeap};

use chrono::{DateTime, FixedOffset};

use super::file_map::Handle;
use crate::prim::{Codec, Media};

/// Holds the `ContentIdentifier` tag from `ExifTool`, which identifies which
/// images and videos are a part of the same Live Photo.
#[derive(Clone, PartialEq, Eq, Hash)]
pub struct LivePhotoID(pub String);

/// Stores the components of a Live Photo by their `Handle`s.
#[derive(Default)]
pub struct LivePhotoLinker {
  images: BinaryHeap<LivePhotoLinkMetadata>,
  videos: BinaryHeap<LivePhotoLinkMetadata>,
}

impl LivePhotoLinker {
  /// Extract all `Handles`.
  pub fn drain(&mut self) -> impl Iterator<Item = Handle<Media>> + '_ {
    self
      .images
      .drain()
      .map(|i| i.handle())
      .chain(self.videos.drain().map(|v| v.handle()))
  }

  /// Extract all image `Handles`.
  pub fn drain_images(&mut self) -> impl Iterator<Item = Handle<Media>> + '_ {
    self.images.drain().map(|i| i.handle())
  }

  /// Extract all video `Handles`.
  pub fn drain_videos(&mut self) -> impl Iterator<Item = Handle<Media>> + '_ {
    self.videos.drain().map(|v| v.handle())
  }

  /// Returns the "best" image with the associated `ContentIdentifier`.
  pub fn get_image_best(&self) -> Handle<Media> {
    self.images.peek().unwrap().handle()
  }

  /// Returns the "best" video with the associated `ContentIdentifier`.
  pub fn get_video_best(&self) -> Handle<Media> {
    self.videos.peek().unwrap().handle()
  }

  /// Returns whether multiple images share this `ContentIdenfifier`, and
  /// therefore need deduplication.
  pub fn has_duplicate_images(&self) -> bool {
    self.images.len() > 1
  }

  /// Returns whether multiple videos share this `ContentIdenfifier`, and
  /// therefore need deduplication.
  pub fn has_duplicate_videos(&self) -> bool {
    self.videos.len() > 1
  }

  /// Link image via `Handle`.
  pub fn insert_image(&mut self, handle: Handle<Media>, image: &Media) {
    self.images.push(LivePhotoLinkMetadata::new(handle, image));
  }

  /// Link video via `Handle`.
  pub fn insert_video(&mut self, handle: Handle<Media>, video: &Media) {
    self.videos.push(LivePhotoLinkMetadata::new(handle, video));
  }

  /// Returns whether this `ContentIdentifier` has exactly one image and one
  /// video. If this is true, then this Live Photo is good and does not need
  /// deduplication.
  pub fn is_pair(&self) -> bool {
    self.images.len() == 1 && self.videos.len() == 1
  }

  /// Returns whether this `ContentIdentifier` has no linked images. If so, the
  /// linked video is likely leftover from a deleted Live Photo image, and
  /// should be deleted.
  pub fn is_leftover_videos(&self) -> bool {
    self.images.is_empty()
  }
}

/// Stores the subset of a file's metadata needed for deduplicating Live Photo
/// images and videos, alongside its unique `Handle`.
///
/// Implements custom `PartialOrd` and `Ord` traits to sort media files by
/// preference.
#[derive(PartialEq, Eq)]
pub struct LivePhotoLinkMetadata {
  media_handle:  Handle<Media>,
  codec:         Codec,
  last_modified: DateTime<FixedOffset>,
}

impl LivePhotoLinkMetadata {
  /// Creates a new `LivePhotoDedupeMetadata` for `handle` from `media`.
  pub fn new(handle: Handle<Media>, media: &Media) -> Self {
    Self {
      media_handle:  handle,
      codec:         media.get_codec(),
      last_modified: media.get_modify_date(),
    }
  }

  /// Gets the `Handle` this represents.
  pub fn handle(&self) -> Handle<Media> {
    self.media_handle
  }
}

impl PartialOrd for LivePhotoLinkMetadata {
  fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
    Some(self.cmp(other))
  }
}

impl Ord for LivePhotoLinkMetadata {
  fn cmp(&self, other: &Self) -> Ordering {
    match self.codec.cmp(&other.codec) {
      Ordering::Equal => self.last_modified.cmp(&other.last_modified),
      val => val,
    }
  }
}

#[cfg(test)]
mod test_live_photo_link_metadata {
  use std::collections::BinaryHeap;

  use super::*;
  use crate::{prim::Metadata, testing::*};

  /// Builds a max-heap of `LivePhotoLinkMetadata`, sorted by their `Ord`
  /// implementation.
  fn heap(metadata: &[Metadata]) -> BinaryHeap<LivePhotoLinkMetadata> {
    metadata
      .iter()
      .enumerate()
      .map(|(i, m)| LivePhotoLinkMetadata::new(i.into(), &Media::new(m.clone()).unwrap()))
      .collect::<BinaryHeap<_>>()
  }

  /// Converts a max-heap `heap` to a vector sorted in descending order.
  fn to_sorted_vec(heap: BinaryHeap<LivePhotoLinkMetadata>) -> Vec<usize> {
    heap.into_sorted_vec()
      .iter()
      .rev() // BinaryHeap is a max-heap, while into_sorted_vec() is ascending.
      .map(|d| usize::from(d.handle()))
      .collect::<Vec<_>>()
  }

  #[test]
  fn orders_by_date_time_if_same_format() {
    let dupes = heap(&[
      metadata!(
        "ModifyDate": "1975-01-01T00:00:00",
        "FileType": "HEIC",
      ),
      metadata!(
        "ModifyDate": "2025-01-01T00:00:00",
        "FileType": "HEIC",
      ),
      metadata!(
        "ModifyDate": "2000-01-01T00:00:00",
        "FileType": "HEIC",
      ),
    ]);

    assert_eq!(to_sorted_vec(dupes), [1, 2, 0]);
  }

  #[test]
  fn orders_by_format() {
    let dupes = heap(&[
      metadata!(
        "ModifyDate": "2000-01-01T00:00:00",
        "FileType": "HEIC",
      ),
      metadata!(
        "ModifyDate": "2000-01-01T00:00:00",
        "FileType": "PNG",
      ),
      metadata!(
        "ModifyDate": "2000-01-01T00:00:00",
        "FileType": "JPEG",
      ),
    ]);

    assert_eq!(to_sorted_vec(dupes), [0, 2, 1]);
  }

  #[test]
  fn orders_by_format_before_date_time() {
    let dupes = heap(&[
      metadata!(
        "ModifyDate": "2000-01-01T00:00:00",
        "FileType": "HEIC",
      ),
      metadata!(
        "ModifyDate": "1975-01-01T00:00:00",
        "FileType": "HEIC",
      ),
      metadata!(
        "ModifyDate": "2025-01-01T00:00:00",
        "FileType": "JPEG",
      ),
    ]);

    assert_eq!(to_sorted_vec(dupes), [0, 1, 2]);
  }
}

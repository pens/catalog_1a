// Copyright 2023-5 Seth Pendergrass. See LICENSE.

//! Media file handling.

use core::fmt;
use std::{
  cmp::Ordering,
  collections::HashSet,
  fmt::{Display, Formatter},
  path::Path,
  sync::LazyLock,
};

use chrono::{DateTime, FixedOffset};

use super::{Handle, LivePhotoID, Metadata, SidecarDupe, SidecarInitial};
use crate::prim::FileCategory;

static LIVE_PHOTO_IMAGE_EXTS: LazyLock<HashSet<&'static str>> =
  LazyLock::new(|| HashSet::from(["JPEG", "HEIC"]));
static LIVE_PHOTO_VIDEO_EXTS: LazyLock<HashSet<&'static str>> =
  LazyLock::new(|| HashSet::from(["MOV"]));

/// Live Photos are comprised of an image file and a video.
#[derive(PartialEq)]
pub enum LivePhotoComponentType {
  Image,
  Video,
}

/// Known codecs used by media files, used for deduplicating Live Photos.
/// Implements custom `Ord` and `PartialOrd` traits prioritizing preferred
/// codecs.
#[allow(clippy::upper_case_acronyms)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Codec {
  AVC,
  HEIC,
  HEVC,
  JPEG,
  Other,
}

impl Codec {
  fn rank(self) -> u8 {
    match self {
      Codec::HEIC | Codec::HEVC => u8::MAX,
      Codec::JPEG | Codec::AVC => u8::MAX - 1,
      Codec::Other => 0,
    }
  }
}

impl Display for Codec {
  fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
    match self {
      Codec::AVC => write!(f, "AVC"),
      Codec::HEIC => write!(f, "HEIC"),
      Codec::HEVC => write!(f, "HEVC"),
      Codec::JPEG => write!(f, "JPEG"),
      Codec::Other => write!(f, "Other"),
    }
  }
}

impl Ord for Codec {
  fn cmp(&self, other: &Self) -> Ordering {
    self.rank().cmp(&other.rank())
  }
}

impl PartialOrd for Codec {
  fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
    Some(self.rank().cmp(&other.rank()))
  }
}

/// Represents a single media file loaded from disk, including its metadata and,
/// optionally, handles to associated sidecars.
pub struct Media {
  metadata: Metadata,
  sidecar:  Option<Handle<SidecarInitial>>,
  dupes:    HashSet<Handle<SidecarDupe>>,
}

impl Media {
  /// Create from scanned `metadata`.
  pub fn new(metadata: Metadata) -> Result<Self, String> {
    let media = Self {
      metadata,
      sidecar: None,
      dupes: HashSet::new(),
    };

    if media.metadata.get_file_category() != FileCategory::Media {
      return Err(format!(
        "{}: Invalid media file type ({}).",
        media.metadata, media.metadata.file_type
      ));
    }

    let codec = media.get_codec();

    match media.get_live_photo_component_type() {
      Some(LivePhotoComponentType::Image) => {
        if codec != Codec::JPEG && codec != Codec::HEIC {
          return Err(format!(
            "{}: Unexpected Live Photo codec ({codec}).",
            media.metadata
          ));
        }
      }
      Some(LivePhotoComponentType::Video) => {
        if codec != Codec::AVC && codec != Codec::HEVC {
          return Err(format!(
            "{}: Unexpected Live Photo codec ({codec}).",
            media.metadata
          ));
        }
      }
      None => {
        if media.metadata.content_identifier.is_some() {
          return Err(format!(
            "{}: Unexpected Live Photo file type ({}).",
            media.metadata, media.metadata.file_type
          ));
        }
      }
    }

    Ok(media)
  }

  /// Gets the `ContentIdentifier` for this media file. Assumes this is a Live
  /// Photo.
  pub fn content_id(&self) -> Option<LivePhotoID> {
    Some(LivePhotoID(
      self.metadata.content_identifier.as_ref()?.clone(),
    ))
  }

  /// Adds a `Handle` to a duplicate sidecar, which holds metadata for
  /// additional edits to the same base media file in darktable.
  pub fn add_dupe(&mut self, sidecar: Handle<SidecarDupe>) {
    assert!(self.dupes.insert(sidecar));
  }

  /// Gets the `Codec` this media file is encodec with.
  pub fn get_codec(&self) -> Codec {
    match self.metadata.file_type.as_str() {
      "JPEG" => Codec::JPEG,
      "HEIC" => Codec::HEIC,
      "MOV" => match self.metadata.compressor_id.as_deref() {
        Some("avc1") => Codec::AVC,
        Some("hev1" | "hvc1") => Codec::HEVC,
        _ => Codec::Other,
      },
      _ => Codec::Other,
    }
  }

  /// Gets whether this media file is the image or video component of a Live
  /// Photo, or neither.
  pub fn get_live_photo_component_type(&self) -> Option<LivePhotoComponentType> {
    if self.metadata.content_identifier.is_some() {
      if LIVE_PHOTO_IMAGE_EXTS.contains(&self.metadata.file_type.as_str()) {
        Some(LivePhotoComponentType::Image)
      } else if LIVE_PHOTO_VIDEO_EXTS.contains(&self.metadata.file_type.as_str()) {
        Some(LivePhotoComponentType::Video)
      } else {
        None
      }
    } else {
      None
    }
  }

  /// Returns loaded metadata.
  pub fn get_metadata(&self) -> &Metadata {
    &self.metadata
  }

  /// Gets the most recent date of modification, either from the `ModifyDate`
  /// tag, if present, else the filesystem's modification timestamp.
  pub fn get_modify_date(&self) -> DateTime<FixedOffset> {
    // filter is to clear out `QuickTime:ModifyDate`, which reports as `0000:00:00
    // 00:00:00` when deleted.
    let modify_date = self
      .metadata
      .sub_sec_modify_date
      .as_deref()
      .or(self.metadata.modify_date.as_deref())
      .filter(|d| *d != "0000:00:00 00:00:00")
      .unwrap_or(&self.metadata.file_modify_date);

    let (date_time, tz) = super::parse_date_time(modify_date).unwrap();

    date_time
      .and_local_timezone(tz.unwrap_or(super::get_offset_local(&date_time)))
      .unwrap()
  }

  /// Returns the `Handle` to the initial (primary) sidecar, if it exists.
  pub fn get_sidecar(&self) -> Option<Handle<SidecarInitial>> {
    self.sidecar
  }

  /// Returns if this file does not have a sidecar linked. This does not
  /// necessarily reflect whether a sidecar exists on disk.
  pub fn is_missing_sidecar(&self) -> bool {
    self.sidecar.is_none()
  }

  /// Iterate over all duplicate sidecars.
  pub fn iter_dupes(&self) -> impl Iterator<Item = Handle<SidecarDupe>> + '_ {
    self.dupes.iter().copied()
  }

  /// Link a sidecar to this media file by `Handle`.
  pub fn set_sidecar(&mut self, sidecar: Handle<SidecarInitial>) {
    assert!(self.sidecar.is_none());
    self.sidecar = Some(sidecar);
  }

  /// Replace the metadata for this media file. For keeping this in sync after
  /// writing to metadata on disk.
  pub fn update_metadata(&mut self, metadata: Metadata) {
    self.metadata = metadata;
  }
}

impl AsRef<Path> for Media {
  fn as_ref(&self) -> &Path {
    self.metadata.as_ref()
  }
}

impl Display for Media {
  fn fmt(&self, f: &mut Formatter) -> fmt::Result {
    write!(f, "{}", self.metadata)
  }
}

#[cfg(test)]
mod test_new {
  use super::*;
  use crate::testing::*;

  #[test]
  fn errors_if_live_photo_and_unexpected_codec() {
    let metadata = metadata!(
      "SourceFile": "test.mov",
      "FileType": "MOV",
      "CompressorID": "h263",
      "ContentIdentifier": "ID",
    );

    assert_err!(Media::new(metadata), "Unexpected Live Photo codec");
  }

  #[test]
  fn errors_if_live_photo_and_unexpected_format() {
    let metadata = metadata!(
      "SourceFile": "test.png",
      "FileType": "PNG",
      "ContentIdentifier": "ID",
    );

    assert_err!(Media::new(metadata), "Unexpected Live Photo file type");
  }

  #[test]
  fn errors_if_xmp_sidecar() {
    let metadata = metadata!(
      "SourceFile": "test.xmp",
      "FileType": "XMP",
      "FileTypeExtension": "xmp",
    );

    assert_err!(Media::new(metadata), "Invalid media file type");
  }
}

#[cfg(test)]
mod test_get_live_photo_component_type {
  use super::*;
  use crate::testing::*;

  #[test]
  fn identifies_live_avc() {
    let video = Media::new(metadata!(
      "SourceFile": "test.mov",
      "FileType": "MOV",
      "CompressorID": "avc1",
      "ContentIdentifier": "ID",
    ))
    .unwrap();

    assert!(
      video
        .get_live_photo_component_type()
        .is_some_and(|t| t == LivePhotoComponentType::Video)
    );
  }

  #[test]
  fn identifies_live_heic() {
    let image = Media::new(metadata!(
      "SourceFile": "test.heic",
      "FileType": "HEIC",
      "ContentIdentifier": "ID",
    ))
    .unwrap();

    assert!(
      image
        .get_live_photo_component_type()
        .is_some_and(|t| t == LivePhotoComponentType::Image)
    );
  }

  #[test]
  fn identifies_live_hevc() {
    let video = Media::new(metadata!(
      "SourceFile": "test.mov",
      "FileType": "MOV",
      "CompressorID": "hvc1",
      "ContentIdentifier": "ID",
    ))
    .unwrap();

    assert!(
      video
        .get_live_photo_component_type()
        .is_none_or(|t| t != LivePhotoComponentType::Image)
    );
  }

  #[test]
  fn identifies_live_jpg() {
    let image = Media::new(metadata!(
      "SourceFile": "test.jpg",
      "FileType": "JPEG",
      "ContentIdentifier": "ID",
    ))
    .unwrap();

    assert!(
      image
        .get_live_photo_component_type()
        .is_some_and(|t| t == LivePhotoComponentType::Image)
    );
  }

  #[test]
  fn identifies_non_live_image() {
    let image = Media::new(metadata!(
      "SourceFile": "test.jpg",
      "FileType": "JPEG",
    ))
    .unwrap();

    assert!(
      image
        .get_live_photo_component_type()
        .is_none_or(|t| t != LivePhotoComponentType::Image)
    );
  }

  #[test]
  fn identifies_non_live_video() {
    let video = Media::new(metadata!(
      "SourceFile": "test.mov",
      "FileType": "MOV",
      "CompressorID": "avc1",
    ))
    .unwrap();

    assert!(
      video
        .get_live_photo_component_type()
        .is_none_or(|t| t != LivePhotoComponentType::Video)
    );
  }
}

#[cfg(test)]
mod test_get_codec {
  use super::*;
  use crate::testing::*;

  #[test]
  fn identifies_avc() {
    let media = Media::new(metadata!(
      "SourceFile": "test.mov",
      "FileType": "MOV",
      "CompressorID": "avc1",
    ))
    .unwrap();

    assert_eq!(media.get_codec(), Codec::AVC);
  }

  #[test]
  fn identifies_heic() {
    let media = Media::new(metadata!(
      "SourceFile": "test.heic",
      "FileType": "HEIC",
    ))
    .unwrap();

    assert_eq!(media.get_codec(), Codec::HEIC);
  }

  #[test]
  fn identifies_hevc() {
    {
      let media = Media::new(metadata!(
        "SourceFile": "test.mov",
        "FileType": "MOV",
        "CompressorID": "hvc1",
      ))
      .unwrap();

      assert_eq!(media.get_codec(), Codec::HEVC);
    }
    {
      let media = Media::new(metadata!(
        "SourceFile": "test.mov",
        "FileType": "MOV",
        "CompressorID": "hvc1",
      ))
      .unwrap();

      assert_eq!(media.get_codec(), Codec::HEVC);
    }
  }

  #[test]
  fn identifies_jpg() {
    let media = Media::new(metadata!(
      "SourceFile": "test.jpg",
      "FileType": "JPEG",
    ))
    .unwrap();

    assert_eq!(media.get_codec(), Codec::JPEG);
  }
}

#[cfg(test)]
mod test_get_modify_date {
  use super::*;
  use crate::testing::*;

  #[test]
  fn parses_from_exif() {
    let media = Media::new(metadata!(
      "SourceFile": "image.jpg",
      "FileType": "JPEG",
      "ModifyDate": "2000-01-01T00:00:00",
      "OffsetTime": "-08:00",
      "SubSecTime": 999,
      "SubSecModifyDate": "2000-01-01T00:00:00.999-08:00"
    ))
    .unwrap();

    assert_eq!(
      media.get_modify_date(),
      make_date(2000, 1, 1, 0, 0, 0, 999, -8)
    );
  }

  #[test]
  fn parses_zero_from_quicktime() {
    let media = Media::new(metadata!(
      "SourceFile": "image.mov",
      "FileType": "MOV",
      "CompressorID": "avc1",
      "FileModifyDate": "2000-01-01T00:00:00",
      "ModifyDate": "0000:00:00 00:00:00",
    ))
    .unwrap();

    assert_eq!(
      media.get_modify_date(),
      make_date_local(2000, 1, 1, 0, 0, 0, 0)
    );
  }

  #[test]
  fn parses_from_xmp() {
    let media = Media::new(metadata!(
      "SourceFile": "image.jpg",
      "FileType": "JPEG",
      "ModifyDate": "2000-01-01T00:00:00.999-08:00"
    ))
    .unwrap();

    assert_eq!(
      media.get_modify_date(),
      make_date(2000, 1, 1, 0, 0, 0, 999, -8)
    );
  }

  #[test]
  fn returns_file_modify_date_if_no_modify_date() {
    let media = Media::new(metadata!(
      "SourceFile": "image.jpg",
      "FileType": "JPEG",
      "FileModifyDate": "2000-01-01T00:00:00+00:00",
    ))
    .unwrap();

    assert_eq!(
      media.get_modify_date(),
      make_date(2000, 1, 1, 0, 0, 0, 0, 0)
    );
  }

  #[test]
  fn returns_modify_date_over_file_modify_date() {
    let media = Media::new(metadata!(
      "SourceFile": "image.jpg",
      "FileType": "JPEG",
      "FileModifyDate": "2025-01-01T00:00:00+00:00",
      "ModifyDate": "2000-01-01T00:00:00+00:00",
    ))
    .unwrap();

    assert_eq!(
      media.get_modify_date(),
      make_date(2000, 1, 1, 0, 0, 0, 0, 0)
    );
  }
}

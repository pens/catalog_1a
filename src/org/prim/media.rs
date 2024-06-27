//! Media file handling.
//!
//! Copyright 2023-4 Seth Pendergrass. See LICENSE.

use super::super::gbl;
use super::super::gbl::FileHandle;
use super::Metadata;
use std::{collections::HashSet, path::PathBuf};

pub struct Media {
  pub metadata: Metadata,
  pub sidecars: HashSet<FileHandle>,
}

impl Media {
  //
  // Constructor.
  //

  /// Creates a new Media object with Metadata and no referenced sidecars.
  pub fn new(metadata: Metadata) -> Self {
    Self::validate(&metadata);

    Self {
      metadata,
      sidecars: HashSet::new(),
    }
  }

  //
  // Public.
  //

  /// Returns the path to the base sidecar (i.e. not representing a duplicate). This is of the
  /// format `basename.ext.xmp` (not `basename_nn.ext.xmp`).
  /// This does *not* guaranee the sidecar exists.
  pub fn get_base_sidecar_path(&self) -> PathBuf {
    let mut ext = self
      .metadata
      .source_file
      .extension()
      .unwrap()
      .to_os_string();
    ext.push(".xmp");

    self.metadata.source_file.with_extension(ext)
  }

  /// Returns whether this file is a Live Photo image.
  pub fn is_live_photo_image(&self) -> bool {
    self.metadata.content_identifier.is_some()
      && gbl::LIVE_PHOTO_IMAGE_EXTS.contains(&self.metadata.file_type.as_str())
  }

  /// Returns whether this file is a Live Photo video.
  pub fn is_live_photo_video(&self) -> bool {
    self.metadata.content_identifier.is_some()
      && gbl::LIVE_PHOTO_VIDEO_EXTS.contains(&self.metadata.file_type.as_str())
  }

  //
  // Private.
  //

  /// Checks that the file has an extension, is not an XMP file and is a known Live Photo type.
  fn validate(metadata: &Metadata) {
    assert!(
      metadata.source_file.extension().is_some(),
      "{}: Media file without extension.",
      metadata.source_file.display()
    );

    assert!(
      metadata.source_file.extension().unwrap() != "xmp",
      "{}: Media file with `.xmp` extension.",
      metadata.source_file.display()
    );

    if metadata.content_identifier.is_some() {
      assert!(
        gbl::LIVE_PHOTO_IMAGE_EXTS.contains(&metadata.file_type.as_str())
          || gbl::LIVE_PHOTO_VIDEO_EXTS.contains(&metadata.file_type.as_str()),
        "{}: Unknown Live Photo type `{}`.",
        metadata.source_file.display(),
        metadata.file_type
      );
    }
  }
}

#[cfg(test)]
mod test {
  use super::*;

  fn new_media(path: &str) -> Media {
    let metadata = Metadata {
      source_file: PathBuf::from(path),
      ..Default::default()
    };

    Media::new(metadata)
  }

  fn new_live_media(path: &str, id: &str, ext: &str) -> Media {
    let metadata = Metadata {
      source_file: PathBuf::from(path),
      content_identifier: Some(id.to_string()),
      file_type: ext.to_string(),
      ..Default::default()
    };

    Media::new(metadata)
  }

  /// filename.ext -> filename.ext.xmp.
  #[test]
  fn test_get_base_sidecar_path() {
    let m1 = new_media("test.jpg");
    assert_eq!(m1.get_base_sidecar_path(), PathBuf::from("test.jpg.xmp"));

    let m2 = new_media("test.JPG");
    assert_eq!(m2.get_base_sidecar_path(), PathBuf::from("test.JPG.xmp"));

    let m3 = new_media("/path/to/test.JPG");
    assert_eq!(
      m3.get_base_sidecar_path(),
      PathBuf::from("/path/to/test.JPG.xmp")
    );
  }

  /// Do we correctly identify when an image is part of a Live Photo?
  #[test]
  fn test_is_live_photo_image() {
    let image = new_media("test.jpg");
    assert!(!image.is_live_photo_image());

    let live_jpg = new_live_media("test.jpg", "1", "JPEG");
    assert!(live_jpg.is_live_photo_image());

    let live_heic = new_live_media("test.heic", "1", "HEIC");
    assert!(live_heic.is_live_photo_image());

    let live_video = new_live_media("test.mov", "1", "MOV");
    assert!(!live_video.is_live_photo_image());
  }

  /// Do we correctly identify when an video is part of a Live Photo?
  #[test]
  fn test_is_live_photo_video() {
    let video = new_media("test.jpg");
    assert!(!video.is_live_photo_video());

    let live_jpg = new_live_media("test.jpg", "1", "JPEG");
    assert!(!live_jpg.is_live_photo_video());

    let live_heic = new_live_media("test.heic", "1", "HEIC");
    assert!(!live_heic.is_live_photo_video());

    let live_video = new_live_media("test.mov", "1", "MOV");
    assert!(live_video.is_live_photo_video());
  }

  /// Should panic to there is no file extension, even if exiftool can figure it out.
  #[test]
  #[should_panic(expected = "test: Media file without extension.")]
  fn test_missing_extension_panics() {
    new_media("test");
  }

  /// Should panic if unknown Live Photo type (e.g. in case of new standard).
  #[test]
  #[should_panic(expected = "test.jpg: Unknown Live Photo type `PNG`.")]
  fn test_unknown_live_photo_type_panics() {
    new_live_media("test.jpg", "1", "PNG");
  }

  /// Should panic if .xmp extension (which is not a media file).
  #[test]
  #[should_panic(expected = "test.xmp: Media file with `.xmp` extension.")]
  fn test_xmp_extension_panics() {
    new_media("test.xmp");
  }
}

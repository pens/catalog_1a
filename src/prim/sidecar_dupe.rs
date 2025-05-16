// Copyright 2023-5 Seth Pendergrass. See LICENSE.

//! Metadata sidecar file handling, for duplicate sidecar files as created by
//! dartktable.

use core::fmt;
use std::{
  ffi::OsStr,
  fmt::{Display, Formatter},
  path::Path,
};

use super::{Handle, Media, Metadata, Sidecar};
use crate::prim::FileCategory;

/// Holds metadata from a duplicate XMP sidecar, and an optional handle to the
/// associate media file.
pub struct SidecarDupe {
  metadata: Metadata,
  media:    Option<Handle<Media>>,
}

impl SidecarDupe {
  /// Create a new duplicate sidecar from the given metadata.
  pub fn new(metadata: Metadata) -> Result<Self, String> {
    if metadata.get_file_category() != FileCategory::SidecarDupe {
      return Err(format!(
        "{metadata}: Invalid sidecar duplicate file type ({}).",
        metadata.file_type
      ));
    }

    let parsed_name = metadata.parse_file_name();

    if parsed_name.is_none_or(|p| p.base_ext.eq_ignore_ascii_case("xmp")) {
      return Err(format!(
        "{metadata}: Invalid sidecar duplicate file extension."
      ));
    }

    Ok(Self {
      metadata,
      media: None,
    })
  }

  /// Extract the duplicate number from the sidecar's filename. Duplicate
  /// sidecars are identified by the present of this duplicate number, and as
  /// such will always have one.
  ///
  /// For example: Given a file `dir/image_01.jpg.xmp`, this would be `01`.
  pub fn get_dupe_number(&self) -> &OsStr {
    self
      .metadata
      .parse_file_name()
      .unwrap()
      .dupe_number
      .unwrap()
  }
}

impl AsRef<Path> for SidecarDupe {
  fn as_ref(&self) -> &Path {
    self.metadata.as_ref()
  }
}

impl Display for SidecarDupe {
  fn fmt(&self, f: &mut Formatter) -> fmt::Result {
    write!(f, "{}", self.metadata)
  }
}

impl Sidecar for SidecarDupe {
  fn get_media_handle(&self) -> Option<Handle<Media>> {
    self.media
  }

  fn get_metadata(&self) -> &Metadata {
    &self.metadata
  }

  fn set_media_handle(&mut self, media: Handle<Media>) {
    assert!(self.media.is_none());
    self.media = Some(media);
  }

  fn update_metadata(&mut self, metadata: Metadata) {
    self.metadata = metadata;
  }
}

#[cfg(test)]
mod test_new {
  use super::*;
  use crate::testing::*;

  #[test]
  fn errors_if_extension_only_xmp() {
    let metadata = metadata!(
      "SourceFile": "image_01.xmp",
      "FileType": "XMP",
    );

    assert_err!(
      SidecarDupe::new(metadata),
      "Invalid sidecar duplicate file extension."
    );
  }

  #[test]
  fn errors_if_not_dupe() {
    let metadata = metadata!(
      "SourceFile": "image.jpg.xmp",
      "FileType": "XMP",
    );

    assert_err!(
      SidecarDupe::new(metadata),
      "Invalid sidecar duplicate file type"
    );
  }
}

#[cfg(test)]
mod test_get_dupe_number {
  use super::*;
  use crate::testing::*;

  #[test]
  fn extracts_dupe_number() {
    let sidecar = SidecarDupe::new(metadata!(
      "SourceFile": "dir/image_01.jpg.xmp",
      "FileType": "XMP",
    ))
    .unwrap();

    assert_eq!(sidecar.get_dupe_number(), "01");
  }

  #[test]
  fn extracts_dupe_number_without_exiftool_dupe_letter() {
    let sidecar = SidecarDupe::new(metadata!(
      "SourceFile": "dir/image_b_01.jpg.xmp",
      "FileType": "XMP",
    ))
    .unwrap();

    assert_eq!(sidecar.get_dupe_number(), "01");
  }
}

#[cfg(test)]
mod test_get_media_path {
  use std::path::PathBuf;

  use super::*;
  use crate::testing::*;

  #[test]
  fn returns_media_path() {
    let sidecar = SidecarDupe::new(metadata!(
      "SourceFile": "dir/image_01.jpg.xmp",
      "FileType": "XMP",
    ))
    .unwrap();

    assert_eq!(sidecar.get_media_path(), PathBuf::from("dir/image.jpg"));
  }

  #[test]
  fn returns_media_path_with_exiftool_dupe_number() {
    let sidecar = SidecarDupe::new(metadata!(
      "SourceFile": "dir/image_b_01.jpg.xmp",
      "FileType": "XMP",
    ))
    .unwrap();

    assert_eq!(sidecar.get_media_path(), PathBuf::from("dir/image_b.jpg"));
  }
}

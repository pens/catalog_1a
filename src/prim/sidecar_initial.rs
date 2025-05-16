// Copyright 2023-5 Seth Pendergrass. See LICENSE.

//! Metadata sidecar file handling, for the initial (i.e. base/main/primary)
//! sidecar file.

use core::fmt;
use std::{
  fmt::{Display, Formatter},
  path::Path,
};

use super::{Handle, Media, Metadata, Sidecar};
use crate::prim::FileCategory;

/// Holds metadata from an XMP sidecar file on disk, and an optional handle to
/// the associated media file.
pub struct SidecarInitial {
  metadata: Metadata,
  media:    Option<Handle<Media>>,
}

impl SidecarInitial {
  /// Creates a new sidecar object with metadata but no linked media file.
  pub fn new(metadata: Metadata) -> Result<Self, String> {
    if metadata.get_file_category() != FileCategory::SidecarInitial {
      return Err(format!(
        "{metadata}: Invalid sidecar file type ({}).",
        metadata.file_type
      ));
    }

    let parsed_name = metadata.parse_file_name();

    if parsed_name.is_none_or(|p| p.base_ext.eq_ignore_ascii_case("xmp")) {
      return Err(format!("{metadata}: Invalid sidecar file extension."));
    }

    Ok(Self {
      metadata,
      media: None,
    })
  }
}

impl AsRef<Path> for SidecarInitial {
  fn as_ref(&self) -> &Path {
    self.metadata.as_ref()
  }
}

impl Display for SidecarInitial {
  fn fmt(&self, f: &mut Formatter) -> fmt::Result {
    write!(f, "{}", self.metadata)
  }
}

impl Sidecar for SidecarInitial {
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
      "SourceFile": "image.xmp",
      "FileType": "XMP",
    );

    assert_err!(
      SidecarInitial::new(metadata),
      "Invalid sidecar file extension."
    );
  }

  #[test]
  fn errors_if_not_xmp() {
    let metadata = metadata!(
      "SourceFile": "image.jpg",
      "FileType": "JPEG",
    );

    assert_err!(SidecarInitial::new(metadata), "Invalid sidecar file type");
  }
}

#[cfg(test)]
mod test_get_media_path {
  use std::path::PathBuf;

  use super::*;
  use crate::testing::*;

  #[test]
  fn returns_media_path() {
    let sidecar = SidecarInitial::new(metadata!(
      "SourceFile": "dir/image.jpg.xmp",
      "FileType": "XMP",
    ))
    .unwrap();

    assert_eq!(sidecar.get_media_path(), PathBuf::from("dir/image.jpg"));
  }

  #[test]
  fn returns_media_path_with_exiftool_dupe_number() {
    let sidecar = SidecarInitial::new(metadata!(
      "SourceFile": "dir/image_b.jpg.xmp",
      "FileType": "XMP",
    ))
    .unwrap();

    assert_eq!(sidecar.get_media_path(), PathBuf::from("dir/image_b.jpg"));
  }
}

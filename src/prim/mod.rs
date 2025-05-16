// Copyright 2023-5 Seth Pendergrass. See LICENSE.

//! Primitive types for representing multimedia files and their metadata, and
//! the relationships between them.

mod conv;
mod file_map;
mod live_photos;
mod media;
mod metadata;
mod sidecar_dupe;
mod sidecar_initial;

use std::path::PathBuf;

pub use conv::*;
pub use file_map::*;
pub use live_photos::*;
pub use media::*;
pub use metadata::*;
pub use sidecar_dupe::*;
pub use sidecar_initial::*;

/// Provides a shared interface to both "initial" and "duplicate" sidecars.
/// <https://docs.darktable.org/usermanual/development/en/overview/sidecar-files/sidecar/>.
pub trait Sidecar {
  /// Get handle to media file, if linked.
  fn get_media_handle(&self) -> Option<Handle<Media>>;

  /// Gets the path to the source file for this sidecar.
  /// This does *not* guarantee the file exists.
  fn get_media_path(&self) -> PathBuf {
    let parsed_file_name = self.get_metadata().parse_file_name().unwrap();
    PathBuf::from(parsed_file_name.parent_and_stem).with_extension(parsed_file_name.base_ext)
  }

  /// Get the metadata read from this sidecar file.
  fn get_metadata(&self) -> &Metadata;

  /// This file is a "leftover" sidecar file if it no longer has an associated
  /// media file. This generally means that the media file was deleted, and as
  /// such this sidecar should be, too.
  fn is_leftover(&self) -> bool {
    self.get_media_handle().is_none()
  }

  /// Link this sidecar to a media file via its handle.
  fn set_media_handle(&mut self, media: Handle<Media>);

  /// Update the metadata for this sidecar.
  /// This should be used after writing the the file on disk to keep the
  /// metadata in sync.
  fn update_metadata(&mut self, metadata: Metadata);
}

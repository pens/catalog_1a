// Copyright 2023-5 Seth Pendergrass. See LICENSE.

//! Core organizer module for managing a catalog of media files and their
//! sidecars.

mod stage_1_cleanup;
mod stage_2_sidecars;
mod stage_3_metadata;
mod stage_4_synchronization;
mod stage_5_validation;
mod stage_6_organization;

use std::{
  collections::{HashMap, HashSet},
  path::{Path, PathBuf},
};

use stage_3_metadata::MetadataUpdateConfig;
use stage_5_validation::ValidationConfig;

use crate::{
  io,
  prim::{
    FileCategory,
    FileMap,
    Handle,
    LivePhotoComponentType,
    LivePhotoID,
    LivePhotoLinker,
    Media,
    Metadata,
    Sidecar,
    SidecarDupe,
    SidecarInitial,
  },
};

/// Main type for organizing a multimedia catalog.
///
/// This can both clean up an existing catalog, or import new files into one.
/// Files are processed in a multi-stage process, which includes:
///
/// 1. Automatic removal of some files to a trash directory (or skip during
///    import).
/// 2. Creation of sidecars for any files without.
/// 3. Automatic updates to some basic metadata to save manual effort.
/// 4. Synchronization of metadata across files, including Live Photos.
/// 5. Validation of metadata, to prevent adding files not meeting standards
///    until fixed.
/// 6. Automatic moving and renaming by timestamp.
///
/// Note that the `Organizer` will assume that any file associations (e.g.
/// sidecars or Live Photos) are represented within the input directory. This
/// means, for example, that a Live Photo video imported separately from its
/// image will not be linked correctly.
#[derive(Default)]
pub struct Organizer {
  source: PathBuf,
  trash:  Option<PathBuf>,

  media:    FileMap<Media>,
  sidecars: FileMap<SidecarInitial>,
  dupes:    FileMap<SidecarDupe>,

  live_photo_map: HashMap<LivePhotoID, LivePhotoLinker>,

  metadata_updates: MetadataUpdateConfig,

  validation:  ValidationConfig,
  valid_media: HashSet<Handle<Media>>,
}

impl Organizer {
  /// Create a new `Organizer` importing all multimedia files from path
  /// (recursively).
  pub fn import(path: impl AsRef<Path>) -> Result<Self, String> {
    Self::new(path, None::<&Path>)
  }

  /// Create a new `Organizer` cleaning up an existing catalog at `path`,
  /// optionally moving files to `trash`.
  pub fn load_catalog(
    path: impl AsRef<Path>,
    trash: Option<impl AsRef<Path>>,
  ) -> Result<Self, String> {
    Self::new(path, trash)
  }

  /// Create a new `Organizer`.
  fn new(path: impl AsRef<Path>, trash: Option<impl AsRef<Path>>) -> Result<Self, String> {
    if path.as_ref().is_relative() {
      return Err(format!(
        "{}: Catalog path is not absolute.",
        path.as_ref().display()
      ));
    }

    if !path.as_ref().exists() {
      return Err(format!(
        "{}: Catalog path does not exist.",
        path.as_ref().display()
      ));
    }

    if let Some(trash) = &trash {
      if trash.as_ref().is_relative() {
        return Err(format!(
          "{}: Trash path is not absolute.",
          trash.as_ref().display()
        ));
      }
      if !trash.as_ref().exists() {
        return Err(format!(
          "{}: Trash path does not exist.",
          trash.as_ref().display()
        ));
      }
    }

    log::info!("{}: Building catalog.", path.as_ref().display());

    let mut organizer = Self {
      source: path.as_ref().to_path_buf(),
      trash: trash.map(|p| p.as_ref().to_path_buf()),
      ..Default::default()
    };

    let metadata = io::read_metadata_recursive(path, organizer.trash.as_ref())?;

    organizer.load_metadata(metadata)?;
    organizer.link_sidecars();
    organizer.link_live_photos();

    Ok(organizer)
  }

  /// Loads in all metadata (generally for `ExifTool`'s scan).
  fn load_metadata(&mut self, metadata: impl IntoIterator<Item = Metadata>) -> Result<(), String> {
    log::info!("Loading metadata.");

    load_metadata(
      &mut self.source,
      &mut self.media,
      &mut self.sidecars,
      &mut self.dupes,
      metadata,
    )
  }

  /// Links sidecar files to their associated media files. This is required as a
  /// second pass, as the organizer must be aware of all scanned files before
  /// linking can occur.
  fn link_sidecars(&mut self) {
    log::info!("Linking sidecar files to media.");

    link_sidecars_by_type(
      &self.source,
      &mut self.sidecars,
      &mut self.media,
      Media::set_sidecar,
    );
    link_sidecars_by_type(
      &mut self.source,
      &mut self.dupes,
      &mut self.media,
      Media::add_dupe,
    );
  }

  fn link_live_photos(&mut self) {
    log::info!("Linking Live Photos images to videos.");

    link_live_photos(&mut self.media, &mut self.live_photo_map);
  }
}

fn to_abs_path(dir: impl AsRef<Path>, path_rel: impl AsRef<Path>) -> PathBuf {
  dir.as_ref().join(path_rel).clone()
}

/// Converts metadata into collections of media files and sidecars.
fn load_metadata(
  dir_root: impl AsRef<Path>,
  media: &mut FileMap<Media>,
  sidecars: &mut FileMap<SidecarInitial>,
  dupes: &mut FileMap<SidecarDupe>,
  metadata: impl IntoIterator<Item = Metadata>,
) -> Result<(), String> {
  for m in metadata {
    match m.get_file_category() {
      FileCategory::Media => {
        media.insert(to_abs_path(&dir_root, &m), Media::new(m)?);
      }
      FileCategory::SidecarInitial => {
        sidecars.insert(to_abs_path(&dir_root, &m), SidecarInitial::new(m)?);
      }
      FileCategory::SidecarDupe => {
        dupes.insert(to_abs_path(&dir_root, &m), SidecarDupe::new(m)?);
      }
    }
  }

  Ok(())
}

/// Links sidecars to their associated media files. Generic over initial and
/// duplicate sidecars.
fn link_sidecars_by_type<S: Sidecar>(
  dir_root: impl AsRef<Path>,
  sidecar_map: &mut FileMap<S>,
  media_map: &mut FileMap<Media>,
  add_sidecar: fn(&mut Media, Handle<S>),
) {
  for (handle_sidecar, sidecar) in sidecar_map.iter_data_mut_indexed() {
    if let Some(handle_media) = media_map.find(to_abs_path(&dir_root, sidecar.get_media_path())) {
      add_sidecar(&mut media_map[handle_media], handle_sidecar);
      sidecar.set_media_handle(handle_media);
    }
  }
}

/// Link Live Photo images to their videos, and vice versa. This is based on the
/// `ContentIdentifier` tag from `ExifTool`.
fn link_live_photos(
  media_map: &mut FileMap<Media>,
  live_photo_map: &mut HashMap<LivePhotoID, LivePhotoLinker>,
) {
  for (media_handle, media) in media_map.iter_data_mut_indexed() {
    if let Some(comp_type) = media.get_live_photo_component_type() {
      let link = live_photo_map
        .entry(media.content_id().unwrap())
        .or_default();

      match comp_type {
        LivePhotoComponentType::Image => {
          link.insert_image(media_handle, media);
        }
        LivePhotoComponentType::Video => {
          link.insert_video(media_handle, media);
        }
      }
    }
  }
}

#[cfg(test)]
mod test_new {
  use super::*;
  use crate::testing::*;

  #[test]
  fn errors_if_catalog_path_does_not_exist() {
    assert_err!(
      Organizer::new("/path/does/not/exist", None::<&Path>),
      "Catalog path does not exist."
    );
  }

  #[test]
  fn errors_if_catalog_path_is_relative() {
    assert_err!(
      Organizer::new("relative/path", None::<&Path>),
      "Catalog path is not absolute."
    );
  }

  #[test]
  fn errors_if_trash_path_does_not_exist() {
    let d = test_dir!();
    assert_err!(
      Organizer::new(d.root(), Some("/path/does/not/exist")),
      "Trash path does not exist."
    );
  }

  #[test]
  fn errors_if_trash_path_is_relative() {
    let d = test_dir!();
    assert_err!(
      Organizer::new(d.root(), Some("relative/path")),
      "Trash path is not absolute."
    );
  }
}

#[cfg(test)]
mod test_load_metadata {
  use super::*;
  use crate::testing::*;

  #[test]
  fn loads_media() {
    let d = test_dir!(
      "image.jpg": {},
      "image.jpg.xmp": {},
      "image_01.jpg.xmp": {},
    );

    let mut media = FileMap::new();
    let mut sidecars = FileMap::new();
    let mut dupes = FileMap::new();
    let metadata = io::read_metadata_recursive(d.root(), d.some_trash()).unwrap();

    load_metadata(d.root(), &mut media, &mut sidecars, &mut dupes, metadata).unwrap();

    assert!(media.iter_data().count() == 1);
    assert!(media.find(d.get_path("image.jpg")).is_some());
    assert!(sidecars.iter_data().count() == 1);
    assert!(sidecars.find(d.get_path("image.jpg.xmp")).is_some());
    assert!(dupes.iter_data().count() == 1);
    assert!(dupes.find(d.get_path("image_01.jpg.xmp")).is_some());
  }
}

#[cfg(test)]
mod test_link_sidecars {
  use super::*;
  use crate::testing::*;

  #[test]
  fn links_sidecars() {
    let d = test_dir!(
      "image.jpg": {},
      "image.jpg.xmp": {},
      "image_01.jpg.xmp": {},
    );

    let mut media = FileMap::new();
    let mut sidecars = FileMap::new();
    let mut dupes = FileMap::new();
    let metadata = io::read_metadata_recursive(d.root(), d.some_trash()).unwrap();

    load_metadata(d.root(), &mut media, &mut sidecars, &mut dupes, metadata).unwrap();

    let handle_media = media.find(d.get_path("image.jpg")).unwrap();
    let handle_sidecar = sidecars.find(d.get_path("image.jpg.xmp")).unwrap();
    let handle_dupe = dupes.find(d.get_path("image_01.jpg.xmp")).unwrap();

    link_sidecars_by_type(d.root(), &mut sidecars, &mut media, Media::set_sidecar);

    assert_eq!(media[handle_media].get_sidecar(), Some(handle_sidecar));
    assert!(media[handle_media].iter_dupes().count() == 0);
    assert_eq!(
      sidecars[handle_sidecar].get_media_handle(),
      Some(handle_media)
    );
    assert!(dupes[handle_dupe].get_media_handle().is_none());
  }

  #[test]
  fn links_dupes() {
    let d = test_dir!(
      "image.jpg": {},
      "image.jpg.xmp": {},
      "image_01.jpg.xmp": {},
    );

    let mut media = FileMap::new();
    let mut sidecars = FileMap::new();
    let mut dupes = FileMap::new();
    let metadata = io::read_metadata_recursive(d.root(), d.some_trash()).unwrap();

    load_metadata(d.root(), &mut media, &mut sidecars, &mut dupes, metadata).unwrap();

    let handle_media = media.find(d.get_path("image.jpg")).unwrap();
    let handle_sidecar = sidecars.find(d.get_path("image.jpg.xmp")).unwrap();
    let handle_dupe = dupes.find(d.get_path("image_01.jpg.xmp")).unwrap();

    link_sidecars_by_type(d.root(), &mut dupes, &mut media, Media::add_dupe);

    assert!(media[handle_media].get_sidecar().is_none());
    assert_eq!(media[handle_media].iter_dupes().collect::<Vec<_>>(), vec![
      handle_dupe
    ]);
    assert!(sidecars[handle_sidecar].get_media_handle().is_none());
    assert_eq!(dupes[handle_dupe].get_media_handle(), Some(handle_media));
  }
}

#[cfg(test)]
mod test_link_live_photos {
  use super::*;
  use crate::testing::*;

  #[test]
  fn links_live_photo() {
    let d = test_dir!(
      "image.heic": {
        "ContentIdentifier": "ID",
      },
      "image_dupe.jpg": {
        "ContentIdentifier": "ID",
      },
      "video.mov": {
        "ContentIdentifier": "ID",
        "CompressorID": "hvc1",
      },
      "video_dupe.mov": {
        "ContentIdentifier": "ID",
        "CompressorID": "avc1",
      },
      "not_live.jpg": {}
    );

    let mut media = FileMap::new();
    let mut sidecars = FileMap::new();
    let mut dupes = FileMap::new();
    let mut live_photos = HashMap::new();
    let metadata = io::read_metadata_recursive(d.root(), d.some_trash()).unwrap();

    load_metadata(d.root(), &mut media, &mut sidecars, &mut dupes, metadata).unwrap();

    let handle_image = media.find(d.get_path("image.heic")).unwrap();
    let handle_image_dupe = media.find(d.get_path("image_dupe.jpg")).unwrap();
    let handle_video = media.find(d.get_path("video.mov")).unwrap();
    let handle_video_dupe = media.find(d.get_path("video_dupe.mov")).unwrap();

    link_live_photos(&mut media, &mut live_photos);

    let id = LivePhotoID("ID".to_string());
    assert!(live_photos.contains_key(&id));
    let mut link = live_photos.remove(&id).unwrap();
    assert!(link.drain_images().collect::<Vec<_>>() == vec![handle_image, handle_image_dupe]);
    assert!(link.drain_videos().collect::<Vec<_>>() == vec![handle_video, handle_video_dupe]);
  }
}

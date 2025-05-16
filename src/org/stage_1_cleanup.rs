// Copyright 2023-5 Seth Pendergrass. See LICENSE.

//! Organizer Stage 1: Automatic deletion of duplicates and leftovers.

use std::{collections::HashMap, path::Path};

use super::Organizer;
use crate::{
  io,
  prim::{Handle, LivePhotoLinker, Media, Sidecar},
};

/// Allows using `LivePhotoLink::drain_images` and `drain_videos` as generics in
/// `remove_live_photo_duplicates_by_type`, without calls to those functions
/// borrowing `self` mutably past the point at which their returned iterators
/// are collected.
///
/// This trait requires that any implementer be a function of the form:
/// `fn(&mut LivePhotoLink) -> I`, where `I` is an iterator defined by the
/// implementer. By pushing the iterator into the implementation, this trait can
/// be correctly annotated as `impl for<'a> DrainFn<'a>`, which allows the
/// compiler to understand that the lifetime of the borrow only lasts until the
/// iterator is dropped.
trait DrainFn<'a>: Fn(&'a mut LivePhotoLinker) -> <Self as DrainFn<'a>>::Iter {
  type Iter: Iterator<Item = Handle<Media>>;
}

impl<'a, F, I> DrainFn<'a> for F
where
  F: Fn(&'a mut LivePhotoLinker) -> I,
  I: Iterator<Item = Handle<Media>>,
{
  type Iter = I;
}

impl Organizer {
  /// Removes leftover Live Photos videos. These are video files that were part
  /// of a Live Photo, where the corresponding image no longer exists. It is
  /// assumed this means the image was purposefully deleted, and as such, so
  /// too should the video.
  pub fn remove_live_photo_leftovers(&mut self) -> Result<(), String> {
    log::info!("Removing videos from deleted Live Photo images.");

    let (leftover, good): (HashMap<_, _>, HashMap<_, _>) = self
      .live_photo_map
      .drain()
      .partition(|(_, l)| l.is_leftover_videos());

    self.live_photo_map.extend(good);

    for (_, mut link) in leftover {
      for media_handle in link.drain() {
        let media = self
          .media
          .get_entry_mut(media_handle)
          .take()
          .ok_or(format!("Cannot find media handle `{media_handle}` in map."))?;
        remove_by_path(&self.source, media, self.trash.as_ref())?;
      }
    }

    Ok(())
  }

  /// Removes duplicated Live Photo images and videos. Based on the
  /// `ContentIdentifier` tag, duplicate Live Photos can be identified. This
  /// prioritizes files based on their codec, followed by their modification
  /// date, assuming that duplicates generally come from downloads
  /// being converted from their original formats (e.g. HEIC) to those more
  /// "compatible" (e.g. JPEG).
  pub fn remove_live_photo_duplicates(&mut self) -> Result<(), String> {
    log::info!("Removing Live Photo duplicates.");

    self.remove_live_photo_duplicates_by_type(
      LivePhotoLinker::has_duplicate_images,
      LivePhotoLinker::get_image_best,
      LivePhotoLinker::drain_images,
      LivePhotoLinker::insert_image,
    )?;

    self.remove_live_photo_duplicates_by_type(
      LivePhotoLinker::has_duplicate_videos,
      LivePhotoLinker::get_video_best,
      LivePhotoLinker::drain_videos,
      LivePhotoLinker::insert_video,
    )
  }

  /// Removes duplicates of one type of Live Photo component, based on codec
  /// preference followed by most recent date of modification.
  fn remove_live_photo_duplicates_by_type(
    &mut self,
    has_duplicates: fn(&LivePhotoLinker) -> bool,
    get: fn(&LivePhotoLinker) -> Handle<Media>,
    drain: impl for<'a> DrainFn<'a>,
    insert: fn(&mut LivePhotoLinker, Handle<Media>, &Media),
  ) -> Result<(), String> {
    for link in self.live_photo_map.values_mut() {
      if !has_duplicates(link) {
        continue;
      }

      let handle = get(link);

      for removed in drain(link) {
        if removed == handle {
          continue;
        }

        let media = self
          .media
          .get_entry_mut(removed)
          .take()
          .ok_or(format!("Cannot find media handle `{removed}` in map."))?;
        remove_by_path(&self.source, media, self.trash.as_ref())?;
      }

      insert(link, handle, &self.media[handle]);
    }

    Ok(())
  }

  /// Removes leftover XMP sidecars. These are sidecars that no longer have a
  /// corresponding media file, assumably because it was deleted on purpose.
  pub fn remove_sidecar_leftovers(&mut self) -> Result<(), String> {
    log::info!("Removing XMP sidecars missing associated media files.");

    for sidecar in self.sidecars.iter_entries_mut() {
      if let Some(sidecar) = sidecar.take_if(|s| s.is_leftover()) {
        remove_by_path(&self.source, sidecar, self.trash.as_ref())?;
      }
    }

    for sidecar in self.dupes.iter_entries_mut() {
      if let Some(sidecar) = sidecar.take_if(|s| s.is_leftover()) {
        remove_by_path(&self.source, sidecar, self.trash.as_ref())?;
      }
    }

    Ok(())
  }
}

/// Remove a file to `trash`, if `Some`, preserving relative path from the
/// scanned input directory.
fn remove_by_path(
  root: impl AsRef<Path>,
  path_relative: impl AsRef<Path>,
  trash: Option<impl AsRef<Path>>,
) -> Result<(), String> {
  if let Some(trash) = trash {
    log::warn!("{}: Moving to trash.", path_relative.as_ref().display());
    io::remove_file(&root, trash, root.as_ref().join(path_relative))?;
  }

  Ok(())
}

#[cfg(test)]
mod test_remove_live_photo_leftovers {
  use super::*;
  use crate::testing::*;

  #[test]
  fn keeps_non_live_photo() {
    let d = test_dir!(
      "image_not_live.jpg": {},
    );

    let mut o = Organizer::load_catalog(d.root(), d.some_trash()).unwrap();
    o.remove_live_photo_leftovers().unwrap();

    assert_dir!(d, ["image_not_live.jpg"]);
  }

  #[test]
  fn keeps_paired_live_photo() {
    let d = test_dir!(
      "image.heic": { "ContentIdentifier": "ID" },
      "video.mov": { "ContentIdentifier": "ID", "CompressorID": "hvc1" },
    );

    let mut o = Organizer::load_catalog(d.root(), d.some_trash()).unwrap();
    o.remove_live_photo_leftovers().unwrap();

    assert_dir!(d, ["image.heic", "video.mov"]);
  }

  #[test]
  fn keeps_leftover_image() {
    let d = test_dir!(
      "image_leftover.heic": { "ContentIdentifier": "ID" },
    );

    let mut o = Organizer::load_catalog(d.root(), d.some_trash()).unwrap();
    o.remove_live_photo_leftovers().unwrap();

    assert_dir!(d, ["image_leftover.heic"]);
  }

  #[test]
  fn removes_leftover_video() {
    let d = test_dir!(
      "video.mov": { "ContentIdentifier": "ID", "CompressorID": "hvc1" },
    );

    let mut o = Organizer::load_catalog(d.root(), d.some_trash()).unwrap();
    o.remove_live_photo_leftovers().unwrap();

    assert_dir!(d, []);
    assert_trash!(d, ["video.mov"]);
  }
}

#[cfg(test)]
mod test_remove_live_photo_duplicates {
  use super::*;
  use crate::testing::*;

  #[test]
  fn keeps_heic_over_jpg() {
    let d = test_dir!(
      "image.jpg": { "ContentIdentifier": "ID" },
      "image.heic": { "ContentIdentifier": "ID" },
      "video.mov": { "ContentIdentifier": "ID", "CompressorID": "hvc1" },
    );

    let mut o = Organizer::load_catalog(d.root(), d.some_trash()).unwrap();
    o.remove_live_photo_duplicates().unwrap();

    assert_dir!(d, ["image.heic", "video.mov"]);
    assert_trash!(d, ["image.jpg"]);
  }

  #[test]
  fn keeps_hevc_over_avc() {
    let d = test_dir!(
      "image.heic": { "ContentIdentifier": "ID" },
      "video_good.mov": { "ContentIdentifier": "ID", "CompressorID": "hvc1" },
      "video_bad.mov": { "ContentIdentifier": "ID", "CompressorID": "avc1" },
    );

    let mut o = Organizer::load_catalog(d.root(), d.some_trash()).unwrap();
    o.remove_live_photo_duplicates().unwrap();

    assert_dir!(d, ["image.heic", "video_good.mov"]);
    assert_trash!(d, ["video_bad.mov"]);
  }

  #[test]
  fn keeps_most_recently_modified() {
    let d = test_dir!(
      "image_new.heic": { "ContentIdentifier": "ID", "ModifyDate": "2025-01-01T00:00:00" },
      "image_old.heic": { "ContentIdentifier": "ID", "ModifyDate": "2000-01-01T00:00:00" },
      "video_new.mov": {
        "CompressorID": "hvc1",
        "ContentIdentifier": "ID",
        "ModifyDate": "2025-01-01T00:00:00",
      },
      "video_old.mov": {
        "CompressorID": "hvc1",
        "ContentIdentifier": "ID",
        "ModifyDate": "2000-01-01T00:00:00",
      },
    );

    let mut o = Organizer::load_catalog(d.root(), d.some_trash()).unwrap();
    o.remove_live_photo_duplicates().unwrap();

    assert_dir!(d, ["image_new.heic", "video_new.mov"]);
    assert_trash!(d, ["image_old.heic", "video_old.mov"]);
  }

  #[test]
  fn keeps_non_duplicated_live_photos() {
    let d = test_dir!(
      "image.heic": { "ContentIdentifier": "ID", "ModifyDate": "2000-01-01T00:00:00" },
      "video.mov": {
        "CompressorID": "hvc1",
        "ContentIdentifier": "ID",
        "ModifyDate": "2000-01-01T00:00:00",
      },
    );

    let mut o = Organizer::load_catalog(d.root(), d.some_trash()).unwrap();
    o.remove_live_photo_duplicates().unwrap();

    assert_dir!(d, ["image.heic", "video.mov"]);
  }
}

#[cfg(test)]
mod test_remove_sidecar_leftovers {
  use super::*;
  use crate::testing::*;

  #[test]
  fn keeps_paired_sidecar() {
    let d = test_dir!(
      "image.jpg": {},
      "image.jpg.xmp": {},
    );

    let mut o = Organizer::load_catalog(d.root(), d.some_trash()).unwrap();
    o.remove_sidecar_leftovers().unwrap();

    assert_dir!(d, ["image.jpg", "image.jpg.xmp"]);
  }

  #[test]
  fn keeps_paired_dupe_missing_sidecar() {
    let d = test_dir!(
      "image.jpg": {},
      "image_01.jpg.xmp": {},
    );

    let mut o = Organizer::load_catalog(d.root(), d.some_trash()).unwrap();
    o.remove_sidecar_leftovers().unwrap();

    assert_dir!(d, ["image.jpg", "image_01.jpg.xmp",]);
  }

  #[test]
  fn removes_leftover_sidecar() {
    let d = test_dir!(
      "image.jpg.xmp": {},
    );

    let mut o = Organizer::load_catalog(d.root(), d.some_trash()).unwrap();
    o.remove_sidecar_leftovers().unwrap();

    assert_dir!(d, []);
    assert_trash!(d, ["image.jpg.xmp"]);
  }

  #[test]
  fn removes_leftover_dupe() {
    let d = test_dir!(
      "image_01.jpg.xmp": {},
    );

    let mut o = Organizer::load_catalog(d.root(), d.some_trash()).unwrap();
    o.remove_sidecar_leftovers().unwrap();

    assert_dir!(d, []);
    assert_trash!(d, ["image_01.jpg.xmp"]);
  }
}

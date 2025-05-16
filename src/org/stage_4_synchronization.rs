// Copyright 2023-5 Seth Pendergrass. See LICENSE.

//! Organizer Stage 4: Metadata synchronization.

use std::fmt::Write;

use super::Organizer;
use crate::{io, org, prim::Sidecar};

impl Organizer {
  /// Synchronizes metadata from Live Photo images to their corresponding
  /// videos. This means that any manual changes only need to be made for the
  /// image, and it can be copied here automatically.
  pub fn sync_live_photo_metadata(&mut self) -> Result<(), String> {
    log::info!("Synchronizing metadata across Live Photo components.");

    for l in self.live_photo_map.values_mut() {
      if !l.is_pair() {
        log::warn!(
          "Cannot synchronize Live Photo with duplicates:{}",
          l.drain()
            .map(|h| { &self.media[h] })
            .fold(String::new(), |mut s, d| {
              write!(s, " {d}").unwrap();
              s
            })
        );
      }

      let Some(handle_image_sidecar) = self.media[l.get_image_best()].get_sidecar() else {
        log::debug!(
          "{}: Cannot synchronize from Live Photo image without sidecar.",
          self.media[l.get_image_best()]
        );
        continue;
      };
      let image_sidecar_path = self.sidecars[handle_image_sidecar].as_ref().to_path_buf();

      let Some(handle_video_sidecar) = self.media[l.get_video_best()].get_sidecar() else {
        log::debug!(
          "{}: Cannot synchronize to Live Photo video without sidecar.",
          self.media[l.get_video_best()]
        );
        continue;
      };
      let video_sidecar = &mut self.sidecars[handle_video_sidecar];

      log::trace!(
        "{} -> {}: Synchronizing metadata.",
        image_sidecar_path.display(),
        video_sidecar
      );

      let metadata = io::copy_metadata(
        org::to_abs_path(&self.source, image_sidecar_path),
        org::to_abs_path(&self.source, &video_sidecar),
      )?;
      video_sidecar.update_metadata(metadata);
    }

    Ok(())
  }

  /// Synchronizes metadata from initial (base/main) sidecars to duplicate
  /// sidecars, as made by darktable. Manual changes only need to be applied
  /// to the initial sidecar, and this function will propagate changes to the
  /// duplicates.
  pub fn sync_dupe_metadata(&mut self) -> Result<(), String> {
    log::info!("Synchronizing metadata from initial sidecars to duplicates.");

    for sidecar in self.sidecars.iter_data() {
      let Some(handle_media) = sidecar.get_media_handle() else {
        log::debug!("{sidecar}: Leftover sidecar, cannot synchronize to duplicates.");
        continue;
      };

      let media = &self.media[handle_media];

      for handle_dupe in media.iter_dupes() {
        let dupe = &mut self.dupes[handle_dupe];

        log::trace!("{sidecar} -> {dupe}: Synchronizing metadata.");

        let metadata = io::copy_metadata(
          org::to_abs_path(&self.source, sidecar),
          org::to_abs_path(&self.source, &dupe),
        )?;
        dupe.update_metadata(metadata);
      }
    }

    Ok(())
  }

  /// Synchronizes metadata from initial sidecars to their associated media
  /// files. This is useful in keeping metadata changes in case XMP files are
  /// lost or overwritten erroneously, but some prefer to never update media
  /// metadata files directly for some formats (e.g. raw files).
  pub fn sync_media_metadata(&mut self) -> Result<(), String> {
    log::info!("Synchronizing metadata from initial sidecars to media.");

    for media in self.media.iter_data_mut() {
      let Some(handle_sidecar) = media.get_sidecar() else {
        log::debug!("{media}: Missing sidecar, cannot synchronize.");
        continue;
      };

      let sidecar = &self.sidecars[handle_sidecar];

      log::trace!("{sidecar} -> {media}: Synchronizing metadata.");

      let metadata = io::copy_metadata(
        org::to_abs_path(&self.source, sidecar),
        org::to_abs_path(&self.source, &media),
      )?;
      media.update_metadata(metadata);
    }

    Ok(())
  }
}

#[cfg(test)]
mod test_sync_live_photo_metadata {
  use super::*;
  use crate::testing::*;

  #[test]
  fn overwrites_video_with_image_metadata() {
    let d = test_dir!(
      "image.heic": { "ContentIdentifier": "ID" },
      "image.heic.xmp": { "Creator": "Image" },
      "video.mov": {
        "CompressorID": "avc1",
        "ContentIdentifier": "ID",
        "Creator": "Video"
      },
      "video.mov.xmp": { "Creator": "Video" },
    );

    let mut o = Organizer::import(d.root()).unwrap();
    o.sync_live_photo_metadata().unwrap();

    assert_tag!(d, "image.heic.xmp", "Creator", "Image");
    assert_tag!(d, "video.mov.xmp", "Creator", "Image");
  }

  #[test]
  fn skips_if_missing_image_sidecar() {
    let d = test_dir!(
      "image.heic": { "ContentIdentifier": "ID", "Creator": "Image" },
      "video.mov": {
        "CompressorID": "avc1",
        "ContentIdentifier": "ID",
        "Creator": "Video"
      },
      "video.mov.xmp": { "Creator": "Sidecar" },
    );

    let mut o = Organizer::import(d.root()).unwrap();
    o.sync_live_photo_metadata().unwrap();

    assert_tag!(d, "video.mov", "Creator", "Video");
    assert_tag!(d, "video.mov.xmp", "Creator", "Sidecar");
  }

  #[test]
  fn skips_if_missing_video_sidecar() {
    let d = test_dir!(
      "image.heic": { "ContentIdentifier": "ID", "Creator": "Image" },
      "image.heic.xmp": { "Creator": "Sidecar" },
      "video.mov": {
        "CompressorID": "avc1",
        "ContentIdentifier": "ID",
        "Creator": "Video"
      },
    );

    let mut o = Organizer::import(d.root()).unwrap();
    o.sync_live_photo_metadata().unwrap();

    assert_tag!(d, "video.mov", "Creator", "Video");
  }

  #[test]
  fn syncs_per_live_photo_group() {
    let d = test_dir!(
      "image1.heic": { "ContentIdentifier": "ID1", "Creator": "Image1" },
      "image1.heic.xmp": { "Creator": "ImageSidecar1" },
      "video1.mov": {
        "CompressorID": "avc1",
        "ContentIdentifier": "ID1",
        "Creator": "Video1"
      },
      "video1.mov.xmp": { "Creator": "VideoSidecar1" },
      "image2.heic": { "ContentIdentifier": "ID2", "Creator": "Image2" },
      "image2.heic.xmp": { "Creator": "ImageSidecar2" },
      "video2.mov": {
        "CompressorID": "avc1",
        "ContentIdentifier": "ID2",
        "Creator": "Video2"
      },
      "video2.mov.xmp": { "Creator": "VideoSidecar2" },
    );

    let mut o = Organizer::import(d.root()).unwrap();
    o.sync_live_photo_metadata().unwrap();

    assert_tag!(d, "video1.mov.xmp", "Creator", "ImageSidecar1");
    assert_tag!(d, "video2.mov.xmp", "Creator", "ImageSidecar2");
  }

  #[test]
  fn writes_only_video_sidecar() {
    let d = test_dir!(
      "image.heic": { "ContentIdentifier": "ID", "Creator": "Image" },
      "image.heic.xmp": { "Creator": "ImageSidecar" },
      "image_01.heic.xmp": { "Creator": "ImageDuplicate" },
      "video.mov": {
        "CompressorID": "avc1",
        "ContentIdentifier": "ID",
        "Creator": "Video"
      },
      "video.mov.xmp": { "Creator": "VideoSidecar" },
      "video_01.mov.xmp": { "Creator": "VideoDuplicate" },
    );

    let mut o = Organizer::import(d.root()).unwrap();
    o.sync_live_photo_metadata().unwrap();

    assert_tag!(d, "video.mov.xmp", "Creator", "ImageSidecar");
    assert_tag!(d, "image.heic", "Creator", "Image");
    assert_tag!(d, "image.heic.xmp", "Creator", "ImageSidecar");
    assert_tag!(d, "image_01.heic.xmp", "Creator", "ImageDuplicate");
    assert_tag!(d, "video.mov", "Creator", "Video");
    assert_tag!(d, "video_01.mov.xmp", "Creator", "VideoDuplicate");
  }
}

#[cfg(test)]
mod test_sync_dupe_metadata {
  use super::*;
  use crate::testing::*;

  #[test]
  fn overwrites_dupe_with_sidecar_metadata() {
    let d = test_dir!(
      "image.jpg": { "Creator": "Media" },
      "image.jpg.xmp": { "Creator": "Sidecar" },
      "image_01.jpg.xmp": { "Creator": "Dupe1" },
      "image_02.jpg.xmp": { "Creator": "Dupe2" },
    );

    let mut o = Organizer::import(d.root()).unwrap();
    o.sync_dupe_metadata().unwrap();

    assert_tag!(d, "image_01.jpg.xmp", "Creator", "Sidecar");
    assert_tag!(d, "image_02.jpg.xmp", "Creator", "Sidecar");
  }

  #[test]
  fn skips_if_missing_sidecar() {
    let d = test_dir!(
      "image.jpg": { "Creator": "Media" },
      "image_01.jpg.xmp": { "Creator": "Dupe1" },
      "image_02.jpg.xmp": { "Creator": "Dupe2" },
    );

    let mut o = Organizer::import(d.root()).unwrap();
    o.sync_dupe_metadata().unwrap();

    assert_tag!(d, "image.jpg", "Creator", "Media");
    assert_tag!(d, "image_01.jpg.xmp", "Creator", "Dupe1");
    assert_tag!(d, "image_02.jpg.xmp", "Creator", "Dupe2");
  }

  #[test]
  fn syncs_per_dupe_group() {
    let d = test_dir!(
      "image1.jpg": { "Creator": "Media1" },
      "image1.jpg.xmp": { "Creator": "Sidecar1" },
      "image1_01.jpg.xmp": { "Creator": "Dupe1" },
      "image2.jpg": { "Creator": "Media2" },
      "image2.jpg.xmp": { "Creator": "Sidecar2" },
      "image2_01.jpg.xmp": { "Creator": "Dupe2" },
    );

    let mut o = Organizer::import(d.root()).unwrap();
    o.sync_dupe_metadata().unwrap();

    assert_tag!(d, "image1_01.jpg.xmp", "Creator", "Sidecar1");
    assert_tag!(d, "image2_01.jpg.xmp", "Creator", "Sidecar2");
  }

  #[test]
  fn writes_only_dupes() {
    let d = test_dir!(
      "image.jpg": { "Creator": "Media" },
      "image.jpg.xmp": { "Creator": "Sidecar" },
      "image_01.jpg.xmp": { "Creator": "Dupe" },
    );

    let mut o = Organizer::import(d.root()).unwrap();
    o.sync_dupe_metadata().unwrap();

    assert_tag!(d, "image.jpg", "Creator", "Media");
    assert_tag!(d, "image.jpg.xmp", "Creator", "Sidecar");
    assert_tag!(d, "image_01.jpg.xmp", "Creator", "Sidecar");
  }
}

#[cfg(test)]
mod test_sync_media_metadata {
  use super::*;
  use crate::testing::*;

  #[test]
  fn overwrites_media_with_sidecar_metadata() {
    let d = test_dir!(
      "image.jpg": { "Creator": "Media" },
      "image.jpg.xmp": { "Creator": "Sidecar" },
      "image_01.jpg.xmp": { "Creator": "Dupe" },
    );

    let mut o = Organizer::import(d.root()).unwrap();
    o.sync_media_metadata().unwrap();

    assert_tag!(d, "image.jpg", "Creator", "Sidecar");
  }

  #[test]
  fn skips_if_missing_sidecar() {
    let d = test_dir!(
      "image.jpg": {},
      "image_01.jpg.xmp": { "Creator": "Sidecar" },
    );

    let mut o = Organizer::import(d.root()).unwrap();
    o.sync_media_metadata().unwrap();

    assert_tag!(d, "image.jpg", "Creator", None);
  }

  #[test]
  fn syncs_per_media_group() {
    let d = test_dir!(
      "image1.jpg": { "Creator": "Media1" },
      "image1.jpg.xmp": { "Creator": "Sidecar1" },
      "image2.jpg": { "Creator": "Media2" },
      "image2.jpg.xmp": { "Creator": "Sidecar2" },
    );

    let mut o = Organizer::import(d.root()).unwrap();
    o.sync_media_metadata().unwrap();

    assert_tag!(d, "image1.jpg", "Creator", "Sidecar1");
    assert_tag!(d, "image2.jpg", "Creator", "Sidecar2");
  }

  #[test]
  fn writes_only_media() {
    let d = test_dir!(
      "image.jpg": { "Creator": "Media" },
      "image.jpg.xmp": { "Creator": "Sidecar" },
      "image_01.jpg.xmp": { "Creator": "Dupe" },
    );

    let mut o = Organizer::import(d.root()).unwrap();
    o.sync_media_metadata().unwrap();

    assert_tag!(d, "image.jpg", "Creator", "Sidecar");
    assert_tag!(d, "image.jpg.xmp", "Creator", "Sidecar");
    assert_tag!(d, "image_01.jpg.xmp", "Creator", "Dupe");
  }
}

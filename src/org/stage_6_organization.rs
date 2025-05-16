// Copyright 2023-5 Seth Pendergrass. See LICENSE.

//! Stage 6: Moving and renaming of files.

use std::{
  ffi::OsString,
  path::{Path, PathBuf},
};

use super::Organizer;
use crate::{
  io,
  org,
  prim::{FileMap, Handle, Media, SidecarDupe, SidecarInitial},
};

impl Organizer {
  /// Moves loaded files to `dst`, organizing them into subdirectories and
  /// renaming them based on their timestamps.
  /// Unless `force` is true, this will only touch validated files.
  pub fn move_and_rename_files(mut self, dst: impl AsRef<Path>, force: bool) -> Result<(), String> {
    if dst.as_ref().is_relative() {
      return Err(format!(
        "{}: Destination path is not absolute.",
        dst.as_ref().display()
      ));
    }

    if !dst.as_ref().exists() {
      return Err(format!(
        "{}: Destination path does not exist.",
        dst.as_ref().display()
      ));
    }

    if !self.validation.enabled() && !force {
      log::warn!("Skipping move and rename: Validation disabled.");
      return Ok(());
    }

    log::info!("Moving and renaming Live Photos.");

    for mut link in self.live_photo_map.into_values() {
      if link.is_leftover_videos() {
        continue;
      }

      let handle_main = link.get_image_best();

      let image_main = take_media(handle_main, &mut self.media);
      let sidecar_main = take_sidecar(&image_main, &mut self.sidecars);
      let dupes_main = take_dupes(&image_main, &mut self.dupes);
      let metadata_source = pick_source(&image_main, sidecar_main.as_ref());

      let should_move = force || self.valid_media.contains(&handle_main);

      for handle in link.drain() {
        if handle == handle_main {
          continue;
        }

        let media = take_media(handle, &mut self.media);
        let sidecar = take_sidecar(&media, &mut self.sidecars);
        let dupes = take_dupes(&media, &mut self.dupes);

        if should_move {
          move_media_with_deps(&self.source, &dst, &metadata_source, media, sidecar, dupes)?;
        }
      }

      if should_move {
        move_media_with_deps(
          &self.source,
          &dst,
          &metadata_source,
          image_main,
          sidecar_main,
          dupes_main,
        )?;
      } else {
        log::warn!("{image_main}: Not moving or renaming. File did not pass validation.");
      }
    }

    log::info!("Moving and renaming all other media files.");

    for (handle, entry) in self.media.iter_entries_mut_indexed() {
      let media = entry.take().unwrap();
      let sidecar = take_sidecar(&media, &mut self.sidecars);
      let dupes = take_dupes(&media, &mut self.dupes);
      let metadata_source = pick_source(&media, sidecar.as_ref());

      if force || self.valid_media.contains(&handle) {
        move_media_with_deps(&self.source, &dst, &metadata_source, media, sidecar, dupes)?;
      } else {
        log::warn!("{media}: Not moving or renaming. File did not pass validation.");
      }
    }

    Ok(())
  }
}

fn take_media(handle: Handle<Media>, media_map: &mut FileMap<Media>) -> Media {
  media_map.get_entry_mut(handle).take().unwrap()
}

fn take_sidecar(
  media: &Media,
  sidecar_map: &mut FileMap<SidecarInitial>,
) -> Option<SidecarInitial> {
  media
    .get_sidecar()
    .map(|h| sidecar_map.get_entry_mut(h).take().unwrap())
}

fn take_dupes(media: &Media, dupe_map: &mut FileMap<SidecarDupe>) -> Vec<SidecarDupe> {
  media
    .iter_dupes()
    .map(|h| dupe_map.get_entry_mut(h).take().unwrap())
    .collect()
}

fn pick_source(media: &Media, sidecar: Option<&SidecarInitial>) -> PathBuf {
  sidecar
    .as_ref()
    .map_or(media.as_ref(), std::convert::AsRef::as_ref)
    .to_path_buf()
}

fn move_media_with_deps(
  dir_src: impl AsRef<Path>,
  dir_dst: impl AsRef<Path>,
  metadata_source: impl AsRef<Path>,
  media: Media,
  sidecar: Option<SidecarInitial>,
  dupes: impl IntoIterator<Item = SidecarDupe>,
) -> Result<(), String> {
  log::trace!("{media}: Moving and renaming.");

  let media_file_ext = media.get_metadata().file_type_extension.clone();

  for dupe in dupes {
    let mut dupe_ending = OsString::from("_");
    dupe_ending.push(dupe.get_dupe_number());
    dupe_ending.push(".");
    dupe_ending.push(&media_file_ext);
    dupe_ending.push(".xmp");

    io::move_file(
      org::to_abs_path(&dir_src, dupe),
      Some(&org::to_abs_path(&dir_src, &metadata_source)),
      &dir_dst,
      dupe_ending,
    )?;
  }

  io::move_file(
    org::to_abs_path(&dir_src, media),
    Some(&org::to_abs_path(&dir_src, &metadata_source)),
    &dir_dst,
    format!(".{media_file_ext}"),
  )?;

  if let Some(sidecar) = sidecar {
    io::move_file(
      org::to_abs_path(&dir_src, sidecar),
      Some(&org::to_abs_path(dir_src, metadata_source)),
      dir_dst,
      format!(".{media_file_ext}.xmp"),
    )?;
  }

  Ok(())
}

#[cfg(test)]
mod test_move_and_rename_files {
  use super::*;
  use crate::testing::*;

  #[test]
  fn errors_if_destination_path_does_not_exist() {
    let d = test_dir!();

    let o = Organizer::import(d.root()).unwrap();
    assert_err!(
      o.move_and_rename_files("/path/does/not/exist", false),
      "Destination path does not exist."
    );
  }

  #[test]
  fn errors_if_destination_path_is_relative() {
    let d = test_dir!();

    let o = Organizer::import(d.root()).unwrap();
    assert_err!(
      o.move_and_rename_files("relative/path", false),
      "Destination path is not absolute."
    );
  }

  #[test]
  fn moves_file_and_sidecars_as_group() {
    let d = test_dir!(
      "image.jpg": {},
      "image.jpg.xmp": { "DateTimeOriginal": "2000-01-01T00:00:00" },
      "image_01.jpg.xmp": { "DateTimeOriginal": "2025-01-01T00:00:00" },
    );

    let o = Organizer::import(d.root()).unwrap();
    o.move_and_rename_files(d.root(), true).unwrap();

    assert_dir!(d, [
      "2000/01/000101_000000000.jpg",
      "2000/01/000101_000000000.jpg.xmp",
      "2000/01/000101_000000000_01.jpg.xmp",
    ]);
  }

  #[test]
  fn moves_file_and_dupe_as_group() {
    let d = test_dir!(
      "image.jpg": { "DateTimeOriginal": "2000-01-01T00:00:00" },
      "image_01.jpg.xmp": { "DateTimeOriginal": "2025-01-01T00:00:00" },
    );

    let o = Organizer::import(d.root()).unwrap();
    o.move_and_rename_files(d.root(), true).unwrap();

    assert_dir!(d, [
      "2000/01/000101_000000000.jpg",
      "2000/01/000101_000000000_01.jpg.xmp",
    ]);
  }

  #[test]
  fn moves_groups_at_same_time_separately() {
    let d = test_dir!(
      "image1.jpg": { "DateTimeOriginal": "2000-01-01T00:00:00", "Creator": "A" },
      "image1.jpg.xmp": { "DateTimeOriginal": "2000-01-01T00:00:00", "Creator": "A" },
      "image1_01.jpg.xmp": { "DateTimeOriginal": "2000-01-01T00:00:00", "Creator": "A" },
      "image2.jpg": { "DateTimeOriginal": "2000-01-01T00:00:00", "Creator": "B" },
      "image2.jpg.xmp": { "DateTimeOriginal": "2000-01-01T00:00:00", "Creator": "B" },
      "image2_01.jpg.xmp": { "DateTimeOriginal": "2000-01-01T00:00:00", "Creator": "B" },
    );

    let o = Organizer::import(d.root()).unwrap();
    o.move_and_rename_files(d.root(), true).unwrap();

    assert_dir!(d, [
      "2000/01/000101_000000000.jpg",
      "2000/01/000101_000000000.jpg.xmp",
      "2000/01/000101_000000000_01.jpg.xmp",
      "2000/01/000101_000000000_b.jpg",
      "2000/01/000101_000000000_b.jpg.xmp",
      "2000/01/000101_000000000_b_01.jpg.xmp",
    ]);

    let creator_image =
      read_tag(d.root(), "2000/01/000101_000000000.jpg", None, "Creator").unwrap();
    let creator_sidecar = read_tag(
      d.root(),
      "2000/01/000101_000000000.jpg.xmp",
      None,
      "Creator",
    )
    .unwrap();
    let creator_dupe = read_tag(
      d.root(),
      "2000/01/000101_000000000_01.jpg.xmp",
      None,
      "Creator",
    )
    .unwrap();
    let creator_image_b =
      read_tag(d.root(), "2000/01/000101_000000000_b.jpg", None, "Creator").unwrap();
    let creator_sidecar_b = read_tag(
      d.root(),
      "2000/01/000101_000000000_b.jpg.xmp",
      None,
      "Creator",
    )
    .unwrap();
    let creator_dupe_b = read_tag(
      d.root(),
      "2000/01/000101_000000000_b_01.jpg.xmp",
      None,
      "Creator",
    )
    .unwrap();

    let creator_exp = if creator_image == "A" { "A" } else { "B" };

    assert_eq!(creator_image, creator_exp);
    assert_eq!(creator_sidecar, creator_exp);
    assert_eq!(creator_dupe, creator_exp);

    let creator_exp_b = if creator_image == "A" { "B" } else { "A" };

    assert_eq!(creator_image_b, creator_exp_b);
    assert_eq!(creator_sidecar_b, creator_exp_b);
    assert_eq!(creator_dupe_b, creator_exp_b);
  }

  #[test]
  fn moves_live_photo_if_image_valid() {
    let d = test_dir!(
      "image.heic": {
        "ContentIdentifier": "ID",
        "CreateDate": "2000-01-01T00:00:00",
        "DateTimeOriginal": "2000-01-01T00:00:00",
      },
      "image.mov": { "ContentIdentifier": "ID", "CompressorID": "hvc1" }
    );

    let mut o = Organizer::import(d.root()).unwrap();
    o.enable_date_time_validation();
    o.validate();
    o.move_and_rename_files(d.root(), false).unwrap();

    assert_dir!(d, [
      "2000/01/000101_000000000.heic",
      "2000/01/000101_000000000.mov",
    ]);
  }

  #[test]
  fn moves_live_photo_as_group() {
    let d = test_dir!(
      "image.heic": { "ContentIdentifier": "ID", "DateTimeOriginal": "2000-01-01T00:00:00" },
      "image.heic.xmp": { "DateTimeOriginal": "2000-01-01T00:00:00" },
      "video.mov": {
        "ContentIdentifier": "ID",
        "CompressorID": "hvc1",
        "DateTimeOriginal": "2025-01-01T00:00:00",
      },
      "video.mov.xmp": { "DateTimeOriginal": "2025-01-01T00:00:00" },
    );

    let o = Organizer::import(d.root()).unwrap();
    o.move_and_rename_files(d.root(), true).unwrap();

    assert_dir!(d, [
      "2000/01/000101_000000000.heic",
      "2000/01/000101_000000000.heic.xmp",
      "2000/01/000101_000000000.mov",
      "2000/01/000101_000000000.mov.xmp",
    ]);
  }

  #[test]
  fn moves_live_photo_with_dupes_as_group() {
    let d = test_dir!(
      "image1.heic": { "ContentIdentifier": "ID", "DateTimeOriginal": "2000-01-01T00:00:00" },
      "image2.jpg": { "ContentIdentifier": "ID", "DateTimeOriginal": "2025-01-01T00:00:00" },
      "video1.mov": {
        "CompressorID": "hvc1",
        "ContentIdentifier": "ID",
        "DateTimeOriginal": "2000-01-01T00:00:00"
      },
      "video2.mov": {
        "CompressorID": "avc1",
        "ContentIdentifier": "ID",
        "DateTimeOriginal": "2025-01-01T00:00:00"
      }
    );

    let o = Organizer::import(d.root()).unwrap();
    o.move_and_rename_files(d.root(), true).unwrap();

    assert_dir!(d, [
      "2000/01/000101_000000000.heic",
      "2000/01/000101_000000000.mov",
      "2000/01/000101_000000000.jpg",
      "2000/01/000101_000000000_b.mov",
    ]);
  }

  #[test]
  fn moves_live_photo_leftovers_separately() {
    let d = test_dir!(
      "video1.mov": {
        "CompressorID": "hvc1",
        "ContentIdentifier": "ID",
        "DateTimeOriginal": "2000-01-01T00:00:00"
      },
      "video2.mov": {
        "CompressorID": "avc1",
        "ContentIdentifier": "ID",
        "DateTimeOriginal": "2025-01-01T00:00:00"
      }
    );

    let o = Organizer::import(d.root()).unwrap();
    o.move_and_rename_files(d.root(), true).unwrap();

    assert_dir!(d, [
      "2000/01/000101_000000000.mov",
      "2025/01/250101_000000000.mov",
    ]);
  }

  #[test]
  fn renames_by_date_time_from_sidecar() {
    let d = test_dir!(
      "image.jpg": {
        "DateTimeOriginal": "2025-01-01T00:00:00",
        "OffsetTimeOriginal": "+00:00",
        "SubSecTimeOriginal": "0",
      },
      "image.jpg.xmp": { "DateTimeOriginal": "2000-01-01T00:00:00.999-08:00" },
    );

    let o = Organizer::import(d.root()).unwrap();
    o.move_and_rename_files(d.root(), true).unwrap();

    assert_dir!(d, [
      "2000/01/000101_080000999.jpg",
      "2000/01/000101_080000999.jpg.xmp",
    ]);
  }

  #[test]
  fn renames_by_date_time_from_media_if_missing_initial_sidecar() {
    let d = test_dir!(
      "image.jpg": {
        "DateTimeOriginal": "2025-01-01T00:00:00",
        "OffsetTimeOriginal": "-08:00",
        "SubSecTimeOriginal": "999",
      },
      "image_01.jpg.xmp": { "DateTimeOriginal": "2000-01-01T00:00:00+00:00" },
    );

    let o = Organizer::import(d.root()).unwrap();
    o.move_and_rename_files(d.root(), true).unwrap();

    assert_dir!(d, [
      "2025/01/250101_080000999.jpg",
      "2025/01/250101_080000999_01.jpg.xmp",
    ]);
  }

  #[test]
  fn skips_invalid_files() {
    let d = test_dir!(
      "image1.jpg": {},
      "image1.jpg.xmp": {
        "CreateDate": "2000-01-01T00:00:00",
        "DateTimeOriginal": "2000-01-01T00:00:00",
      },
      "image2.jpg": {},
      "image2.jpg.xmp": {},
    );

    let mut o = Organizer::import(d.root()).unwrap();
    o.enable_date_time_validation();
    o.validate();
    o.move_and_rename_files(d.root(), false).unwrap();

    assert_dir!(d, [
      "2000/01/000101_000000000.jpg",
      "2000/01/000101_000000000.jpg.xmp",
      "image2.jpg",
      "image2.jpg.xmp",
    ]);
  }

  #[test]
  fn skips_leftover_sidecars() {
    let d = test_dir!(
      "image1.jpg.xmp": { "DateTimeOriginal": "2000-01-01T00:00:00+00:00" },
      "image1_01.jpg.xmp": { "DateTimeOriginal": "2000-01-01T00:00:00+00:00" },
    );

    let o = Organizer::import(d.root()).unwrap();
    o.move_and_rename_files(d.root(), true).unwrap();

    assert_dir!(d, ["image1.jpg.xmp", "image1_01.jpg.xmp",]);
  }

  #[test]
  fn skips_live_photo_if_image_invalid() {
    let d = test_dir!(
      "image.heic": { "ContentIdentifier": "ID" },
      "image.mov": {
        "ContentIdentifier": "ID",
        "CompressorID": "hvc1",
        "CreateDate": "2000-01-01T00:00:00",
        "DateTimeOriginal": "2000-01-01T00:00:00",
      }
    );

    let mut o = Organizer::import(d.root()).unwrap();
    o.enable_date_time_validation();
    o.validate();
    o.move_and_rename_files(d.root(), false).unwrap();

    assert_dir!(d, ["image.heic", "image.mov",]);
  }
}

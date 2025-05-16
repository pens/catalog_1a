// Copyright 2023-5 Seth Pendergrass. See LICENSE.

//! Organizer Stage 3: Automatic metadata updates.

use std::ffi::OsStr;

use tzf_rs::{Finder, r#gen::tzf::v1::Timezones};

use super::Organizer;
use crate::{
  io,
  org,
  prim::{self, FileCategory, Sidecar},
};

/// Holds which metadata update passes are enabled.
#[allow(clippy::struct_excessive_bools)]
#[derive(Default)]
pub struct MetadataUpdateConfig {
  align_mwg_tags:             bool,
  set_copyright_from_creator: bool,
  set_location_from_gps:      bool,
  set_time_zone_from_gps:     bool,
}

impl MetadataUpdateConfig {
  /// If any update is enabled.
  fn enabled(&self) -> bool {
    self.align_mwg_tags
      || self.set_copyright_from_creator
      || self.set_location_from_gps
      || self.set_time_zone_from_gps
  }
}

impl Organizer {
  /// Turns on MWG tag alignment, whereby tags identified by the Metadata
  /// Working Group (MWG) as representing the same information are set to the
  /// same value from whichever tag is highest priority.
  /// See <https://exiftool.org/TagNames/MWG.html>.
  pub fn enable_align_mwg_tags(&mut self) {
    log::info!("Enabling MWG tag alignment.");
    self.metadata_updates.align_mwg_tags = true;
  }

  /// Automatically writes the `Copyright` tag from `Creator`, if `Creator` is
  /// set and `Copyright` not.
  pub fn enable_set_copyrights_from_creator(&mut self) {
    log::info!("Enabling automatic copyright.");
    self.metadata_updates.set_copyright_from_creator = true;
  }

  /// Overwrites the `City`, `State`, and `Country` tags from GPS coordinates,
  /// if GPS coordinate tags are set.
  pub fn enable_set_location_from_gps(&mut self) {
    log::info!("Enabling automatic location.");
    self.metadata_updates.set_location_from_gps = true;
  }

  /// Sets time zone based on the location, date and time of each file.
  pub fn enable_set_time_zone_from_gps(&mut self) {
    log::info!("Enabling automatic time zone.");
    self.metadata_updates.set_time_zone_from_gps = true;
  }

  /// Runs metadata updates, as enabled by `enable_*` methods. Operations are
  /// batched into this call for performance reasons (i.e. reducing the number
  /// of calls to `ExifTool`).
  pub fn apply_metadata_updates(&mut self) -> Result<(), String> {
    if !self.metadata_updates.enabled() {
      log::debug!("No metadata updates enabled. Skipping.");
      return Ok(());
    }

    log::info!("Applying metadata updates.");

    let finder = if self.metadata_updates.set_time_zone_from_gps {
      Finder::from_pb(
        Timezones::try_from(
          include_bytes!("../../third_party/tzf-rel/combined-with-oceans.bin").to_vec(),
        )
        .unwrap(),
      )
    } else {
      Finder::new()
    };

    for media in self.media.iter_data_mut() {
      // Main pass (copyright, location & time zone).
      {
        let metadata = media
          .get_sidecar()
          .map_or(media.get_metadata(), |h| self.sidecars[h].get_metadata());

        let mut args = Vec::new();

        if self.metadata_updates.set_copyright_from_creator
          && metadata.creator.is_some()
          && metadata.copyright.is_none()
        {
          args.push(OsStr::new("-Copyright<Copyright ${Creator}"));
        }

        if self.metadata_updates.set_location_from_gps
          && metadata.gps_latitude.is_some()
          && metadata.gps_longitude.is_some()
        {
          args.push(OsStr::new("-geolocate<GPSPosition"));
        }

        let time_zone_args;

        if self.metadata_updates.set_time_zone_from_gps
          && let Some(lat_lon) = metadata.get_lat_lon()
            && let Some((date_time, _)) = metadata.get_date_time_original() {
              let time_zone = finder.get_tz_name(f64::from(lat_lon.1), f64::from(lat_lon.0));

              let offset = prim::get_offset_for_time_zone(&date_time, time_zone);

              let date_time_new = date_time.and_local_timezone(offset).unwrap();

              time_zone_args = Vec::from([
                format!("-DateTimeOriginal={}", date_time_new.to_rfc3339()),
                format!("-OffsetTimeOriginal={offset}"),
              ]);

              args.push(OsStr::new(&time_zone_args[0]));
              args.push(OsStr::new(&time_zone_args[1]));
            }

        if !args.is_empty() {
          let path = org::to_abs_path(&self.source, &metadata.source_file);
          args.push(path.as_os_str());

          io::run_exiftool(Some(&self.source), args)?;

          let metadata = io::read_metadata(&path)?;

          if let Some(sidecar) = media.get_sidecar().map(|h| &mut self.sidecars[h]) {
            sidecar.update_metadata(metadata.clone());
          } else {
            media.update_metadata(metadata);
          }
        }
      }

      // Align MWG tags.
      // This is separate due to an issue where ExifTool is not applying the
      // OffsetTimeOriginal value when aligning MWG tags.
      {
        let metadata = media
          .get_sidecar()
          .map_or(media.get_metadata(), |h| self.sidecars[h].get_metadata());

        let path = org::to_abs_path(&self.source, &metadata.source_file);

        // XMP sidecars can only hold XMP metadata, so no need to synchronize across
        // EXIF/IPTC/XMP.
        if self.metadata_updates.align_mwg_tags
          && metadata.get_file_category() == FileCategory::Media
        {
          io::run_exiftool(Some(&self.source), vec![
            OsStr::new("-MWG:all<MWG:all"),
            path.as_os_str(),
          ])?;
        }
      }
    }

    Ok(())
  }
}

#[cfg(test)]
mod test_align_mwg_tags {
  use super::*;
  use crate::testing::*;

  #[test]
  fn aligns_existing_tags() {
    let d = test_dir!(
      "image.jpg": {
        "EXIF:Artist": "EXIF",
        "IPTC:By-line": "IPTC",
        "XMP-dc:Creator": "XMP",
      },
    );

    let mut o = Organizer::import(d.root()).unwrap();
    o.enable_align_mwg_tags();
    o.apply_metadata_updates().unwrap();

    assert_tag!(d, "image.jpg", "EXIF:Artist", "EXIF");
    assert_tag!(d, "image.jpg", "IPTC:By-line", "EXIF");
    assert_tag!(d, "image.jpg", "XMP-dc:Creator", "EXIF");
  }
}

#[cfg(test)]
mod test_apply_metadata_updates {
  use super::*;
  use crate::testing::*;

  #[test]
  fn writes_to_media_if_no_sidecar() {
    let d = test_dir!(
      "image.jpg": {
        // Align MWG tags.
        "Artist": "Creator", // EXIF.
        "By-line": "IPTC",
        // Set copyright from creator.
        // Note: Due to MWG applying *after* other passes, the above values will
        // not propagate into `Copyright`.
        "Creator": "Creator",
        // Set location from GPS.
        "GPSLatitude": "47.6061",
        "GPSLatitudeRef": "N",
        "GPSLongitude": "122.3328",
        "GPSLongitudeRef": "W",
        // Set time zone from GPS.
        "DateTimeOriginal": "2000-01-01T00:00:00",
        "OffsetTimeOriginal": "+00:00",
      },
    );

    let mut o = Organizer::import(d.root()).unwrap();
    o.enable_align_mwg_tags();
    o.enable_set_copyrights_from_creator();
    o.enable_set_location_from_gps();
    o.enable_set_time_zone_from_gps();
    o.apply_metadata_updates().unwrap();

    assert_tag!(d, "image.jpg", "Copyright", "Copyright Creator");
    assert_tag!(d, "image.jpg", "City", "Seattle");
    assert_tag!(d, "image.jpg", "OffsetTimeOriginal", "-08:00");
  }

  #[test]
  fn writes_to_sidecar() {
    let d = test_dir!(
      "image.jpg": {},
      "image.jpg.xmp": {
        // Skipping MWG alignment; only XMP.
        // Set copyright from creator.
        "Creator": "Creator",
        // Set location from GPS.
        "GPSLatitude": "47.6061 N",
        "GPSLongitude": "122.3328 W",
        // Set time zone from GPS.
        "DateTimeOriginal": "2000-01-01T00:00:00+00:00",
      },
    );

    let mut o = Organizer::import(d.root()).unwrap();
    o.enable_align_mwg_tags();
    o.enable_set_copyrights_from_creator();
    o.enable_set_location_from_gps();
    o.enable_set_time_zone_from_gps();
    o.apply_metadata_updates().unwrap();

    assert_tag!(d, "image.jpg", "Copyright", None);
    assert_tag!(d, "image.jpg", "City", None);
    assert_tag!(d, "image.jpg", "DateTimeOriginal", None);
    assert_tag!(d, "image.jpg", "OffsetTimeOriginal", None);

    assert_tag!(d, "image.jpg.xmp", "Copyright", "Copyright Creator");
    assert_tag!(d, "image.jpg.xmp", "City", "Seattle");
    assert_tag!(
      d,
      "image.jpg.xmp",
      "DateTimeOriginal",
      "2000-01-01T00:00:00-08:00"
    );
  }

  #[test]
  fn skips_leftover_sidecars() {
    let d = test_dir!(
      "image.jpg.xmp": { "Creator": "Sidecar" },
      "image_01.jpg.xmp": { "Creator": "Dupe" },
    );

    let mut o = Organizer::import(d.root()).unwrap();
    o.enable_set_copyrights_from_creator();
    o.apply_metadata_updates().unwrap();

    assert_tag!(d, "image.jpg.xmp", "Copyright", None);
    assert_tag!(d, "image_01.jpg.xmp", "Copyright", None);
  }

  #[test]
  fn skips_dupes() {
    let d = test_dir!(
      "image.jpg": {},
      "image.jpg.xmp": { "Creator": "Sidecar" },
      "image_01.jpg.xmp": { "Creator": "Dupe" },
    );

    let mut o = Organizer::import(d.root()).unwrap();
    o.enable_set_copyrights_from_creator();
    o.apply_metadata_updates().unwrap();

    assert_tag!(d, "image_01.jpg.xmp", "Copyright", None);
  }
}

#[cfg(test)]
mod test_set_copyright_from_creator {
  use super::*;
  use crate::testing::*;

  #[test]
  fn preserves_existing_copyright() {
    let d = test_dir!(
      "image.jpg": {},
      "image.jpg.xmp": { "Copyright": "Copyright", "Creator": "Creator" },
    );

    let mut o = Organizer::import(d.root()).unwrap();
    o.enable_set_copyrights_from_creator();
    o.apply_metadata_updates().unwrap();

    assert_tag!(d, "image.jpg.xmp", "Copyright", "Copyright");
  }

  #[test]
  fn sets_copyright_if_missing() {
    let d = test_dir!(
      "image.jpg": {},
      "image.jpg.xmp": { "Creator": "Creator" },
    );

    let mut o = Organizer::import(d.root()).unwrap();
    o.enable_set_copyrights_from_creator();
    o.apply_metadata_updates().unwrap();

    assert_tag!(d, "image.jpg.xmp", "Copyright", "Copyright Creator");
  }
}

#[cfg(test)]
mod test_set_location_from_gps {
  use super::*;
  use crate::testing::*;

  #[test]
  fn overwrites_existing_location() {
    let d = test_dir!(
      "image.jpg": {},
      "image.jpg.xmp": {
        "GPSLatitude": "47.6061 N",
        "GPSLongitude": "122.3328 W",
        "City": "Unknown",
        "State": "Unknown",
        "Country": "Unknown",
      },
    );

    let mut o = Organizer::import(d.root()).unwrap();
    o.enable_set_location_from_gps();
    o.apply_metadata_updates().unwrap();

    assert_tag!(d, "image.jpg.xmp", "City", "Seattle");
    assert_tag!(d, "image.jpg.xmp", "State", "Washington");
    assert_tag!(d, "image.jpg.xmp", "Country", "United States");
  }

  #[test]
  fn sets_missing_location() {
    let d = test_dir!(
      "image.jpg": {},
      "image.jpg.xmp": {
        "GPSLatitude": "47.6061 N",
        "GPSLongitude": "122.3328 W",
      },
    );

    let mut o = Organizer::import(d.root()).unwrap();
    o.enable_set_location_from_gps();
    o.apply_metadata_updates().unwrap();

    assert_tag!(d, "image.jpg.xmp", "City", "Seattle");
    assert_tag!(d, "image.jpg.xmp", "State", "Washington");
    assert_tag!(d, "image.jpg.xmp", "Country", "United States");
  }
}

#[cfg(test)]
mod test_set_time_zone_from_location {
  use super::*;
  use crate::testing::*;

  #[test]
  fn overwrites_time_zone_exif() {
    let d = test_dir!(
      "image.jpg": {
        "DateTimeOriginal": "2000-01-01T00:00:00",
        "OffsetTimeOriginal": "+00:00",
        "GPSLatitude": "47.6061",
        "GPSLatitudeRef": "N",
        "GPSLongitude": "122.3328",
        "GPSLongitudeRef": "W",
      }
    );

    let mut o = Organizer::import(d.root()).unwrap();
    o.enable_set_time_zone_from_gps();
    o.apply_metadata_updates().unwrap();

    assert_tag!(d, "image.jpg", "OffsetTimeOriginal", "-08:00");
  }

  // Kept separate from XMP test, as their is a `UserData:DateTimeOriginal` tag
  // in QuickTime not present in XMP.
  #[test]
  fn overwrites_time_zone_quicktime() {
    let d = test_dir!(
      "video.mov": {
        "CompressorID": "hvc1",
        "DateTimeOriginal": "2000-01-01T00:00:00+00:00",
        "GPSLatitude": "47.6061 N",
        "GPSLongitude": "122.3328 W",
      }
    );

    let mut o = Organizer::import(d.root()).unwrap();
    o.enable_set_time_zone_from_gps();
    o.apply_metadata_updates().unwrap();

    assert_tag!(
      d,
      "video.mov",
      "DateTimeOriginal",
      "2000-01-01T00:00:00-08:00"
    );
  }

  #[test]
  fn overwrites_time_zone_xmp() {
    let d = test_dir!(
      "image.jpg": {},
      "image.jpg.xmp": {
        "DateTimeOriginal": "2000-01-01T00:00:00+00:00",
        "GPSLatitude": "47.6061 N",
        "GPSLongitude": "122.3328 W",
      }
    );

    let mut o = Organizer::import(d.root()).unwrap();
    o.enable_set_time_zone_from_gps();
    o.apply_metadata_updates().unwrap();

    assert_tag!(
      d,
      "image.jpg.xmp",
      "DateTimeOriginal",
      "2000-01-01T00:00:00-08:00"
    );
  }

  #[test]
  fn sets_time_zone_for_exif() {
    let d = test_dir!(
      "image.jpg": {
        "DateTimeOriginal": "2000-01-01T00:00:00",
        "GPSLatitude": "47.6061",
        "GPSLatitudeRef": "N",
        "GPSLongitude": "122.3328",
        "GPSLongitudeRef": "W",
      }
    );

    let mut o = Organizer::import(d.root()).unwrap();
    o.enable_set_time_zone_from_gps();
    o.apply_metadata_updates().unwrap();

    assert_tag!(d, "image.jpg", "OffsetTimeOriginal", "-08:00");
  }

  #[test]
  fn sets_time_zone_for_quicktime() {
    let d = test_dir!(
      "video.mov": {
        "CompressorID": "hvc1",
        "DateTimeOriginal": "2000-01-01T00:00:00",
        "GPSLatitude": "47.6061 N",
        "GPSLongitude": "122.3328 W",
      }
    );

    let mut o = Organizer::import(d.root()).unwrap();
    o.enable_set_time_zone_from_gps();
    o.apply_metadata_updates().unwrap();

    assert_tag!(
      d,
      "video.mov",
      "DateTimeOriginal",
      "2000-01-01T00:00:00-08:00"
    );
  }

  #[test]
  fn sets_time_zone_for_xmp() {
    let d = test_dir!(
      "image.jpg": {},
      "image.jpg.xmp": {
        "DateTimeOriginal": "2000-01-01T00:00:00",
        "GPSLatitude": "47.6061 N",
        "GPSLongitude": "122.3328 W",
      }
    );

    let mut o = Organizer::import(d.root()).unwrap();
    o.enable_set_time_zone_from_gps();
    o.apply_metadata_updates().unwrap();

    assert_tag!(
      d,
      "image.jpg.xmp",
      "DateTimeOriginal",
      "2000-01-01T00:00:00-08:00"
    );
  }
}

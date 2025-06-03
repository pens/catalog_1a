// Copyright 2023-5 Seth Pendergrass. See LICENSE.

//! `ExifTool` metadata handling, for both media files and sidecars.

use core::fmt;
use std::{
  ffi::OsStr,
  fmt::{Display, Formatter},
  path::{Path, PathBuf},
};

use chrono::{FixedOffset, NaiveDateTime};
use regex::Regex;
use serde::Deserialize;

/// Represents whether a file is a media file or sidecar, and if a sidecar,
/// whether the initial (i.e. base or primary) sidecar or a duplicate from
/// darktable.
#[derive(Debug, PartialEq, Eq)]
pub enum FileCategory {
  Media,
  SidecarInitial,
  SidecarDupe,
}

/// Holds the parsed components of a file name, used to determine file type and
/// sidecar <-> media file relationships.
#[derive(Debug, PartialEq, Eq)]
pub struct ParsedFileName<'a> {
  pub parent_and_stem: &'a OsStr,
  pub dupe_number:     Option<&'a OsStr>,
  pub base_ext:        &'a OsStr,
}

/// Metadata for an image or video file.
///
/// Names are from `ExifTool`'s tags: <https://exiftool.org/TagNames/>.
#[derive(Default, Clone, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct Metadata {
  // General.
  pub source_file:         PathBuf,
  pub file_type:           String,
  pub file_type_extension: String,

  // For Live Photos.
  #[serde(rename = "CompressorID")]
  pub compressor_id:      Option<String>,
  pub content_identifier: Option<String>, // Live Photo images & videos.

  // Attribution.
  pub creator:   Option<String>,
  pub copyright: Option<String>,

  // Camera.
  pub make:  Option<String>,
  pub model: Option<String>,

  // Date & Time.
  //
  // Note that SubSec* fields are composite tags for EXIF metadata. Composite
  // tags do not actually exist in the metadata on disk, but are interfaces
  // ExifTool provides to make reading/writing tags easier.
  //
  // SubSec* tags comprise the relevant *Date* tag, OffsetTime* and SubSecTime*
  // tags, joining them into a single date time string. As configured here, this
  // will be an RFC3339 date time string (but potentially without time zone).
  // Note that these tags will only be present if their base *Date* tag is
  // present, *and* either the OffsetTime* or SubSecTime* tag.
  //
  // For XMP metadata, this SubSec* tag will not be present, but the *Date* tag
  // will have the same format as the Composite SubSec* tag. This enables the
  // two to be used interchangeably depending on metadata source.

  // Most recent system file modification (e.g. renaming).
  pub file_modify_date: String,

  // Date of most recent edit.
  pub modify_date:         Option<String>,
  pub sub_sec_modify_date: Option<String>,

  // Date of media file creation (e.g. saving to SD card or scanning film).
  pub create_date:         Option<String>,
  pub sub_sec_create_date: Option<String>,

  // Date of media capture (e.g. actuating the shutter).
  pub date_time_original:         Option<String>,
  // pub offset_time_original:       Option<String>,
  // pub sub_sec_time_original:      Option<u32>,
  pub sub_sec_date_time_original: Option<String>,

  // GPS.
  //
  // Note that XMP metadata will have the GPS references (i.e. N/S and E/W) in
  // the GPSLatitude and GPSLongitude tags, unlike EXIF which will report this
  // in the *Ref tags.
  #[serde(rename = "GPSLatitude")]
  pub gps_latitude: Option<String>,

  // #[serde(rename = "GPSLatitudeRef")]
  // pub gps_latitude_ref: Option<String>,
  #[serde(rename = "GPSLongitude")]
  pub gps_longitude: Option<String>,

  // #[serde(rename = "GPSLongitudeRef")]
  // pub gps_longitude_ref: Option<String>,

  // Composite tag describing entire GPS position.
  #[serde(rename = "GPSPosition")]
  pub gps_position: Option<String>,

  // Location.
  pub city:    Option<String>,
  pub state:   Option<String>,
  pub country: Option<String>,
}

impl Metadata {
  pub fn get_date_time_original(&self) -> Option<(NaiveDateTime, Option<FixedOffset>)> {
    let date_time_original = self
      .sub_sec_date_time_original
      .as_deref()
      .or(self.date_time_original.as_deref())?;

    super::parse_date_time(date_time_original).ok()
  }

  /// Get the type of file this metadata represents.
  pub fn get_file_category(&self) -> FileCategory {
    if self.file_type == "XMP" {
      if self
        .parse_file_name()
        .is_some_and(|f| f.dupe_number.is_some())
      {
        FileCategory::SidecarDupe
      } else {
        FileCategory::SidecarInitial
      }
    } else {
      assert!(self.file_type != "-", "FileType is not set.");
      FileCategory::Media
    }
  }

  /// Parses the GPS metadata values into a latitude and longitude, if possible.
  pub fn get_lat_lon(&self) -> Option<(f32, f32)> {
    let re = Regex::new(
      r#"^(\d+) deg (\d+)\' (\d+\.?\d*)" ([NnSs]), (\d+) deg (\d+)\' (\d+\.?\d*)" ([WwEe])"#,
    )
    .unwrap();

    let Some(caps) = re.captures(self.gps_position.as_deref()?) else {
      log::warn!(
        "Unable to parse GPSPosition: {}",
        self.gps_position.as_deref()?
      );
      return None;
    };

    let (Some(lat_deg), Some(lat_min), Some(lat_sec), Some(lat_ref)) =
      (caps.get(1), caps.get(2), caps.get(3), caps.get(4))
    else {
      log::warn!(
        "Unable to parse latitude components: {}",
        self.gps_position.as_deref()?
      );
      return None;
    };

    let (Ok(lat_deg), Ok(lat_min), Ok(lat_sec)) = (
      lat_deg.as_str().parse::<f32>(),
      lat_min.as_str().parse::<f32>(),
      lat_sec.as_str().parse::<f32>(),
    ) else {
      log::warn!(
        "Unable to parse latitude components to float: {}",
        self.gps_position.as_deref()?
      );
      return None;
    };

    let mut latitude = super::dms_to_lat_lon(lat_deg, lat_min, lat_sec);
    if lat_ref.as_str() == "S" || lat_ref.as_str() == "s" {
      latitude *= -1.0;
    }

    let (Some(lon_deg), Some(lon_min), Some(lon_sec), Some(lon_ref)) =
      (caps.get(5), caps.get(6), caps.get(7), caps.get(8))
    else {
      log::warn!(
        "Unable to parse longitude components: {}",
        self.gps_position.as_deref()?
      );
      return None;
    };

    let (Ok(lon_deg), Ok(lon_min), Ok(lon_sec)) = (
      lon_deg.as_str().parse::<f32>(),
      lon_min.as_str().parse::<f32>(),
      lon_sec.as_str().parse::<f32>(),
    ) else {
      log::warn!(
        "Unable to parse longitude components to float: {}",
        self.gps_position.as_deref()?
      );
      return None;
    };

    let mut longitude = super::dms_to_lat_lon(lon_deg, lon_min, lon_sec);
    if lon_ref.as_str() == "W" || lon_ref.as_str() == "w" {
      longitude *= -1.0;
    }

    Some((latitude, longitude))
  }

  /// Extract the components of the source file name (e.g.
  /// `dir/image_01.jpg.xmp`).
  pub fn parse_file_name(&self) -> Option<ParsedFileName> {
    let re = Regex::new(r"^(?:./)?([^.]*?)(?:_(\d{2}))?\.([^.]*)(?:\.[Xx][Mm][Pp])?$").unwrap();

    let caps = re.captures(self.source_file.to_str()?)?;

    Some(ParsedFileName {
      parent_and_stem: OsStr::new(caps.get(1)?.as_str()),
      dupe_number:     caps.get(2).map(|m| m.as_str()).map(OsStr::new),
      base_ext:        OsStr::new(caps.get(3)?.as_str()),
    })
  }
}

impl AsRef<Path> for Metadata {
  fn as_ref(&self) -> &Path {
    &self.source_file
  }
}

impl Display for Metadata {
  fn fmt(&self, f: &mut Formatter) -> fmt::Result {
    write!(f, "{}", self.source_file.display())
  }
}

#[cfg(test)]
mod test_get_date_time_original {
  use crate::testing::*;

  #[test]
  fn parses_from_exif() {
    let metadata = metadata!(
      "SourceFile": "test.jpg",
      "DateTimeOriginal": "2000-01-01T00:00:00",
      "OffsetTimeOriginal": "-08:00",
      "SubSecTimeOriginal": 999,
      "SubSecDateTimeOriginal": "2000-01-01T00:00:00.999-08:00",
    );

    let (date_time, time_zone) = metadata.get_date_time_original().unwrap();

    let date_time_expected = make_date(2000, 1, 1, 0, 0, 0, 999, -8);

    assert_eq!(date_time, date_time_expected.naive_local());
    assert_eq!(time_zone.unwrap(), *date_time_expected.offset());
  }

  // This is also the same format as QuickTime.
  #[test]
  fn parses_from_xmp() {
    let metadata = metadata!(
      "SourceFile": "test.jpg.xmp",
      "DateTimeOriginal": "2000-01-01T00:00:00.999-08:00",
    );

    let (date_time, time_zone) = metadata.get_date_time_original().unwrap();

    let date_time_expected = make_date(2000, 1, 1, 0, 0, 0, 999, -8);

    assert_eq!(date_time, date_time_expected.naive_local());
    assert_eq!(time_zone.unwrap(), *date_time_expected.offset());
  }
}

#[cfg(test)]
mod test_get_file_category {
  use super::*;
  use crate::testing::*;

  #[test]
  fn identifies_dupe() {
    let metadata = metadata!(
      "SourceFile": "image_01.jpg.xmp",
      "FileType": "XMP",
    );

    assert_eq!(metadata.get_file_category(), FileCategory::SidecarDupe);
  }

  #[test]
  fn identifies_media() {
    let metadata = metadata!(
      "SourceFile": "test.jpg",
      "FileType": "JPEG",
    );

    assert_eq!(metadata.get_file_category(), FileCategory::Media);
  }

  #[test]
  fn identifies_sidecar() {
    let metadata = metadata!(
      "SourceFile": "test.jpg.xmp",
      "FileType": "XMP",
    );

    assert_eq!(metadata.get_file_category(), FileCategory::SidecarInitial);
  }
}

#[cfg(test)]
mod test_get_lat_lon {
  use crate::testing::*;

  #[test]
  fn parses_from_exif() {
    let metadata = metadata!(
      "GPSLatitude": "47.6061",
      "GPSLatitudeRef": "N",
      "GPSLongitude": "122.3328",
      "GPSLongitudeRef": "W",
      "GPSPosition": "47 deg 36' 21.96\" N, 122 deg 19' 58.08\" W",
    );

    assert_eq!(metadata.get_lat_lon(), Some((47.6061, -122.3328)));
  }

  #[test]
  fn parses_from_xmp() {
    let metadata = metadata!(
      "GPSLatitude": "47.6061 N",
      "GPSLongitude": "122.3328 W",
      "GPSPosition": "47 deg 36' 21.96\" N, 122 deg 19' 58.08\" W",
    );

    assert_eq!(metadata.get_lat_lon(), Some((47.6061, -122.3328)));
  }
}

#[cfg(test)]
mod test_parse_file_name {
  use super::*;
  use crate::testing::*;

  #[test]
  fn parses_dupe() {
    let metadata = metadata!(
      "SourceFile": "dir/image_01.jpg.xmp",
    );

    assert_eq!(
      metadata.parse_file_name(),
      Some(ParsedFileName {
        parent_and_stem: OsStr::new("dir/image"),
        dupe_number:     Some(OsStr::new("01")),
        base_ext:        OsStr::new("jpg"),
      })
    );
  }

  #[test]
  fn parses_dupe_with_exiftool_dupe_letter() {
    let metadata = metadata!(
      "SourceFile": "dir/image_b_01.jpg.xmp",
    );

    assert_eq!(
      metadata.parse_file_name(),
      Some(ParsedFileName {
        parent_and_stem: OsStr::new("dir/image_b"),
        dupe_number:     Some(OsStr::new("01")),
        base_ext:        OsStr::new("jpg"),
      })
    );
  }

  #[test]
  fn parses_media() {
    let metadata = metadata!(
      "SourceFile": "dir/image.jpg",
    );

    assert_eq!(
      metadata.parse_file_name(),
      Some(ParsedFileName {
        parent_and_stem: OsStr::new("dir/image"),
        dupe_number:     None,
        base_ext:        OsStr::new("jpg"),
      })
    );
  }

  #[test]
  fn parses_sidecar() {
    let metadata = metadata!(
      "SourceFile": "dir/image.jpg.xmp",
    );

    assert_eq!(
      metadata.parse_file_name(),
      Some(ParsedFileName {
        parent_and_stem: OsStr::new("dir/image"),
        dupe_number:     None,
        base_ext:        OsStr::new("jpg"),
      })
    );
  }
}

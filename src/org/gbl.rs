//! Copyright 2023-4 Seth Pendergrass. See LICENSE.

use std::collections::HashSet;

pub type FileHandle = u32;

lazy_static! {
  pub static ref LIVE_PHOTO_IMAGE_EXTS: HashSet<&'static str> = HashSet::from(["JPEG", "HEIC"]);
  pub static ref LIVE_PHOTO_VIDEO_EXTS: HashSet<&'static str> = HashSet::from(["MOV"]);
  pub static ref MY_CAMERAS: HashSet<&'static str> = HashSet::from([
    "Canon EOS RP",
    "Canon EOS 100D",
    "D3100",
    "iPhone 12 Mini",
    "iPhone XS",
    "iPad (6th generation)",
    "iPhone X",
    "XT1575",
    "iPad Air",
    "iPhone 6 Plus",
    "iPhone 6",
    "Pixel",
    "iPhone 5",
    "PC36100",
  ]);
}

pub const DATETIME_FMT: &str = "%Y-%m-%d %H:%M:%S %z";

// These args will be synchronized in `copy_metadata`.
pub const ARGS_SYNC: [&str; 12] = [
  "-Artist",
  "-Copyright",
  "-CreateDate",
  "-DateTimeOriginal",
  "-GPSAltitude",
  "-GPSAltitudeRef",
  "-GPSLatitude",
  "-GPSLatitudeRef",
  "-GPSLongitude",
  "-GPSLongitudeRef",
  "-Make",
  "-Model",
];

pub const ARGS_SYS: [&str; 6] = [
  "-d",
  DATETIME_FMT,
  "-FileModifyDate",
  "-FileType",
  "-FileTypeExtension",
  "-ContentIdentifier",
];

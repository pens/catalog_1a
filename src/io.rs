// Copyright 2023-5 Seth Pendergrass. See LICENSE.

//! Functions for manipulating files.

use std::{
  ffi::{OsStr, OsString},
  fs,
  path::{Path, PathBuf},
  process::Command,
};

use regex::Regex;

use crate::prim::Metadata;

/// All `ExifTool` operations will use this format when extracting date & time.
/// Follows RFC 3339 format for easy parsing with `chrono`.
pub const DATETIME_READ_FORMAT: &str = "%Y-%m-%dT%H:%M:%S%f%:z";

/// Formats file path and name to keep images sorted by time as best possible,
/// and allow for darktable's duplicate file naming to work. darktable appends a
/// two-digit number to the end of the file name, before the extension, on the
/// duplicated sidecar (e.g. `image_01.jpg.xmp`).
///
/// Example:
/// Input: January 1st, 2024 at 12:30:01.050, second image at this exact time.
/// Output: `2024/01/240101_123001050_b.jpg`.
/// darktable duplicate: `2024/01/240101_123001050_b_01.jpg.xmp`.
const DATETIME_WRITE_FORMAT: &str = "%Y/%m/%y%m%d_%H%M%S%-3f%+lc";

/// When using `ExifTool` to read metadata, this converts the time zone to UTC
/// in RFC 3339 format, and puts the output into JSON for easy parsing with
/// `serde_json`.
const READ_ARGS: [&str; 3] = ["-d", DATETIME_READ_FORMAT, "-json"];

/// Arguments for converting metadata from EXIF to XMP format.
const COPY_EXIF_2_XMP: &str = include_str!("../third_party/exiftool/arg_files/exif2xmp.args");

/// Arguments for converting metadata from XMP to EXIF format.
const COPY_XMP_2_EXIF: &str = include_str!("../third_party/exiftool/arg_files/xmp2exif.args");

/// Minimum supported (tested) version of `ExifTool`.
const EXIFTOOL_MIN_VERSION: (u32, u32) = (13, 29);

/// Returns an iterator over the `ExifTool` arguments needed to supporting
/// copying between any two files containing XMP and/or EXIF metadata.
fn make_copy_args<'a>() -> impl Iterator<Item = &'a OsStr> {
  // Using `-all:all < X:all` to preserve family 1 group (e.g. `XMP-exif`).
  // See <https://exiftool.org/metafiles.html>.
  COPY_EXIF_2_XMP
    .lines()
    .filter(|l| l.trim_start().starts_with('-'))
    .chain(
      COPY_XMP_2_EXIF
        .lines()
        .filter(|l| l.trim_start().starts_with('-')),
    )
    .chain(["-all:all<XMP:all", "-all:all<EXIF:all"])
    .map(OsStr::new)
}

/// Copies metadata from `file_src` to `file_dst`, and returns the new metadata
/// from `file_dst`.
pub fn copy_metadata(
  file_src: impl AsRef<Path>,
  file_dst: impl AsRef<Path>,
) -> Result<Metadata, String> {
  let file_src = make_canonical(file_src)?;
  let file_dst = make_canonical(file_dst)?;

  let mut args = Vec::from([OsStr::new("-tagsFromFile"), file_src.as_os_str()]);
  args.extend(make_copy_args());
  args.push(file_dst.as_os_str());
  run_exiftool(None::<&Path>, args)?;

  read_metadata(&file_dst)
}

/// Creates XMP for `file_media`, and reads back its metadata.
pub fn create_xmp(file_media: impl AsRef<Path>) -> Result<Metadata, String> {
  let file_media = make_canonical(file_media)?;

  if file_media.extension().is_none_or(|e| e == "xmp") {
    return Err(format!(
      "{}: Cannot create XMP (invalid extension).",
      file_media.display()
    ));
  }

  let mut file_xmp = file_media.clone();
  file_xmp.add_extension("xmp");

  if file_xmp.exists() {
    return Err(format!(
      "{}: Cannot create XMP (file already exists).",
      file_xmp.display()
    ));
  }

  let mut args = Vec::from([OsStr::new("-tagsFromFile"), file_media.as_os_str()]);
  args.extend(make_copy_args());
  args.push(file_xmp.as_os_str());
  run_exiftool(None::<&Path>, args)?;

  read_metadata(file_xmp)
}

/// Check that `ExifTool` is present and new enough.
pub fn exiftool_check() -> Result<(), String> {
  version_check(run_exiftool(None::<&Path>, ["-ver"])?, EXIFTOOL_MIN_VERSION)
}

/// Moves `file_src` to `yyyy/mm/yymmdd_hhmmssfff_c.ext` under `dir_dst`.
/// Optionally, if `metadata_src` is `Some`, uses its metadata for the date and
/// time instead. Returns the path to the new file.
pub fn move_file(
  file_src: impl AsRef<Path>,
  metadata_src: Option<impl AsRef<Path>>,
  dir_dst: impl AsRef<Path>,
  ext: impl AsRef<OsStr>,
) -> Result<PathBuf, String> {
  let file_src = make_canonical(file_src)?;
  let metadata_src = metadata_src.map(make_canonical).transpose()?;
  let dir_dst = make_canonical(dir_dst)?;
  let ext = ext.as_ref();

  let mut args = Vec::new();

  let metadata_src_path;
  if let Some(metadata_src) = metadata_src {
    metadata_src_path = metadata_src.clone();
    args.extend([OsStr::new("-tagsFromFile"), metadata_src_path.as_os_str()]);
  }

  // `-v` needed to report renaming.
  args.extend(["-v", "-d", DATETIME_WRITE_FORMAT].map(OsStr::new));

  let mut args_rename = Vec::new();

  for date_time_tag in [
    "CreateDate",
    "SubSecCreateDate",
    "DateTimeOriginal",
    "SubSecDateTimeOriginal",
  ] {
    let mut rename_format = OsString::from("-FileName<");
    rename_format.push(dir_dst.as_os_str());
    rename_format.push(format!("/${{{date_time_tag}}}"));
    rename_format.push(ext);

    args_rename.push(rename_format);
  }

  for name_arg in &args_rename {
    args.push(name_arg.as_os_str());
  }

  args.push(file_src.as_os_str());

  let stdout = String::from_utf8(run_exiftool(Some(&dir_dst), args)?)
    .map_err(|e| format!("Could not parse ExifTool output as UTF-8 ({e})."))?;

  if stdout.contains("0 image files updated") {
    return Err(format!("{}: Failed to move file.", file_src.display()));
  }

  make_canonical(dir_dst.join(extract_destination(&stdout)?))
}

/// Gets metadata for `file`.
pub fn read_metadata(file: impl AsRef<Path>) -> Result<Metadata, String> {
  let file = make_canonical(file)?;

  let mut args = Vec::from(READ_ARGS.map(OsStr::new));
  args.push(file.as_os_str());

  Ok(parse_vec(run_exiftool(None::<&Path>, args)?)?.remove(0))
}

/// Reads metadata from `dir_root` and all subdirectories, excluding `exclude`
/// (e.g. `trash/`).
pub fn read_metadata_recursive(
  dir_root: impl AsRef<Path>,
  dir_exclude: Option<impl AsRef<Path>>,
) -> Result<Vec<Metadata>, String> {
  let dir_root = make_canonical(dir_root)?;
  let dir_exclude = dir_exclude.map(make_canonical).transpose()?;

  let mut args = Vec::from(READ_ARGS.map(OsStr::new));
  args.extend(["-r", "."].map(OsStr::new));

  let exclude_relative;

  if let Some(exclude_path) = dir_exclude {
    exclude_relative = exclude_path
      .strip_prefix(&dir_root)
      .map_err(|_| {
        format!(
          "{}: Exclude path must be within the read directory ({}).",
          exclude_path.display(),
          dir_root.display()
        )
      })?
      .to_path_buf();

    args.extend([OsStr::new("-i"), exclude_relative.as_os_str()]);
  }

  parse_vec(run_exiftool(Some(dir_root), args)?)
}

/// Moves `file` under `dir_trash`, maintaining its directory structure relative
/// to `dir_root`
pub fn remove_file(
  dir_root: impl AsRef<Path>,
  dir_trash: impl AsRef<Path>,
  file: impl AsRef<Path>,
) -> Result<(), String> {
  let dir_root = make_canonical(dir_root)?;
  let dir_trash = make_canonical(dir_trash)?;
  let file = make_canonical(file)?;

  if file.starts_with(&dir_trash) {
    return Err(format!(
      "{}: Cannot remove file already in trash ({}).",
      file.display(),
      dir_trash.display()
    ));
  }

  let path_relative = file.strip_prefix(&dir_root).map_err(|_| {
    format!(
      "{}: Cannot remove file outside root directory ({}).",
      file.display(),
      dir_root.display()
    )
  })?;

  let path_trash = dir_trash.join(path_relative);

  if path_trash.exists() {
    return Err(format!(
      "{}: Cannot remove file due to name collision in trash ({}).",
      file.display(),
      path_trash.display()
    ));
  }

  fs::create_dir_all(path_trash.parent().unwrap()).unwrap();
  fs::rename(file, path_trash).unwrap();

  Ok(())
}

/// Runs `ExifTool` with `args`, from optional working directory `dir_root`.
/// Panics if `ExifTool` fails.
pub fn run_exiftool<I: IntoIterator<Item = S>, S: AsRef<OsStr>>(
  dir_root: Option<impl AsRef<Path>>,
  args: I,
) -> Result<Vec<u8>, String> {
  let dir_root = dir_root.map(make_canonical).transpose()?;

  let mut cmd = Command::new(PathBuf::from(env!("OUT_DIR")).join("exiftool"));
  if let Some(dir_root) = dir_root {
    cmd.current_dir(dir_root);
  }
  cmd.args(args);

  let output = cmd.output().map_err(|e| {
    format!(
      "ExifTool failed to run.\nArgs:\n{}\nError:\n{e}",
      cmd
        .get_args()
        .collect::<Vec<_>>()
        .join(OsStr::new(" "))
        .display(),
    )
  })?;

  if !output.status.success() {
    return Err(format!(
      "ExifTool did not run successfully.\nArgs:\n{}\nstderr:\n{}",
      cmd
        .get_args()
        .collect::<Vec<_>>()
        .join(OsStr::new(" "))
        .display(),
      String::from_utf8_lossy(&output.stderr)
    ));
  }

  Ok(output.stdout)
}

/// Given a byte stream `stdout` from `ExifTool`, extracts the destination of a
/// rename or move. Expects the format: 'OLDNAME.jpg' --> 'NEWNAME.jpg'.
fn extract_destination(stdout: &str) -> Result<PathBuf, String> {
  let re = Regex::new(r"'.+' --> '(.+)'").unwrap();

  let caps = re.captures(stdout).ok_or(format!(
    "ExifTool output did not match regex.\nstdout:\n{stdout}"
  ))?;

  let file_dst = caps.get(1).ok_or(format!(
    "ExifTool output did not contain a destination.\nstdout:\n{stdout}"
  ))?;

  Ok(PathBuf::from(file_dst.as_str()))
}

/// Converts a path to an absolute, canonical form. Panics if `path` is not
/// absolute or does not point to a real file or directory.
fn make_canonical(path: impl AsRef<Path>) -> Result<PathBuf, String> {
  let path = path.as_ref();

  if !path.is_absolute() {
    return Err(format!("{}: Path is not absolute.", path.display()));
  }
  if !path.exists() {
    return Err(format!("{}: Path does not exist.", path.display()));
  }

  path
    .canonicalize()
    .map_err(|e| format!("{}: Path failed to canonicalize ({e}).", path.display()))
}

/// Parses `ExifTool`'s JSON-formatted output `metadata` into Rust types.
fn parse_vec(metadata: impl AsRef<[u8]>) -> Result<Vec<Metadata>, String> {
  // `serde_json` doesn't handle the empty case.
  if metadata.as_ref().is_empty() {
    return Ok(Vec::new());
  }

  serde_json::from_slice(metadata.as_ref()).map_err(|e| {
    format!(
      "Failed to parsed ExifTool output as metadata ({e}).\nstdout:\n{}",
      String::from_utf8_lossy(metadata.as_ref())
    )
  })
}

/// Returns whether `version` is as new or newer than `version_required_min`,
/// where `version` is from `ExifTool`'s stdout.
fn version_check(version: Vec<u8>, version_required_min: (u32, u32)) -> Result<(), String> {
  let version = String::from_utf8(version).unwrap();
  let Some((major, minor)) = version.trim().split_once('.') else {
    return Err(format!("Unexpected ExifTool version string: \"{version}\""));
  };

  let major = major.parse::<u32>();
  let minor = minor.parse::<u32>();
  let (Ok(major), Ok(minor)) = (major, minor) else {
    return Err(format!("Unexpected ExifTool version: {version}"));
  };

  if major > version_required_min.0
    || (major == version_required_min.0 && minor >= version_required_min.1)
  {
    Ok(())
  } else {
    Err(format!(
      "ExifTool version {major}.{minor} is too old (needs {}.{} or newer).",
      version_required_min.0, version_required_min.1
    ))
  }
}

#[cfg(test)]
mod test_copy_metadata {
  use super::*;
  use crate::testing::*;

  #[test]
  fn converts_exif_tags_to_quicktime() {
    let d = test_dir!(
      "image.jpg": {
        "DateTimeOriginal": "2000-01-01T00:00:00",
        "SubSecTimeOriginal": "999",
        "OffsetTimeOriginal": "-08:00",
        "GPSLatitude": "47.6061",
        "GPSLatitudeRef": "N",
        "GPSLongitude": "122.3328",
        "GPSLongitudeRef": "W",
      },
      "video.mov": { "CompressorID": "avc1" },
    );

    copy_metadata(d.get_path("image.jpg"), d.get_path("video.mov")).unwrap();

    let metadata = read_metadata(d.get_path("video.mov")).unwrap();
    assert_eq!(
      metadata.date_time_original,
      Some("2000-01-01T00:00:00.999-08:00".to_string())
    );
    assert_eq!(
      metadata.gps_position,
      Some("47 deg 36' 21.96\" N, 122 deg 19' 58.08\" W".to_string())
    );
  }

  #[test]
  fn converts_exif_tags_to_xmp() {
    let d = test_dir!(
      "image.jpg": {
        "DateTimeOriginal": "2000-01-01T00:00:00",
        "SubSecTimeOriginal": "999",
        "OffsetTimeOriginal": "-08:00",
        "GPSLatitude": "47.6061",
        "GPSLatitudeRef": "N",
        "GPSLongitude": "122.3328",
        "GPSLongitudeRef": "W",
      },
      "image.jpg.xmp": {},
    );

    copy_metadata(d.get_path("image.jpg"), d.get_path("image.jpg.xmp")).unwrap();

    let metadata = read_metadata(d.get_path("image.jpg.xmp")).unwrap();
    assert_eq!(
      metadata.date_time_original,
      Some("2000-01-01T00:00:00.999-08:00".to_string())
    );
    assert_eq!(
      metadata.gps_position,
      Some("47 deg 36' 21.96\" N, 122 deg 19' 58.08\" W".to_string())
    );
  }

  #[test]
  fn converts_quicktime_tags_to_exif() {
    let d = test_dir!(
      "video.mov": {
        "CompressorID": "avc1",
        "DateTimeOriginal": "2000-01-01T00:00:00.999-08:00",
        "GPSLatitude": "47.6061 N",
        "GPSLongitude": "122.3328 W",
      },
      "image.jpg": {},
    );

    copy_metadata(d.get_path("video.mov"), d.get_path("image.jpg")).unwrap();

    let metadata = read_metadata(d.get_path("image.jpg")).unwrap();
    assert_eq!(
      metadata.sub_sec_date_time_original,
      Some("2000-01-01T00:00:00.999-08:00".to_string())
    );
    assert_eq!(
      metadata.gps_position,
      Some("47 deg 36' 21.96\" N, 122 deg 19' 58.08\" W".to_string())
    );
  }

  #[test]
  fn converts_quicktime_tags_to_xmp() {
    let d = test_dir!(
      "video.mov": {
        "CompressorID": "avc1",
        "DateTimeOriginal": "2000-01-01T00:00:00.999-08:00",
        "GPSLatitude": "47.6061 N",
        "GPSLongitude": "122.3328 W",
      },
      "video.mov.xmp": {},
    );

    copy_metadata(d.get_path("video.mov"), d.get_path("video.mov.xmp")).unwrap();

    let metadata = read_metadata(d.get_path("video.mov.xmp")).unwrap();
    assert_eq!(
      metadata.date_time_original,
      Some("2000-01-01T00:00:00.999-08:00".to_string())
    );
    assert_eq!(
      metadata.gps_position,
      Some("47 deg 36' 21.96\" N, 122 deg 19' 58.08\" W".to_string())
    );
  }

  #[test]
  fn converts_xmp_tags_to_exif() {
    let d = test_dir!(
      "image.jpg.xmp": {
        "DateTimeOriginal": "2000-01-01T00:00:00.999-08:00",
        "GPSLatitude": "47.6061 N",
        "GPSLongitude": "122.3328 W",
      },
      "image.jpg": {},
    );

    copy_metadata(d.get_path("image.jpg.xmp"), d.get_path("image.jpg")).unwrap();

    let metadata = read_metadata(d.get_path("image.jpg")).unwrap();
    assert_eq!(
      metadata.sub_sec_date_time_original,
      Some("2000-01-01T00:00:00.999-08:00".to_string())
    );
    assert_eq!(
      metadata.gps_position,
      Some("47 deg 36' 21.96\" N, 122 deg 19' 58.08\" W".to_string())
    );
  }

  #[test]
  fn converts_xmp_tags_to_quicktime() {
    let d = test_dir!(
      "video.mov.xmp": {
        "DateTimeOriginal": "2000-01-01T00:00:00.999-08:00",
        "GPSLatitude": "47.6061 N",
        "GPSLongitude": "122.3328 W",
      },
      "video.mov": { "CompressorID": "avc1" },
    );

    copy_metadata(d.get_path("video.mov.xmp"), d.get_path("video.mov")).unwrap();

    let metadata = read_metadata(d.get_path("video.mov")).unwrap();
    assert_eq!(
      metadata.date_time_original,
      Some("2000-01-01T00:00:00.999-08:00".to_string())
    );
    assert_eq!(
      metadata.gps_position,
      Some("47 deg 36' 21.96\" N, 122 deg 19' 58.08\" W".to_string())
    );
  }

  /// Check that metadata copies over.
  #[test]
  fn copies_tag() {
    let d = test_dir!(
      "image1.jpg": { "Creator": "Creator" },
      "image2.jpg": {},
    );

    copy_metadata(d.get_path("image1.jpg"), d.get_path("image2.jpg")).unwrap();

    let metadata = read_metadata(d.get_path("image2.jpg")).unwrap();
    assert_eq!(metadata.creator, Some("Creator".to_string()));
  }

  #[test]
  fn returns_destination_metadata() {
    let d = test_dir!(
      "image.jpg": { "Creator": "Creator" },
      "image.jpg.xmp": {},
    );

    let metadata_returned =
      copy_metadata(d.get_path("image.jpg"), d.get_path("image.jpg.xmp")).unwrap();

    let metadata_read = read_metadata(d.get_path("image.jpg.xmp")).unwrap();
    assert_eq!(metadata_returned.source_file, metadata_read.source_file);
    assert_eq!(metadata_returned.creator, metadata_read.creator);
  }
}

#[cfg(test)]
mod test_create_xmp {
  use super::*;
  use crate::testing::*;

  #[test]
  fn converts_exif_tags_to_xmp() {
    let d = test_dir!(
      "image.jpg": {
        "DateTimeOriginal": "2000-01-01T00:00:00",
        "SubSecTimeOriginal": "999",
        "OffsetTimeOriginal": "-08:00",
        "GPSLatitude": "47.6061",
        "GPSLatitudeRef": "N",
        "GPSLongitude": "122.3328",
        "GPSLongitudeRef": "W",
      },
    );

    create_xmp(d.get_path("image.jpg")).unwrap();

    let metadata = read_metadata(d.get_path("image.jpg.xmp")).unwrap();
    assert_eq!(
      metadata.date_time_original,
      Some("2000-01-01T00:00:00.999-08:00".to_string())
    );
    assert_eq!(
      metadata.gps_position,
      Some("47 deg 36' 21.96\" N, 122 deg 19' 58.08\" W".to_string())
    );
  }

  #[test]
  fn converts_quicktime_tags_to_xmp() {
    let d = test_dir!(
      "video.mov": {
        "CompressorID": "avc1",
        "DateTimeOriginal": "2000-01-01T00:00:00.999-08:00",
        "GPSLatitude": "47.6061 N",
        "GPSLongitude": "122.3328 W",
      },
    );

    create_xmp(d.get_path("video.mov")).unwrap();

    let metadata = read_metadata(d.get_path("video.mov.xmp")).unwrap();
    assert_eq!(
      metadata.date_time_original,
      Some("2000-01-01T00:00:00.999-08:00".to_string())
    );
    assert_eq!(
      metadata.gps_position,
      Some("47 deg 36' 21.96\" N, 122 deg 19' 58.08\" W".to_string())
    );
  }

  #[test]
  fn copies_metadata_to_xmp() {
    let d = test_dir!(
      "image.jpg": { "Creator": "Creator" },
    );

    create_xmp(d.get_path("image.jpg")).unwrap();

    let metadata = read_metadata(d.get_path("image.jpg.xmp")).unwrap();
    assert_eq!(metadata.source_file, d.get_path("image.jpg.xmp"));
    assert_eq!(metadata.creator, Some("Creator".to_string()));
  }

  #[test]
  fn creates_xmp() {
    let d = test_dir!(
      "image.jpg": { "Creator": "Creator" },
    );

    create_xmp(d.get_path("image.jpg")).unwrap();

    assert_dir!(d, ["image.jpg", "image.jpg.xmp"]);
  }

  #[test]
  fn errors_if_extension_is_xmp() {
    let d = test_dir!(
      "image.jpg.xmp": {},
    );

    assert_err!(
      create_xmp(d.get_path("image.jpg.xmp")),
      "Cannot create XMP (invalid extension)."
    );
  }

  #[test]
  fn errors_if_xmp_already_exists() {
    let d = test_dir!(
      "image.jpg": {},
      "image.jpg.xmp": {},
    );

    assert_err!(
      create_xmp(d.get_path("image.jpg")),
      "Cannot create XMP (file already exists)."
    );
  }

  #[test]
  fn returns_xmp_metadata() {
    let d = test_dir!(
      "image.jpg": { "Creator": "Creator" },
    );

    let metadata_returned = create_xmp(d.get_path("image.jpg")).unwrap();

    let metadata_read = read_metadata(d.get_path("image.jpg.xmp")).unwrap();
    assert_eq!(metadata_returned.source_file, metadata_read.source_file);
    assert_eq!(metadata_returned.creator, metadata_read.creator);
  }
}

#[cfg(test)]
mod test_extract_destination {
  use super::*;
  use crate::testing::*;

  #[test]
  fn parses_destination() {
    let d = test_dir!(
      "image.jpg": {}
    );

    let stdout = String::from_utf8(
      run_exiftool(Some(d.root()), ["image.jpg", "-TestName=image_new.jpg"]).unwrap(),
    )
    .unwrap();

    assert_eq!(
      extract_destination(&stdout).unwrap(),
      PathBuf::from("image_new.jpg")
    );
  }
}

#[cfg(test)]
mod test_make_canonical {
  use super::make_canonical;
  use crate::testing::*;

  #[test]
  fn errors_if_path_does_not_exist() {
    assert_err!(
      make_canonical("/path/does/not/exist"),
      "Path does not exist."
    );
  }

  #[test]
  fn errors_if_path_is_relative() {
    assert_err!(make_canonical("relative/path"), "Path is not absolute.");
  }

  #[test]
  fn returns_canonical_path() {
    let d = test_dir!(
      "image.jpg": {},
    );

    let path = make_canonical(d.get_path("image.jpg")).unwrap();

    assert!(path.is_absolute());
    assert!(path.exists());
    assert!(!path.is_symlink());
  }
}

#[cfg(test)]
mod test_move_file {
  use super::*;
  use crate::testing::*;

  #[test]
  fn adds_counter_when_same_time() {
    let d = test_dir!(
      "image1.jpg": { "DateTimeOriginal": "2000-01-01T00:00:00", "OffsetTimeOriginal": "+00:00" },
      "image2.jpg": { "DateTimeOriginal": "2000-01-01T00:00:00", "OffsetTimeOriginal": "+00:00" },
    );

    move_file(d.get_path("image1.jpg"), None::<&Path>, d.root(), ".jpg").unwrap();
    move_file(d.get_path("image2.jpg"), None::<&Path>, d.root(), ".jpg").unwrap();

    assert_dir!(d, [
      "2000/01/000101_000000000.jpg",
      "2000/01/000101_000000000_b.jpg",
    ]);
  }

  #[test]
  fn errors_if_no_date_time_tags() {
    let d = test_dir!(
      "image.jpg": {},
    );

    assert_err!(
      move_file(d.get_path("image.jpg"), None::<&Path>, d.root(), ".jpg"),
      "Failed to move file."
    );
  }

  #[test]
  fn uses_create_date_as_fallback_from_exif() {
    let d = test_dir!(
      "image.jpg": {
        "CreateDate": "2025-01-01T00:00:00",
        "OffsetTimeDigitized": "+00:00",
      },
    );

    move_file(d.get_path("image.jpg"), None::<&Path>, d.root(), ".jpg").unwrap();

    assert_dir!(d, ["2025/01/250101_000000000.jpg"]);
  }

  #[test]
  fn uses_create_date_as_fallback_from_xmp() {
    let d = test_dir!(
      "image.jpg.xmp": {
        "CreateDate": "2025-01-01T00:00:00+00:00",
      },
    );

    move_file(
      d.get_path("image.jpg.xmp"),
      None::<&Path>,
      d.root(),
      ".jpg.xmp",
    )
    .unwrap();

    assert_dir!(d, ["2025/01/250101_000000000.jpg.xmp"]);
  }

  #[test]
  fn uses_date_time_original_as_primary_from_exif() {
    let d = test_dir!(
      "image.jpg": {
        "DateTimeOriginal": "2000-01-01T00:00:00",
        "OffsetTimeOriginal": "+00:00",
        "CreateDate": "2025-01-01T00:00:00",
        "OffsetTimeDigitized": "+00:00",
      },
    );

    move_file(d.get_path("image.jpg"), None::<&Path>, d.root(), ".jpg").unwrap();

    assert_dir!(d, ["2000/01/000101_000000000.jpg"]);
  }

  #[test]
  fn uses_date_time_original_as_primary_from_xmp() {
    let d = test_dir!(
      "image.jpg.xmp": {
        "DateTimeOriginal": "2000-01-01T00:00:00+00:00",
        "CreateDate": "2025-01-01T00:00:00+00:00",
      },
    );

    move_file(
      d.get_path("image.jpg.xmp"),
      None::<&Path>,
      d.root(),
      ".jpg.xmp",
    )
    .unwrap();

    assert_dir!(d, ["2000/01/000101_000000000.jpg.xmp"]);
  }

  #[test]
  fn renames_in_utc_from_exif() {
    let d = test_dir!(
      "image.jpg": {
        "DateTimeOriginal": "2000-01-01T00:00:00",
        "OffsetTimeOriginal": "-08:00",
      },
    );

    move_file(d.get_path("image.jpg"), None::<&Path>, d.root(), ".jpg").unwrap();

    assert_dir!(d, ["2000/01/000101_080000000.jpg"]);
  }

  #[test]
  fn renames_in_utc_from_xmp() {
    let d = test_dir!(
      "image.jpg": {
        "DateTimeOriginal": "2000-01-01T00:00:00-08:00",
      },
    );

    move_file(d.get_path("image.jpg"), None::<&Path>, d.root(), ".jpg").unwrap();

    assert_dir!(d, ["2000/01/000101_080000000.jpg"]);
  }

  #[test]
  fn renames_with_subseconds_from_exif() {
    let d = test_dir!(
      "image.jpg": {
        "DateTimeOriginal": "2000-01-01T00:00:00",
        "SubSecTimeOriginal": "999",
        "OffsetTimeOriginal": "+00:00",
      },
    );

    move_file(d.get_path("image.jpg"), None::<&Path>, d.root(), ".jpg").unwrap();

    assert_dir!(d, ["2000/01/000101_000000999.jpg"]);
  }

  #[test]
  fn renames_with_subseconds_from_xmp() {
    let d = test_dir!(
      "image.jpg.xmp": {
        "DateTimeOriginal": "2000-01-01T00:00:00.999+00:00",
      },
    );

    move_file(
      d.get_path("image.jpg.xmp"),
      None::<&Path>,
      d.root(),
      ".jpg.xmp",
    )
    .unwrap();

    assert_dir!(d, ["2000/01/000101_000000999.jpg.xmp"]);
  }

  #[test]
  fn returns_new_path() {
    let d = test_dir!(
      "image.jpg": { "DateTimeOriginal": "2000-01-01T00:00:00" },
    );

    let p = move_file(d.get_path("image.jpg"), None::<&Path>, d.root(), ".jpg").unwrap();

    assert_eq!(p, d.get_path("2000/01/000101_000000000.jpg"));
  }

  #[test]
  fn uses_tag_source() {
    let d = test_dir!(
      "image.jpg": { "DateTimeOriginal": "2000-01-01T00:00:00" },
      "image.jpg.xmp": { "DateTimeOriginal": "2025-01-01T00:00:00" },
    );

    move_file(
      d.get_path("image.jpg"),
      Some(d.get_path("image.jpg.xmp")),
      d.root(),
      ".jpg",
    )
    .unwrap();

    assert_dir!(d, ["2025/01/250101_000000000.jpg", "image.jpg.xmp"]);
  }
}

#[cfg(test)]
mod test_read_metadata {
  use chrono::NaiveDate;

  use super::*;
  use crate::{prim, testing::*};

  #[test]
  fn errors_if_file_does_not_exist() {
    let d = test_dir!();
    assert_err!(
      read_metadata(d.get_path("image.jpg")),
      "Path does not exist."
    );
  }

  #[test]
  fn reads_no_time_zone_as_local() {
    let d = test_dir!(
      "image.jpg.xmp": { "DateTimeOriginal": "2000-01-01T00:00:00" },
    );

    let metadata = read_metadata(d.get_path("image.jpg.xmp")).unwrap();

    let local_date_time = NaiveDate::from_ymd_opt(2000, 1, 1)
      .and_then(|d| d.and_hms_opt(0, 0, 0))
      .unwrap();
    let local_offset = prim::get_offset_local(&local_date_time).to_string();

    assert_eq!(
      metadata.date_time_original,
      Some(format!("2000-01-01T00:00:00{local_offset}"))
    );
  }

  #[test]
  fn reads_tag() {
    let d = test_dir!(
      "image.jpg": { "Creator": "Creator" },
    );

    let metadata = read_metadata(d.get_path("image.jpg")).unwrap();

    assert_eq!(metadata.creator, Some("Creator".to_string()));
  }

  #[test]
  fn reads_time_zone() {
    let d = test_dir!(
      "image.jpg.xmp": { "DateTimeOriginal": "2000-01-01T00:00:00-08:00" },
    );

    let metadata = read_metadata(d.get_path("image.jpg.xmp")).unwrap();

    assert_eq!(
      metadata.date_time_original,
      Some("2000-01-01T00:00:00-08:00".to_string())
    );
  }

  #[test]
  fn returns_source_file_relative_to_root() {
    let d = test_dir!(
      "dir/image.jpg": {},
    );

    let metadata = read_metadata(d.get_path("dir/image.jpg")).unwrap();

    assert!(d.root().join(metadata.source_file).exists());
  }
}

#[cfg(test)]
mod test_read_metadata_recursive {
  use std::collections::HashSet;

  use super::*;
  use crate::testing::*;

  #[test]
  fn errors_if_directory_does_not_exist() {
    let d = test_dir!();
    assert_err!(
      read_metadata_recursive(d.root().join("dir"), None::<&Path>),
      "Path does not exist."
    );
  }

  #[test]
  fn reads_all_files() {
    let d = test_dir!(
      "image1.jpg": {},
      "image2.jpg": {},
      "dir/image3.jpg": {},
    );

    let metadata = read_metadata_recursive(d.root(), None::<&Path>).unwrap();

    assert_eq!(
      metadata
        .into_iter()
        .map(|m| d.get_path(m.source_file))
        .collect::<HashSet<_>>(),
      HashSet::from(["image1.jpg", "image2.jpg", "dir/image3.jpg"].map(|p| d.get_path(p)))
    );
  }

  #[test]
  fn returns_empty_vec_if_directory_empty() {
    let d = test_dir!();

    let metadata = read_metadata_recursive(d.root(), None::<&Path>).unwrap();

    assert!(metadata.is_empty());
  }

  #[test]
  fn skips_excluded_subdirectory() {
    let d = test_dir!(
      "image1.jpg": {},
      "image2.jpg": {},
    );
    fs::copy(d.get_path("image1.jpg"), d.trash().join("image3.jpg")).unwrap();

    let metadata = read_metadata_recursive(d.root(), d.some_trash()).unwrap();

    assert_eq!(
      metadata
        .into_iter()
        .map(|m| d.get_path(m.source_file))
        .collect::<HashSet<_>>(),
      HashSet::from(["image1.jpg", "image2.jpg"].map(|p| d.get_path(p)))
    );
  }
}

#[cfg(test)]
mod test_remove_file {
  use super::*;
  use crate::testing::*;

  #[test]
  fn errors_if_file_already_in_trash() {
    let d = test_dir!(
      "image.jpg": {},
    );
    fs::rename(d.get_path("image.jpg"), d.trash().join("image.jpg")).unwrap();

    assert_err!(
      remove_file(d.root(), d.trash(), d.trash().join("image.jpg")),
      "Cannot remove file already in trash"
    );
  }

  #[test]
  fn errors_if_name_collision_in_trash() {
    let d = test_dir!(
      "image.jpg": {},
    );
    fs::copy(d.get_path("image.jpg"), d.trash().join("image.jpg")).unwrap();

    assert_err!(
      remove_file(d.root(), d.trash(), d.get_path("image.jpg")),
      "Cannot remove file due to name collision in trash"
    );
  }

  #[test]
  fn errors_if_not_under_root() {
    let d = test_dir!(
      "image1.jpg": {},
      "dir/image2.jpg": {}
    );

    assert_err!(
      remove_file(d.root().join("dir"), d.trash(), d.root().join("image1.jpg")),
      "Cannot remove file outside root directory"
    );
  }

  #[test]
  fn moves_file_to_trash() {
    let d = test_dir!(
      "image.jpg": {},
    );

    remove_file(d.root(), d.trash(), d.get_path("image.jpg")).unwrap();

    assert_dir!(d, []);
    assert_trash!(d, ["image.jpg"]);
  }

  #[test]
  fn preserves_subdirectory_structure() {
    let d = test_dir!(
      "dir/image.jpg": {},
    );

    remove_file(d.root(), d.trash(), d.get_path("dir/image.jpg")).unwrap();

    assert_dir!(d, []);
    assert_trash!(d, ["dir/image.jpg"]);
  }
}

#[cfg(test)]
mod test_version_check {
  use super::*;

  #[test]
  fn does_not_treat_minor_as_fraction() {
    let version = "13.3".as_bytes().to_vec();

    assert!(version_check(version, (13, 29)).is_err());
  }

  #[test]
  fn fails_older_major() {
    let version = "12.29".as_bytes().to_vec();

    assert!(version_check(version, (13, 29)).is_err());
  }

  #[test]
  fn fails_older_minor() {
    let version = "13.28".as_bytes().to_vec();

    assert!(version_check(version, (13, 29)).is_err());
  }

  #[test]
  fn passes_equal() {
    let version = "13.29".as_bytes().to_vec();

    assert!(version_check(version, (13, 29)).is_ok());
  }

  #[test]
  fn passes_newer_major() {
    let version = "14.0".as_bytes().to_vec();

    assert!(version_check(version, (13, 29)).is_ok());
  }

  #[test]
  fn passes_newer_minor() {
    let version = "13.30".as_bytes().to_vec();

    assert!(version_check(version, (13, 29)).is_ok());
  }
}

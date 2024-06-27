//! Module-private `exiftool` wrapper functions.
//!
//! Copyright 2023-4 Seth Pendergrass. See LICENSE.

use super::super::gbl;
use std::path::PathBuf;
use std::{ffi::OsStr, path::Path, process::Command};

use regex::Regex;

//
// Public.
//

/// Copies metadata from src to dst. Returns the new metadata for dst.
pub fn copy_metadata(src: &Path, dst: &Path) {
  let mut args = Vec::new();
  args.extend(["-tagsFromFile", src.to_str().unwrap()]);
  args.extend(gbl::ARGS_SYNC);
  // exiftool prefers JSON or XML over CSV.
  args.extend([dst.to_str().unwrap()]);
  run_exiftool(args);
}

/// Creates an XMP file for path, with all tags duplicated. Returns metadata for the XMP file.
pub fn create_xmp(path: &Path) -> PathBuf {
  assert!(
    path.extension().unwrap() == "xmp",
    "{} is not an XMP file. Cannot create XMP.",
    path.display()
  );
  // -v needed to report renaming.
  extract_destination(run_exiftool([
    "-v",
    "-o",
    path.to_str().unwrap(),
    path.with_extension("").to_str().unwrap(),
  ]))
}

/// Renames path according to fmt, optionally copying tags from `tag_src`.
pub fn move_file(
  src: &Path,
  dir: &Path,
  datetime_tag: &str,
  ext: &str,
  tag_src: Option<&Path>,
) -> PathBuf {
  assert!(src.exists(), "{} does not exist.", src.display());
  assert!(
    dir.is_dir(),
    "{} is not an existing directory.",
    dir.display()
  );
  assert!(!ext.starts_with('.'), "ext should not start with a period.");

  let fmt = format!(
    "-FileName<{}/${{{}}}.{}",
    dir.to_str().unwrap(),
    datetime_tag,
    ext
  );
  // -v needed to report renaming.
  let mut args = vec![
    "-v",
    "-d",
    "%Y/%m/%y%m%d_%H%M%S%%+c",
    fmt.as_str(),
    src.to_str().unwrap(),
  ];

  if let Some(tag_src) = tag_src {
    let mut args2 = vec!["-tagsFromFile", tag_src.to_str().unwrap()];
    args2.append(&mut args);
    args = args2;
  }

  let stdout = run_exiftool(args);

  extract_destination(stdout)
}

/// Gets metadata for path.
pub fn read_metadata(path: &Path) -> Vec<u8> {
  let mut args = Vec::new();
  args.extend(gbl::ARGS_SYS);
  args.extend(gbl::ARGS_SYNC);
  // exiftool prefers JSON or XML over CSV.
  args.extend(["-json", path.to_str().unwrap()]);

  run_exiftool(args)
}

/// Recursively gathers all metadata within path, optionally excluding a subdirectory (e.g. trash).
pub fn read_metadata_recursive(path: &Path, exclude: Option<&Path>) -> Vec<u8> {
  let mut args = Vec::new();
  args.extend(gbl::ARGS_SYS);
  args.extend(gbl::ARGS_SYNC);
  // exiftool prefers JSON or XML over CSV.
  args.extend(["-json", "-r", path.to_str().unwrap()]);

  if let Some(exclude) = exclude {
    args.extend(["-i", exclude.to_str().unwrap()]);
  };

  run_exiftool(args)
}

//
// Private.
//

/// Given a byte stream from exiftool's stdout, extracts the destination of a rename / move.
/// Expects the format: 'OLDNAME.jpg' --> 'NEWNAME.jpg'.
fn extract_destination(stdout: Vec<u8>) -> PathBuf {
  let stdout_string = String::from_utf8(stdout).unwrap();

  let re = Regex::new(r"'.+' --> '(.+)'").unwrap();
  let caps = re.captures(&stdout_string).unwrap();

  return PathBuf::from(caps.get(1).unwrap().as_str());
}

/// Run exiftool with args, returning stdout.
/// Panics if exiftool fails.
fn run_exiftool<I, S>(args: I) -> Vec<u8>
where
  I: IntoIterator<Item = S>,
  S: AsRef<OsStr>,
{
  let mut cmd = Command::new("exiftool");
  cmd.args(args);
  let output = cmd.output().unwrap();
  log::trace!(
    "exiftool output:\n{}",
    String::from_utf8_lossy(&output.stdout)
  );
  assert!(
    output.status.success(),
    "exiftool failed with args: {:#?}. stderr: {}",
    cmd.get_args().collect::<Vec<&OsStr>>(),
    String::from_utf8_lossy(&output.stderr)
  );

  output.stdout
}

#[cfg(test)]
mod test {
  use super::*;

  /// Tests that we correctly parse file renaming format from `exiftool`.
  #[test]
  fn test_extract_destination() {
    let stdout = b"'old/path/name.jpg' --> 'new/path/name.jpg'";
    assert_eq!(
      extract_destination(stdout.to_vec()),
      PathBuf::from("new/path/name.jpg")
    );
  }
}

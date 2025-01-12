//! Test-only utilities.
//!
//! Copyright 2023-4 Seth Pendergrass. See LICENSE.

use std::path::Path;
use std::{fs, path::PathBuf, process::Command};

use crate::org::gbl;

lazy_static! {
  pub static ref ASSET_ROOT: PathBuf = PathBuf::from("assets");
  pub static ref TEST_ROOT: PathBuf = PathBuf::from("tmp");
}

/// Helper for creating directories for tests needing actual files.
pub struct TestDir {
  pub root: PathBuf,
  pub trash: PathBuf,
}

impl TestDir {
  //
  // Constructor.
  //

  /// Creates a new directory under `tmp/` for tests involving file operations.
  /// Note: Prefer using `test_dir!()` macro to not have to fill in the name.
  pub fn new(name: &str) -> Self {
    let root = TEST_ROOT.join(name);
    if root.exists() {
      fs::remove_dir_all(&root).unwrap();
    }
    fs::create_dir_all(&root).unwrap();

    let trash = root.join("trash");
    fs::create_dir(&trash).unwrap();

    Self { root, trash }
  }

  /// Create a `jpg`.
  pub fn add_jpg(&self, name: &str, exiftool_args: &[&str]) -> PathBuf {
    self.add_from("img.jpg", name, exiftool_args)
  }

  /// Create a `heic`.
  pub fn add_heic(&self, name: &str, exiftool_args: &[&str]) -> PathBuf {
    self.add_from("img.heic", name, exiftool_args)
  }

  /// Create an AVC `mov`.
  pub fn add_avc(&self, name: &str, exiftool_args: &[&str]) -> PathBuf {
    self.add_from("avc.mov", name, exiftool_args)
  }

  /// Create an HEVC `mov`.
  pub fn add_hevc(&self, name: &str, exiftool_args: &[&str]) -> PathBuf {
    self.add_from("hevc.mov", name, exiftool_args)
  }

  /// Create an XMP file.
  pub fn add_xmp(&self, name: &str, exiftool_args: &[&str]) -> PathBuf {
    self.add_from("img.xmp", name, exiftool_args)
  }

  //
  // Private.
  //

  /// Helper to copy a file from `assets/` to the test directory.
  fn add_from(&self, src: &str, name: &str, exiftool_args: &[&str]) -> PathBuf {
    let path = self.root.join(name);
    fs::copy(ASSET_ROOT.join(src), &path).unwrap();

    if !exiftool_args.is_empty() {
      Command::new("exiftool")
        .args(exiftool_args)
        .args(["-q", "-overwrite_original", path.to_str().unwrap()])
        .status()
        .unwrap();
    }

    path
  }
}

impl Drop for TestDir {
  /// Removes the test directory once finished.
  fn drop(&mut self) {
    fs::remove_dir_all(&self.root).unwrap();
  }
}

/// Helper macro to create a `TestDir` with directory name matching that of the test function.
#[macro_export]
macro_rules! test_dir {
  () => {{
    // HACK: Figuring out `__FUNC__`.
    fn type_of<T>(_: T) -> &'static str {
      std::any::type_name::<T>()
    }
    let path = type_of(|| ());
    let parent = path.strip_suffix("::{{closure}}").unwrap();
    let name = parent.split("::").last().unwrap();

    $crate::org::testing::TestDir::new(name)
  }};
}

pub(super) use test_dir;

/// Gets tag value for path via `exiftool`.
pub fn read_tag(path: &Path, tag: &str) -> String {
  let output = Command::new("exiftool")
    .args([
      "-s3",
      "-d",
      gbl::DATETIME_READ_FORMAT,
      tag,
      path.to_str().unwrap(),
    ])
    .output()
    .unwrap();

  assert!(
    output.status.success(),
    "exiftool failed: {:?}",
    String::from_utf8_lossy(&output.stderr)
  );

  String::from_utf8(output.stdout).unwrap().trim().to_string()
}

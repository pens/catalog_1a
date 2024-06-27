//! Sidecar file handling.
//!
//! Copyright 2023-4 Seth Pendergrass. See LICENSE.

use super::super::gbl::FileHandle;
use super::Metadata;
use std::path::PathBuf;

pub struct Sidecar {
  pub metadata: Metadata,
  pub media: Option<FileHandle>,
}

impl Sidecar {
  //
  // Constructor.
  //

  /// Creates a new sidecar object with metadata but no linked media file.
  pub fn new(metadata: Metadata) -> Self {
    Self::validate(&metadata);

    Self {
      metadata,
      media: None,
    }
  }

  //
  // Public.
  //

  /// Gets the path to the source file for this sidecar.
  /// This does *not* guarantee the file exists.
  pub fn get_source_file(&self) -> PathBuf {
    // Find _nn.ext.xmp.
    let re = regex::Regex::new(r"^(.+)_\d{2}\.(\S+)\.(?:xmp|XMP)$").unwrap();

    if let Some(caps) = re.captures(self.metadata.source_file.to_str().unwrap()) {
      let base = caps.get(1).unwrap().as_str();
      let ext = caps.get(2).unwrap().as_str();

      PathBuf::from(base).with_extension(ext)
    } else {
      self.metadata.source_file.with_extension("")
    }
  }

  //
  // Private.
  //

  /// Checks that the file is of the format basename[_nn].ext.xmp.
  fn validate(metadata: &Metadata) {
    assert!(
      metadata
        .source_file
        .extension()
        .unwrap()
        .eq_ignore_ascii_case("xmp"),
      "{}: XMP file without `.xmp` extension.",
      metadata.source_file.display()
    );

    assert!(
      metadata
        .source_file
        .with_extension("")
        .extension()
        .is_some(),
      "{}: XMP file without `.ext.xmp` extension.",
      metadata.source_file.display()
    );
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  fn new_sidecar(path: &str) -> Sidecar {
    let metadata = Metadata {
      source_file: PathBuf::from(path),
      ..Default::default()
    };

    Sidecar::new(metadata)
  }

  /// Should panic if Adobe format.
  #[test]
  #[should_panic(expected = "test.xmp: XMP file without `.ext.xmp` extension.")]
  fn test_bad_extension_format_panics() {
    new_sidecar("test.xmp");
  }

  /// Check that basename.ext.xmp -> basename.ext.
  #[test]
  fn test_get_source_file_initial_edit() {
    let s1 = new_sidecar("test.jpg.xmp");
    assert_eq!(s1.get_source_file(), PathBuf::from("test.jpg"));

    let s2 = new_sidecar("test.jpg.XMP");
    assert_eq!(s2.get_source_file(), PathBuf::from("test.jpg"));

    let s3 = new_sidecar("test.JPG.xmp");
    assert_eq!(s3.get_source_file(), PathBuf::from("test.JPG"));

    let s4 = new_sidecar("/path/to/test.jpg.xmp");
    assert_eq!(s4.get_source_file(), PathBuf::from("/path/to/test.jpg"));
  }

  /// Check that `basename_nn.ext.xmp` -> `basename.ext`.
  #[test]
  fn test_get_source_file_versioned() {
    let s1 = new_sidecar("test_01.jpg.xmp");
    assert_eq!(s1.get_source_file(), PathBuf::from("test.jpg"));

    let s2 = new_sidecar("test_01.jpg.XMP");
    assert_eq!(s2.get_source_file(), PathBuf::from("test.jpg"));

    let s3 = new_sidecar("test_01.JPG.xmp");
    assert_eq!(s3.get_source_file(), PathBuf::from("test.JPG"));

    let s4 = new_sidecar("/path/to/test_01.jpg.xmp");
    assert_eq!(s4.get_source_file(), PathBuf::from("/path/to/test.jpg"));
  }

  /// Should panic if not .xmp.
  #[test]
  #[should_panic(expected = "test.jpg: XMP file without `.xmp` extension.")]
  fn test_missing_extension_panics() {
    new_sidecar("test.jpg");
  }
}

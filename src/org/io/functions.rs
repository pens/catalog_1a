//! Copyright 2023-4 Seth Pendergrass. See LICENSE.

use super::super::prim::Metadata;
use super::exiftool;
use std::fs;
use std::path::{Path, PathBuf};

//
// Public.
//

pub fn copy_metadata(from: &Path, to: &Path) -> Metadata {
  exiftool::copy_metadata(from, to);
  parse(exiftool::read_metadata(to).as_slice())
}

pub fn create_xmp(path: &Path) -> Metadata {
  let xmp_path = exiftool::create_xmp(path);
  parse(exiftool::read_metadata(&xmp_path).as_slice())
}

pub fn move_file(src: &Path, dir: &Path, datetime_tag: &str, ext: &str, tag_src: Option<&Path>) -> PathBuf {
  exiftool::move_file(src, dir, datetime_tag, ext, tag_src)
}

pub fn read_metadata(path: &Path) -> Metadata {
  parse(exiftool::read_metadata(path).as_slice())
}

pub fn read_metadata_recursive(path: &Path, exclude: Option<&Path>) -> Vec<Metadata> {
  parse_vec(exiftool::read_metadata_recursive(path, exclude).as_slice())
}

pub fn remove_file(path: &Path, trash: &Path) {
  // Canonicalize in case of symlink.
  assert!(
    !path
      .canonicalize()
      .unwrap()
      .starts_with(trash.canonicalize().unwrap()),
    "{} is already in {}.",
    path.display(),
    trash.display()
  );
  let path_trash = trash.join(path);
  assert!(
    !path_trash.exists(),
    "Cannot safely delete {} due to name collision in {}.",
    path.display(),
    trash.display()
  );
  fs::create_dir_all(path_trash.parent().unwrap()).unwrap();
  fs::rename(path, path_trash).unwrap();
}

//
// Private.
//

fn parse(metadata: &[u8]) -> Metadata {
  parse_vec(metadata).remove(0)
}

fn parse_vec(metadata: &[u8]) -> Vec<Metadata> {
  serde_json::from_slice::<Vec<Metadata>>(metadata).unwrap()
}

#[cfg(test)]
mod test {
  use super::*;
  use crate::org::testing;
  use std::process::Command;

  /// Write exiftool tag (as '-TAG=VALUE') to path.
  fn write_metadata(arg: &str, path: &Path) {
    Command::new("exiftool")
      .args([arg, path.to_str().unwrap()])
      .status()
      .unwrap();
  }

  /// Check that metadata copies over.
  #[test]
  fn test_copy_metadata() {
    let d = testing::test_dir!();
    let i1 = d.add_jpg("img1.jpg", &["-Artist=TEST"]);
    let i2 = d.add_jpg("img2.jpg", &[]);

    let m = copy_metadata(&i1, &i2);

    assert_eq!(m.artist, Some("TEST".to_string()));
  }

  /// Should create an xmp of the format basename.ext.xmp.
  #[test]
  fn test_create_xmp() {
    let d = testing::test_dir!();
    let i = d.add_jpg("img.jpg", &["-Artist=TEST"]);

    let m = create_xmp(&i.with_extension("jpg.xmp"));

    assert_eq!(m.source_file, d.root.join("img.jpg.xmp"));
    assert_eq!(m.artist, Some("TEST".to_string()));
  }

  /// Should panic if requested file isn't actually an xmp.
  #[test]
  #[should_panic(
    expected = "tmp/test_create_xmp_wrong_extension_panics/img.jpg is not an XMP file. Cannot create XMP."
  )]
  fn test_create_xmp_wrong_extension_panics() {
    let d = testing::test_dir!();
    let i = d.add_jpg("img.jpg", &["-Artist=TEST"]);

    create_xmp(&i);
  }

  /// Move file to `YYYY/MM/YYYYMM_DDHHMM.ext` format based on provided datetime tag, in this case
  /// `DateTimeOriginal`.
  #[test]
  fn test_move_file() {
    let d = testing::test_dir!();
    let i = d.add_jpg("img.jpg", &["-DateTimeOriginal=2024:06:20 22:09:00"]);

    let p = move_file(
      &i,
      &d.root,
      "DateTimeOriginal",
      "jpg",
      None
    );

    assert!(!i.exists());
    assert_eq!(p, d.root.join("2024/06/240620_220900.jpg"));
  }

  /// Move file to `YYYY/MM/YYYYMM_DDHHMM[_c].ext` format, where `_c` is a counter for duplicates.
  #[test]
  fn test_move_file_duplicates() {
    let d = testing::test_dir!();
    let i1 = d.add_jpg("img1.jpg", &["-DateTimeOriginal=2024:06:20 22:09:00"]);
    let i2 = d.add_jpg("img2.jpg", &["-DateTimeOriginal=2024:06:20 22:09:00"]);

    let p1 = move_file(
      &i1,
      &d.root,
      "DateTimeOriginal",
      "jpg",
      None
    );
    let p2 = move_file(
      &i2,
      &d.root,
      "DateTimeOriginal",
      "jpg",
      None
    );

    assert!(!i1.exists());
    assert!(!i2.exists());
    assert_eq!(p1, d.root.join("2024/06/240620_220900.jpg"));
    assert_eq!(p2, d.root.join("2024/06/240620_220900_1.jpg"));
  }

  /// Move file to `YYYY/MM/YYYYMM_DDHHMM.ext` format based on the provided datetime tag of a
  /// different file.
  #[test]
  fn test_move_file_with_separate_metadata_source() {
    let d = testing::test_dir!();
    let i1 = d.add_jpg("img1.jpg", &["-DateTimeOriginal=2024:06:20 22:09:00"]);
    let i2 = d.add_jpg("img2.jpg", &["-DateTimeOriginal=2024:06:20 22:09:00"]);

    let new_path = move_file(
      &i2,
      &d.root,
      "DateTimeOriginal",
      "jpg",
     Some(&i1)
    );

    assert!(i1.exists());
    assert!(!i2.exists());
    assert_eq!(new_path, d.root.join("2024/06/240620_220900.jpg"));
  }

  /// Does read, read?
  #[test]
  fn test_read_metadata() {
    let d = testing::test_dir!();
    let i = d.add_jpg("img.jpg", &["-Artist=TEST"]);
    write_metadata("-Artist=TEST", &i);

    let m = read_metadata(&i);

    assert_eq!(m.artist, Some("TEST".to_string()));
  }

  /// Should be recursive.
  #[test]
  fn test_read_metadata_recursive_finds_subdir() {
    let d = testing::test_dir!();
    let i1 = d.add_jpg("img1.jpg", &[]);
    let i2 = d.add_jpg("img2.jpg", &[]);

    let m = read_metadata_recursive(&d.root, None);

    assert!(m.len() == 2);
    assert!(m.iter().any(|m| m.source_file == i1));
    assert!(m.iter().any(|m| m.source_file == i2));
  }

  /// Should ignore trash if told to.
  #[test]
  fn test_read_metadata_recursive_ignores_trash() {
    let d = testing::test_dir!();
    let i1 = d.add_jpg("img1.jpg", &[]);
    let i2 = d.add_jpg("img2.jpg", &[]);
    fs::copy(&i1, d.trash.join("img1.jpg")).unwrap();

    let m = read_metadata_recursive(&d.root, Some(&d.trash));

    assert!(m.len() == 2);
    assert!(m.iter().any(|m| m.source_file == i1));
    assert!(m.iter().any(|m| m.source_file == i2));
  }

  /// Crash if we try to move a file from trash into trash.
  #[test]
  #[should_panic(
    expected = "tmp/test_remove_file_already_in_trash_panics/img.jpg is already in tmp/test_remove_file_already_in_trash_panics."
  )]
  fn test_remove_file_already_in_trash_panics() {
    let d = testing::test_dir!();
    let i = d.add_jpg("img.jpg", &[]);

    remove_file(&i, &d.root);
  }

  /// Move file to trash.
  #[test]
  fn test_remove_file_moves_to_trash() {
    let d = testing::test_dir!();
    let i = d.add_jpg("img.jpg", &[]);

    remove_file(&i, &d.trash);

    assert!(!i.exists());
  }

  /// Tests that we maintain the relative structure of files moved to trash, to ease reversion.
  #[test]
  fn test_remove_file_preserves_subdir() {
    let d = testing::test_dir!();
    let i = d.add_jpg("img.jpg", &[]);

    remove_file(&i, &d.trash);

    assert!(!i.exists());
    assert!(d.trash.join(&i).exists());
  }

  /// Crash if name collision in trash.
  #[test]
  #[should_panic(
    expected = "Cannot safely delete tmp/test_remove_file_name_collision_panics/img.jpg due to name collision in tmp/test_remove_file_name_collision_panics/trash."
  )]
  fn test_remove_file_name_collision_panics() {
    let d = testing::test_dir!();
    let i = d.add_jpg("img.jpg", &[]);
    let t = d.trash.join(&i);
    fs::create_dir_all(t.parent().unwrap()).unwrap();
    fs::copy(&i, t).unwrap();

    remove_file(&i, &d.trash);
  }
}

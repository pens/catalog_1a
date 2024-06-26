//! Copyright 2023-4 Seth Pendergrass. See LICENSE.

use super::super::primitives::Metadata;
use super::exiftool;
use std::fs;
use std::path::{Path, PathBuf};

//
// Public.
//

pub fn copy_metadata(from: &Path, to: &Path) -> Metadata {
    exiftool::copy_metadata(from, to);
    parse(exiftool::read_metadata(to))
}

pub fn create_xmp(path: &Path) -> Metadata {
    assert!(path.extension().unwrap() == "xmp", "{} is not an XMP file. Cannot create XMP.", path.display());
    let xmp_path = exiftool::create_xmp(path);
    parse(exiftool::read_metadata(&xmp_path))
}

pub fn move_file(fmt: &str, path: &Path, tag_src: &Path) -> PathBuf {
    exiftool::move_file(fmt, path, tag_src)
}

pub fn read_metadata(path: &Path) -> Metadata {
    parse(exiftool::read_metadata(path))
}

pub fn read_metadata_recursive(path: &Path, exclude: Option<&Path>) -> Vec<Metadata> {
    parse_vec(exiftool::read_metadata_recursive(path, exclude))
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

fn parse(metadata: Vec<u8>) -> Metadata {
    parse_vec(metadata).remove(0)
}

fn parse_vec(metadata: Vec<u8>) -> Vec<Metadata> {
    serde_json::from_slice::<Vec<Metadata>>(metadata.as_slice()).unwrap()
}

#[cfg(test)]
mod test {
    use super::*;
    use std::process::Command;
    use crate::organization::testing;

    lazy_static! {
        static ref TEST_IMG: PathBuf = testing::ASSET_ROOT.join("img.jpg");
    }

    struct Directory {
        root: PathBuf,
        img1: PathBuf,
        img2: PathBuf,
        trash: PathBuf,
    }

    // Clean up test directories on exit.
    impl Drop for Directory {
        fn drop(&mut self) {
            fs::remove_dir_all(&self.root).unwrap();
        }
    }

    /// Build test directory for all of the below tests, and return the paths.
    ///
    /// name/
    /// ├── image1.jpg
    /// ├── subdir/
    /// |   └── image2.jpg
    /// └── trash/
    ///
    fn make_dir(name: &str) -> Directory {
        // Create name/.
        let root = testing::TEST_ROOT.join(name);
        if root.exists() {
            fs::remove_dir_all(&root).unwrap();
        }
        fs::create_dir(&root).unwrap();

        let img1 = root.join("image1.jpg");
        fs::copy(TEST_IMG.as_path(), &img1).unwrap();

        let img2 = root.join("subdir/image2.jpg");
        fs::create_dir(root.join("subdir")).unwrap();
        fs::copy(TEST_IMG.as_path(), &img2).unwrap();

        let trash = root.join("trash");
        fs::create_dir(&trash).unwrap();

        Directory {
            root,
            img1,
            img2,
            trash,
        }
    }

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
        let dir = make_dir("test_copy_metadata");
        write_metadata("-Artist=TEST", &dir.img1);

        let m = copy_metadata(&dir.img1, &dir.img2);

        assert_eq!(m.artist, Some("TEST".to_string()));
    }

    /// Should create an xmp of the format basename.ext.xmp.
    #[test]
    fn test_create_xmp() {
        let dir = make_dir("test_create_xmp");
        write_metadata("-Artist=TEST", &dir.img1);

        let m = create_xmp(&dir.img1.with_extension("jpg.xmp"));

        assert_eq!(m.source_file, dir.root.join("image1.jpg.xmp"));
        assert_eq!(m.artist, Some("TEST".to_string()));
    }

    /// Should panic if requested file isn't actually an xmp.
    #[test]
    #[should_panic(expected = "tmp/test_create_xmp_wrong_extension_panics/image1.jpg is not an XMP file. Cannot create XMP.")]
    fn test_create_xmp_wrong_extension_panics() {
        let dir = make_dir("test_create_xmp_wrong_extension_panics");
        write_metadata("-Artist=TEST", &dir.img1);

        create_xmp(&dir.img1);
    }

    /// Move file to YYYY/MM/YYYYMM_DDHHMM.ext format based on provided datetime tag, in this case
    /// DateTimeOriginal.
    #[test]
    fn test_move_file() {
        let dir = make_dir("test_move_file");
        write_metadata("-DateTimeOriginal=2024:06:20 22:09:00", &dir.img1);

        let p = move_file(
            &format!(
                "-FileName<{}/${{DateTimeOriginal}}.jpg",
                dir.root.to_str().unwrap()
            ),
            &dir.img1,
            &dir.img1,
        );

        assert!(!dir.img1.exists());
        assert_eq!(p, dir.root.join("2024/06/240620_220900.jpg"));
    }

    /// Move file to YYYY/MM/YYYYMM_DDHHMM[_c].ext format, where _c is a counter for duplicates.
    #[test]
    fn test_move_file_duplicates() {
        let dir = make_dir("test_move_file_duplicates");
        write_metadata("-DateTimeOriginal=2024:06:20 22:09:00", &dir.img1);
        write_metadata("-DateTimeOriginal=2024:06:20 22:09:00", &dir.img2);

        let p1 = move_file(
            &format!(
                "-FileName<{}/${{DateTimeOriginal}}.jpg",
                dir.root.to_str().unwrap()
            ),
            &dir.img1,
            &dir.img1,
        );
        let p2 = move_file(
            &format!(
                "-FileName<{}/${{DateTimeOriginal}}.jpg",
                dir.root.to_str().unwrap()
            ),
            &dir.img2,
            &dir.img2,
        );

        assert!(!dir.img1.exists());
        assert!(!dir.img2.exists());
        assert_eq!(p1, dir.root.join("2024/06/240620_220900.jpg"));
        assert_eq!(p2, dir.root.join("2024/06/240620_220900_1.jpg"));
    }

    /// Move file to YYYY/MM/YYYYMM_DDHHMM.ext format based on the provided datetime tag of a
    /// different file.
    #[test]
    fn test_move_file_with_separate_metadata_source() {
        let dir = make_dir("test_move_file_with_separate_metadata_source");
        write_metadata("-DateTimeOriginal=2024:06:20 22:09:00", &dir.img1);

        let new_path = move_file(
            &format!(
                "-FileName<{}/${{DateTimeOriginal}}.jpg",
                dir.root.to_str().unwrap()
            ),
            &dir.img2,
            &dir.img1,
        );

        assert!(dir.img1.exists());
        assert!(!dir.img2.exists());
        assert_eq!(new_path, dir.root.join("2024/06/240620_220900.jpg"));
    }

    /// Does read, read?
    #[test]
    fn test_read_metadata() {
        let dir = make_dir("test_read_metadata");
        write_metadata("-Artist=TEST", &dir.img1);

        let m = read_metadata(&dir.img1);

        assert_eq!(m.artist, Some("TEST".to_string()));
    }

    /// Should be recursive.
    #[test]
    fn test_read_metadata_recursive_finds_subdir() {
        let dir = make_dir("test_read_metadata_recursive_finds_subdir");

        let m = read_metadata_recursive(&dir.root, None);

        assert!(m.len() == 2);
        assert!(m.iter().any(|m| m.source_file == dir.img1));
        assert!(m.iter().any(|m| m.source_file == dir.img2));
    }

    /// Should ignore trash if told to.
    #[test]
    fn test_read_metadata_recursive_ignores_trash() {
        let dir = make_dir("test_read_metadata_recursive_ignores_trash");
        fs::copy(&dir.img1, dir.trash.join("image1.jpg")).unwrap();

        let m = read_metadata_recursive(&dir.root, Some(Path::new("trash")));

        assert!(m.len() == 2);
        assert!(m.iter().any(|m| m.source_file == dir.img1));
        assert!(m.iter().any(|m| m.source_file == dir.img2));
    }

    /// Crash if we try to move a file from trash into trash.
    #[test]
    #[should_panic(expected = "tmp/test_remove_file_already_in_trash_panics/image1.jpg is already in tmp/test_remove_file_already_in_trash_panics.")]
    fn test_remove_file_already_in_trash_panics() {
        let dir = make_dir("test_remove_file_already_in_trash_panics");

        remove_file(&dir.img1, &dir.root);
    }

    /// Move file to trash.
    #[test]
    fn test_remove_file_moves_to_trash() {
        let dir = make_dir("test_remove_file_moves_to_trash");

        remove_file(&dir.img1, &dir.trash);

        assert!(!dir.img1.exists());
    }

    /// Tests that we maintain the relative structure of files moved to trash, to ease reversion.
    #[test]
    fn test_remove_file_preserves_subdir() {
        let dir = make_dir("test_remove_file_preserves_subdir");

        remove_file(&dir.img2, &dir.trash);

        assert!(!dir.img2.exists());
        assert!(dir.trash.join(&dir.img2).exists());
    }

    /// Crash if name collision in trash.
    #[test]
    #[should_panic(expected = "Cannot safely delete tmp/test_remove_file_name_collision_panics/image1.jpg due to name collision in tmp/test_remove_file_name_collision_panics/trash.")]
    fn test_remove_file_name_collision_panics() {
        let dir = make_dir("test_remove_file_name_collision_panics");
        let trash_path = dir.trash.join(&dir.img1);
        fs::create_dir_all(trash_path.parent().unwrap()).unwrap();
        fs::copy(&dir.img1, trash_path).unwrap();

        remove_file(&dir.img1, &dir.trash);
    }
}

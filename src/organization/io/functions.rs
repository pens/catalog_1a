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

// Doesn't check that new path is as expected.
pub fn create_xmp(path: &Path) -> Metadata {
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
    assert!(
        !trash.canonicalize().unwrap().starts_with(path),
        "{} is already in {}.",
        path.display(),
        trash.display()
    );
    let path_trash = trash.join(path.file_name().unwrap());
    assert!(
        !path_trash.exists(),
        "Cannot safely delete {} due to name collision in {}.",
        path.display(),
        trash.display()
    );
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
    use std::process::Command;
    use super::*;

    lazy_static! {
        static ref TEST_ROOT: PathBuf = PathBuf::from("test_data");
        static ref TEST_IMG: PathBuf = TEST_ROOT.join("base.jpg");
    }

    fn make_dir(name: &str) -> PathBuf {
        // Create test directory.
        let dir_path = TEST_ROOT.join(name);
        if dir_path.exists() {
            fs::remove_dir_all(&dir_path).unwrap();
        }
        fs::create_dir(&dir_path).unwrap();

        // Create test image.
        let img_path = dir_path.join("image.jpg");
        fs::copy(TEST_IMG.as_path(), img_path).unwrap();

        dir_path
    }

    // TODO cleanup!!!!

    #[test]
    fn test_copy_metadata() {
        let dir = make_dir("test_copy_metadata");
        fs::copy(dir.join("image.jpg"), dir.join("image1.jpg")).unwrap();
        let image_path = dir.join("image.jpg");
        let image2_path = dir.join("image1.jpg");
        Command::new("exiftool")
            .args([
                "-Artist=TEST",
                image_path.to_str().unwrap(),
            ])
            .status()
            .unwrap();

        let metadata2 = copy_metadata(&image_path, &image2_path);

        assert_eq!(metadata2.artist, Some("TEST".to_string()));
    }

    #[test]
    fn test_create_xmp() {
        let dir = make_dir("test_create_xmp");
        let image_path = dir.join("image.jpg");
        Command::new("exiftool")
            .args([
                "-Artist=TEST",
                image_path.to_str().unwrap(),
            ])
            .status()
            .unwrap();

        let metadata = create_xmp(&image_path);

        assert_eq!(metadata.source_file, dir.join("image.jpg.xmp"));
        assert_eq!(metadata.artist, Some("TEST".to_string()));
    }

    #[test]
    fn test_move_file() {
        let dir = make_dir("test_move_file");
        let image_path = dir.join("image.jpg");
        Command::new("exiftool")
            .args([
                "-DateTimeOriginal=2024:06:20 22:09:00",
                image_path.to_str().unwrap(),
            ])
            .status()
            .unwrap();

        let new_path = move_file(&format!("-FileName<{}/${{DateTimeOriginal}}.jpg", dir.to_str().unwrap()), &image_path, &image_path);

        assert_eq!(new_path, dir.join("2024/06/240620_220900.jpg"));
    }

    #[test]
    fn test_read_metadata() {
        let dir = make_dir("test_read_metadata");
        let image_path = dir.join("image.jpg");
        Command::new("exiftool")
            .args([
                "-Artist=TEST",
                image_path.to_str().unwrap(),
            ])
            .status()
            .unwrap();

        let metadata = read_metadata(&image_path);

        assert_eq!(metadata.artist, Some("TEST".to_string()));
    }

    #[test]
    fn test_read_metadata_recursive_finds_subdir() {
        let dir = make_dir("test_read_metadata_recursive_finds_subdir");
        let image_path = dir.join("image.jpg");
        fs::create_dir_all(dir.join("subdir")).unwrap();
        fs::copy(image_path.as_path(), dir.join("subdir").join("image2.jpg")).unwrap();

        let metadata = read_metadata_recursive(&dir, None);

        assert!(metadata.len() == 2);
        assert!(metadata.iter().any(|m| m.source_file == image_path));
        assert!(metadata.iter().any(|m| m.source_file == dir.join("subdir").join("image2.jpg")));
    }

    #[test]
    fn test_read_metadata_recursive_ignores_trash() {
        let dir = make_dir("test_read_metadata_recursive_ignores_trash");
        let image_path = dir.join("image.jpg");
        fs::create_dir_all(dir.join("trash")).unwrap();
        fs::copy(image_path.as_path(), dir.join("trash").join("image2.jpg")).unwrap();

        let metadata = read_metadata_recursive(&dir, Some(Path::new("trash")));

        assert!(metadata.len() == 1);
        assert!(metadata[0].source_file == image_path);
    }

    #[test]
    #[should_panic]
    fn test_remove_file_already_in_trash_panics() {
        let dir = make_dir("test_remove_file_already_in_trash_panics");
        let image_path = dir.join("image.jpg");

        remove_file(&image_path, &dir);
    }

    #[test]
    fn test_remove_file_moves_to_trash() {
        let dir = make_dir("test_remove_file_moves_to_trash");
        let image_path = dir.join("image.jpg");
        let trash_path = dir.join("trash");
        if trash_path.exists() {
            fs::remove_dir_all(&trash_path).unwrap();
        }
        fs::create_dir_all(&trash_path).unwrap();

        remove_file(&image_path, &trash_path);

        assert!(!image_path.exists());
        assert!(trash_path.join("image.jpg").exists());
    }

    #[test]
    #[should_panic]
    fn test_remove_file_name_collision_panics() {
        let dir = make_dir("test_remove_file_name_collision_panics");
        let image_path = dir.join("image.jpg");
        let trash_path = dir.join("trash");
        if trash_path.exists() {
            fs::remove_dir_all(&trash_path).unwrap();
        }
        fs::create_dir_all(&trash_path).unwrap();
        fs::copy(image_path.as_path(), trash_path.join("image.jpg")).unwrap();

        remove_file(&image_path, &trash_path);

        assert!(!image_path.exists());
        assert!(trash_path.join("image.jpg").exists());
    }
}
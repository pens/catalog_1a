use std::path::Path;
use std::{fs, path::PathBuf, process::Command};

use crate::org::gbl;

lazy_static! {
    pub static ref ASSET_ROOT: PathBuf = PathBuf::from("assets");
    pub static ref TEST_ROOT: PathBuf = PathBuf::from("tmp");
}

pub struct TestDir {
    pub root: PathBuf,
    pub trash: PathBuf,
}

impl TestDir {
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

    pub fn add(&self, name: &str, exiftool_args: &[&str]) -> PathBuf {
        let dst_rel = PathBuf::from(name);
        let ext = dst_rel.extension().unwrap().to_str().unwrap();
        let src = if ext == "mov" {
            dst_rel.clone()
        } else {
            PathBuf::from("img.".to_string() + ext)
        };
        let dst = self.root.join(dst_rel);

        fs::copy(ASSET_ROOT.join(src), &dst).unwrap();

        if !exiftool_args.is_empty() {
            write_metadata(exiftool_args, &dst);
        }

        dst
    }

    fn add_from(&self, src: &str, name: &str, exiftool_args: &[&str]) -> PathBuf {
        let path = self.root.join(name);
        fs::copy(ASSET_ROOT.join(src), &path).unwrap();

        if !exiftool_args.is_empty() {
            write_metadata(exiftool_args, &path);
        }

        path
    }

    pub fn add_jpg(&self, name: &str, exiftool_args: &[&str]) -> PathBuf {
        self.add_from("img.jpg", name, exiftool_args)
    }

    pub fn add_heic(&self, name: &str, exiftool_args: &[&str]) -> PathBuf {
        self.add_from("img.heic", name, exiftool_args)
    }

    pub fn add_avc(&self, name: &str, exiftool_args: &[&str]) -> PathBuf {
        self.add_from("avc.mov", name, exiftool_args)
    }

    pub fn add_hevc(&self, name: &str, exiftool_args: &[&str]) -> PathBuf {
        self.add_from("hevc.mov", name, exiftool_args)
    }

    pub fn add_xmp(&self, name: &str, exiftool_args: &[&str]) -> PathBuf {
        self.add_from("img.xmp", name, exiftool_args)
    }
}

impl Drop for TestDir {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.root).unwrap();
    }
}

pub fn read_tag(path: &Path, tag: &str) -> String {
    let output = Command::new("exiftool")
        .args(["-s3", "-d", gbl::DATETIME_FMT, tag, path.to_str().unwrap()])
        .output()
        .unwrap();

    assert!(
        output.status.success(),
        "exiftool failed: {:?}",
        String::from_utf8_lossy(&output.stderr)
    );

    String::from_utf8(output.stdout).unwrap().trim().to_string()
}

/// Write exiftool tag (as '-TAG=VALUE') to path.
fn write_metadata(args: &[&str], path: &Path) {
    Command::new("exiftool")
        .args(args)
        .args(["-q", "-overwrite_original", path.to_str().unwrap()])
        .status()
        .unwrap();
}

use std::{fs, path::PathBuf, process::Command};
use std::path::Path;

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
        fs::create_dir(&root).unwrap();

        let trash = root.join("trash");
        fs::create_dir(&trash).unwrap();

        Self { root, trash }
    }

    pub fn add(&self, name: &str, exiftool_args: &[&str]) {
        let dst = PathBuf::from(name);
        let ext = dst.extension().unwrap().to_str().unwrap();

        let src =
            if ext == "mov" {
                dst.clone()
            } else {
                PathBuf::from("img.".to_string() + ext)
            };

        fs::copy(ASSET_ROOT.join(src), self.root.join(&dst)).unwrap();

        write_metadata(exiftool_args, &self.root.join(dst));
    }
}

impl Drop for TestDir {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.root).unwrap();
    }
}

pub fn read_tag(path: &Path, tag: &str) -> String {
    let output = Command::new("exiftool")
        .args(["-s3", tag, path.to_str().unwrap()])
        .output()
        .unwrap();

    assert!(output.status.success(), "exiftool failed: {:?}", String::from_utf8_lossy(&output.stderr));

    String::from_utf8(output.stdout).unwrap().trim().to_string()
}

pub fn setup() {
    // TODO needs to log to file
    crate::setup::configure_logging(2);
}

/// Write exiftool tag (as '-TAG=VALUE') to path.
fn write_metadata(args: &[&str], path: &Path) {
    Command::new("exiftool")
        .args(args)
        .args(["-overwrite_original", path.to_str().unwrap()])
        .status()
        .unwrap();
}


// Copyright 2023-5 Seth Pendergrass. See LICENSE.

//! Helper for settings up test directories with multimedia files and sidecars.

use std::{
  collections::{HashMap, HashSet, VecDeque},
  env,
  ffi::OsString,
  fs,
  path::{Path, PathBuf},
  sync::LazyLock,
};

use crate::io;

static ASSET_ROOT: LazyLock<PathBuf> = LazyLock::new(|| PathBuf::from("assets"));
static TEST_ROOT: LazyLock<PathBuf> = LazyLock::new(|| env::temp_dir().join(format!("{}_tests", env!("CARGO_PKG_NAME"))));

/// Helper for creating directories for tests needing actual files.
pub struct TestDir {
  root:  PathBuf,
  trash: PathBuf,
}

impl TestDir {
  /// Creates a new directory under `TEST_ROOT` for tests involving file
  /// operations. Note: Prefer using `test_dir!()` macro.
  pub fn new(
    test_path: PathBuf,
    files: Vec<(&'static str, HashMap<&'static str, &'static str>)>,
  ) -> Self {
    let root_rel = TEST_ROOT.join(test_path);
    if root_rel.exists() {
      fs::remove_dir_all(&root_rel).unwrap();
    }
    fs::create_dir_all(&root_rel).unwrap();

    let trash_rel = root_rel.join("trash");
    fs::create_dir(&trash_rel).unwrap();

    let root = root_rel.canonicalize().unwrap();
    let trash = trash_rel.canonicalize().unwrap();

    for (file, tags) in files {
      create_file(&root, file, tags);
    }

    Self { root, trash }
  }

  pub fn files_good(&self) -> HashSet<PathBuf> {
    traverse_dir(&self.root, Some(&self.trash))
  }

  pub fn files_trash(&self) -> HashSet<PathBuf> {
    traverse_dir(&self.trash, None::<&Path>)
  }

  pub fn get_path(&self, file: impl AsRef<Path>) -> PathBuf {
    self.root.join(file)
  }

  pub fn get_trash(&self, file: impl AsRef<Path>) -> PathBuf {
    self.trash.join(file)
  }

  pub fn root(&self) -> &Path {
    &self.root
  }

  pub fn some_trash(&self) -> Option<&Path> {
    Some(&self.trash)
  }

  pub fn trash(&self) -> &Path {
    &self.trash
  }
}

fn create_file(working_dir: impl AsRef<Path>, path: impl AsRef<Path>, tags: HashMap<&str, &str>) {
  let full_path = working_dir.as_ref().join(path.as_ref());

  assert!(!full_path.exists(), "File already exists: {full_path:?}");
  fs::create_dir_all(full_path.parent().unwrap()).unwrap();

  // Copy test asset into full_path.
  let ext = full_path.extension().unwrap().to_ascii_lowercase();
  let mut test_asset = OsString::from("test");
  // Select `.mov` codec based on provided tag.
  if ext == "mov" {
    test_asset.push(
      tags
        .get("CompressorID")
        .map(|c| match *c {
          "avc1" => "_avc",
          "hev1" | "hvc1" => "_hevc",
          _ => panic!("Unsupported codec: {c}"),
        })
        .expect("Missing `CompressorID` tag."),
    );
  }
  test_asset.push(".");
  test_asset.push(ext);
  fs::copy(ASSET_ROOT.join(test_asset), &full_path).unwrap();

  // Create ExifTool commands to set tags.
  let mut args = tags
    .iter()
    .map(|(k, v)| OsString::from(format!("-{k}={v}")))
    .collect::<Vec<_>>();
  // If `ContentIdentifier` isn't manually set, strip out the tag from the test
  // file. Required because if the test file doesn't have the tag, ExifTool will
  // not allow it to be added.
  if !tags.contains_key("ContentIdentifier") {
    args.push(OsString::from("-MakerNotes="));
  }

  args.extend([
    OsString::from("-q"),
    OsString::from("-overwrite_original"),
    path.as_ref().as_os_str().to_os_string(),
  ]);

  io::run_exiftool(Some(working_dir), args).unwrap();
}

fn traverse_dir<P: AsRef<Path>, Q: AsRef<Path>>(root: P, exclude: Option<Q>) -> HashSet<PathBuf> {
  let mut dirs = VecDeque::from([root.as_ref().to_owned()]);
  let mut files = HashSet::new();

  while let Some(dir) = dirs.pop_front() {
    if exclude.as_ref().is_some_and(|e| dir.starts_with(e)) {
      continue;
    }

    for entry in fs::read_dir(dir).unwrap().map(Result::unwrap) {
      let file_type = entry.file_type().unwrap();
      if file_type.is_dir() {
        dirs.push_back(entry.path());
      } else if file_type.is_file() {
        files.insert(entry.path());
      } else {
        panic!("Unexpected file type: {:?}", file_type);
      }
    }
  }

  files
}

#[macro_export]
macro_rules! test_path {
  () => {{
    // HACK: Get module hierarchy for caller.
    let mut function = $crate::testing::type_of(|| ()).rsplit("::");
    // 0th element is `{closure}`.
    let case = function.nth(1).unwrap();
    let suite = function.next().unwrap();
    let module = function.next().unwrap();

    std::path::PathBuf::from(format!("{module}/{suite}/{case}"))
  }};
}

#[macro_export]
macro_rules! test_dir {
  ($($file:literal: {$($key:literal: $value:literal),* $(,)?}),* $(,)?) => {{
    let files = vec![
      $(($file, std::collections::HashMap::from([$(($key, $value)),*]))),*
    ];
    TestDir::new(test_path!(), files)
  }};
}

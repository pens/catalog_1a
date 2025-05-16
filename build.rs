// Copyright 2023-5 Seth Pendergrass. See LICENSE.

//! Build script to copy `ExifTool` next to built binary.

extern crate fs_extra;

use std::{env, fs, path::PathBuf};

use fs_extra::dir::CopyOptions;

fn main() {
  let out_dir = PathBuf::from(env::var("OUT_DIR").unwrap());
  fs::copy("third_party/exiftool/exiftool", out_dir.join("exiftool")).unwrap();
  fs_extra::copy_items(
    &["third_party/exiftool/lib"],
    out_dir,
    &CopyOptions::new().overwrite(true),
  )
  .unwrap();
}

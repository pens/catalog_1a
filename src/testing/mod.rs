// Copyright 2023-5 Seth Pendergrass. See LICENSE.

//! Test-only utilities.

mod asserts;
mod dates;
mod test_dir;

use std::{collections::HashMap, path::Path};

pub use dates::*;
use serde_json::Value;
pub use test_dir::*;

use crate::io;
pub use crate::{assert_dir, assert_err, assert_tag, assert_trash, metadata, test_dir, test_path};

/// Gets tag value for path via ExifTool.
pub fn read_tag(
  working_dir: impl AsRef<Path>,
  path: impl AsRef<Path>,
  group: Option<&str>,
  tag: &str,
) -> Option<String> {
  let tag_str = if let Some(group) = group {
    format!("-{group}:{tag}")
  } else {
    format!("-{tag}")
  };

  let args = [
    "-d",
    io::DATETIME_READ_FORMAT,
    "-json",
    tag_str.as_str(),
    path.as_ref().to_str().unwrap(),
  ];
  let stdout = io::run_exiftool(Some(working_dir), args).unwrap();

  let metadata = serde_json::from_slice::<Vec<HashMap<String, Value>>>(&stdout).unwrap();
  metadata[0]
    .get(tag)
    .and_then(|v| v.as_str())
    .map(std::string::ToString::to_string)
}

pub fn type_of<T>(_: T) -> &'static str {
  std::any::type_name::<T>()
}

#[macro_export]
macro_rules! metadata {
  ($($key:literal: $value:literal),* $(,)?) => {
    serde_json::from_value::<$crate::prim::Metadata>(
      serde_json::json!({
        "SourceFile": "-",
        "FileType": "-",
        "FileTypeExtension": "-",
        "FileModifyDate": "1970-01-01T00:00:00",
        $(
          $key: $value,
        )*
      })
    ).unwrap()
  }
}

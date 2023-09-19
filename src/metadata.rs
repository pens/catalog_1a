/*
    Struct to parse and hold metadata from `exiftool`.

    Copyright 2023 Seth Pendergrass. See LICENSE.
*/
use std::collections::HashMap;
use std::path::Path;
use std::process::Command;

use crate::config::Config;

#[derive(Default)]
pub struct Metadata {
    pub tags: HashMap<String, String>,
}

impl Metadata {
    // Get metadata for `path` from `exiftool`, and parse it for `tags`.
    pub fn parse<'a>(path: &Path, config: &Config) -> Option<Metadata> {
        let mut command = Command::new("exiftool");
        command
            .arg("-d") // date format
            .arg("%Y %m %d %H %M %S")
            .arg("-s") // short output (tag names match input)
            .arg("-t"); // tab delimited

        for tag in config.tags.keys() {
            command.arg(format!("-{}", tag));
        }

        let output = command.arg(path).output().ok()?;

        if !output.status.success() || output.stdout.is_empty() {
            return None;
        }
        let stdout = String::from_utf8(output.stdout).ok()?;

        log::trace!("{} metadata:\n{}", path.display(), stdout);

        let mut metadata = Metadata::default();
        for line in stdout.lines() {
            let mut iter = line.split('\t');
            let tag = iter.next()?;
            let value = iter.next()?;

            // Sanity check: format should be exactly `tag\tvalue`.
            assert!(iter.next().is_none());

            metadata.tags.insert(tag.to_owned(), value.to_owned());
        }

        Some(metadata)
    }

    // Helper to get expected path based on DateTimeOriginal, if present.
    pub fn get_datetime_path(&self) -> Option<String> {
        let substrs: Vec<&str> = self
            .tags
            .get("DateTimeOriginal")?
            .split_whitespace()
            .collect();
        let year_short = substrs[0].get(2..4)?;
        Some(format!(
            "{0}/{1}/{6}{1}{2}_{3}{4}{5}",
            substrs[0], substrs[1], substrs[2], substrs[3], substrs[4], substrs[5], year_short
        ))
    }

    // Helper to parse year out of DateTimeOriginal, if present.
    pub fn get_year(&self) -> Option<&str> {
        self.tags
            .get("DateTimeOriginal")
            .and_then(|x| x.split_once(' '))
            .map(|x| x.0)
    }
}

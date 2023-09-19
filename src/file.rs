/*
    Types for holding file data.

    Copyright 2023 Seth Pendergrass. See LICENSE.
*/
use crate::config::Config;
use crate::metadata::Metadata;
use std::path::{Path, PathBuf};

pub enum FileType {
    Media { metadata: Metadata },
    Sidecar,
    Other,
}

pub struct File {
    pub file_type: FileType,
    pub extension: String,
    pub path: PathBuf,
}

impl File {
    pub fn from(path: &Path, config: &Config) -> File {
        let file_type = if config.path_is_media(path) {
            if let Some(metadata) = Metadata::parse(path, config) {
                FileType::Media { metadata }
            } else {
                log::debug!("{}: Unable to parse metadata.", path.display());
                FileType::Media {
                    metadata: Metadata::default(),
                }
            }
        } else if config.path_is_sidecar(path) {
            FileType::Sidecar
        } else {
            FileType::Other
        };

        let extension = path
            .extension()
            .and_then(|e| e.to_str())
            .map_or(String::new(), |e| e.to_owned());

        File {
            file_type,
            extension,
            path: path.to_owned(),
        }
    }

    pub fn is_sidecar(&self) -> bool {
        matches!(self.file_type, FileType::Sidecar { .. })
    }

    pub fn is_media(&self) -> bool {
        matches!(self.file_type, FileType::Media { .. })
    }
}

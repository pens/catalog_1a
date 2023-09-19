/*
    Structure for holding configuration.

    Copyright 2023 Seth Pendergrass. See LICENSE.
*/
use std::collections::HashMap;
use std::path::Path;
use std::path::PathBuf;

mod constants;

pub struct ExtensionConfig {
    pub rename: Option<&'static str>,
    pub video: bool,
}

pub struct FormatConfig {
    pub extension: &'static str,
}

pub struct TagConfig {
    pub required: bool,
    override_from_live_photo_image_tag: bool,
}

pub struct Config {
    pub library_root: PathBuf,
    pub apply_changes: bool,
    pub extensions: HashMap<&'static str, ExtensionConfig>,
    pub formats: HashMap<&'static str, FormatConfig>,
    pub tags: HashMap<&'static str, TagConfig>,
}

impl Config {
    pub fn new(library_root: &Path, apply_changes: bool) -> Config {
        let mut extensions = HashMap::new();
        for extension in constants::EXTENSIONS {
            extensions.insert(
                extension.0,
                ExtensionConfig {
                    rename: extension.1,
                    video: extension.2,
                },
            );
        }

        let mut formats = HashMap::new();
        for format in constants::FORMATS {
            formats.insert(
                format.0,
                FormatConfig {
                    extension: format.1,
                },
            );
        }

        let mut tags = HashMap::new();
        for tag in constants::TAGS {
            tags.insert(
                tag.0,
                TagConfig {
                    required: tag.1,
                    override_from_live_photo_image_tag: tag.2,
                },
            );
        }

        Config {
            library_root: library_root.to_owned(),
            apply_changes,
            extensions,
            formats,
            tags,
        }
    }

    // Gets the ExtensionConfig for the given extension, if present.
    // This is case-insensitive.
    pub fn get_extension(&self, extension: &str) -> Option<&ExtensionConfig> {
        if self.extensions.contains_key(extension) {
            self.extensions.get(extension)
        } else if self
            .extensions
            .contains_key(extension.to_lowercase().as_str())
        {
            self.extensions.get(extension.to_lowercase().as_str())
        } else if self
            .extensions
            .contains_key(extension.to_uppercase().as_str())
        {
            self.extensions.get(extension.to_uppercase().as_str())
        } else {
            None
        }
    }

    pub fn path_is_media(&self, path: &Path) -> bool {
        let Some(extension) = path.extension().and_then(|e| e.to_str()) else {
            return false;
        };
        self.get_extension(extension).is_some()
    }

    pub fn path_is_sidecar(&self, path: &Path) -> bool {
        let Some(extension) = path.extension().and_then(|e| e.to_str()) else {
            return false;
        };
        extension.to_lowercase() == "xmp"
    }
}

/*
    Functionality for scanning a directory for media files and validating their attributes.

    Copyright 2023 Seth Pendergrass. See LICENSE.
*/
use crate::config::Config;
use crate::file::File;
use crate::process;
use std::borrow::Borrow;
use std::hash::Hash;
use std::{
    collections::{HashMap, HashSet},
    path::{Path, PathBuf},
};
use walkdir::WalkDir;

mod directory;
mod media;
mod sidecar;

// TODO break these types out into new module
#[derive(Default)]
pub struct Library {
    files: HashMap<PathBuf, File>,
}

impl Library {
    // Gets a mutable reference to file at path, else adds.
    fn get_else_add(&mut self, path: &Path, config: &Config) -> &mut File {
        if self.files.contains_key(path) {
            return self.files.get_mut(path).unwrap();
        } else {
            let file = File::from(path, config);
            self.files.insert(path.to_owned(), file);
            return self.files.get_mut(path).unwrap();
        }
    }
}

#[derive(Default)]
pub struct Results {
    // TODO these probably don't need to be owned types
    pub content_id_to_media: HashMap<String, HashSet<PathBuf>>,
    pub media_group_uuid_to_media: HashMap<String, HashSet<PathBuf>>,
    pub sidecar_to_targets: HashMap<PathBuf, HashSet<PathBuf>>,
    pub target_to_sidecars: HashMap<PathBuf, HashSet<PathBuf>>,
    pub sidecars_without_targets: HashSet<PathBuf>,
    pub bad_copyright: HashMap<PathBuf, String>,
    pub bad_path: HashMap<PathBuf, PathBuf>,
    pub bad_extension: HashMap<PathBuf, String>,
    pub wrong_format: HashMap<PathBuf, String>,
}

impl Results {
    // Adds a file to the set of potential targets for a sidecar.
    // Unless something has gone wrong, there should always be exactly one target per sidecar.
    fn add_sidecar_target(&mut self, sidecar: &Path, target: &Path) {
        Self::insert_hashmap_hashset(&mut self.sidecar_to_targets, sidecar, target);
        Self::insert_hashmap_hashset(&mut self.target_to_sidecars, target, sidecar);
    }

    // Adds photo or video to a live photo group by ContentIdentifier.
    fn add_live_photo_image(&mut self, id: &str, path: &Path) {
        Self::insert_hashmap_hashset(&mut self.content_id_to_media, id, path);
    }

    // Adds video to live photo group by MediaGroupUUID.
    fn add_live_photo_video(&mut self, id: &str, path: &Path) {
        Self::insert_hashmap_hashset(&mut self.media_group_uuid_to_media, id, path);
    }

    // Helper method for adding to a HashMap<..., HashSet>, creating the HashSet if needed.
    fn insert_hashmap_hashset<T, U>(map: &mut HashMap<T, HashSet<PathBuf>>, key: &U, value: &Path)
    where
        T: Hash + Eq + Borrow<U>,
        U: Hash + Eq + ToOwned<Owned = T> + ?Sized,
    {
        assert!(value.is_file());
        if let Some(set) = map.get_mut(key) {
            set.insert(value.to_owned());
        } else {
            let mut set = HashSet::new();
            set.insert(value.to_owned());
            assert!(map.insert(key.to_owned(), set).is_none());
        }
    }
}

pub fn scan(library_root: &Path, apply_changes: bool) {
    assert!(library_root.is_dir());
    log::info!("Scanning {}", library_root.display());

    let config = Config::new(library_root, apply_changes);
    let mut library = Library::default();
    let mut results = Results::default();

    //let mut test = 0;

    let filter = |e: &walkdir::DirEntry| {
        e.file_name().to_str().map_or(false, |n| n != "trash" && n != "imports")
    };

    for entry in WalkDir::new(config.library_root.clone())
        .into_iter()
        .filter_entry(|e| filter(e))
        .flatten()
    {
        if entry.file_type().is_dir() {
            directory::validate(entry.path());
        } else if entry.file_type().is_file() {
            let file = library.get_else_add(entry.path(), &config);

            if config.path_is_media(&file.path) {
                media::validate(file, &config, &mut results);
            } else if config.path_is_sidecar(&file.path) {
                sidecar::validate(file, &mut results);
            } else {
                log::debug!("{}: Unknown file type. Ignoring.", entry.path().display());
            }
        } else {
            log::debug!(
                "{}: Not a file or directory. Ignoring.",
                entry.path().display()
            );
        }

        /*
        test += 1;
        if test > 10000 {
            break;
        }
        */
    }

    process::process(&config, &results);
}

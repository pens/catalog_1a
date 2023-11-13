//! Functions for processing image & video libraries.
//!
//! Copyright 2023 Seth Pendergrass. See LICENSE.

use serde::Deserialize;
use std::{
    collections::HashMap,
    fs,
    path::{Path, PathBuf},
    process::Command, ffi::OsStr,
};

/// Scans all files under `library`, performing various cleanup tasks. This will move files that
/// are to be deleted to `library/trash`.
pub fn clean(library: &Path) {
    log::info!("Cleaning {}.", library.display());

    let mut catalog = Catalog::new(library, &library.to_path_buf().join("trash"));
    catalog.remove_duplicates_from_live_photos();
    catalog.remove_videos_from_deleted_live_photos();
    catalog.copy_metadata_from_live_photo_image_to_video();
    catalog.remove_sidecars_without_references();
    catalog.move_files_and_rename_empties_catalog(library);
}

/// Performs cleanup on `import` and then moves all files to `library`. Files to be deleted will be
/// placed in `library/trash`.
pub fn import(library: &Path, import: &Path) {
    log::info!("Importing {} into {}.", import.display(), library.display());

    let mut catalog = Catalog::new(import, &library.to_path_buf().join("trash"));
    catalog.remove_duplicates_from_live_photos();
    catalog.remove_videos_from_deleted_live_photos();
    catalog.copy_metadata_from_live_photo_image_to_video();
    catalog.remove_sidecars_without_references();
    catalog.move_files_and_rename_empties_catalog(library);
}

#[derive(Deserialize)]
#[serde(rename_all = "PascalCase")]
struct Metadata {
    source_file: PathBuf,
    file_type: String,

    // Live Photos
    #[serde(rename = "MediaGroupUUID")]
    media_group_uuid: Option<String>, // Image
    content_identifier: Option<String>, // Video

    date_time_original: Option<String>,
}

struct Media {
    xmp: Option<PathBuf>,
    metadata: Metadata,
}

struct Xmp {
    media: Option<PathBuf>,
    metadata: Metadata,
}

struct Catalog {
    trash: PathBuf,
    media_files: HashMap<PathBuf, Media>,
    xmps: HashMap<PathBuf, Xmp>,
    // Key is a Vec in case of duplicate items (e.g. jpg & HEIC).
    live_photo_images: HashMap<String, Vec<PathBuf>>,
    live_photo_videos: HashMap<String, Vec<PathBuf>>,
}

impl Catalog {
    fn exiftool<I, S>(args: I) -> Vec<u8>
        where I: IntoIterator<Item = S>,
        S: AsRef<OsStr>, {
        let mut cmd = Command::new("exiftool");
        cmd.args(args);
        let output = cmd.output().unwrap();
        log::trace!(
            "exiftool output:\n{}",
            String::from_utf8_lossy(&output.stdout)
        );
        assert!(output.status.success(), "exiftool failed with args: `{:#?}`.", cmd.get_args().collect::<Vec<&OsStr>>());

        output.stdout
    }

    ///  To ensure this tool doesn't cause problems if I ever switch to Adobe-style (e.g. .xmp vs
    /// .ext.xmp) XMP file naming, panic if any are detected in the catalog.
    fn sanity_check_xmp_filenames(xmps: &HashMap<PathBuf, Xmp>) {
        log::debug!("Sanity checking XMP filename formats.");
        for xmp in xmps.keys() {
            let stem = xmp.file_stem().unwrap();
            let stem_path = PathBuf::from(stem);
            assert!(stem_path.extension().is_some(),
                    "\n\nWARNING: XMP File in Adobe Format detected. Program not able to continue.\n{}\n\n",
                    xmp.display()
                );
        }
    }

    /// Finds the largest file in `paths, returning it alongside the remainder.
    fn filter_out_largest(paths: &[PathBuf]) -> (PathBuf, Vec<PathBuf>) {
        let mut paths = paths.to_vec();
        paths.sort_by(|a, b| {
            let a_size = fs::metadata(a).unwrap().len();
            let b_size = fs::metadata(b).unwrap().len();
            b_size.cmp(&a_size)
        });
        let largest = paths.pop().unwrap();

        (largest, paths)
    }

    /// Moves `path` to `trash`.
    fn move_to_trash(path: &Path, trash: &Path) {
        let path_trash = trash.join(path.file_name().unwrap());
        // If this trips, instead just switch to `exiftool`.
        assert!(!path_trash.exists(), "Cannot safely delete {} due to name collision in {}.", path.display(), trash.display());
        fs::rename(path, path_trash).unwrap();
    }

    /// Move file to trash.
    fn remove(&mut self, path: &Path) {
        if let Some(media) = self.media_files.remove(path) {
            log::debug!("{}: Removing media file.", path.display());
            Self::move_to_trash(path, &self.trash);

            // Remove references to file in Live Photo mappings.
            if let Some(id) = &media.metadata.media_group_uuid {
                self.live_photo_images.get_mut(id).unwrap().retain(|p| p != path);
            }
            if let Some(id) = &media.metadata.content_identifier {
                self.live_photo_videos.get_mut(id).unwrap().retain(|p| p != path);
            }

            // Delete associated XMP file (if present).
            if let Some(xmp) = media.xmp {
                log::debug!(
                    "{}: Also removing associated XMP file.",
                    xmp.display()
                );
                Self::move_to_trash(&xmp, &self.trash);
            }
        } else if self.xmps.remove(path).is_some() {
            log::debug!("{}: Removing sidecar file.", path.display());
            Self::move_to_trash(path, &self.trash);
        } else {
            panic!("{}: File not found. Cannot remove.", path.display());
        }
    }

    /// Create a new catalog of library, with trash as the desitnation for removed files.
    fn new(library: &Path, trash: &Path) -> Self {
        log::info!("Building catalog.");

        let stdout = Self::exiftool([
            "-FileType",
            "-ContentIdentifier",
            "-MediaGroupUUID",
            "-DateTimeOriginal",
            "-json", // exiftool prefers JSON or XML over CSV.
            "-r",
            library.to_str().unwrap(),
        ]);

        // Parse exiftool output.
        let Ok(metadata) = serde_json::from_slice::<Vec<Metadata>>(&stdout[..]) else {
            panic!("Failed to parse exiftool output.");
        };
        let (file_metadata, xmp_metadata): (Vec<_>, Vec<_>) =
            metadata.into_iter().partition(|m| m.file_type != "XMP");

        // XMP setup.
        let mut media_files = file_metadata
            .into_iter()
            .map(|m| {
                (
                    m.source_file.clone(),
                    Media {
                        xmp: None,
                        metadata: m,
                    },
                )
            })
            .collect::<HashMap<PathBuf, Media>>();
        let mut xmps = xmp_metadata
            .into_iter()
            .map(|m| {
                (
                    m.source_file.clone(),
                    Xmp {
                        media: None,
                        metadata: m,
                    },
                )
            })
            .collect::<HashMap<PathBuf, Xmp>>();
        for (path, metadata) in media_files.iter_mut() {
            // .ext -> .ext.xmp.
            let Some(ext) = path.extension() else {
                panic!("{}: Media file without extension.", path.display());
            };
            let mut ext_s = ext.to_os_string();
            ext_s.push(".xmp");
            let expected_xmp = path.with_extension(ext_s);

            if xmps.contains_key(&expected_xmp) {
                metadata.xmp = Some(expected_xmp.clone());
                xmps.get_mut(&expected_xmp).unwrap().media = Some(path.clone());
            }
        }

        // Sanity check: In case I switch to Adobe at some point (which uses file.xmp instead of
        // file.jpg.xmp).
        Self::sanity_check_xmp_filenames(&xmps);

        // Live Photo setup.
        // Go through all non-XMP files to build Live Photo ID to file mappings.
        let mut live_photo_images = HashMap::new();
        let mut live_photo_videos = HashMap::new();
        for media in media_files.values() {
            if let Some(id) = &media.metadata.media_group_uuid {
                log::debug!(
                    "{}: Live Photo image with ID {}.",
                    &media.metadata.source_file.display(),
                    id
                );
                live_photo_images
                    .entry(id.clone())
                    .or_insert_with(Vec::new)
                    .push(media.metadata.source_file.clone());
            }
            if let Some(id) = &media.metadata.content_identifier {
                log::debug!(
                    "{}: Live Photo video with ID {}.",
                    &media.metadata.source_file.display(),
                    id
                );
                live_photo_videos
                    .entry(id.clone())
                    .or_insert_with(Vec::new)
                    .push(media.metadata.source_file.clone());
            }
        }

        Self {
            trash: trash.to_path_buf(),
            media_files,
            xmps,
            live_photo_images,
            live_photo_videos,
        }
    }

    /// If there are multiple Live Photo images or videos sharing the same ID, assume that there has
    /// been duplication. Most often, this is because a photo exists as both a JPG and HEIC. Images
    /// prefer HEIC over JPG, while videos are purely based on size.
    fn remove_duplicates_from_live_photos(&mut self) {
        log::info!("Removing duplicates from Live Photos.");

        // Delete duplicate images.
        let mut images_to_remove = HashMap::new();
        for paths in self.live_photo_images.values() {
            if paths.len() > 1 {
                let (heic_paths, mut jpg_paths): (Vec<_>, Vec<_>) = paths
                    .clone()
                    .into_iter()
                    .partition(|p| self.media_files.get(p).unwrap().metadata.file_type == "HEIC");
                match heic_paths.len() {
                    0 => {
                        // No HEICs, so just keep the largest JPG.
                        let (largest, others) = Self::filter_out_largest(&jpg_paths);
                        images_to_remove.insert(largest, others);
                    }
                    1 => {
                        // One HEIC, so keep it and delete the rest.
                        images_to_remove.insert(heic_paths[0].clone(), jpg_paths);
                    }
                    _ => {
                        // Multiple HEICs, so keep the largest and delete the remainding HEIC & JPG.
                        let (largest, mut others) = Self::filter_out_largest(&heic_paths);
                        others.append(&mut jpg_paths);
                        images_to_remove.insert(largest, others);
                    }
                }
            }
        }
        for (path_keep, paths_delete) in &images_to_remove {
            log::warn!(
                "{}: Duplicated Live Photo image. Removing:",
                path_keep.display()
            );
            for path in paths_delete.iter() {
                log::warn!("\t- {}", path.display());
                self.remove(path);
            }
        }

        // Delete duplicate videos.
        let mut videos_to_remove = HashMap::new();
        for paths in self.live_photo_videos.values() {
            if paths.len() > 1 {
                let (largest, others) = Self::filter_out_largest(paths);
                videos_to_remove.insert(largest, others);
            }
        }
        for (path_keep, paths_delete) in &videos_to_remove {
            log::warn!(
                "{}: Duplicated Live Photo video. Removing:",
                path_keep.display()
            );
            for path in paths_delete.iter() {
                log::warn!("\t- {}", path.display());
                self.remove(path);
            }
        }
    }

    /// I'm too lazy to delete the videos from Live Photos when I delete the images. If we find a
    /// remaining video, based on its ID missing a corresponding image, remove it.
    fn remove_videos_from_deleted_live_photos(&mut self) {
        log::info!("Removing videos from deleted Live Photos.");

        let mut to_remove = Vec::new();
        for (id, paths) in &self.live_photo_videos {
            if !self.live_photo_images.contains_key(id) {
                to_remove.extend_from_slice(paths);
            }
        }
        for path in to_remove {
            log::warn!(
                "{}: Video remaining from presumably deleted Live Photo image. Removing.",
                path.display()
            );
            self.remove(&path);
        }
    }

    /// Copy specific metadata from Live Photo images to videos. This saves having to duplicate work
    /// tagging, geotagging, etc.
    fn copy_metadata_from_live_photo_image_to_video(&self) {
        log::info!("Copying metadata from Live Photo images to videos.");

        for (id, video_paths) in &self.live_photo_videos {
            // Safety checks.
            let Some(image_paths) = self.live_photo_images.get(id) else {
                for path in video_paths {
                    log::error!("{}: Live Photo video without corresponding image. Cannot retrieve metadata to copy. Skipping.", path.display());
                }
                continue;
            };
            assert!(
                !image_paths.is_empty(),
                "Unexpectedly found Live Photo ID {} with no image target(s).",
                id
            );
            assert!(
                !video_paths.is_empty(),
                "Unexpectedly found Live Photo ID {} with no video target(s).",
                id
            );
            if image_paths.len() > 1 {
                log::error!("{}: Multiple Live Photo images with ID {}. Cannot copy metadata to video. Skipping.", image_paths[0].display(), id);
                continue;
            } else if video_paths.len() > 1 {
                log::error!("{}: Multiple Live Photo images with ID {}. Cannot copy metadata to video. Skipping.", video_paths[0].display(), id);
                continue;
            }

            // Copy metadata.
            let image_path = &image_paths[0];
            let video_path = &video_paths[0];
            log::debug!(
                "{}: Copying metadata to {}.",
                image_path.display(),
                video_path.display()
            );
            assert!(self.media_files.get(image_path).unwrap().xmp.is_none() && self.media_files.get(video_path).unwrap().xmp.is_none(), "Live Photo metadata copying not able to handle XMPs.");
            // TODO build exiftool command
            // TODO implement metadata copying via exiftool
        }
    }

    /// If a sidecar XMP is around with no possible target file, remove it.
    fn remove_sidecars_without_references(&mut self) {
        log::info!("Removing XMP sidecars without corresponding files.");

        let mut to_remove = Vec::new();
        for (path, metadata) in &self.xmps {
            if metadata.media.is_none() {
                to_remove.push(path.clone());
            }
        }
        for path in to_remove {
            log::warn!(
                "{}: XMP sidecar without corresponding file. Removing.",
                path.display()
            );
            self.remove(&path);
        }
    }

    /// Moves files into their final home in `destination`, based on their DateTimeOriginal tag, and
    /// changes their file extensions to match their format. This unifies extensions per file type
    /// (e.g. jpeg vs jpg) and fixes incorrect renaming of mov to mp4.
    /// Note: This renames based on the DateTimeOriginal of the target file, not of the XMP.
    fn move_files_and_rename_empties_catalog(&mut self, destination: &Path) {
        log::info!("Moving and renaming files.");

        for (path, media) in &self.media_files {
            if media.metadata.date_time_original.is_none() {
                log::error!("{}: No DateTimeOriginal tag. Cannot move & rename. Skipping.", path.display());
            }

            // If an XMP exists for this file, move & rename it first.
            // TODO flip order around, saving file type extension to put in xmp path
            if let Some(xmp) = &media.xmp {
                let xmp_name_arg = format!(
                    "-TestName<{}/${{DateTimeOriginal}}.${{FileTypeExtension}}.xmp",
                    destination.to_str().unwrap()
                );
                Self::exiftool([
                    "-tagsFromFile",
                    path.to_str().unwrap(),
                    "-d",
                    "%Y/%m/%Y%m%d_%H%M%S%%+c",
                    &xmp_name_arg,
                    xmp.to_str().unwrap(),
                ]);
            }

            // Move & rename the media file.
            let name_arg = format!(
                "-TestName<{}/${{DateTimeOriginal}}.$FileTypeExtension",
                destination.to_str().unwrap()
            );
            Self::exiftool([
                &name_arg,
                "-d",
                "%Y/%m/%Y%m%d_%H%M%S%%+c",
                path.to_str().unwrap(),
            ]);
        }

        // HACK: Rather than trying to synchronize the catalog with the moves & renames above, just
        // erase the catalog. Nothing should every happen beyond this point, anyway.
        self.media_files.clear();
        self.xmps.clear();
        self.live_photo_images.clear();
        self.live_photo_videos.clear();
    }
}

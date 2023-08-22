/*
    photo_manager is a wrapper around `exiftool` to help organize my photo &
    video library.

    TODO:
    - Collect changes from checks to apply in post
    - Move post proc info to fixes
        - different for global checks vs fixes?

    Copyright 2023 Seth Pendergrass. See LICENSE.
*/
use std::collections::{HashMap, HashSet};
use std::fmt;
use std::io::Write;
use std::path::{Path, PathBuf};
use walkdir::{DirEntry, WalkDir};

use env_logger::Builder;
use log::LevelFilter;

use clap::{Parser, Subcommand};

mod checks_entry;
mod checks_global;
mod fixes;
mod metadata;

use metadata::Metadata;

/*
    Setup
*/

#[derive(Parser)]
struct Config {
    /// Directory of photo library.
    library: PathBuf,

    /// Enable Debug and Trace logs.
    #[arg(short)]
    verbose: bool,

    #[command(subcommand)]
    command: Commands,

}

#[derive(Subcommand)]
enum Commands {
    /// Validates organization & metadata of photo library, and optionally
    /// applies fixes.
    Scan {
        /// Whether to apply fixes. Files to be removed will be sent to the
        /// specified directory for review.
        fix: Option<PathBuf>,
    },

    /// Validates metadata of photos to import into library and, if passing,
    /// organizes them in the library.
    Import {
    },
}

/*
    Global Metadata
*/

#[derive(Default)]
pub struct PostProcessingInfo {
    pub content_id_map: HashMap<String, HashSet<Metadata>>,
    pub media_group_uuid_map: HashMap<String, HashSet<Metadata>>,
}

impl PostProcessingInfo {
    // Helper for adding a Path to a set within a map.
    fn add_to_map(map: &mut HashMap<String, HashSet<Metadata>>, id: &str, metadata: &Metadata) {
        map.entry(id.to_owned()).or_default().insert(metadata.clone());
    }

    pub fn update_from_metadata(&mut self, metadata: &Metadata) {
        if let Some(content_identifier) = &metadata.content_identifier {
            Self::add_to_map(&mut self.content_id_map, content_identifier, metadata);
        }
        if let Some(media_group_uuid) = &metadata.media_group_uuid {
            Self::add_to_map(&mut self.media_group_uuid_map, media_group_uuid, metadata);
        }
    }
}

/*
    Helpers
*/

fn has_file_extension(path: &Path) -> bool {
    path.extension().is_some()
}

fn is_dir(entry: &DirEntry) -> bool {
    entry.file_type().is_dir()
}

fn is_file(entry: &DirEntry) -> bool {
    entry.file_type().is_file()
}

#[derive(Debug)]
struct ValidationError;

impl fmt::Display for ValidationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Validation Error")
    }
}

// Per-file or per-directory checks
fn per_entry_checks(entry: &DirEntry, config: &Config, post_processing_info: &mut PostProcessingInfo) -> Result<(), ValidationError> {
    if is_dir(entry) {
        if entry.path() == config.removal_dir {
            log::info!(
                "{}: Removal directory under scan directory. Skipping directory.",
                entry.path().display()
            );
            return Err(ValidationError);
        }
        checks_entry::all_entries_same_type(entry.path());
    } else if is_file(entry) {
        let path = entry.path();

        if !has_file_extension(path) {
            log::error!("{}: No file extension. Skipping file.", path.display());
            return Err(ValidationError);
        }

        if checks_entry::is_xmp_sidecar(path) {
            checks_entry::xmp_has_referenced_file(path);
            return Ok(());
        }

        let metadata = metadata::parse_metadata(path).map_err(
            |e| {
                log::error!("{}: Failed to parse metadata: {}", path.display(), e);
                ValidationError
            }
        )?;

        post_processing_info.update_from_metadata(&metadata);

        checks_entry::has_xmp_sidecar(path);
        checks_entry::has_date_time_original(&metadata);
        checks_entry::artist_copyright(&metadata);

        checks_entry::path_format(&metadata);
        checks_entry::extension_format(&metadata);
    }

    Ok(())
}

// Global validation checks
// e.g. matching image & video parts of Live Photos
fn global_checks(post_processing_info: &mut PostProcessingInfo) {
    checks_global::duplicate_images_based_on_live_photos(post_processing_info);
    checks_global::correlate_live_photos(post_processing_info);
}

fn apply_fixes(config: &Config, post_processing_info: &mut PostProcessingInfo) {
    // Post-processing
    // remove files if needed
        // update post_proc_info
    // rename files if needed
        // file name
        // extension
        // update post_proc_info with new names
}

fn enable_logging(config: &Config) {
    Builder::new()
        .filter_level(if config.verbose { LevelFilter::Trace } else { LevelFilter::Info })
        .format(|buf, record| {
            writeln!(buf, "{} {}", buf.default_level_style(record.level()).value(record.level()), record.args())
        })
        .init();
}

fn main() {
    let config = Config::parse();
    enable_logging(&config);

    let mut post_processing_info = PostProcessingInfo::default();

    for entry in WalkDir::new(&config.scan_dir).into_iter().flatten() {
        if per_entry_checks(&entry, &config, &mut post_processing_info).is_err() {
            log::error!("{}: Failed to validate.", entry.path().display());
        }
    }

    global_checks(&mut post_processing_info);

    if config.fix {
        apply_fixes(&config, &mut post_processing_info);
    }
}

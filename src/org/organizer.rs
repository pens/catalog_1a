//! Core catalog management type and functionality.
//!
//! Copyright 2023-4 Seth Pendergrass. See LICENSE.

use std::path::{Path, PathBuf};

use chrono::{DateTime, FixedOffset};

use crate::org::gbl;

use super::catalog::Catalog;
use super::gbl::FileHandle;
use super::io;
use super::live_photo::IdLinker;

/// Manager for organizing a multimedia library.
///
/// Uses `catalog` to track files and their metadata, and `live_photo_linker` manage Live Photos.
/// All operations actually manipulating files will be called from here into `io::functions`.
pub struct Organizer {
  trash: Option<PathBuf>,
  catalog: Catalog,
  live_photo_linker: IdLinker,
}

impl Organizer {
  //
  // Constructors.
  //

  /// Scans import for files to import into a catalog.
  pub fn import(import: &Path) -> Self {
    Self::new(import, None)
  }

  /// Loads an existing library for maintenance. Removed files will be moved to trash.
  /// Note: If trash lies within library, files within will not be scanned.
  pub fn load_library(library: &Path, trash: &Path) -> Self {
    Self::new(library, Some(trash))
  }

  //
  // Public.
  //

  /// Remove duplicate images or videos based on Live Photo `ContentIdentifier`. Most often, this
  /// is because a photo exists as both a JPG and HEIC.
  /// This will keep the newest file and remove the rest, preferring HEIC over JPG for images.
  pub fn remove_live_photo_duplicates(&mut self) {
    log::info!("Removing duplicates from Live Photos.");

    let get_file_type = |fh: FileHandle| -> String {
      let m = self.catalog.get_metadata(fh);

      if m.file_type == "MOV" && m.compressor_id.is_some() {
        gbl::live_photo_codec_to_type(m.compressor_id.unwrap().as_str())
      } else {
        m.file_type
      }
    };

    let get_modify_date = |fh: FileHandle| -> DateTime<FixedOffset> {
      self.catalog.get_metadata(fh).get_file_modify_date()
    };

    for (keep, duplicates) in self
      .live_photo_linker
      .remove_duplicates(get_file_type, get_modify_date)
    {
      log::warn!(
        "{}: Live Photo has the following duplicates, removing:",
        self.catalog.get_metadata(keep).source_file.display()
      );
      for path in duplicates {
        log::warn!(
          "\t{}",
          self.catalog.get_metadata(path).source_file.display()
        );
        self.remove_from_catalog(path);
      }
    }
  }

  /// Removes any Live Photo videos without corresponding images. This is based on the
  /// presence and value of the `ContentIdentifier` tag.
  pub fn remove_leftover_live_photo_videos(&mut self) {
    log::info!("Removing videos from deleted Live Photos.");

    for path in self.live_photo_linker.remove_leftover_videos() {
      log::warn!(
        "{}: Video remaining from presumably deleted Live Photo image. Removing.",
        self.catalog.get_metadata(path).source_file.display()
      );
      self.remove_from_catalog(path);
    }
  }

  /// Remove sidecar files for which the expected source file does not exist.
  pub fn remove_leftover_sidecars(&mut self) {
    log::info!("Removing XMP sidecars without corresponding files.");

    for sidecar in self.catalog.remove_leftover_sidecars() {
      let path = sidecar.metadata.source_file;
      log::warn!(
        "{}: XMP sidecar without corresponding media file.",
        path.display()
      );
      self.trash_file(&path);
    }
  }

  /// Copy metadata from Live Photo images to videos.
  /// This keeps datetime, geotags, etc. consistent.
  pub fn synchronize_live_photo_metadata(&mut self) {
    log::info!("Copying metadata from Live Photo images to videos.");

    for (photos, videos) in self.live_photo_linker.iter() {
      // If there are multiple images or videos, warn and skip.
      if photos.len() > 1 || videos.len() > 1 {
        log::warn!(
          "{}: Live Photo can't synchronize metadata due to duplicates:",
          self.catalog.get_metadata(photos[0]).source_file.display()
        );
        for path in photos.iter().skip(1) {
          log::warn!(
            "\t{}: Duplicate Live Photo image",
            self.catalog.get_metadata(*path).source_file.display()
          );
        }
        for path in &videos {
          log::warn!(
            "\t{}: Duplicate Live Photo video",
            self.catalog.get_metadata(*path).source_file.display()
          );
        }
        continue;
      }

      // Select metadata source.
      let source = self.catalog.get_metadata_source_path(photos[0]);

      // Collect metadata sinks.
      let mut sinks = self.catalog.get_sidecars(videos[0]);
      sinks.push((videos[0], self.catalog.get_metadata(videos[0]).source_file));

      // Copy metadata.
      for (handle, sink) in sinks {
        log::debug!(
          "{} -> {}: Synchronizing metadata from Live Photo image.",
          source.display(),
          sink.display()
        );
        let metadata = io::copy_metadata(&source, &sink);

        self.catalog.update(handle, metadata);
      }
    }
  }

  /// Check that all media files have expected metadata tags.
  /// If there are associated XMP files, they will be checked as well, however XMP files without
  /// referenced media files will *not* be checked.
  pub fn validate_tags(&self) {
    log::info!("Checking that all files have required tags.");

    self.catalog.validate_tags();
  }

  /// Ensures every file has an associated XMP sidecar, creating one if not already present.
  pub fn create_missing_sidecars(&mut self) {
    log::info!("Ensuring all media files have associated XMP sidecar.");

    for path in self.catalog.get_missing_sidecars() {
      log::debug!("{}: Creating XMP sidecar.", path.display());
      self.catalog.insert_sidecar(io::create_xmp(&path));
    }
  }

  /// Moves files into their final home in destination, based on their `DateTimeOriginal` tag, and
  /// changes their file extensions to match their format. This unifies extensions per file type
  /// (e.g. jpeg vs jpg) and fixes incorrect renaming of mov to mp4.
  /// If a `SubSecDateTimeOriginal` tag is present, that will be preferred to better keep files
  /// sorted.
  pub fn move_and_rename_files(&mut self, destination: &Path) {
    log::info!("Moving and renaming files.");

    let mut updates = Vec::new();

    for (handle, media) in self.catalog.iter_media() {
      let media_path = &media.metadata.source_file;
      log::debug!("{}: Moving & renaming.", media_path.display());

      // Prefer XMP metadata, if present.
      let source = self.catalog.get_metadata_source_path(handle);

      // Get DateTimeOriginal tag
      if media.metadata.date_time_original.is_none() {
        log::warn!(
          "{}: DateTimeOriginal tag not found. Skipping move & rename.",
          media_path.display()
        );
        continue;
      }

      let datetime_tag = if media.metadata.sub_sec_date_time_original.is_some() { "SubSecDateTimeOriginal" } else { "DateTimeOriginal" };

      let media_file_ext = &media.metadata.file_type_extension;
      let new_path = io::move_file(
        media_path,
        destination,
        datetime_tag,
        media_file_ext,
        Some(&source),
      );
      log::debug!("{}: Moved to {}.", media_path.display(), new_path.display());

      updates.push((handle, io::read_metadata(&new_path)));

      // Move XMPs as well, keeping "file.ext.xmp" format.
      for (sidecar_handle, sidecar_path) in self.catalog.get_sidecars(handle) {
        let new_sidecar_path = io::move_file(
          &sidecar_path,
          destination,
         datetime_tag,
          &(media_file_ext.to_string() + ".xmp"),
          Some(&source),
        );
        log::debug!(
          "\tMoved XMP sidecar {} -> {}.",
          sidecar_path.display(),
          new_sidecar_path.display()
        );

        updates.push((sidecar_handle, io::read_metadata(&new_sidecar_path)));
      }
    }

    // Reload all moved files to ensure metadata is fully up-to-date.
    for (handle, metadata) in updates {
      self.catalog.update(handle, metadata);
    }
  }

  //
  // Private.
  //

  /// Create a new catalog of library, with trash as the destination for removed files.
  fn new(directory: &Path, trash: Option<&Path>) -> Self {
    log::info!("Building catalog.");
    let catalog = Catalog::new(io::read_metadata_recursive(directory, trash));

    log::info!("Building Live Photo image <-> video mapping.");
    let live_photo_linker = IdLinker::new(catalog.iter_media());

    Self {
      trash: trash.map(Path::to_path_buf),
      catalog,
      live_photo_linker,
    }
  }

  /// Remove `file_handle` from catalog, and if a media file, any dependent sidecars.
  /// If self.trash is `Some()`, moves files to trash.
  /// Note: This does *not* remove Live Photo mappings, as this should only be used on files that
  /// the live photo mapping has removed.
  fn remove_from_catalog(&mut self, file_handle: FileHandle) {
    for path in self.catalog.remove(file_handle) {
      self.trash_file(&path);
    }
  }

  /// Moves path to trash, if trash is `Some()`.
  fn trash_file(&self, path: &Path) {
    if let Some(trash) = &self.trash {
      log::debug!("{}: Moving to trash.", path.display());
      io::remove_file(path, trash);
    }
  }
}

#[cfg(test)]
mod test {
  use super::*;
  use crate::org::testing;

  /// Should prefer HEVC over AVC.
  #[test]
  fn test_keeps_hevc_over_avc() {
    let d = testing::test_dir!();
    let i = d.add_heic(
      "img.heic",
      &[
        "-SubSecDateTimeOriginal=2024-06-23 15:28:00",
        "-ContentIdentifier=A",
      ],
    );
    let hevc = d.add_hevc(
      "hevc.mov",
      &[
        "-SubSecDateTimeOriginal=2024-06-23 15:28:00",
        "-ContentIdentifier=A",
      ],
    );
    let avc = d.add_avc(
      "avc.mov",
      &[
        "-SubSecDateTimeOriginal=2024-06-23 15:28:00",
        "-ContentIdentifier=A",
      ],
    );

    let mut o = Organizer::load_library(&d.root, &d.trash);

    o.remove_live_photo_duplicates();

    assert!(i.exists());
    assert!(hevc.exists());
    assert!(!avc.exists());
  }

  /// Should trash duplicated Live Photo images and videos.
  #[test]
  fn test_trashes_live_photo_duplicate() {
    let d = testing::test_dir!();
    let jpg = d.add_jpg(
      "img.jpg",
      &[
        "-SubSecDateTimeOriginal=2024-06-23 15:28:00",
        "-ContentIdentifier=A",
      ],
    );
    let heic = d.add_heic(
      "img.heic",
      &[
        "-SubSecDateTimeOriginal=2024-06-23 15:28:00",
        "-ContentIdentifier=A",
      ],
    );
    let mov = d.add_avc(
      "img.mov",
      &[
        "-SubSecDateTimeOriginal=2024-06-23 15:28:00",
        "-ContentIdentifier=A",
      ],
    );

    let mut o = Organizer::load_library(&d.root, &d.trash);

    o.remove_live_photo_duplicates();

    assert!(!jpg.exists());
    assert!(heic.exists());
    assert!(mov.exists());
  }

  /// Should trash Live Photo videos without image (assuming image was deleted).
  #[test]
  fn test_trashes_leftover_live_photo_video() {
    let d = testing::test_dir!();
    let i = d.add_heic(
      "img1.heic",
      &[
        "-SubSecDateTimeOriginal=2024-06-23 15:28:00-07:00",
        "-ContentIdentifier=A",
      ],
    );
    let v = d.add_avc(
      "img1.mov",
      &[
        "-SubSecDateTimeOriginal=2024-06-23 15:28:00-07:00",
        "-ContentIdentifier=A",
      ],
    );
    let leftover = d.add_hevc(
      "img2.mov",
      &[
        "-SubSecDateTimeOriginal=2024-06-23 15:28:00-07:00",
        "-ContentIdentifier=B",
      ],
    );
    let not_live = d.add_hevc(
      "vid.mov",
      &[
        "-SubSecDateTimeOriginal=2024-06-23 15:28:00",
        "-ContentIdentifier=",
      ],
    );

    let mut o = Organizer::load_library(&d.root, &d.trash);

    o.remove_leftover_live_photo_videos();

    assert!(i.exists());
    assert!(v.exists());
    assert!(!leftover.exists());
    assert!(not_live.exists());
  }

  /// Should trash leftover XMPs, assuming the media file was deleted.
  #[test]
  fn test_trashes_leftover_xmp() {
    let d = testing::test_dir!();
    let i = d.add_jpg("img1.jpg", &["-SubSecDateTimeOriginal=2024-06-23 15:28:00"]);
    let x_i = d.add_xmp("img1.jpg.xmp", &["-SubSecDateTimeOriginal=2024-06-23 15:28:00"]);
    let x_leftover = d.add_xmp("img2.jpg.xmp", &["-SubSecDateTimeOriginal=2024-06-23 15:28:00"]);

    let mut o = Organizer::load_library(&d.root, &d.trash);

    o.remove_leftover_sidecars();

    assert!(i.exists());
    assert!(x_i.exists());
    assert!(!x_leftover.exists());
  }

  /// Should prefer image metadata to video in synchronization. This way, only the Live Photo image
  /// metadata need be updated (e.g. geotags).
  #[test]
  fn test_prioritizes_live_photo_image_over_video_metadata() {
    let d = testing::test_dir!();
    let i = d.add_heic(
      "img.heic",
      &[
        "-DateTimeOriginal=2024-06-23 15:28:00-0700",
        "-ContentIdentifier=A",
      ],
    );
    let v = d.add_avc(
      "vid.mov",
      &[
        "-DateTimeOriginal=2024-01-01 00:00:00-0700",
        "-ContentIdentifier=A",
      ],
    );

    let mut o = Organizer::import(&d.root);

    o.synchronize_live_photo_metadata();

    assert_eq!(
      testing::read_tag(&i, "-DateTimeOriginal"),
      "2024-06-23 15:28:00 -0700"
    );
    assert_eq!(
      testing::read_tag(&v, "-DateTimeOriginal"),
      "2024-06-23 15:28:00 -0700"
    );
  }

  /// Sidecars should be generated for any media file without.
  #[test]
  fn test_creates_missing_sidecars() {
    let d = testing::test_dir!();
    let i = d.add_avc("img.jpg", &["-DateTimeOriginal=2024-06-23 15:28:00"]);

    let mut o = Organizer::import(&d.root);

    o.create_missing_sidecars();

    assert!(i.with_extension("jpg.xmp").exists());
  }

  /// Should move and rename files based on XMP metadata, not media metadata.
  #[test]
  fn test_prioritizes_xmp_metadata_over_media() {
    let d = testing::test_dir!();
    let i = d.add_jpg("img.jpg", &["-DateTimeOriginal=2000-01-01 00:00:00"]);
    let x = d.add_xmp("img.jpg.xmp", &["-DateTimeOriginal=2024-06-23 16:28:00"]);

    let mut o = Organizer::import(&d.root);

    o.move_and_rename_files(&d.root);

    // XMP metadata should take priority when renaming.
    assert!(!i.exists());
    assert!(d.root.join("2024/06/2406231628000000.jpg").exists());
    assert!(!x.exists());
    assert!(d.root.join("2024/06/2406231628000000.jpg.xmp").exists());
  }

  #[test]
  fn test_uses_subseconds_if_present() {
    let d = testing::test_dir!();
    let i = d.add_jpg("img.jpg", &["-SubSecDateTimeOriginal=2000-01-01 00:00:00.0123"]);

    let mut o = Organizer::import(&d.root);

    o.move_and_rename_files(&d.root);

    assert!(!i.exists());
    assert!(d.root.join("2000/01/0001010000000123.jpg").exists());
  }
}

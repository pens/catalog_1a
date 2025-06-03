// Copyright 2023-5 Seth Pendergrass. See LICENSE.

//! Program subcommands for managing photo/video catalog.

use std::path::Path;

use crate::{io, org::Organizer};

pub fn exiftool_check() -> Result<(), String> {
  io::exiftool_check()
}

/// Scans all files under `catalog`, performing various cleanup tasks. This will
/// move files that are to be deleted to `catalog/.trash`.
pub fn org(catalog: impl AsRef<Path>) -> Result<(), String> {
  log::info!("{}: Organizing.", catalog.as_ref().display());

  let trash = catalog.as_ref().join(".trash");
  let organizer = Organizer::load_catalog(&catalog, Some(trash))?;

  run(organizer, catalog, true)
}

/// Performs cleanup on `import` and then moves all *good* files to `catalog`.
/// Other files will remain in place.
pub fn import(catalog: impl AsRef<Path>, import: impl AsRef<Path>) -> Result<(), String> {
  let catalog = catalog.as_ref();
  let import = import.as_ref();

  if import.starts_with(catalog) {
    return Err("Cannot import into self.".to_string());
  }

  log::info!(
    "{}: Importing into {}.",
    import.display(),
    catalog.display()
  );

  let organizer = Organizer::import(import)?;

  run(organizer, catalog, false)
}

/// Runs `organizer` with output to `catalog`.
fn run(mut organizer: Organizer, catalog: impl AsRef<Path>, force_move: bool) -> Result<(), String> {
  // 1. Remove duplicates and leftovers.

  organizer.remove_live_photo_leftovers()?;
  organizer.remove_live_photo_duplicates()?;
  organizer.remove_sidecar_leftovers()?;

  // 2. Create sidecars for files without.

  organizer.create_missing_sidecars()?;

  // 3. Automatic metadata adjustments.

  organizer.enable_align_mwg_tags();
  organizer.enable_set_copyrights_from_creator();
  organizer.enable_set_location_from_gps();
  organizer.enable_set_time_zone_from_gps();
  organizer.apply_metadata_updates()?;

  // 4. Metadata synchronization across files.

  organizer.sync_live_photo_metadata()?;
  organizer.sync_dupe_metadata()?;
  // organizer.sync_media_metadata()?;

  // 5. Validate metadata.

  organizer.enable_attribution_validation();
  organizer.enable_camera_validation();
  organizer.enable_date_time_validation();
  organizer.enable_location_validation();
  organizer.validate();

  // 6. Move/rename files.

  organizer.move_and_rename_files(catalog, force_move)
}

#[cfg(test)]
mod test_import {
  use super::*;
  use crate::testing::*;

  #[test]
  fn errors_if_importing_into_self() {
    let d = test_dir!(
      "import/image.jpg": {},
    );

    assert_err!(
      import(d.root(), d.get_path("import")),
      "Cannot import into self."
    );
  }
}

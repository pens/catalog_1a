// Copyright 2023-5 Seth Pendergrass. See LICENSE.

//! Organizer Stage 2: Automatic sidecar creation.

use super::Organizer;
use crate::{io, org, prim::SidecarInitial};

impl Organizer {
  /// Creates a new XMP sidecar for any file without one, and loads it into the
  /// organizer for future stages.
  pub fn create_missing_sidecars(&mut self) -> Result<(), String> {
    log::info!("Creating XMP sidecars for media files without.");

    for media in self.media.iter_data_mut() {
      if !media.is_missing_sidecar() {
        continue;
      }

      log::debug!("{media}: Creating XMP sidecar.");

      let metadata = io::create_xmp(org::to_abs_path(
        &self.source,
        &media.get_metadata().source_file,
      ))?;

      let path = metadata.as_ref().to_path_buf();
      self.sidecars.insert(path, SidecarInitial::new(metadata)?);
    }

    Ok(())
  }
}

#[cfg(test)]
mod test_create_missing_sidecars {
  use super::*;
  use crate::testing::*;

  #[test]
  fn copies_metadata_from_media() {
    let d = test_dir!(
      "image.jpg": { "Creator": "Creator" }
    );

    let mut o = Organizer::import(d.root()).unwrap();
    o.create_missing_sidecars().unwrap();

    assert_tag!(d, "image.jpg.xmp", "Creator", "Creator");
  }

  #[test]
  fn creates_sidecar() {
    let d = test_dir!(
      "image.jpg": {},
    );

    let mut o = Organizer::import(d.root()).unwrap();
    o.create_missing_sidecars().unwrap();

    assert_dir!(d, ["image.jpg", "image.jpg.xmp"]);
  }

  #[test]
  fn skips_if_sidecar_already_exists() {
    let d = test_dir!(
      "image.jpg": {},
      "image.jpg.xmp": { "Creator": "Creator" }
    );

    let mut o = Organizer::import(d.root()).unwrap();
    o.create_missing_sidecars().unwrap();

    assert_tag!(d, "image.jpg.xmp", "Creator", "Creator");
  }
}

// Copyright 2023-5 Seth Pendergrass. See LICENSE.

//! Organizer Stage 5: Metadata validation.

use super::Organizer;
use crate::prim::{self, FileMap, Handle, Media, Metadata, Sidecar, SidecarInitial};

/// Stores which validation checks are enabled.
#[allow(clippy::struct_excessive_bools)]
#[derive(Default)]
pub struct ValidationConfig {
  pub attribution: bool,
  pub camera:      bool,
  pub date_time:   bool,
  pub location:    bool,
}

impl ValidationConfig {
  /// If any check is enabled.
  pub fn enabled(&mut self) -> bool {
    self.attribution || self.camera || self.date_time || self.location
  }
}

impl Organizer {
  /// Validates whether attribution tags (e.g. `Creator`) are set as expected.
  pub fn enable_attribution_validation(&mut self) {
    log::info!("Attribution metadata validation enabled.");
    self.validation.attribution = true;
  }

  /// Validates whether camera tags (e.g. `Make`, `Model`) are set as expected.
  pub fn enable_camera_validation(&mut self) {
    log::info!("Camera hardware metadata validation enabled.");
    self.validation.camera = true;
  }

  /// Validates whether date and time tags (e.g. `DateTimeOriginal`) are set as
  /// expected.
  pub fn enable_date_time_validation(&mut self) {
    log::info!("Date and time metadata validation enabled.");
    self.validation.date_time = true;
  }

  /// Validates whether GPS and location tags (e.g. `GPSLatitude`, `City`) are
  /// set as expected.
  pub fn enable_location_validation(&mut self) {
    log::info!("GPS and location metadata validation enabled.");
    self.validation.location = true;
  }

  /// Actually runs validation. This batches all operations enabled via calls to
  /// `enable_*_validation` to reduce the number of calls to `ExifTool`.
  pub fn validate(&mut self) {
    if !self.validation.enabled() {
      return;
    }

    log::info!("Validating metadata.");

    self
      .valid_media
      .extend(validate(&self.media, &self.sidecars, &self.validation));
  }
}

/// Based on supplied `config`, runs validation checks and returns an iterator
/// over the `Handle`s to valid media files.
fn validate<'a>(
  media: &'a FileMap<Media>,
  sidecars: &'a FileMap<SidecarInitial>,
  config: &'a ValidationConfig,
) -> impl Iterator<Item = Handle<Media>> + 'a {
  media
    .iter_data_indexed()
    .map(|(handle_media, media)| {
      (
        handle_media,
        media
          .get_sidecar()
          .map_or(media.get_metadata(), |h| sidecars[h].get_metadata()),
      )
    })
    .filter_map(|(handle_media, metadata)| {
      // Only run each validation if enabled, but make sure all run even if already
      // invalid.
      let mut valid = !config.attribution || validate_attribution(metadata);
      valid = (!config.camera || validate_camera(metadata)) && valid;
      valid = (!config.date_time || validate_date_time(metadata)) && valid;
      valid = (!config.location || validate_location(metadata)) && valid;

      valid.then_some(handle_media)
    })
}

/// Validates attribution tags in `metadata`.
fn validate_attribution(metadata: &Metadata) -> bool {
  let creator = metadata.creator.as_ref().ok_or_else(|| {
    log::warn!("{metadata}: Missing `Creator` tag.");
  });

  let copyright = metadata.copyright.as_ref().ok_or_else(|| {
    log::warn!("{metadata}: Missing `Copyright` tag.");
  });

  let Ok(creator) = creator else {
    return false;
  };

  let Ok(copyright) = copyright else {
    return false;
  };

  if *copyright != format!("Copyright {creator}") {
    log::debug!("{metadata}: Unexpected `Copyright` format (\"{copyright}\").");
  }

  true
}

/// Validates camera tags in `metadata`.
fn validate_camera(metadata: &Metadata) -> bool {
  let mut valid = true;

  if metadata.make.is_none() {
    log::warn!("{metadata}: Missing `Make` tag.");
    valid = false;
  }

  if metadata.model.is_none() {
    log::warn!("{metadata}: Missing `Model` tag.");
    valid = false;
  }

  valid
}

/// Validates date and time tags in `metadata`.
/// This checks that all expected tags are set, as well as their time zones.
fn validate_date_time(metadata: &Metadata) -> bool {
  let date_time_original = metadata
    .date_time_original
    .as_deref()
    .ok_or_else(|| {
      log::warn!("{metadata}: Missing `DateTimeOriginal` tag.");
    })
    .and_then(|_| {
      metadata.get_date_time_original().ok_or_else(|| {
        log::warn!("{metadata}: Unable to parse `DateTimeOriginal` tag.");
      })
    })
    .and_then(|(d, t)| {
      t.and_then(|t| d.and_local_timezone(t).single())
        .ok_or_else(|| {
          log::warn!("{metadata}: `DateTimeOriginal` tag is missing time zone.");
        })
    });

  let create_date = metadata
    .sub_sec_create_date
    .as_deref()
    .or(metadata.create_date.as_deref())
    .ok_or_else(|| {
      log::warn!("{metadata}: Missing `CreateDate` tag.");
    })
    .and_then(|d| {
      prim::parse_date_time(d).map_err(|e| {
        log::warn!("{metadata}: Unable to parse `CreateDate` tag ({e}).");
      })
    })
    .and_then(|(d, t)| {
      t.and_then(|t| d.and_local_timezone(t).single())
        .ok_or_else(|| {
          log::warn!("{metadata}: `CreateDate` tag is missing time zone.");
        })
    });

  let Ok(date_time_original) = date_time_original else {
    return false;
  };

  let Ok(create_date) = create_date else {
    return false;
  };

  if create_date < date_time_original {
    log::warn!(
      "{metadata}: `CreateDate` is before `DateTimeOriginal` ({create_date} < \
       {date_time_original})."
    );
    return false;
  }

  true
}

/// Validates GPS and location tags in `metadata` are set.
fn validate_location(metadata: &Metadata) -> bool {
  let mut valid = true;

  if metadata.gps_position.is_none() {
    log::warn!("{metadata}: Missing `GPSPosition` tag.");
    valid = false;
  }
  if metadata.city.is_none() {
    log::warn!("{metadata}: Missing `City` tag.");
    valid = false;
  }
  if metadata.state.is_none() {
    log::warn!("{metadata}: Missing `State` tag.");
    valid = false;
  }
  if metadata.country.is_none() {
    log::warn!("{metadata}: Missing `Country` tag.");
    valid = false;
  }

  valid
}

#[cfg(test)]
mod test_validation {
  use super::*;
  use crate::{io, testing::*};

  #[test]
  fn validates_media_if_no_sidecar() {
    let d = test_dir!(
      "image.jpg": {
        // Attribution.
        "Creator": "Creator",
        "Copyright": "Copyright",
        // Camera.
        "Make": "Make",
        "Model": "Model",
        // Date & Time.
        "DateTimeOriginal": "2000-01-01T00:00:00",
        "OffsetTimeOriginal": "+00:00",
        "CreateDate": "2000-01-01T00:00:00",
        "OffsetTimeDigitized": "+00:00",
        // Location.
        "GPSLatitude": "47.6061",
        "GPSLatitudeRef": "N",
        "GPSLongitude": "122.3328",
        "GPSLongitudeRef": "W",
        "City": "Seattle",
        "State": "Washington",
        "Country": "United States",
      },
    );

    let mut media = FileMap::new();
    media.insert(
      "image.jpg",
      Media::new(io::read_metadata(d.get_path("image.jpg")).unwrap()).unwrap(),
    );
    let handle_media = media.find("image.jpg").unwrap();

    let sidecars = FileMap::new();

    let config = ValidationConfig {
      attribution: true,
      camera:      true,
      date_time:   true,
      location:    true,
    };
    let valid_handles: Vec<_> = validate(&media, &sidecars, &config).collect();

    assert_eq!(valid_handles, vec![handle_media]);
  }

  #[test]
  fn validates_sidecar() {
    let d = test_dir!(
      "image.jpg": {},
      "image.jpg.xmp": {
        // Attribution.
        "Creator": "Creator",
        "Copyright": "Copyright",
        // Camera.
        "Make": "Make",
        "Model": "Model",
        // Date & Time.
        "DateTimeOriginal": "2000-01-01T00:00:00+00:00",
        "CreateDate": "2000-01-01T00:00:00+00:00",
        // Location.
        "GPSLatitude": "47.6061 N",
        "GPSLongitude": "122.3328 W",
        "City": "Seattle",
        "State": "Washington",
        "Country": "United States",
      },
    );

    let mut media = FileMap::new();
    media.insert(
      "image.jpg",
      Media::new(io::read_metadata(d.get_path("image.jpg")).unwrap()).unwrap(),
    );
    let handle_media = media.find("image.jpg").unwrap();

    let mut sidecars = FileMap::new();
    sidecars.insert(
      "image.jpg.xmp",
      SidecarInitial::new(io::read_metadata(d.get_path("image.jpg.xmp")).unwrap()).unwrap(),
    );
    let handle_sidecar = sidecars.find("image.jpg.xmp").unwrap();

    media
      .get_entry_mut(handle_media)
      .as_mut()
      .unwrap()
      .set_sidecar(handle_sidecar);
    sidecars
      .get_entry_mut(handle_sidecar)
      .as_mut()
      .unwrap()
      .set_media_handle(handle_media);

    let config = ValidationConfig {
      attribution: true,
      camera:      true,
      date_time:   true,
      location:    true,
    };
    let valid_handles: Vec<_> = validate(&media, &sidecars, &config).collect();

    assert_eq!(valid_handles, vec![handle_media]);
  }
}

#[cfg(test)]
mod test_validate_attribution {
  use super::*;
  use crate::testing::*;

  #[test]
  fn is_invalid_if_no_copyright() {
    let metadata = metadata!(
      "Creator": "Creator",
    );

    assert!(!validate_attribution(&metadata));
  }

  #[test]
  fn is_invalid_if_no_creator() {
    let metadata = metadata!(
      "Copyright": "Copyright",
    );

    assert!(!validate_attribution(&metadata));
  }

  #[test]
  fn passes_valid() {
    let metadata = metadata!(
      "Creator": "Creator",
      "Copyright": "Copyright",
    );

    assert!(validate_attribution(&metadata));
  }
}

#[cfg(test)]
mod test_validate_camera {
  use super::*;
  use crate::testing::*;

  #[test]
  fn is_invalid_if_no_make() {
    let metadata = metadata!(
      "Model": "Model",
    );

    assert!(!validate_camera(&metadata));
  }

  #[test]
  fn is_invalid_if_no_model() {
    let metadata = metadata!(
      "Make": "Make",
    );

    assert!(!validate_camera(&metadata));
  }

  #[test]
  fn passes_valid() {
    let metadata = metadata!(
      "Make": "Make",
      "Model": "Model",
    );

    assert!(validate_camera(&metadata));
  }
}

#[cfg(test)]
mod test_validate_date_time {
  use super::*;
  use crate::testing::*;

  #[test]
  fn is_invalid_if_create_date_before_date_time_original() {
    let metadata = metadata!(
      "DateTimeOriginal": "2000-01-01T00:00:01+00:00",
      "CreateDate": "2000-01-01T00:00:00+00:00",
    );

    assert!(!validate_date_time(&metadata));
  }

  #[test]
  fn is_invalid_if_no_create_date() {
    let metadata = metadata!(
      "DateTimeOriginal": "2000-01-01T00:00:00+00:00",
    );

    assert!(!validate_date_time(&metadata));
  }

  #[test]
  fn is_invalid_if_no_date_time_original() {
    let metadata = metadata!(
      "CreateDate": "2000-01-01T00:00:00+00:00",
    );

    assert!(!validate_date_time(&metadata));
  }

  #[test]
  fn is_invalid_if_no_create_date_time_zone() {
    let metadata = metadata!(
      "DateTimeOriginal": "2000-01-01T00:00:00+00:00",
      "CreateDate": "2000-01-01T00:00:00",
    );

    assert!(!validate_date_time(&metadata));
  }

  #[test]
  fn is_invalid_if_no_date_time_original_time_zone() {
    let metadata = metadata!(
      "DateTimeOriginal": "2000-01-01T00:00:00",
      "CreateDate": "2000-01-01T00:00:00+00:00",
    );

    assert!(!validate_date_time(&metadata));
  }

  #[test]
  fn passes_valid_exif() {
    let metadata = metadata!(
      "DateTimeOriginal": "2000-01-01T00:00:00",
      "OffsetTimeOriginal": "+00:00",
      "SubSecDateTimeOriginal": "2000-01-01T00:00:00+00:00",
      "CreateDate": "2000-01-01T00:00:00",
      "OffsetTimeDigitized": "+00:00",
      "SubSecCreateDate": "2000-01-01T00:00:00+00:00",
    );
    assert!(validate_date_time(&metadata));
  }

  #[test]
  fn passes_valid_xmp() {
    let metadata = metadata!(
      "DateTimeOriginal": "2000-01-01T00:00:00+00:00",
      "CreateDate": "2000-01-01T00:00:00+00:00",
    );
    assert!(validate_date_time(&metadata));
  }
}

#[cfg(test)]
mod test_validate_location {
  use super::*;
  use crate::testing::*;

  #[test]
  fn is_invalid_if_no_gps() {
    let metadata = metadata!(
      "City": "Seattle",
      "State": "Washington",
      "Country": "United States",
    );

    assert!(!validate_location(&metadata));
  }

  #[test]
  fn is_invalid_if_no_location() {
    let metadata = metadata!(
      "GPSLatitude": "47.6061",
      "GPSLatitudeRef": "N",
      "GPSLongitude": "122.3328",
      "GPSLongitudeRef": "W",
      "GPSPosition": "47 deg 36' 21.96\" N, 122 deg 19' 58.08\" W",
    );

    assert!(!validate_location(&metadata));
  }

  #[test]
  fn passes_valid_exif() {
    let metadata = metadata!(
      "GPSLatitude": "47.6061",
      "GPSLatitudeRef": "N",
      "GPSLongitude": "122.3328",
      "GPSLongitudeRef": "W",
      "GPSPosition": "47 deg 36' 21.96\" N, 122 deg 19' 58.08\" W",
      "City": "Seattle",
      "State": "Washington",
      "Country": "United States",
    );

    assert!(validate_location(&metadata));
  }

  #[test]
  fn passes_valid_xmp() {
    let metadata = metadata!(
      "GPSLatitude": "47.6061 N",
      "GPSLongitude": "122.3328 W",
      "GPSPosition": "47 deg 36' 21.96\" N, 122 deg 19' 58.08\" W",
      "City": "Seattle",
      "State": "Washington",
      "Country": "United States",
    );

    assert!(validate_location(&metadata));
  }
}

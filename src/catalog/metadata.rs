//! Image / video metadata type.
//!
//! Copyright 2023-4 Seth Pendergrass. See LICENSE.

use serde::Deserialize;
use std::collections::HashSet;
use std::path::PathBuf;

lazy_static! {
    static ref MY_CAMERAS: HashSet<&'static str> = HashSet::from([
        "Canon EOS RP",
        "Canon EOS 100D",
        "D3100",
        "iPhone 12 Mini",
        "iPhone XS",
        "iPad (6th generation)",
        "iPhone X",
        "XT1575",
        "iPad Air",
        "iPhone 6 Plus",
        "iPhone 6",
        "Pixel",
        "iPhone 5",
        "PC36100",
    ]);
}

#[derive(Clone, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct Metadata {
    pub source_file: PathBuf,
    pub file_modify_date: String,
    pub file_type: String,
    pub file_type_extension: String,
    pub content_identifier: Option<String>, // Live Photo images & videos.
    pub create_date: Option<String>,        // Time of image write or photo scan.
    pub date_time_original: Option<String>, // Time of shutter actuation.
    pub artist: Option<String>,
    pub copyright: Option<String>,
    pub gps_altitude: Option<String>,
    pub gps_altitude_ref: Option<String>,
    pub gps_latitude: Option<String>,
    pub gps_latitude_ref: Option<String>,
    pub gps_longitude: Option<String>,
    pub gps_longitude_ref: Option<String>,
    pub make: Option<String>,
    pub model: Option<String>,
}

impl Metadata {
    //
    // Public.
    //

    /// Returns whether the camera model is in the list of cameras I've owned.
    pub fn maybe_my_camera(&self) -> bool {
        self.model
            .as_ref()
            .map(|model| MY_CAMERAS.contains(model.as_str()))
            .unwrap_or(false)
    }

    /// Validates metadata tags.
    pub fn validate_tags(&self) {
        log::debug!("{}: Checking tags.", self.source_file.display());

        // GPS.
        if self.gps_altitude.is_none() {
            log::warn!("{}: GPS altitude not assigned.", self.source_file.display());
        }
        if self.gps_altitude_ref.is_none() {
            log::warn!(
                "{}: GPS altitude reference not assigned.",
                self.source_file.display()
            );
        }
        if self.gps_latitude.is_none() {
            log::warn!("{}: GPS latitude not assigned.", self.source_file.display());
        }
        if self.gps_latitude_ref.is_none() {
            log::warn!(
                "{}: GPS latitude reference not assigned.",
                self.source_file.display()
            );
        }
        if self.gps_longitude.is_none() {
            log::warn!(
                "{}: GPS longitude not assigned.",
                self.source_file.display()
            );
        }
        if self.gps_longitude_ref.is_none() {
            log::warn!(
                "{}: GPS longitude reference not assigned.",
                self.source_file.display()
            );
        }

        // Attribution.
        if self.make.is_none() {
            log::warn!("{}: Make not assigned.", self.source_file.display());
        }
        // Special handling if the camera *could* be mine.
        if self.maybe_my_camera() {
            if self.artist.is_none() {
                log::warn!(
                    "{}: Artist not assigned, and camera could be mine.",
                    self.source_file.display()
                );
            }
            if self.copyright.is_none() {
                log::warn!(
                    "{}: Copyright not assigned, and camera could be mine.",
                    self.source_file.display()
                );
            }
        // If not, Artist & Copyright aren't important.
        } else {
            if self.model.is_none() {
                log::warn!("{}: Model not assigned.", self.source_file.display());
            }
            if self.artist.is_none() {
                log::debug!("{}: Artist not assigned.", self.source_file.display());
            }
            if self.copyright.is_none() {
                log::debug!("{}: Copyright not assigned.", self.source_file.display());
            }
        }

        // Date & Time.
        if self.create_date.is_none() {
            log::warn!(
                "{}: CreateDate (time of digitization) not assigned.",
                self.source_file.display()
            );
        }
        if self.date_time_original.is_none() {
            log::warn!(
                "{}: DateTimeOriginal (time of capture) not assigned.",
                self.source_file.display()
            );
        }
    }
}

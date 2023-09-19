/*
    Constants for tags and expected file extensions.

    Copyright 2023 Seth Pendergrass. See LICENSE.
*/

// Tag name, required, Live Photo value overrides video
pub const TAGS: [(&str, bool, bool); 12] = [
    ("Artist", true, true),
    ("ContentIdentifier", false, false),
    ("Copyright", true, true),
    ("DateTimeOriginal", true, true),
    // https://exiftool.org/TagNames/GPS.html recommends all of the below
    ("GPSLatitude", true, true),
    ("GPSLatitudeRef", true, true),
    ("GPSLongitude", true, true),
    ("GPSLongitudeRef", true, true),
    ("GPSAltitude", true, true),
    ("GPSAltitudeRef", true, true),
    ("MajorBrand", false, false),
    ("MediaGroupUUID", false, false),
];

// Extension, rename, is video?
pub const EXTENSIONS: [(&str, Option<&str>, bool); 7] = [
    ("CR2", None, false),
    ("CR3", None, false),
    ("HEIC", None, false),
    ("jpeg", Some("jpg"), false),
    ("jpg", None, false),
    ("mp4", None, true),
    ("mov", None, true),
];

// MajorBrand, expected extension
pub const FORMATS: [(&str, &str); 0] = [];

/*
    Utilities for parsing the output of `exiftool`.

    Copyright 2023 Seth Pendergrass. See LICENSE.
*/
use std::fmt;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::str::Split;

// TODO could add more detail
#[derive(Debug)]
pub enum ParseError {
    FailedToGetMetadata,
    NoMetadata,
    InvalidFormat,
}

impl fmt::Display for ParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ParseError::FailedToGetMetadata => write!(f, "Failed to get metadata"),
            ParseError::NoMetadata => write!(f, "No metadata"),
            ParseError::InvalidFormat => write!(f, "Invalid format"),
        }
    }
}

/*
    DateTime Handling
*/

#[derive(Clone, Eq, Hash, PartialEq)]
pub struct DateTimeOriginal {
    pub year: String,
    pub month: String,
    pub day: String,
    pub hour: String,
    pub minute: String,
    pub second: String,
}

impl DateTimeOriginal {
    // Attempt to parse datetime string from exiftool into a DateTimeOriginal.
    fn from_string(date_time_string: &str) -> Result<Self, ParseError> {
        let mut iter = date_time_string.split(' ');

        let get_next = |iter: &mut Split<'_, char>, len| -> Result<String, ParseError> {
            let string = iter.next().ok_or(ParseError::InvalidFormat)?;
            if string.is_ascii() && string.len() == len {
                Ok(string.to_owned())
            } else {
                Err(ParseError::InvalidFormat)
            }
        };

        let year = get_next(&mut iter, 4)?;
        let month = get_next(&mut iter, 2)?;
        let day = get_next(&mut iter, 2)?;
        let hour = get_next(&mut iter, 2)?;
        let minute = get_next(&mut iter, 2)?;
        let second = get_next(&mut iter, 2)?;

        // Sanity check: there should be no more fields
        if iter.next().is_some() {
            return Err(ParseError::InvalidFormat);
        }

        Ok(DateTimeOriginal {
            year,
            month,
            day,
            hour,
            minute,
            second,
        })
    }

    // Returns expected path formatting (excluding the file extension) for this datetime.
    // Currently this is YYYY/MM/YYMMDD_HHMMSS.
    pub fn get_path_format(&self) -> String {
        // TODO how would this be error checked?
        format!(
            "{0}/{1}/{2}{1}{3}_{4}{5}{6}",
            self.year,
            self.month,
            &self.year[2..4],
            self.day,
            self.hour,
            self.minute,
            self.second
        )
    }
}

/*
    Per-File Metadata
*/

#[derive(Clone, Default, Eq, Hash, PartialEq)]
pub struct Metadata {
    pub path: PathBuf,
    pub artist: Option<String>,
    pub content_identifier: Option<String>,
    pub copyright: Option<String>,
    pub date_time_original: Option<DateTimeOriginal>,
    // https://exiftool.org/TagNames/GPS.html recommends all of the below
    pub gps_latitude: Option<String>,
    pub gps_latitude_ref: Option<String>,
    pub gps_longitude: Option<String>,
    pub gps_longitude_ref: Option<String>,
    pub gps_altitude: Option<String>,
    pub gps_altitude_ref: Option<String>,
    pub major_brand: Option<String>,
    pub media_group_uuid: Option<String>,
}

/*
    Metadata Parsing
*/

// Use `exiftool` to extract metadata from a file.
pub fn parse_metadata(path: &Path) -> Result<Metadata, ParseError> {
    let output = Command::new("exiftool")
        .arg("-d")
        .arg("%Y %m %d %H %M %S")
        .arg("-f")
        .arg("-q")
        .arg("-s")
        .arg("-t")
        .arg("-Artist")
        .arg("-ContentIdentifier")
        .arg("-Copyright")
        .arg("-DateTimeOriginal")
        .arg("-GPSLatitude")
        .arg("-GPSLatitudeRef")
        .arg("-GPSLongitude")
        .arg("-GPSLongitudeRef")
        .arg("-GPSAltitude")
        .arg("-GPSAltitudeRef")
        .arg("-MajorBrand")
        .arg("-MediaGroupUUID")
        .arg(path)
        .output();

    let Ok(output) = output else {
        return Err(ParseError::FailedToGetMetadata);
    };
    if !output.status.success() {
        return Err(ParseError::FailedToGetMetadata);
    }
    if output.stdout.is_empty() {
        return Err(ParseError::NoMetadata);
    }
    let Ok(stdout) = String::from_utf8(output.stdout) else {
        return Err(ParseError::NoMetadata);
    };

    let mut metadata = Metadata {
        path: path.to_owned(),
        ..Default::default()
    };

    log::trace!("{}:\n{}", path.display(), stdout);

    for line in stdout.lines() {
        let mut iter = line.split('\t');
        let tag = iter.next().ok_or(ParseError::InvalidFormat)?;
        let value = iter.next().ok_or(ParseError::InvalidFormat)?;

        // Sanity check: format should be exactly `tag\tvalue`.
        if iter.next().is_some() {
            return Err(ParseError::InvalidFormat);
        }

        // exiftool's default value for missing tags is '-'.
        if value != "-" {
            match tag {
                "Artist" => metadata.artist = Some(value.to_owned()),
                "ContentIdentifier" => metadata.content_identifier = Some(value.to_owned()),
                "Copyright" => metadata.copyright = Some(value.to_owned()),
                "DateTimeOriginal" => metadata.date_time_original = Some(DateTimeOriginal::from_string(value)?),
                "GPSLatitude" => metadata.gps_latitude = Some(value.to_owned()),
                "GPSLatitudeRef" => metadata.gps_latitude_ref = Some(value.to_owned()),
                "GPSLongitude" => metadata.gps_longitude = Some(value.to_owned()),
                "GPSLongitudeRef" => metadata.gps_longitude_ref = Some(value.to_owned()),
                "GPSAltitude" => metadata.gps_altitude = Some(value.to_owned()),
                "GPSAltitudeRef" => metadata.gps_altitude_ref = Some(value.to_owned()),
                "MajorBrand" => metadata.major_brand = Some(value.to_owned()),
                "MediaGroupUUID" => metadata.media_group_uuid = Some(value.to_owned()),
                _ => return Err(ParseError::InvalidFormat),
            }
        }
    }

    Ok(metadata)
}

use chrono::{DateTime, FixedOffset, Local, NaiveDateTime, Offset, TimeZone};
use chrono_tz::Tz;
use regex::Regex;

/// Converts degrees, minutes and seconds to latitude and longitude.
pub fn dms_to_lat_lon(deg: f32, min: f32, sec: f32) -> f32 {
  deg + (min / 60.0) + (sec / 3600.0)
}

/// Determines the time zone offset at a given date and time, within the named
/// time zone.
pub fn get_offset_for_time_zone(date_time: &NaiveDateTime, time_zone: &str) -> FixedOffset {
  time_zone
    .parse::<Tz>()
    .unwrap()
    .offset_from_local_datetime(date_time)
    .unwrap()
    .fix()
}

/// Gets the `FixedOffset` for the computer's time zone at a given date & time.
pub fn get_offset_local(date_time: &NaiveDateTime) -> FixedOffset {
  *Local.from_local_datetime(date_time).unwrap().offset()
}

/// Converts a date & time string to a `NaiveDateTime` and an optional
/// `FixedOffset`. Assumes RFC3339 format, but optionally without a time zone
/// offset.
pub fn parse_date_time(date_time: &str) -> Result<(NaiveDateTime, Option<FixedOffset>), String> {
  let date_time = date_time.to_string();

  let re =
    Regex::new(r"^(\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}(?:.\d{1,3})?)([+-]\d{2}:\d{2})?$").unwrap();

  let caps = re.captures(&date_time).ok_or(format!(
    "Date Time string `{date_time}` did not match regex."
  ))?;

  // If a time zone is present.
  if caps.get(2).is_some() {
    let date_time_parsed = DateTime::parse_from_rfc3339(caps.get(0).unwrap().as_str())
      .map_err(|e| format!("Unable to parse date & time `{date_time}` ({e})"))?;
    return Ok((
      date_time_parsed.naive_local(),
      Some(*date_time_parsed.offset()),
    ));
  }

  NaiveDateTime::parse_from_str(caps.get(1).unwrap().as_str(), "%Y-%m-%dT%H:%M:%S%.f")
    .map_err(|e| format!("Unable to parse date & time `{date_time}` ({e})."))
    .map(|d| (d, None))
}

#[cfg(test)]
mod test_get_offset_for_time_zone {
  use chrono::NaiveDate;

  use super::*;

  #[test]
  fn returns_pdt_before_spring_clock_change() {
    let date = NaiveDate::from_ymd_opt(2025, 3, 9)
      .and_then(|d| d.and_hms_opt(1, 59, 59))
      .unwrap();

    let offset = get_offset_for_time_zone(&date, "America/Los_Angeles");

    assert_eq!(offset, FixedOffset::east_opt(-8 * 3600).unwrap());
  }

  #[test]
  fn returns_pst_after_spring_clock_change() {
    let date = NaiveDate::from_ymd_opt(2025, 3, 9)
      .and_then(|d| d.and_hms_opt(3, 0, 0))
      .unwrap();

    let offset = get_offset_for_time_zone(&date, "America/Los_Angeles");

    assert_eq!(offset, FixedOffset::east_opt(-7 * 3600).unwrap());
  }
}

#[cfg(test)]
mod test_parse_date_time {
  use chrono::FixedOffset;

  use super::*;
  use crate::testing::*;

  #[test]
  fn parses_string_without_subseconds_or_time_zone() {
    let date_time = "2000-01-01T00:00:00";

    let parsed = parse_date_time(date_time).unwrap();

    assert_eq!(parsed.0, make_date_naive(2000, 1, 1, 0, 0, 0, 0));
    assert!(parsed.1.is_none());
  }

  #[test]
  fn parses_string_with_subseconds_and_time_zone() {
    let date_time = "2000-01-01T00:00:00.999-08:00";

    let parsed = parse_date_time(date_time).unwrap();

    assert_eq!(parsed.0, make_date_naive(2000, 1, 1, 0, 0, 0, 999));
    assert_eq!(parsed.1.unwrap(), FixedOffset::east_opt(-8 * 3600).unwrap());
  }

  #[test]
  fn parses_string_with_subseconds_without_time_zone() {
    let date_time = "2000-01-01T00:00:00.999";

    let parsed = parse_date_time(date_time).unwrap();

    assert_eq!(parsed.0, make_date_naive(2000, 1, 1, 0, 0, 0, 999));
    assert!(parsed.1.is_none());
  }

  #[test]
  fn parses_string_without_subseconds_with_time_zone() {
    let date_time = "2000-01-01T00:00:00-08:00";

    let parsed = parse_date_time(date_time).unwrap();

    assert_eq!(parsed.0, make_date_naive(2000, 1, 1, 0, 0, 0, 0));
    assert_eq!(parsed.1.unwrap(), FixedOffset::east_opt(-8 * 3600).unwrap());
  }
}

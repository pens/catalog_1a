use chrono::{DateTime, FixedOffset, NaiveDate, NaiveDateTime};

use crate::prim;

pub fn make_date(
  year: i32,
  month: u32,
  day: u32,
  hour: u32,
  min: u32,
  sec: u32,
  sec_frac: u32,
  tz: i32,
) -> DateTime<FixedOffset> {
  let date_time =
    make_date_internal(year, month, day, hour, min, sec, sec_frac).unwrap_or_else(|| {
      panic!("Invalid date & time: {year}-{month}-{day}T{hour}:{min}:{sec}.{sec_frac}{tz}")
    });

  date_time
    .and_local_timezone(FixedOffset::east_opt(tz * 3600).unwrap())
    .unwrap()
}

pub fn make_date_local(
  year: i32,
  month: u32,
  day: u32,
  hour: u32,
  min: u32,
  sec: u32,
  sec_frac: u32,
) -> DateTime<FixedOffset> {
  let date_time =
    make_date_internal(year, month, day, hour, min, sec, sec_frac).unwrap_or_else(|| {
      panic!("Invalid local date & time: {year}-{month}-{day}T{hour}:{min}:{sec}.{sec_frac}")
    });

  let offset = prim::get_offset_local(&date_time);

  date_time.and_local_timezone(offset).unwrap()
}

pub fn make_date_naive(
  year: i32,
  month: u32,
  day: u32,
  hour: u32,
  min: u32,
  sec: u32,
  sec_frac: u32,
) -> NaiveDateTime {
  make_date_internal(year, month, day, hour, min, sec, sec_frac).unwrap_or_else(|| {
    panic!("Invalid naive date & time: {year}-{month}-{day}T{hour}:{min}:{sec}.{sec_frac}")
  })
}

fn make_date_internal(
  year: i32,
  month: u32,
  day: u32,
  hour: u32,
  min: u32,
  sec: u32,
  mut sec_frac: u32,
) -> Option<NaiveDateTime> {
  let nano = if sec_frac == 0 {
    0
  } else {
    while sec_frac < 100_000_000 {
      sec_frac *= 10;
    }
    sec_frac
  };

  NaiveDate::from_ymd_opt(year, month, day).and_then(|d| d.and_hms_nano_opt(hour, min, sec, nano))
}

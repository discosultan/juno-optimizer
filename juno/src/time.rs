use chrono::prelude::*;
use once_cell::sync::Lazy;
use regex::Regex;
use std::{
    collections::HashMap,
    time::{SystemTime, UNIX_EPOCH},
};

use crate::math::{ceil_multiple, ceil_multiple_offset, floor_multiple, floor_multiple_offset};

pub const SEC_MS: u64 = 1000;
pub const MIN_MS: u64 = 60_000;
pub const HOUR_MS: u64 = 3_600_000;
pub const DAY_MS: u64 = 86_400_000;
pub const WEEK_MS: u64 = 604_800_000;
pub const MONTH_MS: u64 = 2_629_746_000;
pub const YEAR_MS: u64 = 31_556_952_000;

const WEEK_OFFSET_MS: u64 = 345_600_000;

// Interval.

// Is assumed to be ordered by values descending.
const INTERVAL_FACTORS: [(&str, u64); 8] = [
    ("y", YEAR_MS),
    ("M", MONTH_MS),
    ("w", WEEK_MS),
    ("d", DAY_MS),
    ("h", HOUR_MS),
    ("m", MIN_MS),
    ("s", SEC_MS),
    ("ms", 1),
];

static INTERVAL_FACTOR_MAP: Lazy<HashMap<&'static str, u64>> =
    Lazy::new(|| INTERVAL_FACTORS.iter().cloned().collect());

static INTERVAL_GROUP_RE: Lazy<Regex> = Lazy::new(|| Regex::new(r"(\d+[a-zA-Z]+)").unwrap());

fn str_to_interval(representation: &str) -> u64 {
    INTERVAL_GROUP_RE
        .find_iter(representation)
        .fold(0, |acc, group| acc + calc_interval_group(group.as_str()))
}

fn calc_interval_group(group: &str) -> u64 {
    for (i, c) in group.chars().enumerate() {
        if c.is_alphabetic() {
            return group[0..i].parse::<u64>().unwrap() * INTERVAL_FACTOR_MAP[&group[i..]];
        }
    }
    panic!("Invalid interval group: {}", group);
}

fn interval_to_string(value: u64) -> String {
    let mut result = String::new();
    let mut remainder = value;
    for (letter, factor) in INTERVAL_FACTORS.iter() {
        let quotient = remainder / factor;
        remainder %= factor;
        if quotient > 0 {
            result.push_str(&format!("{}{}", quotient, letter));
        }
        if remainder == 0 {
            break;
        }
    }
    if result.is_empty() {
        result.push_str("0ms");
    }
    result
}

pub trait IntervalStrExt {
    fn to_interval(&self) -> u64;
}

impl IntervalStrExt for str {
    fn to_interval(&self) -> u64 {
        str_to_interval(self)
    }
}

pub trait IntervalIntExt {
    fn to_interval_repr(self) -> String;
}

impl IntervalIntExt for u64 {
    fn to_interval_repr(self) -> String {
        interval_to_string(self)
    }
}

// Timestamp.

pub fn timestamp() -> u64 {
    let start = SystemTime::now();
    let since_the_epoch = start
        .duration_since(UNIX_EPOCH)
        .expect("duration since epoch");
    since_the_epoch.as_secs() * 1000 + u64::from(since_the_epoch.subsec_nanos()) / 1_000_000
}

fn str_to_timestamp(representation: &str) -> u64 {
    Err(())
        .or_else(|_| {
            representation
                .parse::<DateTime<Utc>>()
                .map(|x| x.timestamp() as u64 * 1000 + u64::from(x.timestamp_subsec_millis()))
        })
        .or_else(|_| {
            representation
                .parse::<NaiveDateTime>()
                .map(|x| x.timestamp() as u64 * 1000 + u64::from(x.timestamp_subsec_millis()))
        })
        .or_else(|_| {
            representation
                .parse::<NaiveDate>()
                .map(|x| x.and_hms(0, 0, 0).timestamp() as u64 * 1000)
        })
        .expect("parsed timestamp")
}

fn timestamp_to_string(value: u64) -> String {
    let datetime = Utc.timestamp_millis(value as i64);
    // datetime.to_rfc3339()
    datetime.format("%Y-%m-%dT%H:%M:%S%:z").to_string()
}

fn datetime_timestamp_ms(dt: DateTime<Utc>) -> u64 {
    dt.timestamp_millis() as u64
}

fn datetime_utcfromtimestamp_ms(timestamp: u64) -> DateTime<Utc> {
    Utc.timestamp_millis(timestamp as i64)
}

fn floor_timestamp(timestamp: u64, interval: u64) -> u64 {
    if interval < WEEK_MS {
        return floor_multiple(timestamp, interval);
    }
    if interval == WEEK_MS {
        return floor_multiple_offset(timestamp, interval, WEEK_OFFSET_MS);
    }
    if interval == MONTH_MS {
        let dt = datetime_utcfromtimestamp_ms(timestamp);
        return datetime_timestamp_ms(
            dt.date()
                .with_day(1)
                .unwrap()
                .and_time(NaiveTime::from_hms(0, 0, 0))
                .unwrap(),
        );
    }
    unimplemented!();
}

fn ceil_timestamp(timestamp: u64, interval: u64) -> u64 {
    if interval < WEEK_MS {
        return ceil_multiple(timestamp, interval);
    }
    if interval == WEEK_MS {
        return ceil_multiple_offset(timestamp, interval, WEEK_OFFSET_MS);
    }
    if interval == MONTH_MS {
        let dt = datetime_utcfromtimestamp_ms(timestamp);
        return datetime_timestamp_ms(DateTime::<Utc>::from_utc(
            NaiveDate::from_ymd(dt.year(), dt.month() + 1, 1).and_hms(0, 0, 0),
            Utc,
        ));
    }
    unimplemented!();
}

pub trait TimestampStrExt {
    fn to_timestamp(&self) -> u64;
}

impl TimestampStrExt for str {
    fn to_timestamp(&self) -> u64 {
        str_to_timestamp(self)
    }
}

pub trait TimestampIntExt {
    fn to_timestamp_repr(&self) -> String;
    fn ceil_timestamp(&self, interval: u64) -> Self;
    fn floor_timestamp(&self, interval: u64) -> Self;
}

impl TimestampIntExt for u64 {
    fn to_timestamp_repr(&self) -> String {
        timestamp_to_string(*self)
    }

    fn ceil_timestamp(&self, interval: u64) -> Self {
        ceil_timestamp(*self, interval)
    }

    fn floor_timestamp(&self, interval: u64) -> Self {
        floor_timestamp(*self, interval)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_interval_to_repr() {
        assert_eq!((DAY_MS * 2).to_interval_repr(), "2d");
        assert_eq!(123.to_interval_repr(), "123ms");
        assert_eq!(1234.to_interval_repr(), "1s234ms");
        assert_eq!(0.to_interval_repr(), "0ms");
    }

    #[test]
    fn test_interval_from_repr() {
        assert_eq!("1d".to_interval(), DAY_MS);
        assert_eq!("2d".to_interval(), DAY_MS * 2);
        assert_eq!("1s1ms".to_interval(), SEC_MS + 1);
        assert_eq!("1m1s".to_interval(), MIN_MS + SEC_MS);
    }

    #[test]
    fn test_timestamp_to_repr() {
        assert_eq!(
            1546300800000.to_timestamp_repr(),
            "2019-01-01T00:00:00+00:00"
        );
    }

    #[test]
    fn test_timestamp_from_repr() {
        assert_eq!("2019-01-01".to_timestamp(), 1546300800000);
    }

    #[test]
    fn test_ceil_timestamp() {
        assert_eq!(1.ceil_timestamp(SEC_MS), SEC_MS);
        assert_eq!(1001.ceil_timestamp(DAY_MS), DAY_MS);
        // "2020-01-01T00:00:00Z" -> 2020-01-06T00:00:00Z
        assert_eq!(1577836800000.ceil_timestamp(WEEK_MS), 1578268800000);
        // 2020-01-02T00:00:00Z -> 2020-02-01T00:00:00Z
        assert_eq!(1577923200000.ceil_timestamp(MONTH_MS), 1580515200000);
    }

    #[test]
    fn test_floor_timestamp() {
        assert_eq!(1.floor_timestamp(SEC_MS), 0);
        assert_eq!(1001.floor_timestamp(DAY_MS), 0);
        // 2020-01-01T00:00:00Z -> 2019-12-30T00:00:00Z
        assert_eq!(1577836800000.floor_timestamp(WEEK_MS), 1577664000000);
        // 2020-01-02T00:00:00Z -> 2020-01-01T00:00:00Z
        assert_eq!(1577923200000.floor_timestamp(MONTH_MS), 1577836800000);
    }
}

use ::time::{
    format_description::{self, well_known::Rfc3339, FormatItem},
    Date, Month, OffsetDateTime, UtcOffset,
};
use once_cell::sync::Lazy;
use serde::{de, Deserialize, Deserializer, Serialize};
use std::{
    fmt,
    ops::{Add, Rem, Sub},
    str::FromStr,
    time,
};

use crate::math::{ceil_multiple, ceil_multiple_offset, floor_multiple, floor_multiple_offset};

use super::Interval;

const WEEK_OFFSET_MS: u64 = 345_600_000;

static DATE_FORMAT: Lazy<Vec<FormatItem<'_>>> =
    Lazy::new(|| format_description::parse("[year]-[month]-[day]").unwrap());

fn datetime_timestamp_ms(dt: OffsetDateTime) -> u64 {
    (dt.unix_timestamp_nanos() / 1_000_000) as u64
}

fn datetime_utcfromtimestamp_ms(timestamp: u64) -> OffsetDateTime {
    OffsetDateTime::from_unix_timestamp_nanos(timestamp as i128 * 1_000_000).unwrap()
}

#[derive(Debug, Default, Clone, Copy, Hash, PartialEq, Eq, PartialOrd, Ord, Serialize)]
pub struct Timestamp(pub u64);

impl Timestamp {
    pub fn now() -> Self {
        let start = time::SystemTime::now();
        let since_the_epoch = start
            .duration_since(time::UNIX_EPOCH)
            .expect("duration since epoch");
        Self(
            since_the_epoch.as_secs() * 1000
                + u64::from(since_the_epoch.subsec_nanos()) / 1_000_000,
        )
    }

    pub fn floor(&self, interval: Interval) -> Self {
        if interval < Interval::WEEK_MS {
            return Self(floor_multiple(self.0, interval.0));
        }
        if interval == Interval::WEEK_MS {
            return Self(floor_multiple_offset(self.0, interval.0, WEEK_OFFSET_MS));
        }
        if interval == Interval::MONTH_MS {
            let dt = datetime_utcfromtimestamp_ms(self.0);
            let (year, month, _) = dt.to_calendar_date();
            return Self(datetime_timestamp_ms(
                Date::from_calendar_date(year, month, 1)
                    .unwrap()
                    .with_hms(0, 0, 0)
                    .unwrap()
                    .assume_offset(UtcOffset::UTC),
            ));
        }
        unimplemented!();
    }

    pub fn ceil(&self, interval: Interval) -> Self {
        if interval < Interval::WEEK_MS {
            return Self(ceil_multiple(self.0, interval.0));
        }
        if interval == Interval::WEEK_MS {
            return Self(ceil_multiple_offset(self.0, interval.0, WEEK_OFFSET_MS));
        }
        if interval == Interval::MONTH_MS {
            let dt = datetime_utcfromtimestamp_ms(self.0);
            let (mut year, mut month, _) = dt.to_calendar_date();
            if month == Month::December {
                year += 1;
            }
            month = month.next();
            return Self(datetime_timestamp_ms(
                Date::from_calendar_date(year, month, 1)
                    .unwrap()
                    .with_hms(0, 0, 0)
                    .unwrap()
                    .assume_offset(UtcOffset::UTC),
            ));
        }
        unimplemented!();
    }
}

impl fmt::Display for Timestamp {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let datetime =
            OffsetDateTime::from_unix_timestamp_nanos(self.0 as i128 * 1_000_000).unwrap();
        datetime.format(&Rfc3339).unwrap().fmt(f)
    }
}

impl FromStr for Timestamp {
    type Err = super::ParseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Err(())
            .or_else(|_| {
                OffsetDateTime::parse(s, &Rfc3339)
                    .map(|x| (x.unix_timestamp_nanos() / 1_000_000) as u64)
            })
            .or_else(|_| {
                Date::parse(s, &DATE_FORMAT).map(|x| {
                    (x.with_hms(0, 0, 0)
                        .unwrap()
                        .assume_offset(UtcOffset::UTC)
                        .unix_timestamp_nanos()
                        / 1_000_000) as u64
                })
            })
            .map(Self)
            .map_err(|_| Self::Err {})
    }
}

impl From<u64> for Timestamp {
    fn from(v: u64) -> Self {
        Self(v)
    }
}

impl From<Timestamp> for u64 {
    fn from(v: Timestamp) -> u64 {
        v.0
    }
}

impl Add<Interval> for Timestamp {
    type Output = Self;

    fn add(self, rhs: Interval) -> Self::Output {
        Self(self.0 + rhs.0)
    }
}

impl Sub for Timestamp {
    type Output = Interval;

    fn sub(self, rhs: Self) -> Self::Output {
        Interval(self.0 - rhs.0)
    }
}

impl Rem<Interval> for Timestamp {
    type Output = Self;

    fn rem(self, rhs: Interval) -> Self::Output {
        Self(self.0 % rhs.0)
    }
}

impl PartialEq<u64> for Timestamp {
    fn eq(&self, other: &u64) -> bool {
        self.0 == *other
    }
}

struct TimestampVisitor;

impl<'de> de::Visitor<'de> for TimestampVisitor {
    type Value = Timestamp;

    fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        formatter.write_str("an integer or a string")
    }

    fn visit_u64<E>(self, v: u64) -> Result<Self::Value, E>
    where
        E: de::Error,
    {
        Ok(Timestamp(v))
    }

    fn visit_str<E>(self, v: &str) -> Result<Self::Value, E>
    where
        E: de::Error,
    {
        Timestamp::from_str(v).map_err(|_| E::invalid_value(de::Unexpected::Str(v), &self))
    }
}

impl<'de> Deserialize<'de> for Timestamp {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        deserializer.deserialize_any(TimestampVisitor)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_timestamp_to_repr() {
        assert_eq!(
            Timestamp(1_546_300_800_000).to_string(),
            "2019-01-01T00:00:00Z"
        );
    }

    #[test]
    fn test_timestamp_from_repr() {
        assert_eq!(
            Timestamp::from_str("2019-01-01"),
            Ok(Timestamp(1_546_300_800_000))
        );
        assert_eq!(
            Timestamp::from_str("2019-01-01T00:00:00Z"),
            Ok(Timestamp(1_546_300_800_000))
        );
        assert_eq!(
            Timestamp::from_str("2019-01-01T00:00:00+00:00"),
            Ok(Timestamp(1_546_300_800_000))
        );
    }

    #[test]
    fn test_ceil_timestamp() {
        assert_eq!(
            Timestamp(1).ceil(Interval::SEC_MS),
            Timestamp(Interval::SEC_MS.0)
        );
        assert_eq!(
            Timestamp(1001).ceil(Interval::DAY_MS),
            Timestamp(Interval::DAY_MS.0)
        );
        // "2020-01-01T00:00:00Z" -> 2020-01-06T00:00:00Z
        assert_eq!(
            Timestamp(1_577_836_800_000).ceil(Interval::WEEK_MS),
            Timestamp(1_578_268_800_000)
        );
        // 2020-01-02T00:00:00Z -> 2020-02-01T00:00:00Z
        assert_eq!(
            Timestamp(1_577_923_200_000).ceil(Interval::MONTH_MS),
            Timestamp(1_580_515_200_000)
        );
    }

    #[test]
    fn test_floor_timestamp() {
        assert_eq!(Timestamp(1).floor(Interval::SEC_MS), Timestamp(0));
        assert_eq!(Timestamp(1001).floor(Interval::DAY_MS), Timestamp(0));
        // 2020-01-01T00:00:00Z -> 2019-12-30T00:00:00Z
        assert_eq!(
            Timestamp(1_577_836_800_000).floor(Interval::WEEK_MS),
            Timestamp(1_577_664_000_000)
        );
        // 2020-01-02T00:00:00Z -> 2020-01-01T00:00:00Z
        assert_eq!(
            Timestamp(1_577_923_200_000).floor(Interval::MONTH_MS),
            Timestamp(1_577_836_800_000)
        );
    }
}

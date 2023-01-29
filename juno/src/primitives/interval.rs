use once_cell::sync::Lazy;
use regex::Regex;
use serde::{de, Deserialize, Deserializer, Serialize};
use std::{
    collections::HashMap,
    fmt,
    ops::{Add, AddAssign, Mul},
    str::FromStr,
};

use crate::math::ceil_multiple;

const SEC_MS: u64 = 1000;
const MIN_MS: u64 = 60_000;
const HOUR_MS: u64 = 3_600_000;
const DAY_MS: u64 = 86_400_000;
const WEEK_MS: u64 = 604_800_000;
const MONTH_MS: u64 = 2_629_746_000;
const YEAR_MS: u64 = 31_556_952_000;

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

fn calc_interval_group(group: &str) -> u64 {
    for (i, c) in group.chars().enumerate() {
        if c.is_alphabetic() {
            return group[0..i].parse::<u64>().unwrap() * INTERVAL_FACTOR_MAP[&group[i..]];
        }
    }
    panic!("Invalid interval group: {group}");
}

#[derive(Debug, Default, Clone, Copy, Hash, PartialEq, Eq, PartialOrd, Ord, Serialize)]
pub struct Interval(pub u64);

impl Interval {
    pub const SEC_MS: Interval = Interval(SEC_MS);
    pub const MIN_MS: Interval = Interval(MIN_MS);
    pub const HOUR_MS: Interval = Interval(HOUR_MS);
    pub const DAY_MS: Interval = Interval(DAY_MS);
    pub const WEEK_MS: Interval = Interval(WEEK_MS);
    pub const MONTH_MS: Interval = Interval(MONTH_MS);
    pub const YEAR_MS: Interval = Interval(YEAR_MS);

    pub fn ceil(&self, interval: Interval) -> Self {
        Self(ceil_multiple(self.0, interval.0))
    }
}

impl fmt::Display for Interval {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut result = String::new();
        let mut remainder = self.0;
        for (letter, factor) in INTERVAL_FACTORS.iter() {
            let quotient = remainder / factor;
            remainder %= factor;
            if quotient > 0 {
                result.push_str(&quotient.to_string());
                result.push_str(letter);
            }
            if remainder == 0 {
                break;
            }
        }
        if result.is_empty() {
            result.push_str("0ms");
        }
        f.write_str(&result)
    }
}

impl FromStr for Interval {
    type Err = super::ParseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(Self(
            INTERVAL_GROUP_RE
                .find_iter(s)
                .fold(0, |acc, group| acc + calc_interval_group(group.as_str())),
        ))
    }
}

impl From<u64> for Interval {
    fn from(v: u64) -> Self {
        Self(v)
    }
}

impl From<Interval> for u64 {
    fn from(v: Interval) -> u64 {
        v.0
    }
}

impl Add for Interval {
    type Output = Interval;

    fn add(self, rhs: Self) -> Self::Output {
        Self(self.0 + rhs.0)
    }
}

impl Add<u64> for Interval {
    type Output = Interval;

    fn add(self, rhs: u64) -> Self::Output {
        Self(self.0 + rhs)
    }
}

impl Add<Interval> for u64 {
    type Output = Interval;

    fn add(self, rhs: Interval) -> Self::Output {
        rhs + self
    }
}

impl AddAssign for Interval {
    fn add_assign(&mut self, rhs: Self) {
        self.0 += rhs.0
    }
}

impl Mul<u64> for Interval {
    type Output = Interval;

    fn mul(self, rhs: u64) -> Self::Output {
        Self(self.0 * rhs)
    }
}

impl Mul<Interval> for u64 {
    type Output = Interval;

    fn mul(self, rhs: Interval) -> Self::Output {
        rhs * self
    }
}

impl PartialEq<u64> for Interval {
    fn eq(&self, other: &u64) -> bool {
        self.0 == *other
    }
}

struct IntervalVisitor;

impl<'de> de::Visitor<'de> for IntervalVisitor {
    type Value = Interval;

    fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        formatter.write_str("an integer or a string")
    }

    fn visit_u64<E>(self, v: u64) -> Result<Self::Value, E>
    where
        E: de::Error,
    {
        Ok(Interval(v))
    }

    fn visit_str<E>(self, v: &str) -> Result<Self::Value, E>
    where
        E: de::Error,
    {
        Interval::from_str(v).map_err(|_| E::invalid_value(de::Unexpected::Str(v), &self))
    }
}

impl<'de> Deserialize<'de> for Interval {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        deserializer.deserialize_any(IntervalVisitor)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::str::FromStr;

    #[test]
    fn test_interval_to_repr() {
        assert_eq!((2 * Interval::DAY_MS).to_string(), "2d");
        assert_eq!(Interval(123).to_string(), "123ms");
        assert_eq!(Interval(1234).to_string(), "1s234ms");
        assert_eq!(Interval(0).to_string(), "0ms");
    }

    #[test]
    fn test_interval_from_repr() {
        assert_eq!(Interval::from_str("1d"), Ok(Interval::DAY_MS));
        assert_eq!(Interval::from_str("2d"), Ok(Interval::DAY_MS * 2));
        assert_eq!(Interval::from_str("1s1ms"), Ok(Interval::SEC_MS + 1));
        assert_eq!(
            Interval::from_str("1m1s"),
            Ok(Interval::MIN_MS + Interval::SEC_MS)
        );
    }
}

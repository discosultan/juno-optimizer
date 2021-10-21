use crate::time::{deserialize_timestamp, IntervalIntExt, TimestampIntExt};
use serde::{Deserialize, Serialize};
use std::ops::AddAssign;
use thiserror::Error;

type Result<T> = std::result::Result<T, Error>;

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq)]
pub struct Candle {
    #[serde(deserialize_with = "deserialize_timestamp")]
    pub time: u64,
    pub open: f64,
    pub high: f64,
    pub low: f64,
    pub close: f64,
    pub volume: f64,
}

impl AddAssign<&Candle> for Candle {
    fn add_assign(&mut self, other: &Self) {
        self.high = f64::max(self.high, other.high);
        self.low = f64::min(self.low, other.low);
        self.close = other.close;
        self.volume += other.volume;
    }
}

#[derive(Error, Debug)]
pub enum Error {
    #[error(
        "missing {exchange} {symbol} {} candle(s) from the start of the period; cannot fill; start {}, current {}",
        .interval.to_interval_repr(),
        .start.to_timestamp_repr(),
        .current.to_timestamp_repr()
    )]
    MissingStartCandles {
        exchange: String,
        symbol: String,
        interval: u64,
        start: u64,
        current: u64,
    },
    #[error(
        "missing {exchange} {symbol} {} candle(s) from the end of the period; cannot fill; current {}, end {}",
        .interval.to_interval_repr(),
        .current.to_timestamp_repr(),
        .end.to_timestamp_repr()
    )]
    MissingEndCandles {
        exchange: String,
        symbol: String,
        interval: u64,
        current: u64,
        end: u64,
    },
    #[error(transparent)]
    Reqwest(#[from] reqwest::Error),
}

pub async fn list_candles(
    exchange: &str,
    symbol: &str,
    interval: u64,
    start: u64,
    end: u64,
    fill_missing_with_last: bool,
) -> Result<Vec<Candle>> {
    let client = reqwest::Client::new();
    let candles = client
        .get("http://localhost:3030/candles")
        .query(&[
            ("exchange", exchange),
            ("symbol", symbol),
            ("interval", &interval.to_string()),
            ("start", &start.to_string()),
            ("end", &end.to_string()),
            (
                "fill_missing_with_last",
                &fill_missing_with_last.to_string(),
            ),
        ])
        .send()
        .await?
        .json()
        .await?;
    Ok(candles)
}

pub fn candles_to_prices(candles: &[Candle], multipliers: Option<&[f64]>) -> Vec<f64> {
    let mut prices = Vec::with_capacity(candles.len() + 1);
    prices.push(candles[0].open * multipliers.map_or(1.0, |m| m[0]));
    for (i, candle) in candles.iter().enumerate() {
        let multiplier_i = i + 1; // Has to be offset by 1.
        prices.push(candle.close * multipliers.map_or(1.0, |m| m[multiplier_i]));
    }
    prices
}

pub async fn list_intervals(exchange: &str) -> Result<Vec<u64>> {
    let client = reqwest::Client::new();
    let intervals = client
        .get("http://localhost:3030/candle_intervals")
        .query(&[("exchange", exchange)])
        .send()
        .await?
        .json()
        .await?;
    Ok(intervals)
}

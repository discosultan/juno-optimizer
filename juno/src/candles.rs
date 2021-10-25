use crate::{Interval, Timestamp};
use serde::{Deserialize, Serialize};
use std::ops::AddAssign;
use thiserror::Error;

type Result<T> = std::result::Result<T, Error>;

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq)]
pub struct Candle {
    pub time: Timestamp,
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
        .interval,
        .start,
        .current,
    )]
    MissingStartCandles {
        exchange: String,
        symbol: String,
        interval: Interval,
        start: Timestamp,
        current: Timestamp,
    },
    #[error(
        "missing {exchange} {symbol} {} candle(s) from the end of the period; cannot fill; current {}, end {}",
        .interval,
        .current,
        .end,
    )]
    MissingEndCandles {
        exchange: String,
        symbol: String,
        interval: Interval,
        current: Timestamp,
        end: Timestamp,
    },
    #[error(transparent)]
    Reqwest(#[from] reqwest::Error),
}

pub async fn list_candles(
    exchange: &str,
    symbol: &str,
    interval: Interval,
    start: Timestamp,
    end: Timestamp,
    fill_missing_with_last: bool,
) -> Result<Vec<Candle>> {
    let client = reqwest::Client::new();
    let candles = client
        .get("http://localhost:3030/candles")
        .query(&[
            ("exchange", exchange),
            ("symbol", symbol),
            ("interval", &interval.0.to_string()),
            ("start", &start.0.to_string()),
            ("end", &end.0.to_string()),
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

pub async fn list_intervals(exchange: &str) -> Result<Vec<Interval>> {
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

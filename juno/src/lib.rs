pub mod clients;
pub mod easing;
pub mod filters;
pub mod genetics;
pub mod indicators;
pub mod itertools;
pub mod math;
pub mod statistics;
pub mod stop_loss;
pub mod strategies;
pub mod take_profit;
pub mod trading;
pub mod utils;

mod primitives;

pub use crate::filters::Filters;
pub use primitives::*;

use serde::{Deserialize, Serialize};
use std::{collections::HashMap, ops::AddAssign};
use strum::AsRefStr;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Advice {
    None,
    Long,
    Short,
    Liquidate,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize)]
pub struct BorrowInfo {
    pub interest_interval: u64,
    pub interest_rate: f64,
    pub limit: f64,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct AssetInfo {
    pub precision: u32,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize)]
pub struct Fees {
    pub maker: f64,
    pub taker: f64,
}

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

#[derive(AsRefStr, Debug, Deserialize, Serialize)]
pub enum CandleType {
    #[serde(rename = "regular")]
    #[strum(serialize = "regular")]
    Regular,
    #[serde(rename = "heikin-ashi")]
    #[strum(serialize = "heikin-ashi")]
    HeikinAshi,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct ExchangeInfo {
    // Key: asset
    pub assets: HashMap<String, AssetInfo>,
    // Key: symbol
    pub fees: HashMap<String, Fees>,
    // Key: symbol
    pub filters: HashMap<String, Filters>,
    // Keys: account, asset
    pub borrow_info: HashMap<String, HashMap<String, BorrowInfo>>,
}

#[derive(Deserialize, Serialize)]
pub struct Fill {
    pub price: f64,
    pub size: f64,
    pub quote: f64,
    pub fee: f64,
}

impl Fill {
    pub fn total_size(fills: &[Fill]) -> f64 {
        fills.iter().map(|fill| fill.size).sum()
    }

    pub fn total_quote(fills: &[Fill]) -> f64 {
        fills.iter().map(|fill| fill.quote).sum()
    }

    pub fn total_fee(fills: &[Fill]) -> f64 {
        fills.iter().map(|fill| fill.fee).sum()
    }
}

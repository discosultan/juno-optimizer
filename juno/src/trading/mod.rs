mod evaluation;
mod traders;

pub use evaluation::*;
pub use traders::*;

use crate::{
    genetics::Chromosome,
    stop_loss::{StopLossParams, StopLossParamsContext},
    strategies::{StrategyParams, StrategyParamsContext},
    take_profit::{TakeProfitParams, TakeProfitParamsContext},
    Fill, Interval, Timestamp,
};
use juno_derive::*;
use rand::prelude::*;
use serde::{Deserialize, Serialize};
use std::mem;

#[derive(Chromosome, Clone, Copy, Debug, Deserialize, Serialize)]
pub struct TradingParams {
    #[chromosome]
    pub strategy: StrategyParams,
    #[chromosome]
    pub trader: TraderParams,
    #[chromosome]
    pub stop_loss: StopLossParams,
    #[chromosome]
    pub take_profit: TakeProfitParams,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize)]
pub struct TraderParams {
    pub interval: Interval,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
pub struct TraderParamsContext {
    pub intervals: Vec<Interval>,
}

impl Chromosome for TraderParams {
    type Context = TraderParamsContext;

    fn len() -> usize {
        1
    }

    fn generate(rng: &mut StdRng, ctx: &Self::Context) -> Self {
        Self {
            interval: match ctx.intervals.len() {
                0 => panic!(),
                1 => ctx.intervals[0],
                _ => *ctx.intervals.choose(rng).unwrap(),
            },
        }
    }

    fn cross(&mut self, other: &mut Self, i: usize) {
        match i {
            0 => mem::swap(&mut self.interval, &mut other.interval),
            _ => panic!(),
        };
    }

    fn mutate(&mut self, rng: &mut StdRng, i: usize, ctx: &Self::Context) {
        match i {
            0 => {
                self.interval = match ctx.intervals.len() {
                    0 => panic!(),
                    1 => ctx.intervals[0],
                    _ => *ctx.intervals.choose(rng).unwrap(),
                }
            }
            _ => panic!(),
        };
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub enum CloseReason {
    Strategy,
    StopLoss,
    TakeProfit,
    Cancelled,
}

pub enum OpenPosition {
    Long(OpenLongPosition),
    Short(OpenShortPosition),
}

pub struct OpenLongPosition {
    pub time: Timestamp,
    pub fills: [Fill; 1],
}

impl OpenLongPosition {
    pub fn close(self, time: Timestamp, fills: [Fill; 1], reason: CloseReason) -> LongPosition {
        LongPosition {
            open_time: self.time,
            open_fills: self.fills,

            close_time: time,
            close_fills: fills,
            close_reason: reason,
        }
    }

    pub fn cost(&self) -> f64 {
        Fill::total_quote(&self.fills)
    }

    pub fn base_gain(&self) -> f64 {
        Fill::total_size(&self.fills) - Fill::total_fee(&self.fills)
    }
}

pub struct OpenShortPosition {
    pub time: Timestamp,
    pub collateral: f64,
    pub borrowed: f64,
    pub fills: [Fill; 1],
}

impl OpenShortPosition {
    pub fn close(self, time: Timestamp, fills: [Fill; 1], reason: CloseReason) -> ShortPosition {
        ShortPosition {
            open_time: self.time,
            collateral: self.collateral,
            borrowed: self.borrowed,
            open_fills: self.fills,

            close_time: time,
            close_fills: fills,
            close_reason: reason,
        }
    }
}

#[derive(Deserialize, Serialize)]
#[serde(tag = "type")]
pub enum Position {
    Long(LongPosition),
    Short(ShortPosition),
}

#[derive(Deserialize, Serialize)]
pub struct LongPosition {
    pub open_time: Timestamp,
    pub open_fills: [Fill; 1],

    pub close_time: Timestamp,
    pub close_fills: [Fill; 1],
    pub close_reason: CloseReason,
}

impl LongPosition {
    pub fn cost(&self) -> f64 {
        Fill::total_quote(&self.open_fills)
    }

    pub fn base_gain(&self) -> f64 {
        Fill::total_size(&self.open_fills) - Fill::total_fee(&self.open_fills)
    }

    pub fn base_cost(&self) -> f64 {
        Fill::total_size(&self.close_fills)
    }

    pub fn gain(&self) -> f64 {
        Fill::total_quote(&self.close_fills) - Fill::total_fee(&self.close_fills)
    }

    pub fn profit(&self) -> f64 {
        self.gain() - self.cost()
    }

    pub fn duration(&self) -> Interval {
        self.close_time - self.open_time
    }
}

#[derive(Deserialize, Serialize)]
pub struct ShortPosition {
    pub open_time: Timestamp,
    pub collateral: f64,
    pub borrowed: f64,
    pub open_fills: [Fill; 1],
    pub close_time: Timestamp,
    pub close_fills: [Fill; 1],
    pub close_reason: CloseReason,
}

impl ShortPosition {
    pub fn cost(&self) -> f64 {
        self.collateral
    }

    pub fn base_gain(&self) -> f64 {
        self.borrowed
    }

    pub fn base_cost(&self) -> f64 {
        self.borrowed
    }

    pub fn gain(&self) -> f64 {
        Fill::total_quote(&self.open_fills) - Fill::total_fee(&self.open_fills) + self.collateral
            - Fill::total_quote(&self.close_fills)
    }

    pub fn duration(&self) -> Interval {
        self.close_time - self.open_time
    }

    pub fn profit(&self) -> f64 {
        self.gain() - self.cost()
    }
}

#[derive(Deserialize, Serialize)]
pub struct TradingSummary {
    pub positions: Vec<Position>,

    pub start: Timestamp,
    pub end: Timestamp,
    pub quote: f64,
}

impl TradingSummary {
    pub fn new(start: Timestamp, end: Timestamp, quote: f64) -> Self {
        Self {
            positions: Vec::new(),
            start,
            end,
            quote,
        }
    }
}

mod core;
mod extended;
use std::collections::HashMap;

pub use self::core::*;
pub use extended::*;

use crate::{
    math::annualized,
    trading::{CloseReason, Position, TradingSummary},
    Interval, Timestamp,
};
use serde::{Deserialize, Serialize};

// TODO: Use const fn when `365.0.sqrt()` is supported.
pub(crate) const SQRT_365: f64 = 19.104_973_174_542_8;

#[derive(Deserialize, Serialize)]
pub enum PositionType {
    Long,
    Short,
}

#[derive(Deserialize, Serialize)]
pub struct PositionStatistics {
    #[serde(rename = "type")]
    pub type_: PositionType,
    pub open_time: Timestamp,
    pub close_time: Timestamp,
    pub cost: f64,
    pub gain: f64,
    pub profit: f64,
    pub duration: Interval,
    pub roi: f64,
    pub annualized_roi: f64,
    pub close_reason: CloseReason,
}

impl PositionStatistics {
    pub fn compose(pos: &Position) -> Self {
        match pos {
            Position::Long(pos) => {
                let duration = pos.duration();
                let profit = pos.profit();
                let roi = profit / pos.cost();
                Self {
                    type_: PositionType::Long,
                    open_time: pos.open_time,
                    close_time: pos.close_time,
                    cost: pos.cost(),
                    gain: pos.gain(),
                    profit,
                    duration,
                    roi,
                    annualized_roi: annualized(duration.0, roi),
                    close_reason: pos.close_reason,
                }
            }
            Position::Short(pos) => {
                let duration = pos.duration();
                let profit = pos.profit();
                let roi = profit / pos.cost();
                Self {
                    type_: PositionType::Short,
                    open_time: pos.open_time,
                    close_time: pos.close_time,
                    cost: pos.cost(),
                    gain: pos.gain(),
                    profit,
                    duration,
                    roi,
                    annualized_roi: annualized(duration.0, roi),
                    close_reason: pos.close_reason,
                }
            }
        }
    }
}

#[derive(Serialize)]
pub struct Statistics {
    pub core: CoreStatistics,
    pub extended: ExtendedStatistics,
    pub positions: Vec<PositionStatistics>,
}

impl Statistics {
    pub fn compose(
        summary: &TradingSummary,
        symbol: &str,
        prices: &HashMap<String, Vec<f64>>,
        stats_interval: Interval,
    ) -> Self {
        Self {
            core: CoreStatistics::compose(summary),
            extended: ExtendedStatistics::compose(summary, symbol, prices, stats_interval),
            positions: summary
                .positions
                .iter()
                .map(PositionStatistics::compose)
                .collect(),
        }
    }
}

#[cfg(test)]
mod test_utils {
    use crate::{
        trading::{CloseReason, LongPosition, Position, TradingSummary},
        Fill,
    };

    pub fn get_populated_trading_summary() -> TradingSummary {
        let mut summary = TradingSummary::new(0.into(), 10.into(), 1.0);
        summary.positions.push(Position::Long(LongPosition {
            open_time: 2.into(),
            open_fills: [Fill {
                price: 0.5,
                size: 2.0,
                quote: 1.0,
                fee: 0.2,
            }],
            close_time: 4.into(),
            close_fills: [Fill {
                price: 0.5,
                size: 1.8,
                quote: 0.9,
                fee: 0.09,
            }],
            close_reason: CloseReason::Strategy,
        }));
        summary.positions.push(Position::Long(LongPosition {
            open_time: 6.into(),
            open_fills: [Fill {
                price: 0.5,
                size: 1.62,
                quote: 0.81,
                fee: 0.02,
            }],
            close_time: 8.into(),
            close_fills: [Fill {
                price: 0.75,
                size: 1.6,
                quote: 1.2,
                fee: 0.1,
            }],
            close_reason: CloseReason::Strategy,
        }));
        summary
    }
}

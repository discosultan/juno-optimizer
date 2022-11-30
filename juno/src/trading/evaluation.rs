use super::{TradeInput, TradingParams};
use crate::{
    clients::juno_core,
    genetics::{Evaluation, Individual},
    statistics,
    trading::trade,
    BorrowInfo, Candle, ExchangeInfo, Fees, Filters, Interval, SymbolExt, Timestamp,
};
use rayon::prelude::*;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use thiserror::Error;

#[derive(Clone, Copy, Deserialize, Serialize)]
pub enum EvaluationStatistic {
    Profit,
    ReturnOverMaxDrawdown,
    SharpeRatio,
    SortinoRatio,
}

impl EvaluationStatistic {
    pub fn values() -> [Self; 4] {
        [
            Self::Profit,
            Self::ReturnOverMaxDrawdown,
            Self::SharpeRatio,
            Self::SortinoRatio,
        ]
    }
}

#[derive(Clone, Copy, Deserialize, Serialize)]
pub enum EvaluationAggregation {
    Linear,
    Log10,
    Log10Factored,
}

impl EvaluationAggregation {
    pub fn values() -> [Self; 3] {
        [Self::Linear, Self::Log10, Self::Log10Factored]
    }
}

#[derive(Error, Debug)]
pub enum EvaluationError {
    #[error(transparent)]
    JunoCore(#[from] juno_core::Error),
}

struct SymbolCtx {
    symbol: String,
    interval_candles: HashMap<Interval, Vec<Candle>>,
    fees: Fees,
    filters: Filters,
    borrow_info: BorrowInfo,
}

pub struct BasicEvaluation {
    symbol_ctxs: Vec<SymbolCtx>,
    prices: HashMap<String, Vec<f64>>,
    quote: f64,
    stats_interval: Interval,
    evaluation_statistic: EvaluationStatistic,
    evaluation_aggregation_fn: fn(f64, f64) -> f64,
}

pub struct BasicEvaluationInput<'a> {
    pub exchange_info: &'a ExchangeInfo,
    pub candles: &'a HashMap<String, HashMap<Interval, Vec<Candle>>>,
    pub prices: &'a HashMap<String, Vec<f64>>,
    pub symbols: &'a [String],
    pub intervals: &'a [Interval],
    pub start: Timestamp,
    pub end: Timestamp,
    pub quote: f64,
    pub evaluation_statistic: EvaluationStatistic,
    pub evaluation_aggregation: EvaluationAggregation,
}

impl BasicEvaluation {
    pub fn new(input: &BasicEvaluationInput<'_>) -> Self {
        let stats_interval = Interval::DAY_MS;

        let symbol_ctxs = input
            .symbols
            .iter()
            .map(|symbol| {
                // TODO: Remove clone.
                let interval_candles = input.candles[symbol].clone();
                // Store context variables.
                SymbolCtx {
                    symbol: symbol.clone(),
                    interval_candles,
                    fees: input.exchange_info.fees[symbol],
                    filters: input.exchange_info.filters[symbol],
                    borrow_info: input.exchange_info.borrow_info[symbol][symbol.base_asset()],
                }
            })
            .collect();

        Self {
            symbol_ctxs,
            stats_interval,
            quote: input.quote,
            evaluation_statistic: input.evaluation_statistic,
            evaluation_aggregation_fn: match input.evaluation_aggregation {
                EvaluationAggregation::Linear => sum_linear,
                EvaluationAggregation::Log10 => sum_log10,
                EvaluationAggregation::Log10Factored => sum_log10_factored,
            },
            prices: input.prices.clone(),
        }
    }

    fn evaluate_symbol(&self, symbol_ctx: &SymbolCtx, chromosome: &TradingParams) -> f64 {
        let summary = trade(
            chromosome,
            &TradeInput {
                candles: &symbol_ctx.interval_candles[&chromosome.trader.interval],
                fees: &symbol_ctx.fees,
                filters: &symbol_ctx.filters,
                borrow_info: &symbol_ctx.borrow_info,
                margin_multiplier: 2,
                quote: self.quote,
                long: true,
                short: true,
            },
        );
        match self.evaluation_statistic {
            EvaluationStatistic::Profit => statistics::get_profit(&summary),
            EvaluationStatistic::ReturnOverMaxDrawdown => {
                statistics::get_return_over_max_drawdown(&summary)
            }
            EvaluationStatistic::SharpeRatio => statistics::get_sharpe_ratio(
                &summary,
                &symbol_ctx.symbol,
                &self.prices,
                self.stats_interval,
            ),
            EvaluationStatistic::SortinoRatio => statistics::get_sortino_ratio(
                &summary,
                &symbol_ctx.symbol,
                &self.prices,
                self.stats_interval,
            ),
        }
    }
}

impl Evaluation for BasicEvaluation {
    type Chromosome = TradingParams;

    fn evaluate(&self, population: &mut [Individual<Self::Chromosome>]) {
        // TODO: Support different strategies here. A la parallel cpu or gpu, for example.
        // let fitnesses = Vec::with_capacity(population.len());
        // let fitness_slices = fitnesses.chunks_exact_mut(1).collect();

        population
            // .iter_mut()
            .par_iter_mut()
            .for_each(|ind| {
                ind.fitness = self
                    .symbol_ctxs
                    .iter()
                    .map(|ctx| self.evaluate_symbol(ctx, &ind.chromosome))
                    .fold(0.0, self.evaluation_aggregation_fn)
            });
    }
}

fn sum_linear(acc: f64, val: f64) -> f64 {
    acc + val
}
fn sum_log10(acc: f64, val: f64) -> f64 {
    const LOG_SHIFT_FACTOR: f64 = 1.0;
    acc + if val >= 0.0 {
        (val + LOG_SHIFT_FACTOR).log10()
    } else {
        // -(-val + LOG_SHIFT_FACTOR).log10()
        -(10.0_f64).powf(-val + LOG_SHIFT_FACTOR)
    }
}
fn sum_log10_factored(acc: f64, val: f64) -> f64 {
    const FACTOR: f64 = 10.0;
    sum_log10(acc, val * FACTOR)
}

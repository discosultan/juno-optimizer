use super::TradingParams;
use crate::{
    clients::juno_core,
    genetics::{Evaluation, Individual},
    statistics, symbol,
    trading::trade,
    BorrowInfo, Candle, CandleType, Fees, Filters, Interval, SymbolExt, Timestamp,
};
use futures::future::{try_join, try_join_all};
use rayon::prelude::*;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use thiserror::Error;

type Result<T> = std::result::Result<T, EvaluationError>;

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

impl BasicEvaluation {
    pub async fn new(
        juno_core_client: &juno_core::Client,
        exchange: &str,
        symbols: &[String],
        intervals: &[Interval],
        start: Timestamp,
        end: Timestamp,
        quote: f64,
        evaluation_statistic: EvaluationStatistic,
        evaluation_aggregation: EvaluationAggregation,
    ) -> Result<Self> {
        let exchange_info = juno_core_client.get_exchange_info(exchange).await?;
        let stats_interval = Interval::DAY_MS;

        let symbol_ctxs_task =
            try_join_all(symbols.iter().map(|symbol| (symbol, &exchange_info)).map(
                |(symbol, exchange_info)| async {
                    let interval_candles = try_join_all(intervals.iter().map(|interval| async {
                        Ok::<_, juno_core::Error>((
                            *interval,
                            juno_core_client
                                .list_candles(
                                    exchange,
                                    symbol,
                                    *interval,
                                    start,
                                    end,
                                    CandleType::Regular,
                                )
                                .await?,
                        ))
                    }))
                    .await?;

                    let interval_candles = interval_candles.into_iter().collect();

                    // Store context variables.
                    Ok::<_, juno_core::Error>(SymbolCtx {
                        symbol: symbol.clone(),
                        interval_candles,
                        fees: exchange_info.fees[symbol],
                        filters: exchange_info.filters[symbol],
                        borrow_info: exchange_info.borrow_info[symbol][symbol.base_asset()],
                    })
                },
            ));

        let mut assets = symbol::list_assets(symbols);
        // Benchmark asset for extended statistics.
        assets.push("btc");
        let prices_task = juno_core_client.map_asset_prices(
            exchange,
            &assets,
            stats_interval,
            start,
            end,
            "usdt",
        );

        let (symbol_ctxs, prices) = try_join(symbol_ctxs_task, prices_task).await?;

        Ok(Self {
            symbol_ctxs,
            stats_interval,
            quote,
            evaluation_statistic,
            evaluation_aggregation_fn: match evaluation_aggregation {
                EvaluationAggregation::Linear => sum_linear,
                EvaluationAggregation::Log10 => sum_log10,
                EvaluationAggregation::Log10Factored => sum_log10_factored,
            },
            prices,
        })
    }

    pub fn evaluate_symbols(&self, chromosome: &TradingParams) -> Vec<f64> {
        self.symbol_ctxs
            .par_iter()
            .map(|symbol_ctx| self.evaluate_symbol(symbol_ctx, chromosome))
            .collect()
    }

    fn evaluate_symbol(&self, symbol_ctx: &SymbolCtx, chromosome: &TradingParams) -> f64 {
        let summary = trade(
            chromosome,
            &symbol_ctx.interval_candles[&chromosome.trader.interval],
            &symbol_ctx.fees,
            &symbol_ctx.filters,
            &symbol_ctx.borrow_info,
            2,
            self.quote,
            true,
            true,
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

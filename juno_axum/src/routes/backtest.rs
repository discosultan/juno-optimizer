use axum::{
    extract::State,
    http::StatusCode,
    response::{IntoResponse, Json},
    routing, Router,
};
use itertools::Itertools;
use juno::{
    statistics::Statistics,
    trading::{trade, TradeInput, TradingParams},
    Candle, ExchangeInfo, Interval, SymbolExt, Timestamp,
};
use rayon::prelude::*;
use serde::{Deserialize, Serialize};
use std::{collections::HashMap, sync::Arc};
use tracing::info;

use crate::error::Error;
use juno::clients::juno_core;

#[derive(Debug, Deserialize)]
struct Params {
    exchange: String,
    symbols: Vec<String>,
    start: Timestamp,
    end: Timestamp,
    quote: f64,
    trading: TradingParams,
}

#[derive(Serialize)]
struct BacktestResult {
    symbol_stats: HashMap<String, Statistics>,
}

pub fn routes() -> Router<Arc<juno_core::Client>> {
    Router::new().route("/", routing::post(post))
}

async fn post(
    State(juno_core_client): State<Arc<juno_core::Client>>,
    Json(args): Json<Params>,
) -> Result<impl IntoResponse, Error> {
    // Gather data.
    info!("gathering data");
    let symbols: Vec<_> = args.symbols.into_iter().unique().collect();
    let (exchange_info, candles, prices) = crate::exchange::gather_exchange_info_candles_prices(
        &juno_core_client,
        &args.exchange,
        &symbols,
        &[args.trading.trader.interval],
        args.start,
        args.end,
    )
    .await?;

    // Backtest in parallel.
    info!("backtesting");
    let symbols_clone = symbols.clone();
    let backtest_result = tokio_rayon::spawn(move || {
        par_backtest(
            exchange_info,
            candles,
            prices,
            symbols_clone,
            args.trading,
            args.quote,
        )
    })
    .await;

    Ok((StatusCode::OK, Json(backtest_result)))
}

fn par_backtest(
    exchange_info: ExchangeInfo,
    candles: HashMap<String, HashMap<Interval, Vec<Candle>>>,
    prices: HashMap<String, Vec<f64>>,
    symbols: Vec<String>,
    trading: TradingParams,
    quote: f64,
) -> BacktestResult {
    let stats_interval = Interval::DAY_MS;
    let symbol_stats = symbols
        .par_iter()
        .map(|symbol| {
            let summary = trade(
                &trading,
                &TradeInput {
                    candles: &candles[symbol][&trading.trader.interval],
                    fees: &exchange_info.fees[symbol],
                    filters: &exchange_info.filters[symbol],
                    borrow_info: &exchange_info.borrow_info[symbol][symbol.base_asset()],
                    margin_multiplier: 2,
                    quote: quote,
                    long: true,
                    short: true,
                },
            );
            let stats = Statistics::compose(&summary, symbol, &prices, stats_interval);
            (symbol.clone(), stats)
        })
        .collect();
    BacktestResult { symbol_stats }
}

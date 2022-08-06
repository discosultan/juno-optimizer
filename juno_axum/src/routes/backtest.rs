use axum::{
    http::StatusCode,
    response::{IntoResponse, Json},
    routing, Extension, Router,
};
use futures::future::{try_join, try_join_all};
use juno::{
    statistics::Statistics,
    trading::{trade, TradingParams, TradingSummary},
    Interval, SymbolExt, Timestamp,
};
use serde::{Deserialize, Serialize};
use std::{collections::HashMap, sync::Arc};
use tracing::{info, instrument};

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

pub fn routes() -> Router {
    Router::new().route("/", routing::post(post))
}

async fn post(
    Json(args): Json<Params>,
    Extension(juno_core_client): Extension<Arc<juno_core::Client>>,
) -> Result<impl IntoResponse, Error> {
    let symbol_summary_tasks = args.symbols.iter().map(|symbol| async {
        let summary = backtest(&juno_core_client, &args, symbol).await?;
        Ok::<_, anyhow::Error>((symbol.clone(), summary))
    });
    let symbol_summaries = try_join_all(symbol_summary_tasks)
        .await
        .map_err(Error::from)?;

    let symbol_stat_tasks = symbol_summaries.iter().map(|(symbol, summary)| async {
        let stats = get_stats(&juno_core_client, &args, symbol, summary).await?;
        Ok::<_, anyhow::Error>((symbol.clone(), stats))
    });
    let symbol_stats = try_join_all(symbol_stat_tasks)
        .await
        .map_err(Error::from)?
        .into_iter()
        .collect();

    Ok((StatusCode::OK, Json(BacktestResult { symbol_stats })))
}

#[instrument(skip(args))]
async fn backtest(
    juno_core_client: &juno_core::Client,
    args: &Params,
    symbol: &str,
) -> anyhow::Result<TradingSummary> {
    info!("gathering necessary info");
    let exchange_info_task = juno_core_client.get_exchange_info(&args.exchange);
    let candles_task = juno_core_client.list_candles(
        &args.exchange,
        symbol,
        args.trading.trader.interval,
        args.start,
        args.end,
        juno::CandleType::Regular, // TODO: variable
    );

    let (exchange_info, candles) = try_join(exchange_info_task, candles_task).await?;

    info!("running backtest");
    Ok(trade(
        &args.trading,
        &candles,
        &exchange_info.fees[symbol],
        &exchange_info.filters[symbol],
        &exchange_info.borrow_info[symbol][symbol.base_asset()],
        2,
        args.quote,
        true,
        true,
    ))
}

async fn get_stats(
    juno_core_client: &juno_core::Client,
    args: &Params,
    symbol: &str,
    summary: &TradingSummary,
) -> anyhow::Result<Statistics> {
    let stats_interval = Interval::DAY_MS;
    let start = args.start;
    let end = args.end;

    let mut assets = vec![symbol.base_asset(), symbol.quote_asset()];
    // Benchmark asset for extended statistics.
    assets.push("btc");
    let prices = juno_core_client
        .map_asset_prices(&args.exchange, &assets, stats_interval, start, end, "usdt")
        .await?;

    let stats = Statistics::compose(summary, symbol, &prices, stats_interval);

    Ok(stats)
}

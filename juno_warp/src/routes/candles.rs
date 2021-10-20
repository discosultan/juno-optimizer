use super::custom_reject;
use futures::future::try_join_all;
use juno::candles;
use serde::Deserialize;
use std::collections::HashMap;
use warp::{body, reply, Filter, Rejection, Reply};

#[derive(Debug, Deserialize)]
struct Params {
    exchange: String,
    interval: u64,
    start: u64,
    end: u64,
    symbols: Vec<String>,
}

pub fn routes() -> impl Filter<Extract = impl Reply, Error = Rejection> + Clone {
    warp::path("candles").and(post())
}

fn post() -> impl Filter<Extract = (reply::Json,), Error = Rejection> + Clone {
    warp::post()
        .and(body::json())
        .and_then(|args: Params| async move {
            let symbol_candle_tasks = args.symbols.iter().map(|symbol| (symbol, &args)).map(
                |(symbol, args)| async move {
                    let candles = candles::list_candles(
                        &args.exchange,
                        symbol,
                        args.interval,
                        args.start,
                        args.end,
                        true,
                    )
                    .await?;
                    Ok::<_, candles::Error>((symbol, candles))
                },
            );

            try_join_all(symbol_candle_tasks)
                .await
                .map(|symbol_candles| {
                    reply::json(&symbol_candles.into_iter().collect::<HashMap<_, _>>())
                })
                .map_err(|error| custom_reject(error))
        })
}

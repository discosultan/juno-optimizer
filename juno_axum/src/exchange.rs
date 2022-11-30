use std::{collections::HashMap, hash::Hash};

use futures::future::{try_join3, try_join_all};
use itertools::Itertools;
use juno::{clients::juno_core, symbol, Candle, CandleType, ExchangeInfo, Interval, Timestamp};

pub async fn gather_exchange_info_candles_prices(
    client: &juno_core::Client,
    exchange: &str,
    symbols: &[String],
    intervals: &[Interval],
    start: Timestamp,
    end: Timestamp,
) -> anyhow::Result<(
    ExchangeInfo,
    HashMap<String, HashMap<Interval, Vec<Candle>>>,
    HashMap<String, Vec<f64>>,
)> {
    let exchange_info_task = client.get_exchange_info(juno_core::GetExchangeInfo { exchange });

    let candle_tasks = try_join_all(symbols.iter().flat_map(|symbol| {
        intervals.iter().map(|interval| {
            client.list_candles(juno_core::ListCandles {
                exchange,
                symbol,
                interval: *interval,
                start,
                end,
                type_: CandleType::Regular,
            })
        })
    }));

    let assets: Vec<_> = symbol::iter_assets(symbols.iter().map(String::as_str))
        .chain(["btc"])
        .unique()
        .collect();
    let stats_interval = Interval::DAY_MS;
    let prices_task = client.map_asset_prices(juno_core::MapAssetPrices {
        exchange,
        assets: &assets,
        interval: stats_interval,
        start,
        end,
        target_asset: "usdt",
    });

    let (exchange_info, candles, prices) =
        try_join3(exchange_info_task, candle_tasks, prices_task).await?;

    Ok((
        exchange_info,
        map_of_maps(symbols, intervals, candles),
        prices,
    ))
}

fn map_of_maps<TK1, TK2, TV>(
    keys1: &[TK1],
    keys2: &[TK2],
    values: Vec<Vec<TV>>,
) -> HashMap<TK1, HashMap<TK2, Vec<TV>>>
where
    TK1: Clone + Eq + Hash,
    TK2: Clone + Eq + Hash,
{
    let mut key1_key2_values: HashMap<TK1, HashMap<TK2, Vec<TV>>> =
        HashMap::with_capacity(keys1.len());
    for (i, values) in values.into_iter().enumerate() {
        let key1 = &keys1[i / keys2.len()];
        let key2 = &keys2[i % keys2.len()];

        match key1_key2_values.get_mut(key1) {
            Some(key2_values) => {
                key2_values.insert(key2.clone(), values);
            }
            None => {
                let mut key2_values = HashMap::with_capacity(keys2.len());
                key2_values.insert(key2.clone(), values);
                key1_key2_values.insert(key1.clone(), key2_values);
            }
        }
    }

    key1_key2_values
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_map_of_maps() {
        let output = map_of_maps(
            &[1, 2],
            &[1, 2, 3],
            vec![
                vec![1, 2, 3],
                vec![1, 2, 3],
                vec![1, 2, 3],
                vec![1, 2, 3],
                vec![1, 2, 3],
                vec![1, 2, 3],
            ],
        );

        let expected_output = HashMap::from([
            (
                1,
                HashMap::from([(1, vec![1, 2, 3]), (2, vec![1, 2, 3]), (3, vec![1, 2, 3])]),
            ),
            (
                2,
                HashMap::from([(1, vec![1, 2, 3]), (2, vec![1, 2, 3]), (3, vec![1, 2, 3])]),
            ),
        ]);

        assert_eq!(output, expected_output);
    }
}

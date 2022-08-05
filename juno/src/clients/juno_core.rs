use std::collections::HashMap;

use crate::{Candle, CandleType, ExchangeInfo, Interval, Timestamp};
use thiserror::Error;

#[derive(Debug)]
pub struct Client {
    url: String,
    client: reqwest::Client,
}

#[derive(Debug, Error)]
pub enum Error {
    #[error(transparent)]
    Request(#[from] reqwest::Error),
}

impl Client {
    pub fn new(url: &str) -> Self {
        Self {
            url: url.to_owned(),
            client: reqwest::Client::new(),
        }
    }

    pub async fn get_exchange_info(&self, exchange: &str) -> Result<ExchangeInfo, Error> {
        let exchange_info = self
            .client
            .get(format!("{}/exchange_info", self.url))
            .query(&[("exchange", exchange)])
            .send()
            .await?
            .json()
            .await?;
        Ok(exchange_info)
    }

    pub async fn list_candles(
        &self,
        exchange: &str,
        symbol: &str,
        interval: Interval,
        start: Timestamp,
        end: Timestamp,
        type_: CandleType,
    ) -> Result<Vec<Candle>, Error> {
        let candles = self
            .client
            .get(format!("{}/candles", self.url))
            .query(&[
                ("exchange", exchange),
                ("symbol", symbol),
                ("interval", &interval.0.to_string()),
                ("start", &start.0.to_string()),
                ("end", &end.0.to_string()),
                ("type", type_.as_ref()),
            ])
            .send()
            .await?
            .json()
            .await?;
        Ok(candles)
    }

    pub async fn list_candles_fill_missing_with_none(
        &self,
        exchange: &str,
        symbol: &str,
        interval: Interval,
        start: Timestamp,
        end: Timestamp,
        type_: CandleType,
    ) -> Result<Vec<Option<Candle>>, Error> {
        let candles = self
            .client
            .get(format!("{}/candles_fill_missing_with_none", self.url))
            .query(&[
                ("exchange", exchange),
                ("symbol", symbol),
                ("interval", &interval.0.to_string()),
                ("start", &start.0.to_string()),
                ("end", &end.0.to_string()),
                ("type", type_.as_ref()),
            ])
            .send()
            .await?
            .json()
            .await?;
        Ok(candles)
    }

    pub async fn list_candle_intervals(&self, exchange: &str) -> Result<Vec<Interval>, Error> {
        let client = reqwest::Client::new();
        let intervals = client
            .get(format!("{}/candle_intervals", self.url))
            .query(&[("exchange", exchange)])
            .send()
            .await?
            .json()
            .await?;
        Ok(intervals)
    }

    pub async fn map_asset_prices(
        &self,
        exchange: &str,
        assets: &[&str],
        interval: Interval,
        start: Timestamp,
        end: Timestamp,
        target_asset: &str,
    ) -> Result<HashMap<String, Vec<f64>>, Error> {
        let prices = self
            .client
            .get(format!("{}/prices", self.url))
            .query(&[
                ("exchange", exchange),
                ("assets", &assets.join(",")),
                ("interval", &interval.0.to_string()),
                ("start", &start.0.to_string()),
                ("end", &end.0.to_string()),
                ("target_asset", target_asset),
            ])
            .send()
            .await?
            .json()
            .await?;
        Ok(prices)
    }
}

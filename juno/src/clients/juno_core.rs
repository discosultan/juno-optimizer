use std::collections::HashMap;

use crate::{Candle, CandleType, ExchangeInfo, Interval, Timestamp};
use async_trait::async_trait;
use reqwest::{Response, StatusCode, Url};
use serde::{Deserialize, Serialize};
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
    #[error("HTTP status client error ({status}) for url ({url}): {message}")]
    Api {
        status: StatusCode,
        url: Url,
        message: String,
    },
}

#[derive(Serialize)]
pub struct GetExchangeInfo<'a> {
    pub exchange: &'a str,
}

#[derive(Serialize)]
pub struct ListCandles<'a> {
    pub exchange: &'a str,
    pub symbol: &'a str,
    pub interval: Interval,
    pub start: Timestamp,
    pub end: Timestamp,
    pub type_: CandleType,
}

#[derive(Serialize)]
pub struct MapAssetPrices<'a> {
    pub exchange: &'a str,
    pub assets: &'a [&'a str],
    pub interval: Interval,
    pub start: Timestamp,
    pub end: Timestamp,
    pub target_asset: &'a str,
}

impl Client {
    pub fn new(url: &str) -> Self {
        Self {
            url: url.to_owned(),
            client: reqwest::Client::new(),
        }
    }

    pub async fn get_exchange_info(&self, request: GetExchangeInfo<'_>) -> Result<ExchangeInfo, Error> {
        let exchange_info = self
            .client
            .post(format!("{}/exchange_info", self.url))
            .json(&request)
            .send()
            .await?
            .error_for_juno_status()
            .await?
            .json()
            .await?;
        Ok(exchange_info)
    }

    pub async fn list_candles(
        &self,
        request: ListCandles<'_>,
    ) -> Result<Vec<Candle>, Error> {
        let candles = self
            .client
            .post(format!("{}/candles", self.url))
            .json(&request)
            .send()
            .await?
            .error_for_juno_status()
            .await?
            .json()
            .await?;
        Ok(candles)
    }

    pub async fn list_candles_fill_missing_with_none(
        &self,
        request: ListCandles<'_>,
    ) -> Result<Vec<Option<Candle>>, Error> {
        let candles = self
            .client
            .post(format!("{}/candles_fill_missing_with_none", self.url))
            .json(&request)
            .send()
            .await?
            .error_for_juno_status()
            .await?
            .json()
            .await?;
        Ok(candles)
    }

    pub async fn list_candle_intervals(&self, request: GetExchangeInfo<'_>) -> Result<Vec<Interval>, Error> {
        let client = reqwest::Client::new();
        let intervals = client
            .post(format!("{}/candle_intervals", self.url))
            .json(&request)
            .send()
            .await?
            .error_for_juno_status()
            .await?
            .json()
            .await?;
        Ok(intervals)
    }

    pub async fn map_asset_prices(
        &self,
        request: MapAssetPrices<'_>,
    ) -> Result<HashMap<String, Vec<f64>>, Error> {
        let prices = self
            .client
            .post(format!("{}/prices", self.url))
            .json(&request)
            .send()
            .await?
            .error_for_juno_status()
            .await?
            .json()
            .await?;
        Ok(prices)
    }
}

#[derive(Deserialize)]
struct ApiError {
    pub message: String,
}

#[async_trait]
trait ResponseExt: Sized {
    async fn error_for_juno_status(self) -> Result<Self, Error>;
}

#[async_trait]
impl ResponseExt for Response {
    async fn error_for_juno_status(self) -> Result<Self, Error> {
        let status = self.status();
        if status.is_client_error() {
            let url = self.url().clone();
            let api_error: ApiError = self.json().await?;
            Err(Error::Api {
                status,
                url,
                message: api_error.message,
            })
        } else {
            Ok(self.error_for_status()?)
        }
    }
}

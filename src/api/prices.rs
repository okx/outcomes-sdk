//! Market data API methods. These call OKX's market-data endpoints
//! (`https://www.okx.com/api/v5/market/*`), which use a different path prefix
//! and a string-typed `code` envelope from the rest of the outcomes API.

use crate::{
    client::OutcomesSdkClient,
    endpoints,
    error::SdkError,
    models::price::{Candle, PmBookDepth, Ticker},
};

impl OutcomesSdkClient {
    /// `GET https://www.okx.com/api/v5/market/ticker` — Latest quote for a single instrument.
    ///
    /// Pass the outcome market's `yesOutcome.assetId` as `inst_id`.
    pub async fn get_ticker(&self, inst_id: &str) -> Result<Ticker, SdkError> {
        let url = format!("{}{}", self.base_url, endpoints::OKX_MARKET_TICKER_PATH);
        let data: Vec<Ticker> = self.http_get_abs(&url, &[("instId", inst_id)]).await?;
        data.into_iter().next().ok_or_else(|| SdkError::Api {
            code: -1,
            message: "ticker not found".to_string(),
        })
    }

    /// `GET https://www.okx.com/api/v5/market/candles` — K-line history for a single instrument.
    ///
    /// # Parameters
    ///
    /// - `inst_id` — Instrument ID (e.g. the market's `yesOutcome.assetId`).
    /// - `bar` — Candlestick granularity, e.g. `"1m"`, `"5m"`, `"1H"`, `"1D"`. Default `"1m"`.
    /// - `after` — Return candles with timestamp **before** this value (pagination, Unix ms).
    /// - `before` — Return candles with timestamp **after** this value (pagination, Unix ms).
    /// - `limit` — Number of candles (max 100, default 100).
    pub async fn get_candles(
        &self,
        inst_id: &str,
        bar: Option<&str>,
        after: Option<&str>,
        before: Option<&str>,
        limit: Option<i32>,
    ) -> Result<Vec<Candle>, SdkError> {
        let limit_str = limit.map(|v| v.to_string());
        let mut params: Vec<(&str, &str)> = vec![("instId", inst_id)];
        if let Some(v) = bar {
            params.push(("bar", v));
        }
        if let Some(v) = after {
            params.push(("after", v));
        }
        if let Some(v) = before {
            params.push(("before", v));
        }
        if let Some(ref v) = limit_str {
            params.push(("limit", v));
        }
        let url = format!("{}{}", self.base_url, endpoints::OKX_MARKET_CANDLES_PATH);
        self.http_get_abs(&url, &params).await
    }

    /// `GET https://www.okx.com/api/v5/market/pm-books` -- Outcome market order
    /// book depth snapshot.
    ///
    /// Rate limit: 40 requests / 2s.
    ///
    /// # Parameters
    ///
    /// - `inst_id` -- Instrument ID (the market's `yesOutcome.assetId`).
    /// - `sz` -- Depth levels per side; max 400 (so up to 800 total entries).
    ///   Defaults to `1` (BBO only) when omitted.
    ///
    /// # Returns
    ///
    /// A single `PmBookDepth` snapshot. The underlying OKX response is
    /// `{"data": [...]}` array-shaped but always returns a single entry; this
    /// unwraps it for caller ergonomics, matching `get_ticker`.
    pub async fn get_pm_books(
        &self,
        inst_id: &str,
        sz: Option<i32>,
    ) -> Result<PmBookDepth, SdkError> {
        let sz_str = sz.map(|v| v.to_string());
        let mut params: Vec<(&str, &str)> = vec![("instId", inst_id)];
        if let Some(ref v) = sz_str {
            params.push(("sz", v));
        }
        let url = format!("{}{}", self.base_url, endpoints::OKX_MARKET_PM_BOOKS_PATH);
        let data: Vec<PmBookDepth> = self.http_get_abs(&url, &params).await?;
        data.into_iter().next().ok_or_else(|| SdkError::Api {
            code: -1,
            message: "pm-books snapshot not found".to_string(),
        })
    }
}

//! Trades (fill history) API methods.

use crate::{client::OutcomesSdkClient, endpoints, error::SdkError, models::trade::TradesResponse};

impl OutcomesSdkClient {
    /// `GET /api/v5/predictions/trades` — Query the authenticated user's trade (fill) history.
    ///
    /// # Parameters
    ///
    /// - `market_id` — Filter by market ID.
    /// - `side` — Filter by direction: `"BUY"` or `"SELL"`.
    /// - `start_time` — Inclusive start timestamp (ms).
    /// - `end_time` — Exclusive end timestamp (ms).
    /// - `cursor` — Pagination cursor from the previous response.
    /// - `limit` — Items per page (max 100, default 20).
    pub async fn get_trades(
        &self,
        market_id: Option<&str>,
        side: Option<&str>,
        start_time: Option<i64>,
        end_time: Option<i64>,
        cursor: Option<&str>,
        limit: Option<i32>,
    ) -> Result<TradesResponse, SdkError> {
        let start_time_str = start_time.map(|v| v.to_string());
        let end_time_str = end_time.map(|v| v.to_string());
        let limit_str = limit.map(|v| v.to_string());
        let mut params: Vec<(&str, &str)> = Vec::new();
        if let Some(v) = market_id {
            params.push(("marketId", v));
        }
        if let Some(v) = side {
            params.push(("side", v));
        }
        if let Some(ref v) = start_time_str {
            params.push(("startTime", v));
        }
        if let Some(ref v) = end_time_str {
            params.push(("endTime", v));
        }
        if let Some(v) = cursor {
            params.push(("cursor", v));
        }
        if let Some(ref v) = limit_str {
            params.push(("limit", v));
        }
        self.http_get(endpoints::TRADES, &params).await
    }
}

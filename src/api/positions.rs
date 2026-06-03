//! Positions API methods — split, merge, redeem, and position queries.

use crate::{
    client::OutcomesSdkClient,
    endpoints,
    error::SdkError,
    models::position::{
        MergeRequest, PositionsResponse, RedeemRequest, SplitRequest, TxHashResponse,
    },
};

impl OutcomesSdkClient {
    /// `GET /api/v5/predictions/positions` — Query the authenticated user's positions.
    ///
    /// # Parameters
    ///
    /// - `status` — `"open"` or `"closed"`; omit for all. Live-verified
    ///   set; any other value is silently mapped to `"open"` server-side.
    /// - `market_id` — Filter by market ID.
    /// - `cursor` — Pagination cursor from the previous response.
    /// - `limit` — Items per page (max 100, default 20).
    pub async fn get_positions(
        &self,
        status: Option<&str>,
        market_id: Option<&str>,
        cursor: Option<&str>,
        limit: Option<i32>,
    ) -> Result<PositionsResponse, SdkError> {
        let limit_str = limit.map(|v| v.to_string());
        let mut params: Vec<(&str, &str)> = Vec::new();
        if let Some(v) = status {
            params.push(("status", v));
        }
        if let Some(v) = market_id {
            params.push(("marketId", v));
        }
        if let Some(v) = cursor {
            params.push(("cursor", v));
        }
        if let Some(ref v) = limit_str {
            params.push(("limit", v));
        }
        self.http_get(endpoints::POSITIONS, &params).await
    }

    /// `POST /api/v5/predictions/positions/split` — Split points into equal YES + NO conditional tokens.
    pub async fn split(&self, req: &SplitRequest) -> Result<TxHashResponse, SdkError> {
        self.http_post(endpoints::SPLIT, req).await
    }

    /// `POST /api/v5/predictions/positions/merge` — Merge equal YES + NO tokens back into pts.
    pub async fn merge(&self, req: &MergeRequest) -> Result<TxHashResponse, SdkError> {
        self.http_post(endpoints::MERGE, req).await
    }

    /// `POST /api/v5/predictions/positions/redeem` — Redeem winning tokens for pts after market settlement (1:1).
    ///
    /// Redeems the caller's full winning token balance (no `size` field required).
    pub async fn redeem(&self, req: &RedeemRequest) -> Result<TxHashResponse, SdkError> {
        self.http_post(endpoints::REDEEM, req).await
    }
}

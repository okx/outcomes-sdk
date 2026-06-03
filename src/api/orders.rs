//! Orders API methods.

use crate::{
    client::OutcomesSdkClient,
    endpoints,
    error::SdkError,
    models::order::{
        CancelAllRequest, CancelOrderRequest, HeartbeatResponse, OrderRecord, OrdersResponse,
        PlaceOrderRequest, TxHashResponse,
    },
};

impl OutcomesSdkClient {
    /// `POST /api/v5/predictions/orders` — Submit a signed limit order.
    ///
    /// The caller is responsible for constructing and EIP-712 signing the calldata.
    pub async fn place_order(&self, req: &PlaceOrderRequest) -> Result<TxHashResponse, SdkError> {
        self.http_post(endpoints::ORDERS, req).await
    }

    /// `POST /api/v5/predictions/orders/cancel` — Cancel a single active order.
    pub async fn cancel_order(&self, req: &CancelOrderRequest) -> Result<TxHashResponse, SdkError> {
        self.http_post(endpoints::ORDERS_CANCEL, req).await
    }

    /// `GET /api/v5/predictions/orders/{orderId}` — Query a single order (active or historical).
    pub async fn get_order(&self, order_id: &str) -> Result<OrderRecord, SdkError> {
        let path = format!("{}/{}", endpoints::ORDERS, order_id);
        self.http_get(&path, &[]).await
    }

    /// `GET /api/v5/predictions/orders` — List the authenticated user's orders with optional filters.
    ///
    /// # Parameters
    ///
    /// - `market_id` — Filter by market ID.
    /// - `status` — `"open"` (pending + active) or `"closed"` (filled / cancelled / expired / failed).
    /// - `cursor` — Pagination cursor from the previous response.
    /// - `limit` — Items per page (max 50, default 20).
    pub async fn list_orders(
        &self,
        market_id: Option<&str>,
        status: Option<&str>,
        cursor: Option<&str>,
        limit: Option<i32>,
    ) -> Result<OrdersResponse, SdkError> {
        let limit_str = limit.map(|v| v.to_string());
        let mut params: Vec<(&str, &str)> = Vec::new();
        if let Some(v) = market_id {
            params.push(("marketId", v));
        }
        if let Some(v) = status {
            params.push(("status", v));
        }
        if let Some(v) = cursor {
            params.push(("cursor", v));
        }
        if let Some(ref v) = limit_str {
            params.push(("limit", v));
        }
        self.http_get(endpoints::ORDERS, &params).await
    }

    /// `POST /api/v5/predictions/orders/cancel-all` — Cancel all active orders.
    ///
    /// Pass specific `asset_ids` in the action to cancel only those markets, or an empty
    /// `Vec` to cancel across all markets.
    pub async fn cancel_all(&self, req: &CancelAllRequest) -> Result<TxHashResponse, SdkError> {
        self.http_post(endpoints::ORDERS_CANCEL_ALL, req).await
    }

    /// `POST /api/v5/predictions/heartbeat` — Renew the dead-man's switch that protects active orders.
    ///
    /// Call this at intervals shorter than 5 minutes. If the heartbeat expires, the
    /// server automatically cancels all active orders using the pre-signed calldata.
    ///
    /// The request body uses the same [`CancelAllRequest`] structure — set `nonce` to
    /// `current_time_ms + 300_000` (5 minutes).
    pub async fn heartbeat(&self, req: &CancelAllRequest) -> Result<HeartbeatResponse, SdkError> {
        self.http_post(endpoints::HEARTBEAT, req).await
    }
}

//! Position model types — split, merge, redeem, and position records.
//!
//! All write operations require an EIP-712 ECDSA signature.

use crate::models::common::SignatureWrapper;

// `TxHashResponse` is the shared write-endpoint response type
// (place_order, cancel, cancel_all, split, merge, redeem). Lives in
// `models::common`; re-exported here so existing
// `models::position::TxHashResponse` imports keep working.
pub use crate::models::common::TxHashResponse;

// ── Split (pts → YES + NO) ───────────────────────────────────────────────────

#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SplitAction {
    /// Always `"predictionSplit"`.
    #[serde(rename = "type")]
    pub action_type: String,
    pub market_id: String,
    /// pts amount in smallest units (e.g. `"100000000"` = 100 pts).
    pub size: String,
}

/// Request body for `POST /api/v5/predictions/positions/split`.
#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
pub struct SplitRequest {
    pub action: SplitAction,
    /// Request timestamp (ms) — anti-replay.
    pub nonce: i64,
    pub signature: SignatureWrapper,
}

// ── Merge (YES + NO → pts) ───────────────────────────────────────────────────

#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MergeAction {
    /// Always `"predictionMerge"`.
    #[serde(rename = "type")]
    pub action_type: String,
    pub market_id: String,
    pub size: String,
}

/// Request body for `POST /api/v5/predictions/positions/merge`.
#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
pub struct MergeRequest {
    pub action: MergeAction,
    pub nonce: i64,
    pub signature: SignatureWrapper,
}

// ── Redeem (settled token → pts) ────────────────────────────────────────────

#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RedeemAction {
    /// Always `"predictionRedeem"`.
    #[serde(rename = "type")]
    pub action_type: String,
    pub market_id: String,
    // No `size` field — redeems the caller's full winning token balance.
}

/// Request body for `POST /api/v5/predictions/positions/redeem`.
#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
pub struct RedeemRequest {
    pub action: RedeemAction,
    pub nonce: i64,
    pub signature: SignatureWrapper,
}

// ── Position Record ───────────────────────────────────────────────────────────

/// A single position record.
#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PositionRecord {
    pub id: String,
    pub token_id: String,
    pub market_id: String,
    /// Token direction: `"1"` = YES, `"2"` = NO.
    pub token_index: String,
    /// Token name: `"Yes"` or `"No"`.
    pub token_name: String,
    /// Current remaining position size.
    pub size: String,
    /// Available position size (= `size` − amount frozen by SELL orders).
    #[serde(default)]
    pub available_size: String,
    /// Current market value (`cur_price × size`).
    pub value: String,
    /// Weighted average entry cost.
    pub avg_price: String,
    /// Unrealized profit/loss.
    pub un_realized_pnl: String,
    /// Unrealized P&L as a percentage.
    pub un_realized_pnl_percentage: String,
    /// Market question text.
    pub title: String,
    /// Market icon URL.
    pub icon: String,
    /// Parent event ID.
    pub event_id: String,
    /// Winning token ID after settlement; `None` while unsettled.
    pub winning_token: Option<String>,
    /// Position status code (see `PositionStatusEnum` in API docs).
    pub position_status: i32,
    /// Current real-time token price.
    pub cur_price: String,
    /// Realized profit/loss.
    pub realized_pnl: String,
    /// Realized P&L as a percentage.
    pub realized_pnl_percentage: String,
    #[serde(default)]
    pub odds_type: crate::models::common::OddsType,
}

/// Response from `GET /api/v5/predictions/positions`.
pub type PositionsResponse = crate::models::common::PagedListResponse<PositionRecord>;

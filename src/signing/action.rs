//! Action types and constructors for EIP-712 signing.
//!
//! `Action` is internally-tagged on the wire with the discriminator key
//! `type`; variant fields land flat next to it. Field declaration order
//! within each variant determines the msgpack byte sequence and therefore
//! the signing hash — reordering is load-bearing.

use serde::Serialize;

use super::types::{CancelRequest, OrderRequest};

/// Action types serialized via rmp-serde for EIP-712 signing.
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum Action {
    #[serde(rename_all = "camelCase")]
    PredictionSplit { market_id: String, size: String },
    #[serde(rename_all = "camelCase")]
    PredictionMerge { market_id: String, size: String },
    #[serde(rename_all = "camelCase")]
    PredictionRedeem { market_id: String },
    /// Field order on the wire: `type, grouping, orders`. `grouping`
    /// defaults to `Na` (the only value OKX currently uses) and emits as
    /// the bare string `"na"`.
    #[serde(rename_all = "camelCase")]
    PlaceOrder {
        grouping: Grouping,
        orders: Vec<OrderRequest>,
    },
    #[serde(rename_all = "camelCase")]
    Cancel { cancels: Vec<CancelRequest> },
    /// Cancel all active orders for the caller. `asset_ids` filters which
    /// markets to cancel in: empty vec = every market, non-empty = those
    /// asset IDs only. Field order is `type, assetIds, marketType`;
    /// `assetIds` is always emitted so the signed bytes line up with what
    /// the backend recomputes.
    #[serde(rename_all = "camelCase")]
    CancelAll {
        asset_ids: Vec<String>,
        market_type: String,
    },
}

/// Order-grouping discriminator. `Na` (the default) is the only value OKX
/// supports today; emitted as the bare string `"na"` per serde's default
/// external tagging of unit variants.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum Grouping {
    #[default]
    Na,
}

// ── Action constructors ─────────────────────────────────────────────

/// Build a PredictionSplit action.
pub fn action_prediction_split(market_id: &str, size: &str) -> Action {
    Action::PredictionSplit {
        market_id: market_id.to_string(),
        size: size.to_string(),
    }
}

pub fn action_prediction_merge(market_id: &str, size: &str) -> Action {
    Action::PredictionMerge {
        market_id: market_id.to_string(),
        size: size.to_string(),
    }
}

pub fn action_prediction_redeem(market_id: &str) -> Action {
    Action::PredictionRedeem {
        market_id: market_id.to_string(),
    }
}

/// Build a PlaceOrder action from a vec of typed [`OrderRequest`]. Uses
/// `Grouping::Na` (the only value OKX currently accepts).
pub fn action_place_order(orders: Vec<OrderRequest>) -> Action {
    Action::PlaceOrder {
        grouping: Grouping::Na,
        orders,
    }
}

/// Build a Cancel action from a vec of typed [`CancelRequest`].
pub fn action_cancel(cancels: Vec<CancelRequest>) -> Action {
    Action::Cancel { cancels }
}

/// Build a CancelAll action.
///
/// `asset_ids` filters which markets are cancelled in: pass an empty `Vec`
/// to cancel across every market for the given `market_type`, or specific
/// asset IDs to cancel only those markets. The OKX backend requires this
/// field on the wire and as part of the signed bytes - omitting it (or
/// silently defaulting to `[]` on only one of the two sides) produces a
/// signature/digest mismatch.
pub fn action_cancel_all(asset_ids: Vec<String>, market_type: &str) -> Action {
    Action::CancelAll {
        asset_ids,
        market_type: market_type.to_string(),
    }
}

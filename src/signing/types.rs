//! Typed inputs to the signing pipeline.
//!
//! These describe the field-level shape of a place-order or cancel request
//! and are the values fed into [`Action::PlaceOrder`] and [`Action::Cancel`].
//!
//! Field declaration order in this file is **load-bearing**: it determines
//! the byte sequence rmp-serde emits, which is in turn what the signing
//! hash is computed over. Reordering struct fields silently changes the
//! signed bytes — the pinned msgpack/hash tests in [`super`] gate this.
//!
//! [`Action::PlaceOrder`]: super::action::Action::PlaceOrder
//! [`Action::Cancel`]: super::action::Action::Cancel

use serde::Serialize;

// `LimitTif`, `LimitOrderType`, `SizeType`, and `SigningOrderSide` are
// wire-shape types shared with the JSON request body, so they live in
// `models::order` (always compiled). Re-exported here so callers
// reaching for `signing::types::*` still see them at this path.
pub use crate::models::order::{LimitOrderType, LimitTif, SigningOrderSide, SizeType};

/// A single place-order request. Field declaration order matches the OKX
/// wire format (`assetId, side, marketType, clientOrderId?, price,
/// reduceOnly, size, sizeType?, orderType`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OrderRequest {
    pub asset_id: String,
    /// See [`SigningOrderSide`]. Field declaration order matters here.
    pub side: SigningOrderSide,
    /// Always `"prediction"` for the outcomes market.
    pub market_type: String,
    /// 34-char client order ID; emitted as `clientOrderId` in msgpack when set.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub client_order_id: Option<String>,
    /// Limit price as a decimal string.
    pub price: String,
    pub reduce_only: bool,
    /// Order size as a decimal string.
    pub size: String,
    /// Defaults to `Base` (omitted on the wire). `Quote` flips to
    /// quote-denominated size.
    #[serde(default, skip_serializing_if = "SizeType::is_base")]
    pub size_type: SizeType,
    pub order_type: OrderType,
}

/// Order type variants. Externally-tagged on the wire — `Limit(_)`
/// serializes as `{ "limit": { "tif": ... } }`. Today only `Limit` is
/// supported; the OKX outcomes API does not document a trigger /
/// stop-loss / take-profit wire shape.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum OrderType {
    Limit(LimitOrderType),
}

/// A single cancel-order request. The cancel-target key (`oid` or
/// `clientOrderId`) is flattened into this map by serde.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CancelRequest {
    pub asset_id: String,
    /// Always `"prediction"` for the outcomes market.
    pub market_type: String,
    #[serde(flatten)]
    pub target: CancelTarget,
}

/// How the order to cancel is identified. Externally-tagged so that
/// `#[serde(flatten)]` on the parent's `target` field promotes the chosen
/// variant key (`oid` or `clientOrderId`) into the parent map.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum CancelTarget {
    /// Server-assigned order ID, as a decimal string. Emitted on the wire
    /// under the msgpack key `"oid"`.
    Oid(String),
    /// Client-assigned order ID, hex-encoded with `0x` prefix. Emitted on
    /// the wire under the msgpack key `"clientOrderId"`; the legacy alias
    /// was `"cloid"`.
    ClientOrderId(String),
}

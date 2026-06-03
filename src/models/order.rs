//! Order model types - request/response structs for all order endpoints.
//!
//! All write operations require an EIP-712 ECDSA signature in the form
//! `{ "Ecdsa": { "r": "0x...", "s": "0x...", "v": 0 } }`.
//! The SDK accepts pre-computed signatures; signing is the caller's responsibility.

use crate::models::common::SignatureWrapper;
#[cfg(feature = "signing")]
use crate::signing::types::{CancelRequest, CancelTarget, OrderRequest, OrderType};

// `TxHashResponse` lives in `models::common` since six write endpoints
// across orders + positions return it. Re-exported here for
// back-compat with existing `models::order::TxHashResponse` imports.
pub use crate::models::common::TxHashResponse;

// -- Place Order ----------------------------------------

/// Limit-order parameters. Wraps a [`LimitTif`] under the `tif` key,
/// matching both the OKX JSON wire shape and the signed msgpack envelope —
/// a single struct serves both transports.
#[derive(Debug, Clone, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LimitOrderType {
    pub tif: LimitTif,
}

/// Time-in-force for a limit order. Unit variants (`Gtc`/`Ioc`/`Fok`/`Alo`)
/// serialize as bare strings (`"gtc"`, `"ioc"`, …). `Gtd { expires_after }`
/// serializes as `{ "gtd": { "expiresAfter": <u64> } }` per OKX's externally-
/// tagged shape. This is serde's default for mixed unit/struct enums; the
/// JSON and msgpack outputs are byte-identical.
///
/// **Placement only.** The query-response side uses different strings
/// (uppercase, plus `POST_ONLY` instead of `alo`) — see [`TimeInForce`].
#[derive(Debug, Clone, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub enum LimitTif {
    Gtc,
    Ioc,
    Fok,
    Alo,
    /// Good-til-date with explicit expiry as a Unix millisecond timestamp.
    #[serde(rename_all = "camelCase")]
    Gtd {
        expires_after: u64,
    },
}

/// Time-in-force as it appears in `OrderRecord.orderType` on the query
/// response. Distinct from [`LimitTif`] because OKX uses different
/// strings on the placement and response sides:
///
/// | concept | placement (`LimitTif`) | response (`TimeInForce`) |
/// |---|---|---|
/// | post-only | `alo` | `POST_ONLY` |
/// | others | `gtc`/`gtd`/`ioc`/`fok` | `GTC`/`GTD`/`IOC`/`FOK` |
///
/// Verified live: an ALO-placed order reads back with `"orderType": "POST_ONLY"`.
/// Do **not** consolidate with `LimitTif`.
#[derive(Debug, Clone, PartialEq, Eq, Default, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum TimeInForce {
    Gtc,
    Gtd,
    Ioc,
    Fok,
    PostOnly,
    #[default]
    #[serde(other)]
    Unknown,
}

impl TimeInForce {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Gtc => "GTC",
            Self::Gtd => "GTD",
            Self::Ioc => "IOC",
            Self::Fok => "FOK",
            Self::PostOnly => "POST_ONLY",
            Self::Unknown => "UNKNOWN",
        }
    }
}

impl std::fmt::Display for TimeInForce {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

/// `buy` / `sell` — order direction on the **placement** wire. Lowercase
/// because these bytes feed into the EIP-712 signature hash; any change
/// to the serialization (casing, variant set, attribute) silently breaks
/// every signed action.
///
/// Distinct from [`crate::models::common::OrderSide`], the response-side
/// enum that uses uppercase `BUY` / `SELL`. The two are intentionally
/// not unified — see the OKX wire-format asymmetry note. Notably this
/// enum has **no `Unknown` variant**: unknown values must fail to
/// deserialize, because we can't ask the server to validate an
/// already-signed unknown side.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "lowercase")]
pub enum SigningOrderSide {
    Buy,
    Sell,
}

impl SigningOrderSide {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Buy => "buy",
            Self::Sell => "sell",
        }
    }
}

impl std::fmt::Display for SigningOrderSide {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

impl std::str::FromStr for SigningOrderSide {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "buy" => Ok(Self::Buy),
            "sell" => Ok(Self::Sell),
            other => Err(format!("invalid side {other:?}: expected `buy` or `sell`")),
        }
    }
}

/// Order size unit. `Base` is the default and is omitted on the wire.
/// Serializes/deserializes as the bare strings `"base"` / `"quote"`.
///
/// **Placement only.** The query-response side uses uppercase
/// (`"BASE"` / `"QUOTE"`) and is always present — see [`OrderSizeType`].
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub enum SizeType {
    #[default]
    Base,
    Quote,
}

impl SizeType {
    pub fn is_base(&self) -> bool {
        matches!(self, SizeType::Base)
    }
}

/// Size unit as it appears in `OrderRecord.sizeType` on the query
/// response — uppercase, always present. Distinct from [`SizeType`]
/// which is the lowercase, omit-if-base placement enum.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum OrderSizeType {
    Base,
    Quote,
    #[default]
    #[serde(other)]
    Unknown,
}

impl OrderSizeType {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Base => "BASE",
            Self::Quote => "QUOTE",
            Self::Unknown => "UNKNOWN",
        }
    }
}

impl std::fmt::Display for OrderSizeType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

/// `OrderRecord.status` lifecycle. Distinct from the WS `OrderStatus`
/// (which has split `PLACE_FAILED` / `CANCEL_FAILED` variants); the
/// REST response collapses both into a single `FAILED` plus a
/// `failReason` string.
#[derive(Debug, Clone, PartialEq, Eq, Default, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum RestOrderStatus {
    PendingPlace,
    Active,
    PendingCancel,
    Filled,
    PartiallyFilled,
    Failed,
    Cancelled,
    Expired,
    #[default]
    #[serde(other)]
    Unknown,
}

impl RestOrderStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::PendingPlace => "PENDING_PLACE",
            Self::Active => "ACTIVE",
            Self::PendingCancel => "PENDING_CANCEL",
            Self::Filled => "FILLED",
            Self::PartiallyFilled => "PARTIALLY_FILLED",
            Self::Failed => "FAILED",
            Self::Cancelled => "CANCELLED",
            Self::Expired => "EXPIRED",
            Self::Unknown => "UNKNOWN",
        }
    }
}

impl std::fmt::Display for RestOrderStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Order type wrapper. The wire format only carries a `limit` shape.
#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
pub struct OrderTypeSpec {
    pub limit: LimitOrderType,
}

/// A single order entry within a place-order action.
///
/// Field order matches the OKX place-order wire shape so the hand-written
/// msgpack encoder and serde JSON serializer produce the same canonical
/// wire order:
///   `assetId, side, marketType, clientOrderId?, price, reduceOnly, size, sizeType?, orderType`
#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OrderItem {
    /// Outcome asset ID (from `OutcomeObject.asset_id`).
    pub asset_id: String,
    pub side: SigningOrderSide,
    /// Always `"prediction"`.
    pub market_type: String,
    /// Spec-compliant 34-char client order ID (`0x{region}{env}{30-hex random}`).
    /// With the `signing` feature enabled, generate via
    /// `okx_outcomes_sdk::signing::generate_client_order_id_default`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub client_order_id: Option<String>,
    /// Limit price as decimal string, e.g. `"0.65"`.
    pub price: String,
    /// Reduce-only flag (default `false`).
    pub reduce_only: bool,
    /// Order size, e.g. `"100"`.
    pub size: String,
    /// Defaults to `Base` (omitted on the wire). `Quote` flips to
    /// quote-denominated size.
    #[serde(default, skip_serializing_if = "SizeType::is_base")]
    pub size_type: SizeType,
    pub order_type: OrderTypeSpec,
}

/// Build the wire-format [`OrderItem`] from the signing-side [`OrderRequest`].
///
/// This is the canonical CLI flow: the caller constructs `OrderRequest`
/// (which gets fed into the signed action) and derives the matching
/// `OrderItem` for the JSON request body, avoiding double-construction of
/// the same field values.
#[cfg(feature = "signing")]
impl From<&OrderRequest> for OrderItem {
    fn from(req: &OrderRequest) -> Self {
        let OrderType::Limit(limit) = &req.order_type;
        OrderItem {
            asset_id: req.asset_id.clone(),
            side: req.side,
            market_type: req.market_type.clone(),
            client_order_id: req.client_order_id.clone(),
            price: req.price.clone(),
            reduce_only: req.reduce_only,
            size: req.size.clone(),
            size_type: req.size_type,
            order_type: OrderTypeSpec {
                limit: limit.clone(),
            },
        }
    }
}

/// The `action` object for `POST /api/v5/predictions/orders`.
#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
pub struct PlaceOrderAction {
    /// Always `"placeOrder"`.
    #[serde(rename = "type")]
    pub action_type: String,
    /// Always `"na"`.
    pub grouping: String,
    pub orders: Vec<OrderItem>,
}

/// Request body for `POST /api/v5/predictions/orders`.
#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
pub struct PlaceOrderRequest {
    pub action: PlaceOrderAction,
    /// Request timestamp (ms) - anti-replay.
    pub nonce: i64,
    pub signature: SignatureWrapper,
}

// -- Cancel Order ----------------------------------------

/// A single cancel entry within a cancel action.
///
/// The order to cancel is identified by [`CancelBy`] - exactly one of
/// `oid` or `clientOrderId` ends up on the wire, enforced by the enum
/// rather than by runtime convention.
#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CancelItem {
    pub asset_id: String,
    /// Always `"prediction"`.
    pub market_type: String,
    /// Identifier used to select the order to cancel.
    #[serde(flatten)]
    pub by: CancelBy,
}

/// Discriminator for `CancelItem`: pick one of server-assigned `oid` or
/// client-assigned `clientOrderId`.
///
/// `#[serde(flatten)]` on `CancelItem.by` inlines the variant's single
/// field directly into the parent JSON object - so on the wire you see
/// `{"assetId": ..., "marketType": ..., "oid": "578840"}` or
/// `{"assetId": ..., "marketType": ..., "clientOrderId": "0x..."}`,
/// never both.
///
/// `rename_all = "camelCase"` lives on the `ClientOrderId` variant because
/// serde's enum-level `rename_all` only renames variant identifiers; for
/// struct-style variant fields the rename has to be on the variant itself.
#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
#[serde(untagged)]
pub enum CancelBy {
    /// Server-assigned order ID, as a decimal string.
    Oid { oid: String },
    /// Client-assigned order ID, hex-encoded with `0x` prefix.
    #[serde(rename_all = "camelCase")]
    ClientOrderId { client_order_id: String },
}

/// Build the wire-format [`CancelItem`] from the signing-side
/// [`CancelRequest`].
///
/// The signing layer's [`CancelTarget`] variant determines whether `oid` or
/// `client_order_id` is set on the wire; building both layers from a single
/// `CancelRequest` keeps the signed bytes and the JSON body in sync.
#[cfg(feature = "signing")]
impl From<&CancelRequest> for CancelItem {
    fn from(req: &CancelRequest) -> Self {
        let by = match &req.target {
            CancelTarget::Oid(o) => CancelBy::Oid { oid: o.clone() },
            CancelTarget::ClientOrderId(c) => CancelBy::ClientOrderId {
                client_order_id: c.clone(),
            },
        };
        CancelItem {
            asset_id: req.asset_id.clone(),
            market_type: req.market_type.clone(),
            by,
        }
    }
}

/// The `action` object for `POST /api/v5/predictions/orders/cancel`.
#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
pub struct CancelOrderAction {
    /// Always `"cancel"`.
    #[serde(rename = "type")]
    pub action_type: String,
    pub cancels: Vec<CancelItem>,
}

/// Request body for `POST /api/v5/predictions/orders/cancel`.
#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
pub struct CancelOrderRequest {
    pub action: CancelOrderAction,
    pub nonce: i64,
    pub signature: SignatureWrapper,
}

// -- Cancel All ----------------------------------------

/// The `action` object for `POST /api/v5/predictions/orders/cancel-all` and `POST /api/v5/predictions/heartbeat`.
#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CancelAllAction {
    /// Always `"cancelAll"`. Must match the signed msgpack tag - see
    /// [`crate::signing::Action::CancelAll`].
    #[serde(rename = "type")]
    pub action_type: String,
    /// Asset IDs to cancel; pass an empty `Vec` to cancel orders across all markets,
    /// or specific IDs to cancel only those markets.
    pub asset_ids: Vec<String>,
    /// Always `"prediction"`.
    pub market_type: String,
}

/// Request body for `POST /api/v5/predictions/orders/cancel-all` and `POST /api/v5/predictions/heartbeat`.
///
/// For heartbeat: set `nonce` to `current_time_ms + 300_000` (5 minutes ahead).
#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CancelAllRequest {
    pub action: CancelAllAction,
    pub nonce: i64,
    /// Expiry timestamp (ms).
    pub expires_after: i64,
    pub signature: SignatureWrapper,
}

// -- Heartbeat ----------------------------------------

/// Response from `POST /api/v5/predictions/heartbeat`.
#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HeartbeatResponse {
    /// Server's current timestamp (ms).
    pub server_timestamp: i64,
    /// Timestamp at which this heartbeat expires (ms).
    pub expire_at: i64,
}

// -- Order Record ----------------------------------------

/// A single order record returned by query endpoints.
#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OrderRecord {
    pub id: String,
    /// Order oid (distinct from `id`).
    #[serde(default)]
    pub oid: String,
    pub market_id: String,
    /// YES/NO token contract address.
    #[serde(default)]
    pub token_id: String,
    pub asset_id: String,
    /// Client-assigned order ID supplied at placement time;
    /// `None` for orders placed without one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub client_order_id: Option<String>,
    pub side: crate::models::common::OrderSide,
    pub order_type: TimeInForce,
    pub size_type: OrderSizeType,
    pub size: String,
    pub price: String,
    /// GTD expiry timestamp (ms, as string); `None` for non-GTD orders.
    pub expiration: Option<String>,
    pub tx_hash: String,
    pub status: RestOrderStatus,
    pub filled_size: String,
    pub filled_amount: String,
    /// Failure reason; present only when `status == "FAILED"`.
    pub fail_reason: Option<String>,
    /// System-triggered cancellation reason.
    pub cancel_reason: Option<String>,
    #[serde(default)]
    pub odds_type: crate::models::common::OddsType,
    pub created_at: String,
    pub updated_at: String,
}

/// Response from `GET /api/v5/predictions/orders`.
pub type OrdersResponse = crate::models::common::PagedListResponse<OrderRecord>;

#[cfg(all(test, feature = "signing"))]
mod try_from_order_request_tests {
    use super::*;
    use crate::signing::types::{LimitOrderType, LimitTif, OrderRequest, OrderType, SizeType};

    fn limit_plain_request() -> OrderRequest {
        OrderRequest {
            asset_id: "63000".to_string(),
            side: SigningOrderSide::Buy,
            market_type: "prediction".to_string(),
            client_order_id: Some("0x0197a98c91312671ca83f15ccbd5186f".to_string()),
            price: "0.5".to_string(),
            reduce_only: false,
            size: "1.0".to_string(),
            size_type: SizeType::Base,
            order_type: OrderType::Limit(LimitOrderType { tif: LimitTif::Gtc }),
        }
    }

    #[test]
    fn limit_plain_round_trips_field_by_field() {
        let req = limit_plain_request();
        let item: OrderItem = (&req).into();

        assert_eq!(item.asset_id, req.asset_id);
        assert_eq!(item.side, req.side);
        assert_eq!(item.market_type, req.market_type);
        assert_eq!(item.client_order_id, req.client_order_id);
        assert_eq!(item.price, req.price);
        assert_eq!(item.reduce_only, req.reduce_only);
        assert_eq!(item.size, req.size);
        // SizeType is now the same enum on both sides.
        assert_eq!(item.size_type, SizeType::Base);
        assert!(
            matches!(&item.order_type.limit.tif, LimitTif::Gtc),
            "expected Gtc, got {:?}",
            &item.order_type.limit.tif
        );
    }

    fn limit_gtd_request() -> OrderRequest {
        OrderRequest {
            order_type: OrderType::Limit(LimitOrderType {
                tif: LimitTif::Gtd {
                    expires_after: 1_800_000_000_000,
                },
            }),
            ..limit_plain_request()
        }
    }

    #[test]
    fn gtd_converts_with_expires_after_as_u64() {
        let req = limit_gtd_request();
        let item: OrderItem = (&req).into();

        match &item.order_type.limit.tif {
            LimitTif::Gtd { expires_after } => {
                assert_eq!(*expires_after, 1_800_000_000_000_u64);
            }
            other => panic!("expected Gtd variant, got {other:?}"),
        }
    }

    #[test]
    fn gtd_serializes_with_nested_tif_object() {
        let req = limit_gtd_request();
        let item: OrderItem = (&req).into();
        let json = serde_json::to_string(&item).unwrap_or_else(|_| unreachable!());

        assert!(
            json.contains("\"tif\":{\"gtd\":{\"expiresAfter\":1800000000000}}"),
            "nested GTD tif missing from {json}"
        );
        assert!(
            !json.contains("\"expiresAfter\":\""),
            "expiresAfter must be a JSON number, not a quoted string: {json}"
        );
        assert!(
            !json.contains("\"tif\":\"gtd\""),
            "GTD must not serialize as flat string tif: {json}"
        );
        assert!(
            !json.contains("\"expiration\""),
            "legacy expiration key must not appear on the wire: {json}"
        );
    }

    #[test]
    fn gtd_round_trips_through_json() {
        let req = limit_gtd_request();
        let original: OrderItem = (&req).into();
        let json = serde_json::to_string(&original).unwrap_or_else(|_| unreachable!());
        let round_tripped: OrderItem =
            serde_json::from_str(&json).unwrap_or_else(|e| panic!("deserialize failed: {e}"));

        match (
            &original.order_type.limit.tif,
            &round_tripped.order_type.limit.tif,
        ) {
            (LimitTif::Gtd { expires_after: o }, LimitTif::Gtd { expires_after: r }) => {
                assert_eq!(o, r);
            }
            (o, r) => panic!("expected Gtd on both sides, got {o:?} vs {r:?}"),
        }
    }

    #[test]
    fn plain_round_trips_through_json() {
        let req = limit_plain_request();
        let original: OrderItem = (&req).into();
        let json = serde_json::to_string(&original).unwrap_or_else(|_| unreachable!());
        let round_tripped: OrderItem =
            serde_json::from_str(&json).unwrap_or_else(|e| panic!("deserialize failed: {e}"));

        match (
            &original.order_type.limit.tif,
            &round_tripped.order_type.limit.tif,
        ) {
            (LimitTif::Gtc, LimitTif::Gtc) => {}
            (o, r) => panic!("expected Gtc on both sides, got {o:?} vs {r:?}"),
        }
    }

    #[test]
    fn plain_serializes_as_bare_string_and_omits_expires_after() {
        let req = limit_plain_request();
        let item: OrderItem = (&req).into();
        let json = serde_json::to_string(&item).unwrap_or_else(|_| unreachable!());

        assert!(
            matches!(&item.order_type.limit.tif, LimitTif::Gtc),
            "expected Gtc, got {:?}",
            &item.order_type.limit.tif
        );
        assert!(
            json.contains("\"tif\":\"gtc\""),
            "plain TIF must serialize as bare string: {json}"
        );
        assert!(
            !json.contains("expiresAfter"),
            "expiresAfter must be absent for non-GTD orders: {json}"
        );
    }

    // -- CancelItem wire shape ---------------------------------------

    #[test]
    fn cancel_item_oid_serializes_with_oid_only() {
        let req = CancelRequest {
            asset_id: "100288000".to_string(),
            market_type: "prediction".to_string(),
            target: CancelTarget::Oid("578840".to_string()),
        };
        let item = CancelItem::from(&req);
        assert!(
            matches!(&item.by, CancelBy::Oid { oid } if oid == "578840"),
            "expected Oid variant, got {:?}",
            item.by
        );

        let json = serde_json::to_string(&item).unwrap_or_else(|_| unreachable!());
        assert!(json.contains("\"oid\":\"578840\""), "oid missing: {json}");
        assert!(
            !json.contains("clientOrderId"),
            "clientOrderId should be omitted when oid is set: {json}"
        );
    }

    #[test]
    fn cancel_item_client_order_id_serializes_with_client_order_id_only() {
        let req = CancelRequest {
            asset_id: "100288000".to_string(),
            market_type: "prediction".to_string(),
            target: CancelTarget::ClientOrderId("0x0197a98c91312671ca83f15ccbd5186f".to_string()),
        };
        let item = CancelItem::from(&req);
        assert!(
            matches!(
                &item.by,
                CancelBy::ClientOrderId { client_order_id }
                if client_order_id == "0x0197a98c91312671ca83f15ccbd5186f"
            ),
            "expected ClientOrderId variant, got {:?}",
            item.by
        );

        let json = serde_json::to_string(&item).unwrap_or_else(|_| unreachable!());
        assert!(
            json.contains("\"clientOrderId\":\"0x0197a98c91312671ca83f15ccbd5186f\""),
            "clientOrderId missing: {json}"
        );
        assert!(
            !json.contains("\"oid\""),
            "oid should be omitted when client order ID is set: {json}"
        );
    }

    /// Round-trip: serialize a `CancelItem` then deserialize the resulting
    /// JSON back into a `CancelItem`, and check the recovered value matches
    /// the original. Catches drift in serde rename / `untagged` matching:
    /// e.g. if the `oid` and `clientOrderId` variant order ever flipped or
    /// the `rename_all` on `ClientOrderId` was dropped, deserialize would
    /// pick the wrong variant and this test would fail.
    #[test]
    fn cancel_item_round_trips_both_variants() {
        for target in [
            CancelTarget::Oid("578840".to_string()),
            CancelTarget::ClientOrderId("0x0197a98c91312671ca83f15ccbd5186f".to_string()),
        ] {
            let req = CancelRequest {
                asset_id: "100288000".to_string(),
                market_type: "prediction".to_string(),
                target,
            };
            let original = CancelItem::from(&req);
            let json = serde_json::to_string(&original).unwrap_or_else(|_| unreachable!());
            let round_tripped: CancelItem = serde_json::from_str(&json).unwrap_or_else(|e| {
                panic!("deserialize failed for {json}: {e}");
            });

            assert_eq!(round_tripped.asset_id, original.asset_id);
            assert_eq!(round_tripped.market_type, original.market_type);
            match (&original.by, &round_tripped.by) {
                (CancelBy::Oid { oid: a }, CancelBy::Oid { oid: b }) => assert_eq!(a, b),
                (
                    CancelBy::ClientOrderId { client_order_id: a },
                    CancelBy::ClientOrderId { client_order_id: b },
                ) => assert_eq!(a, b),
                _ => panic!(
                    "variant mismatch after round-trip: {:?} -> {:?}",
                    original.by, round_tripped.by
                ),
            }
        }
    }

    #[test]
    fn order_record_response_enums_route_unknown_values_to_unknown() {
        // Forward-compat: if OKX ever adds a new orderType / sizeType /
        // status before the SDK is updated, the response must still
        // deserialize cleanly. The new values land in `Unknown`.
        let json = r#"{
            "id": "1", "oid": "1", "marketId": "2",
            "tokenId": "3", "assetId": "4",
            "side": "WAT",
            "orderType": "NEW_TIF",
            "sizeType": "PERCENT",
            "size": "0", "price": "0",
            "expiration": null,
            "txHash": "",
            "status": "STUCK",
            "filledSize": "0", "filledAmount": "0",
            "failReason": null,
            "cancelReason": null,
            "oddsType": "",
            "createdAt": "0", "updatedAt": "0"
        }"#;
        let rec: OrderRecord = serde_json::from_str(json).expect("deserialize");
        assert_eq!(rec.order_type, TimeInForce::Unknown);
        assert_eq!(rec.size_type, OrderSizeType::Unknown);
        assert_eq!(rec.status, RestOrderStatus::Unknown);
    }

    #[test]
    fn order_record_response_enums_decode_known_values() {
        // Verified live: an ALO-placed order reads back with orderType
        // "POST_ONLY" (uppercase, distinct from placement's "alo").
        let json = r#"{
            "id": "12299709", "oid": "12299709", "marketId": "100888",
            "tokenId": "1777", "assetId": "100888000",
            "side": "BUY",
            "orderType": "POST_ONLY",
            "sizeType": "BASE",
            "size": "10", "price": "0.45",
            "expiration": "",
            "txHash": "0xabc",
            "status": "ACTIVE",
            "filledSize": "0", "filledAmount": "0",
            "failReason": "",
            "cancelReason": "",
            "oddsType": "points",
            "createdAt": "0", "updatedAt": "0"
        }"#;
        let rec: OrderRecord = serde_json::from_str(json).expect("deserialize");
        assert_eq!(rec.order_type, TimeInForce::PostOnly);
        assert_eq!(rec.size_type, OrderSizeType::Base);
        assert_eq!(rec.status, RestOrderStatus::Active);
    }

    #[test]
    fn signing_order_side_serializes_lowercase() {
        // Wire-format pin: SigningOrderSide MUST emit lowercase bytes
        // because they feed into the EIP-712 signature hash. If this
        // ever changes, every signed action stops verifying server-side.
        assert_eq!(
            serde_json::to_string(&SigningOrderSide::Buy).unwrap(),
            "\"buy\""
        );
        assert_eq!(
            serde_json::to_string(&SigningOrderSide::Sell).unwrap(),
            "\"sell\""
        );
    }

    #[test]
    fn signing_order_side_rejects_unknown_values() {
        // Strict deserialize: unknown sides must fail rather than land
        // in an `Unknown` variant. Signing has no recovery path for an
        // unrecognized side — we'd just produce a signature the server
        // rejects.
        let err = serde_json::from_str::<SigningOrderSide>("\"WAT\"").unwrap_err();
        assert!(err.to_string().contains("unknown variant"));
        assert!("WAT".parse::<SigningOrderSide>().is_err());
    }
}

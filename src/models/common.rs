//! Common types shared across all API groups.

// ---------------------------------------------------------------------------
// Cross-surface wire enums
// ---------------------------------------------------------------------------
//
// These two enums appear identically in both REST responses and WS pushes,
// so they live here rather than in either `models::*` or `ws::models`. Each
// has a `#[serde(other)] Unknown` catch-all so a new server-side value
// degrades gracefully into the `Unknown` variant rather than failing the
// whole payload.
//
// Note these are the *response/read* side. The signed-placement side uses
// different casing (lowercase `buy`/`sell`) and a separate enum
// (`SigningOrderSide`) — see the OKX wire-format asymmetry doc in
// `signing.rs`. Do not consolidate.

/// `BUY` / `SELL` — trade direction on REST and WS response payloads.
#[derive(Debug, Clone, PartialEq, Eq, Default, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum OrderSide {
    Buy,
    Sell,
    #[default]
    #[serde(other)]
    Unknown,
}

impl OrderSide {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Buy => "BUY",
            Self::Sell => "SELL",
            Self::Unknown => "UNKNOWN",
        }
    }
}

impl std::fmt::Display for OrderSide {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

/// `points` (points market) — odds type. `Unknown` is the catch-all for any
/// other value the server might send, so deserialization degrades gracefully
/// instead of failing the whole payload.
#[derive(Debug, Clone, PartialEq, Eq, Default, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum OddsType {
    Points,
    #[default]
    #[serde(other)]
    Unknown,
}

impl OddsType {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Points => "points",
            Self::Unknown => "unknown",
        }
    }
}

impl std::fmt::Display for OddsType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Deserialize `Option<String>` treating empty strings as `None`. Handles a
/// missing field, JSON `null`, and `""` uniformly. Useful for OKX market-data
/// endpoints that emit `""` instead of `null` when no liquidity exists.
pub(crate) mod opt_string_empty_as_none {
    use serde::{self, Deserialize, Deserializer};

    pub fn deserialize<'de, D>(deserializer: D) -> Result<Option<String>, D::Error>
    where
        D: Deserializer<'de>,
    {
        let opt: Option<String> = Option::deserialize(deserializer)?;
        Ok(opt.filter(|s| !s.is_empty()))
    }
}

/// Deserialize a `code` field that the API may send either as a JSON number
/// (`0`) or, more commonly, as a numeric string (`"0"`, `"50105"`). OKX's v5
/// API uses the string form on the wire, but the spec tables document it as an
/// integer, so accept both shapes and normalize to `i64`.
pub(crate) fn de_code_i64<'de, D>(deserializer: D) -> Result<i64, D::Error>
where
    D: serde::Deserializer<'de>,
{
    use serde::de::Error;
    use serde::Deserialize;

    #[derive(Deserialize)]
    #[serde(untagged)]
    enum StringOrInt {
        Int(i64),
        Str(String),
    }

    match StringOrInt::deserialize(deserializer)? {
        StringOrInt::Int(n) => Ok(n),
        StringOrInt::Str(s) => s
            .parse::<i64>()
            .map_err(|_| Error::custom(format!("invalid numeric code string: {s:?}"))),
    }
}

/// Outer envelope used by every OKX Outcomes API response.
///
/// `code == 0` means success; any other value is an error and `data` will be `null`.
#[derive(serde::Deserialize)]
pub(crate) struct ApiEnvelope<T> {
    #[serde(deserialize_with = "de_code_i64")]
    pub code: i64,
    /// Accepts both `"message"` (production API) and `"msg"` (internal test endpoints).
    #[serde(alias = "msg")]
    pub message: String,
    pub data: Option<T>,
}

/// Bare error body returned on a non-2xx HTTP response. Unlike [`ApiEnvelope`]
/// it carries no `data` field; the backend emits only `{ "code": ..., "msg":
/// ... }`. `code` may be a JSON string or number (see [`de_code_i64`]), and the
/// message key is `msg` (aliased from `message` for the rare endpoint that uses
/// the long form).
#[derive(serde::Deserialize)]
pub(crate) struct ApiErrorBody {
    #[serde(deserialize_with = "de_code_i64")]
    pub code: i64,
    #[serde(alias = "message")]
    pub msg: String,
}

/// Outer envelope used by the OKX market data API (`/api/v5/market/*`).
///
/// This API uses `"code": "0"` (a string) and `"msg"` (not `"message"`).
#[derive(serde::Deserialize)]
pub(crate) struct OkxMarketEnvelope<T> {
    pub code: String,
    pub msg: String,
    pub data: Option<T>,
}

/// Returned by every write endpoint that submits a signed action
/// (place order, cancel order, cancel all, split, merge, redeem). The
/// `tx_hash` is the on-chain transaction hash of the resulting action.
#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TxHashResponse {
    pub tx_hash: String,
}

// ---------------------------------------------------------------------------
// Two pagination shapes — kept separate on purpose
// ---------------------------------------------------------------------------
//
// OKX's REST API ships two different paginated-response wire formats and
// the SDK models both faithfully rather than trying to unify them:
//
//   orders / trades / positions       events
//   ----------------------------      -------------------------------------
//   {                                 {
//     "list": [...],                    "events": [...],
//     "nextCursor": "...",              "pagination": {
//     "hasNext": true                     "nextCursor": "...",
//   }                                     "hasMore": true,
//                                         "pageSize": 20
//                                       }
//                                     }
//
// Differences that resist consolidation:
//   1. Items field key: `list` vs `events` (per-endpoint string).
//   2. Pagination layout: flat alongside items vs nested under `pagination`.
//   3. Pagination field names: `hasNext` vs `hasMore`; events also carries
//      `pageSize` which the flat form omits.
//
// A single generic could only cover both shapes by adding hardcoded
// per-endpoint `serde(rename = ...)` attributes — which would defeat the
// genericity. Cleaner to keep two types and let the wire-format
// asymmetry be visible in the code.

/// Flat paged-response shape used by orders / trades / positions.
/// Per-endpoint aliases (`OrdersResponse`, `TradesResponse`,
/// `PositionsResponse`) keep call sites readable and let docs surface
/// the concrete element type.
#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PagedListResponse<T> {
    pub list: Vec<T>,
    pub next_cursor: Option<String>,
    pub has_next: bool,
}

/// Nested pagination block — used inside `EventsResponse.pagination`,
/// not as a top-level response shape. See the comment above for why
/// this lives alongside [`PagedListResponse`] rather than replacing it.
#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Pagination {
    /// Cursor to pass in the next request; `None` when this is the last page.
    /// The server emits `""` (empty string) for "no cursor" — coerced to
    /// `None` here so call sites can rely on the Option semantics.
    #[serde(default, deserialize_with = "opt_string_empty_as_none::deserialize")]
    pub next_cursor: Option<String>,
    /// Whether more data exists beyond this page.
    pub has_more: bool,
    /// Number of items returned in this page.
    pub page_size: i32,
}

/// ECDSA signature components for write operations.
#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
pub struct EcdsaSignature {
    /// r component (hex string, e.g. `"0xabcdef..."`).
    pub r: String,
    /// s component (hex string, e.g. `"0xabcdef..."`).
    pub s: String,
    /// Recovery id: `0` or `1`.
    pub v: u8,
}

/// Signature wrapper matching the API's `{ "Ecdsa": { r, s, v } }` envelope.
#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
pub struct SignatureWrapper {
    #[serde(rename = "Ecdsa")]
    pub ecdsa: EcdsaSignature,
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `search` returns `nextCursor: ""` (empty string) when there's no
    /// usable cursor — even alongside `hasMore: true`. Coerce empty to
    /// `None` so consumers don't print `--cursor ` with a missing value.
    #[test]
    fn pagination_coerces_empty_next_cursor_to_none() {
        let json = r#"{"nextCursor":"","hasMore":true,"pageSize":3}"#;
        let p: Pagination = serde_json::from_str(json).expect("deserialize");
        assert!(p.next_cursor.is_none());
        assert!(p.has_more);
        assert_eq!(p.page_size, 3);

        // Real cursor strings pass through unchanged.
        let json = r#"{"nextCursor":"abc123","hasMore":true,"pageSize":3}"#;
        let p: Pagination = serde_json::from_str(json).expect("deserialize");
        assert_eq!(p.next_cursor.as_deref(), Some("abc123"));
    }

    /// OKX v5 sends `code` as a numeric *string* (`"0"`, `"50105"`), but the
    /// spec tables document it as an integer. The envelope must accept both
    /// shapes — a regression here surfaces as a `Deserialize` error that masks
    /// the real API error message (e.g. `OK-ACCESS-PASSPHRASE incorrect`).
    #[test]
    fn envelope_accepts_string_and_numeric_code() {
        // String error code (the production wire form).
        let json = r#"{"code":"50105","msg":"Request header OK-ACCESS-PASSPHRASE incorrect.","data":null}"#;
        let env: ApiEnvelope<()> = serde_json::from_str(json).expect("string code");
        assert_eq!(env.code, 50105);
        assert_eq!(
            env.message,
            "Request header OK-ACCESS-PASSPHRASE incorrect."
        );

        // String success code.
        let json = r#"{"code":"0","msg":"","data":null}"#;
        let env: ApiEnvelope<()> = serde_json::from_str(json).expect("string zero");
        assert_eq!(env.code, 0);

        // Numeric code (spec-table form) still works.
        let json = r#"{"code":0,"message":"ok","data":null}"#;
        let env: ApiEnvelope<()> = serde_json::from_str(json).expect("numeric code");
        assert_eq!(env.code, 0);
    }
}

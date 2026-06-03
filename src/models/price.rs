//! Market data model types.
//!
//! These types are used with the OKX market data API
//! (`https://www.okx.com/api/v5/market/`), which has a different base URL
//! and envelope format from the outcomes API.
//!
//! Pass the outcome market's `yesOutcome.assetId` as the `instId` parameter.

/// Market ticker snapshot returned by `GET /api/v5/market/ticker`.
#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Ticker {
    pub inst_type: String,
    pub inst_id: String,
    /// Last trade price. `None` when no trades have occurred (API may emit `""`).
    #[serde(
        default,
        deserialize_with = "super::common::opt_string_empty_as_none::deserialize"
    )]
    pub last: Option<String>,
    /// Last trade size. `None` when no trades have occurred.
    #[serde(
        default,
        deserialize_with = "super::common::opt_string_empty_as_none::deserialize"
    )]
    pub last_sz: Option<String>,
    /// Best ask price. `None` when no asks exist (API may emit `""`).
    #[serde(
        default,
        deserialize_with = "super::common::opt_string_empty_as_none::deserialize"
    )]
    pub ask_px: Option<String>,
    /// Best ask size. `None` when no asks exist.
    #[serde(
        default,
        deserialize_with = "super::common::opt_string_empty_as_none::deserialize"
    )]
    pub ask_sz: Option<String>,
    /// Best bid price. `None` when no bids exist (API may emit `""`).
    #[serde(
        default,
        deserialize_with = "super::common::opt_string_empty_as_none::deserialize"
    )]
    pub bid_px: Option<String>,
    /// Best bid size. `None` when no bids exist.
    #[serde(
        default,
        deserialize_with = "super::common::opt_string_empty_as_none::deserialize"
    )]
    pub bid_sz: Option<String>,
    /// 24-hour opening price.
    pub open24h: String,
    /// 24-hour high price.
    pub high24h: String,
    /// 24-hour low price.
    pub low24h: String,
    /// 24-hour volume (in contracts/units).
    pub vol24h: String,
    /// 24-hour volume (in quote currency).
    pub vol_ccy24h: String,
    /// UTC 0 opening price.
    pub sod_utc0: String,
    /// UTC+8 opening price.
    pub sod_utc8: String,
    /// Data update timestamp (Unix ms as string).
    pub ts: String,
}

/// A single K-line (candlestick) bar returned by `GET /api/v5/market/candles`.
///
/// The API returns a 2-D array; each inner array maps to:
///
/// | Index | Field | Description |
/// |-------|-------|-------------|
/// | 0 | `ts` | K-line start time (Unix ms) |
/// | 1 | `o` | Open price |
/// | 2 | `h` | High price |
/// | 3 | `l` | Low price |
/// | 4 | `c` | Close price |
/// | 5 | `vol` | Volume (contracts) |
/// | 6 | `vol_ccy` | Volume (pricing currency) |
/// | 7 | `vol_ccy_quote` | Volume (quote currency) |
/// | 8 | `confirm` | `"0"` = unconfirmed, `"1"` = confirmed |
#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
pub struct Candle(pub Vec<String>);

/// Outcome market order book depth snapshot returned by
/// `GET /api/v5/market/pm-books`.
///
/// Each entry in `asks` / `bids` is `[price, size, order_count]`. Asks are
/// sorted by price ascending (best ask first); bids are sorted by price
/// descending (best bid first).
#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PmBookDepth {
    /// Ask levels sorted ascending by price (lowest first).
    #[serde(default)]
    pub asks: Vec<Vec<String>>,
    /// Bid levels sorted descending by price (highest first).
    #[serde(default)]
    pub bids: Vec<Vec<String>>,
    /// Server snapshot timestamp (Unix ms as string).
    #[serde(default)]
    pub ts: String,
    /// Order book version sequence id. Documented as opaque -- not needed by
    /// most callers but exposed for parity with the API response.
    #[serde(default)]
    pub seq_id: i64,
}

impl Candle {
    pub fn ts(&self) -> Option<&str> {
        self.0.first().map(String::as_str)
    }
    pub fn open(&self) -> Option<&str> {
        self.0.get(1).map(String::as_str)
    }
    pub fn high(&self) -> Option<&str> {
        self.0.get(2).map(String::as_str)
    }
    pub fn low(&self) -> Option<&str> {
        self.0.get(3).map(String::as_str)
    }
    pub fn close(&self) -> Option<&str> {
        self.0.get(4).map(String::as_str)
    }
    pub fn vol(&self) -> Option<&str> {
        self.0.get(5).map(String::as_str)
    }
    /// Trading volume in the quote currency.
    pub fn vol_ccy(&self) -> Option<&str> {
        self.0.get(6).map(String::as_str)
    }
    /// Trading volume in the quote currency (alternate denomination).
    pub fn vol_ccy_quote(&self) -> Option<&str> {
        self.0.get(7).map(String::as_str)
    }
    pub fn confirmed(&self) -> bool {
        self.0.get(8).map(|s| s == "1").unwrap_or(false)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Verbatim sample response from the OKX outcome-market Open API doc
    /// for `GET /api/v5/market/pm-books`. If the shape ever drifts, this test
    /// fails before any live call hits production.
    #[test]
    fn pm_book_depth_deserializes_from_documented_sample() {
        let json = r#"{
            "asks": [["67364.1","0.45478048","5"]],
            "bids": [["67364","1.72315936","17"]],
            "ts": "1774943488756",
            "seqId": 74487243135
        }"#;
        let depth: PmBookDepth = serde_json::from_str(json).expect("deserialize");
        assert_eq!(depth.asks.len(), 1);
        assert_eq!(depth.bids.len(), 1);
        // Inner array order is [price, size, order_count].
        assert_eq!(depth.asks[0], vec!["67364.1", "0.45478048", "5"]);
        assert_eq!(depth.bids[0], vec!["67364", "1.72315936", "17"]);
        assert_eq!(depth.ts, "1774943488756");
        assert_eq!(depth.seq_id, 74_487_243_135i64);
    }

    /// Missing optional-ish fields should default cleanly (server may omit
    /// `seqId` on some snapshots; `asks`/`bids` may be empty arrays).
    #[test]
    fn pm_book_depth_handles_missing_fields() {
        let json = r#"{"asks":[],"bids":[],"ts":"0"}"#;
        let depth: PmBookDepth = serde_json::from_str(json).expect("deserialize");
        assert!(depth.asks.is_empty());
        assert!(depth.bids.is_empty());
        assert_eq!(depth.seq_id, 0);
    }

    /// Pin the 9-column candle layout from the spec
    /// ([ts, o, h, l, c, vol, volCcy, volCcyQuote, confirm]) so a
    /// caller using the accessors gets the right field even if the
    /// underlying Vec layout drifts.
    #[test]
    fn candle_accessors_match_spec_column_order() {
        let c = Candle(vec![
            "1700000000000".to_string(),
            "0.51".to_string(),
            "0.55".to_string(),
            "0.50".to_string(),
            "0.54".to_string(),
            "1000".to_string(),
            "540".to_string(),
            "540".to_string(),
            "1".to_string(),
        ]);
        assert_eq!(c.ts(), Some("1700000000000"));
        assert_eq!(c.open(), Some("0.51"));
        assert_eq!(c.high(), Some("0.55"));
        assert_eq!(c.low(), Some("0.50"));
        assert_eq!(c.close(), Some("0.54"));
        assert_eq!(c.vol(), Some("1000"));
        assert_eq!(c.vol_ccy(), Some("540"));
        assert_eq!(c.vol_ccy_quote(), Some("540"));
        assert!(c.confirmed());
    }
}

//! Trade (fill) model types.

use crate::models::common::OrderSide;

/// Whether the fill was on the maker (resting) or taker (aggressing)
/// side of the book.
#[derive(Debug, Clone, PartialEq, Eq, Default, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum Role {
    Maker,
    Taker,
    #[default]
    #[serde(other)]
    Unknown,
}

impl Role {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Maker => "MAKER",
            Self::Taker => "TAKER",
            Self::Unknown => "UNKNOWN",
        }
    }
}

impl std::fmt::Display for Role {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

/// A single fill (trade) record.
#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TradeRecord {
    /// Server-assigned trade id. Backend returns an empty string for TAKER
    /// rows and for MAKER rows that predate the on-chain trade-id assignment.
    pub trade_id: String,
    pub order_id: String,
    pub market_id: String,
    /// YES or NO token address.
    pub token_id: String,
    pub side: OrderSide,
    /// Number of tokens filled.
    pub size: String,
    /// Filled notional in pts.
    pub amount: String,
    /// Fill price.
    pub price: String,
    /// Fee paid in pts.
    pub fee: String,
    pub role: Role,
    pub tx_hash: String,
    pub created_at: String,
}

/// Response from `GET /api/v5/predictions/trades`.
pub type TradesResponse = crate::models::common::PagedListResponse<TradeRecord>;

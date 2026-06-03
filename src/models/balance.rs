//! Balance model types.

use serde::{Deserialize, Serialize};

use crate::models::common::OddsType;

/// A single entry from `GET /api/v5/predictions/balance`. The response is one entry per odds type
/// the caller holds.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BalanceEntry {
    pub odds_type: OddsType,
    /// Total balance, denominated by `odds_type`.
    pub balance: String,
    /// Available balance (`balance` minus the amount frozen by open orders).
    pub available: String,
}

/// Response wrapper for `GET /api/v5/predictions/balance`.
pub type BalanceResponse = Vec<BalanceEntry>;

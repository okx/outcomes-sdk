//! Event and market model types.

use super::common::Pagination;

// ── Enums ────────────────────────────────────────────────────────────────────

/// Lifecycle of a [`MarketObject`].
#[derive(Debug, Clone, PartialEq, Eq, Default, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum MarketStatus {
    Active,
    Paused,
    Settling,
    Resolved,
    #[default]
    #[serde(other)]
    Unknown,
}

impl MarketStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Active => "active",
            Self::Paused => "paused",
            Self::Settling => "settling",
            Self::Resolved => "resolved",
            Self::Unknown => "unknown",
        }
    }
}

impl std::fmt::Display for MarketStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Lifecycle of an [`EventObject`]. Same vocabulary as [`MarketStatus`]
/// minus `Settling` (events don't carry a settling state — only their
/// markets do).
#[derive(Debug, Clone, PartialEq, Eq, Default, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum EventStatus {
    Active,
    Paused,
    Resolved,
    #[default]
    #[serde(other)]
    Unknown,
}

impl EventStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Active => "active",
            Self::Paused => "paused",
            Self::Resolved => "resolved",
            Self::Unknown => "unknown",
        }
    }
}

impl std::fmt::Display for EventStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Top-level event category. Uppercase on the wire.
#[derive(Debug, Clone, PartialEq, Eq, Default, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum EventCategory {
    Sports,
    CurrentAffairs,
    #[default]
    #[serde(other)]
    Unknown,
}

impl EventCategory {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Sports => "SPORTS",
            Self::CurrentAffairs => "CURRENT_AFFAIRS",
            Self::Unknown => "UNKNOWN",
        }
    }
}

impl std::fmt::Display for EventCategory {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

// ── Outcome ──────────────────────────────────────────────────────────────────

/// A Yes or No outcome option within a market.
#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OutcomeObject {
    /// Conditional token contract address; `None` before on-chain deployment.
    pub token_id: Option<String>,
    /// Asset ID used for placing orders; `None` before on-chain deployment.
    pub asset_id: Option<String>,
    /// Outcome label, e.g. `"Yes"` or `"No"`.
    pub name: String,
    /// Current price as a decimal string in range 0–1, e.g. `"0.65"`.
    pub price: String,
    /// Settlement result: `Some(true)` = winner, `Some(false)` = loser, `None` = unsettled.
    pub final_result: Option<bool>,
}

// ── Market ───────────────────────────────────────────────────────────────────

/// A outcome market within an event.
#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MarketObject {
    /// Central unique market ID.
    pub id: String,
    /// Market ID (used for placing orders).
    pub market_id: String,
    /// Whether this is a negRisk (mutually exclusive) market.
    pub neg_risk: bool,
    pub status: MarketStatus,
    /// Settlement stage: 0=not started … 5=settled.
    pub settle_stage: i32,
    /// Full market question text.
    pub question: String,
    /// Abbreviated question; `None` if not set.
    pub short_question: Option<String>,
    /// Market description.
    pub description: String,
    /// Market icon URL; `None` if not set.
    pub market_icon: Option<String>,
    /// Trading start timestamp (ms, as string).
    pub start_time: String,
    /// Trading end timestamp (ms, as string).
    pub end_time: String,
    /// Resolution window start timestamp (ms, as string).
    pub resolve_start_at: String,
    /// Resolution timestamp (ms, as string).
    pub resolve_at: String,
    /// Best bid price (0–1); `None` when no bids exist.
    pub best_bid: Option<String>,
    /// Best ask price (0–1); `None` when no asks exist.
    pub best_ask: Option<String>,
    /// Most recent trade price (0–1); `None` if no trades have occurred.
    pub last_trade_price: Option<String>,
    /// Total trading volume in pts.
    pub volume: String,
    /// Yes-outcome probability (0–1); `None` before on-chain deployment.
    pub probability: Option<String>,
    /// URLs of data sources used for resolution.
    #[serde(default)]
    pub resolution_sources: Vec<String>,
    /// Yes outcome option.
    pub yes_outcome: OutcomeObject,
    /// No outcome option.
    pub no_outcome: OutcomeObject,
}

// ── Sports ───────────────────────────────────────────────────────────────────

/// A team participating in a sports event.
#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SportsTeam {
    pub team_id: String,
    pub team_name: String,
    pub team_short_name: String,
    pub team_color: String,
    /// Team logo URL; `None` if not available.
    pub team_icon: Option<String>,
    /// `true` for the home team.
    pub home_team: bool,
}

/// Live game score and stage data; present only when `game_status != "upcoming"`.
#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GameData {
    /// `"upcoming"` / `"live"` / `"final"`.
    pub game_status: String,
    /// Home team score; `None` before the game starts.
    pub score_home: Option<i32>,
    /// Away team score; `None` before the game starts.
    pub score_away: Option<i32>,
    /// Current game stage, e.g. `"Q1"`, `"Half-time"`; `None` when not live.
    pub current_stage: Option<String>,
    /// Timestamp (ms, as string) when the current stage ends; `None` when not live.
    pub current_stage_end_time: Option<String>,
}

/// Sports-specific data attached to an event; present only when `category == "SPORTS"`.
#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SportEventData {
    /// `"games"` (includes moneyline markets) or `"props"`.
    pub sports_event_type: String,
    /// League ID.
    pub league_id: String,
    /// Participating teams, ordered `[home, away]`.
    pub teams: Vec<SportsTeam>,
    /// Live game data; present only when `sports_event_type == "games"`.
    pub game_data: Option<GameData>,
}

// ── Event ────────────────────────────────────────────────────────────────────

/// A outcome market event.
#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EventObject {
    /// Central unique event ID.
    pub id: String,
    /// Event ID.
    pub event_id: String,
    pub category: EventCategory,
    /// Whether this is a negRisk event.
    pub neg_risk: bool,
    pub status: EventStatus,
    pub event_title: String,
    pub description: String,
    /// Event icon URL; `None` if not set.
    pub event_icon: Option<String>,
    /// Total trading volume across all markets (pts).
    pub volume: String,
    /// Trading start timestamp (ms, as string); `None` if not yet set.
    pub start_time: Option<String>,
    /// Trading end timestamp (ms, as string); `None` if not yet set.
    pub end_time: Option<String>,
    /// Event creation timestamp (ms, as string).
    pub created_at: String,
    /// Total number of markets under this event.
    pub total_markets_count: i32,
    /// Winning market ID after settlement; `None` when unsettled.
    pub final_outcomes_market_id: Option<String>,
    /// Sports-specific data; `None` for non-sports events.
    pub sport_event_data: Option<SportEventData>,
    /// Market list.
    ///
    /// List endpoints return at most the first 2 markets.
    /// Use [`crate::OutcomesSdkClient::get_event_markets`] to retrieve the full list.
    pub markets: Vec<MarketObject>,
}

// ── Response wrappers ────────────────────────────────────────────────────────

/// Response for `GET /api/v5/predictions/events`.
#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
pub struct EventsResponse {
    pub events: Vec<EventObject>,
    pub pagination: Pagination,
}

/// Response for `GET /api/v5/predictions/events/{eventId}/markets`.
#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
pub struct MarketsResponse {
    pub markets: Vec<MarketObject>,
}

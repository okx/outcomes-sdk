//! WebSocket push data models for all OKX Outcomes channels.
//!
//! All fields are String from the wire (JSON). Conversion happens at the UI layer.

// ---------------------------------------------------------------------------
// Envelope
// ---------------------------------------------------------------------------

/// Generic WS push envelope.
#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
pub struct WsPushEnvelope<T> {
    pub arg: WsArg,
    pub data: Vec<T>,
    /// Only present for pm-books channel (snapshot / update).
    #[serde(default)]
    pub action: Option<String>,
}

/// Subscription arg in the push envelope.
///
/// `inst_id` is set on public channels (the asset/event/game id the
/// subscriber asked for); `uid` is set on private channels (the CEX
/// user id the push is scoped to). Both are optional on the struct so
/// either envelope shape parses cleanly.
#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WsArg {
    pub channel: String,
    #[serde(default)]
    pub inst_id: Option<String>,
    #[serde(default)]
    pub uid: Option<String>,
}

// ---------------------------------------------------------------------------
// Public channels
// ---------------------------------------------------------------------------

/// prediction-market-prices — real-time price data per market.
#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WsPriceTick {
    pub yes_asset_id: String,
    pub last_trade_price: String,
    pub best_bid: String,
    pub best_ask: String,
    pub timestamp: String,
    pub probability: String,
    pub market_volume: String,
    pub event_volume: String,
    pub event_id: String,
}

/// pm-event-status — event settlement result.
#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WsEventStatus {
    pub event_id: String,
    /// Event status, e.g. `"resolved"`.
    pub status: String,
    /// Winning market ID.
    #[serde(default)]
    pub market_id: String,
    /// Winning outcome display: `"yes"` / `"no"` / `"others"` / team name / `"draw"`.
    #[serde(default)]
    pub outcome_option: String,
    pub timestamp: String,
}

/// game-status — sports match progress.
#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WsGameStatus {
    pub game_id: String,
    pub home_team: String,
    pub away_team: String,
    pub status: String,
    pub home_team_score: String,
    pub away_team_score: String,
    pub period: String,
    pub schedule_time: String,
    pub timestamp: String,
}

/// pm-trades — public market trades (aggregated).
#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WsPmTrade {
    #[serde(default)]
    pub inst_id: String,
    /// Trade id, set on per-trade pushes. Absent on window-aggregated pushes,
    /// which use `fId`/`lId` (window endpoints) instead.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub trade_id: Option<String>,
    /// First trade id in the aggregation window. Set on aggregated pushes; absent on per-trade pushes.
    #[serde(rename = "fId", skip_serializing_if = "Option::is_none")]
    pub f_id: Option<String>,
    /// Last trade id in the aggregation window. Set on aggregated pushes; absent on per-trade pushes.
    #[serde(rename = "lId", skip_serializing_if = "Option::is_none")]
    pub l_id: Option<String>,
    pub px: String,
    pub sz: String,
    pub side: String,
    pub ts: String,
}

/// pm-tickers -- real-time ticker payload.
///
/// Aliased to the REST `Ticker` because the WS `pm-tickers` channel and the
/// REST `GET /api/v5/market/ticker` endpoint deliver byte-identical JSON: same
/// 16 fields, same `camelCase`, and the same empty-string-means-no-liquidity
/// convention on bid/ask/last. Keeping them as two structs only created a
/// place for the two definitions to drift. If OKX ever adds a WS-only field
/// (sequence ID, snapshot marker, etc.) promote this alias back to a struct.
pub type WsPmTicker = crate::models::price::Ticker;

/// pm-books — order book snapshot/update.
#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WsPmBookData {
    pub asks: Vec<Vec<String>>,
    pub bids: Vec<Vec<String>>,
    pub ts: String,
    #[serde(default)]
    pub checksum: Option<i64>,
    #[serde(default)]
    pub seq_id: Option<i64>,
    #[serde(default)]
    pub prev_seq_id: Option<i64>,
}

// ---------------------------------------------------------------------------
// Closed-set wire enums
// ---------------------------------------------------------------------------
//
// Each of these mirrors a closed set of string values defined in the WS
// spec. Modelled as Rust enums for typo-safety and exhaustive matching,
// with a `#[serde(other)] Unknown` catch-all so the SDK degrades
// gracefully if the server ever adds a value we don't know yet.
//
// `as_str()` returns the canonical wire string; `Display` delegates to
// it so `format!("{}", x)` round-trips for known variants.

/// pm-order status.
#[derive(Debug, Clone, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum OrderStatus {
    Active,
    Filled,
    PartiallyFilled,
    PlaceFailed,
    CancelFailed,
    Cancelled,
    Expired,
    #[serde(other)]
    Unknown,
}

impl OrderStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Active => "ACTIVE",
            Self::Filled => "FILLED",
            Self::PartiallyFilled => "PARTIALLY_FILLED",
            Self::PlaceFailed => "PLACE_FAILED",
            Self::CancelFailed => "CANCEL_FAILED",
            Self::Cancelled => "CANCELLED",
            Self::Expired => "EXPIRED",
            Self::Unknown => "UNKNOWN",
        }
    }
}

impl std::fmt::Display for OrderStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

// `OrderSide` lives in `models::common` since it appears identically in
// REST responses and WS pushes. Re-exported here so existing
// `okx_outcomes_sdk::ws::models::OrderSide` imports keep working.
pub use crate::models::common::OrderSide;

/// `YES` / `NO` — position direction (which outcome an order takes).
#[derive(Debug, Clone, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum Direction {
    Yes,
    No,
    #[serde(other)]
    Unknown,
}

impl Direction {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Yes => "YES",
            Self::No => "NO",
            Self::Unknown => "UNKNOWN",
        }
    }
}

impl std::fmt::Display for Direction {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

// `OddsType` lives in `models::common` since it appears identically in
// REST responses and WS pushes. Re-exported here so existing
// `okx_outcomes_sdk::ws::models::OddsType` imports keep working.
pub use crate::models::common::OddsType;

/// pm-position status. Covers both the snapshot variant (FILL / REDEEM)
/// and the operational variant (SPLIT / MERGE / DEPOSIT / WITHDRAW), each
/// with a matching `_FAILED`.
#[derive(Debug, Clone, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum PositionStatus {
    Fill,
    FillFailed,
    Redeem,
    RedeemFailed,
    Split,
    SplitFailed,
    Merge,
    MergeFailed,
    Deposit,
    DepositFailed,
    Withdraw,
    WithdrawFailed,
    #[serde(other)]
    Unknown,
}

impl PositionStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Fill => "FILL",
            Self::FillFailed => "FILL_FAILED",
            Self::Redeem => "REDEEM",
            Self::RedeemFailed => "REDEEM_FAILED",
            Self::Split => "SPLIT",
            Self::SplitFailed => "SPLIT_FAILED",
            Self::Merge => "MERGE",
            Self::MergeFailed => "MERGE_FAILED",
            Self::Deposit => "DEPOSIT",
            Self::DepositFailed => "DEPOSIT_FAILED",
            Self::Withdraw => "WITHDRAW",
            Self::WithdrawFailed => "WITHDRAW_FAILED",
            Self::Unknown => "UNKNOWN",
        }
    }

    /// True for the variant-1 (snapshot) statuses that carry pnl / value /
    /// tokenId, false for variant-2 (operational) statuses with tx_hash.
    pub fn is_position_snapshot(&self) -> bool {
        matches!(
            self,
            Self::Fill | Self::FillFailed | Self::Redeem | Self::RedeemFailed
        )
    }

    pub fn is_failed(&self) -> bool {
        matches!(
            self,
            Self::FillFailed
                | Self::RedeemFailed
                | Self::SplitFailed
                | Self::MergeFailed
                | Self::DepositFailed
                | Self::WithdrawFailed
        )
    }
}

impl std::fmt::Display for PositionStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

/// pm-balance `changeType`.
#[derive(Debug, Clone, PartialEq, Eq, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum BalanceChangeType {
    Place,
    Cancel,
    Fill,
    Split,
    Merge,
    Redeem,
    Deposit,
    Withdraw,
    #[serde(other)]
    Unknown,
}

impl BalanceChangeType {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Place => "PLACE",
            Self::Cancel => "CANCEL",
            Self::Fill => "FILL",
            Self::Split => "SPLIT",
            Self::Merge => "MERGE",
            Self::Redeem => "REDEEM",
            Self::Deposit => "DEPOSIT",
            Self::Withdraw => "WITHDRAW",
            Self::Unknown => "UNKNOWN",
        }
    }
}

impl std::fmt::Display for BalanceChangeType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

// ---------------------------------------------------------------------------
// Private channels
// ---------------------------------------------------------------------------

/// pm-order — user order status changes.
///
/// Every field is optional on the wire because the server only populates
/// the ones relevant for the current `status` (see the spec's status →
/// required-fields table). We model that with `#[serde(default)]` so
/// missing keys deserialize to empty strings rather than failing the
/// whole push.
#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WsOrder {
    pub order_id: String,
    pub market_id: String,
    /// Push event type — see the spec's status → required-fields table.
    pub status: OrderStatus,
    /// Trade direction. Always present.
    pub side: OrderSide,

    // Everything below is variant-specific per the spec's required-fields
    // table; the server omits or sends `null` for fields that don't apply
    // to the current `status`. Modelled as `Option<_>` so the type system
    // makes that nullability explicit at every call site.
    #[serde(default)]
    pub client_order_id: Option<String>,
    /// YES asset id or NO asset id (the side of the market this order touches).
    #[serde(default)]
    pub asset_id: Option<String>,
    /// Position direction (which outcome this order takes).
    #[serde(default)]
    pub direction: Option<Direction>,
    /// Cumulative filled size.
    #[serde(default)]
    pub filled_size: Option<String>,
    /// Original order size.
    #[serde(default, alias = "size")]
    pub order_size: Option<String>,
    /// Cumulative average fill price (`= amount / filledSize`).
    #[serde(default)]
    pub avg_price: Option<String>,
    /// Cumulative filled amount in points (BUY = spent, SELL = received).
    #[serde(default)]
    pub amount: Option<String>,
    /// Limit-order resting price (limit orders only).
    #[serde(default, alias = "price")]
    pub limit_price: Option<String>,
    /// Failure message (only for `PLACE_FAILED` / `CANCEL_FAILED`).
    #[serde(default)]
    pub fail_message: Option<String>,
    #[serde(default)]
    pub odds_type: Option<OddsType>,
    /// On-chain transaction hash. Spec uses `txHash` camelCase.
    #[serde(default, rename = "txHash")]
    pub tx_hash: Option<String>,
    /// Trade id (limit-order partial-fill only, `status=ACTIVE`).
    #[serde(default)]
    pub trade_id: Option<String>,
}

/// pm-position — user position changes.
///
/// The spec defines **two** push variants on this channel:
///
/// - `status ∈ {FILL, REDEEM, FILL_FAILED, REDEEM_FAILED}` carries the
///   full position snapshot (id / tokenId / assetId / pnl / value / etc.).
/// - `status ∈ {SPLIT, MERGE, DEPOSIT, WITHDRAW, *_FAILED}` carries
///   only the operational fields (marketId / status / amount / txHash /
///   oddsType, plus a populated `ext` for DEPOSIT).
///
/// Modelled as a single flat struct with every variant-specific field
/// behind `#[serde(default)]` so either shape deserializes without
/// failing. Consumers should branch on `status` to know which fields
/// are meaningful.
#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WsPosition {
    // Common across both variants
    pub market_id: String,
    pub status: PositionStatus,
    /// Type 1: position size (`remain`); `"0"` for REDEEM.
    /// Type 2: split/merge/deposit/withdraw amount.
    #[serde(default)]
    pub amount: String,
    #[serde(default)]
    pub odds_type: Option<OddsType>,

    // Variant 1 (FILL / REDEEM / *_FAILED) — full position snapshot
    #[serde(default)]
    pub id: Option<String>,
    #[serde(default)]
    pub token_id: Option<String>,
    #[serde(default)]
    pub asset_id: Option<String>,
    #[serde(default)]
    pub timestamp: Option<String>,
    #[serde(default)]
    pub un_realized_pnl: Option<String>,
    #[serde(default)]
    pub un_realized_pnl_percentage: Option<String>,
    #[serde(default)]
    pub value: Option<String>,
    #[serde(default)]
    pub avg_price: Option<String>,
    #[serde(default)]
    pub trade_id: Option<String>,

    // Variant 2 (SPLIT / MERGE / DEPOSIT / WITHDRAW / *_FAILED)
    #[serde(default, rename = "txHash")]
    pub tx_hash: Option<String>,
    #[serde(default)]
    pub ext: Option<WsPositionExt>,
}

/// `ext` block on the SPLIT/MERGE/DEPOSIT/WITHDRAW variant. Currently
/// only populated for DEPOSIT, with `to_tx_hash` carrying the TZ-side
/// credit tx hash.
#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WsPositionExt {
    /// TZ-side credit tx hash; `None` if not provided.
    /// Spec marks this `String | null` (outcomes_wss_updated.md:211).
    #[serde(default, rename = "toTxHash")]
    pub to_tx_hash: Option<String>,
}

/// pm-user-trade — user trade execution details.
#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WsUserTrade {
    pub order_id: String,
    /// Client-assigned order ID; `None` when the client didn't supply one.
    /// Spec marks this `string | null` (outcomes_wss_updated.md:273).
    #[serde(default)]
    pub client_order_id: Option<String>,
    pub market_id: String,
    pub token_id: String,
    /// YES/NO asset pair ID (yesAssetId or noAssetId).
    #[serde(default)]
    pub asset_id: String,
    pub side: OrderSide,
    pub size: String,
    pub price: String,
    #[serde(default)]
    pub txhash: String,
    pub timestamp: String,
    /// Trade ID.
    #[serde(default)]
    pub trade_id: String,
}

/// pm-balance — user balance changes.
#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WsBalance {
    pub wallet_address: String,
    pub available: String,
    pub total: String,
    pub frozen: String,
    /// On-chain Point token id (corresponds to `oddsType`).
    pub token_id: String,
    /// Trigger reason for the balance update.
    pub change_type: BalanceChangeType,
    /// Change amount. Spec explicitly says "may be null".
    #[serde(default)]
    pub change_amount: Option<String>,
    pub update_time: String,
    /// Required per spec but defensively optional for resilience against
    /// legacy payloads.
    #[serde(default)]
    pub odds_type: Option<OddsType>,
}

/// pm-pnl — floating P&L. The channel pushes **two distinct payload
/// shapes**:
///
/// - [`WsPnl::Overview`] — portfolio value + per-period summary (1D / 1W / 1M / 6M / 1Y).
/// - [`WsPnl::Timeseries`] — per-period chart points with high/low/current.
///
/// Modelled as a serde `untagged` enum so the right variant is picked
/// automatically based on which discriminating fields are present
/// (`portfolioValue` vs `points`).
#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
#[serde(untagged)]
pub enum WsPnl {
    Overview(WsPnlOverview),
    Timeseries(WsPnlTimeseries),
}

/// Portfolio-value + per-period summary push (variant 1 of `pm-pnl`).
#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WsPnlOverview {
    /// Current total portfolio value (point balance + position market value).
    pub portfolio_value: String,
    pub periods: Vec<WsPnlPeriodSummary>,
}

/// One row in [`WsPnlOverview::periods`].
#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WsPnlPeriodSummary {
    /// Period label: `1D` / `1W` / `1M` / `6M` / `1Y`.
    pub period: String,
    /// Absolute PnL within the period.
    pub period_pnl: String,
    /// PnL percentage within the period.
    pub pnl_percent: String,
}

/// PnL chart timeseries push (variant 2 of `pm-pnl`).
#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WsPnlTimeseries {
    /// Period code: `0`=1D / `1`=1W / `2`=1M / `3`=6M / `4`=1Y.
    pub period: String,
    /// Data-point interval (ms): 600000 / 1800000 / 3600000 / 86400000.
    pub interval: String,
    pub points: Vec<WsPnlPoint>,
    pub current_pnl: String,
    pub high: String,
    pub low: String,
}

/// One sample in [`WsPnlTimeseries::points`].
#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WsPnlPoint {
    /// Timestamp in milliseconds.
    pub time: String,
    /// Total assets at this moment (point balance + position market value).
    pub pnl: String,
}

// ---------------------------------------------------------------------------
// Typed message — single-parse entry point
// ---------------------------------------------------------------------------

/// Typed WS message, parsed once from raw JSON.
///
/// The SDK reader task calls [`parse_ws_message`] to produce this enum.
/// Consumers receive typed data — no further JSON parsing needed.
#[derive(Debug, Clone)]
pub enum WsMessage {
    /// Subscribe/error confirmation from server.
    Event {
        event: String,
        channel: Option<String>,
        inst_id: Option<String>,
        msg: Option<String>,
    },
    /// `prediction-market-prices` — real-time price data.
    Prices(Vec<WsPriceTick>),
    /// `pm-books` — order book snapshot/update.
    Books {
        data: Vec<WsPmBookData>,
        action: String,
    },
    /// `pm-trades` — public trade events.
    Trades(Vec<WsPmTrade>),
    /// `pm-tickers` — real-time ticker data.
    Tickers(Vec<WsPmTicker>),
    /// `game-status` — sports match progress.
    Game(Vec<WsGameStatus>),
    /// `pm-event-status` — event settlement result.
    EventStatus(Vec<WsEventStatus>),
    /// `pm-candle*` — candlestick data (OHLCV arrays, same column
    /// layout as the REST `get_candles` response). See
    /// [`crate::models::price::Candle`] for accessor methods.
    Candle(Vec<crate::models::price::Candle>),
    /// `pm-order` — user order status changes.
    Orders(Vec<WsOrder>),
    /// `pm-position` — user position changes.
    Positions(Vec<WsPosition>),
    /// `pm-user-trade` — user trade execution details.
    UserTrades(Vec<WsUserTrade>),
    /// `pm-balance` — user balance changes.
    Balance(Vec<WsBalance>),
    /// `pm-pnl` — floating P&L.
    Pnl(Vec<WsPnl>),
    /// Unknown channel — raw JSON for fallback rendering.
    Unknown {
        channel: String,
        raw: serde_json::Value,
    },
}

/// Parse a raw WS JSON payload into a typed [`WsMessage`].
///
/// For typed channels, uses `WsPushEnvelope<T>` for a single `from_str` call
/// with no intermediate `Value` allocation. For event confirmations, falls
/// back to `Value`.
///
/// Returns `None` only if the payload is not valid JSON.
pub fn parse_ws_message(channel: &str, payload: &str) -> Option<WsMessage> {
    // Fast path: event confirmations ({"event": "subscribe", ...}).
    // These don't have an "arg.channel" matching the subscription channel,
    // so we detect them by attempting a Value parse and checking for "event".
    // This is the only path that uses Value; all data channels use typed from_str.
    if payload.contains("\"event\"") {
        let val: serde_json::Value = serde_json::from_str(payload).ok()?;
        if let Some(event) = val.get("event").and_then(|e| e.as_str()) {
            let arg = val.get("arg");
            let channel = arg
                .and_then(|a| a.get("channel"))
                .and_then(|v| v.as_str())
                .map(|s| s.to_string());
            let inst_id = arg
                .and_then(|a| a.get("instId"))
                .and_then(|v| v.as_str())
                .map(|s| s.to_string());
            let msg = val
                .get("msg")
                .and_then(|m| m.as_str())
                .map(|s| s.to_string());
            return Some(WsMessage::Event {
                event: event.to_string(),
                channel,
                inst_id,
                msg,
            });
        }
    }

    match channel {
        "prediction-market-prices" => {
            let env: WsPushEnvelope<WsPriceTick> = serde_json::from_str(payload).ok()?;
            Some(WsMessage::Prices(env.data))
        }
        "pm-books" => {
            let env: WsPushEnvelope<WsPmBookData> = serde_json::from_str(payload).ok()?;
            let action = env.action.unwrap_or_else(|| "update".to_string());
            Some(WsMessage::Books {
                data: env.data,
                action,
            })
        }
        "pm-trades" => {
            let env: WsPushEnvelope<WsPmTrade> = serde_json::from_str(payload).ok()?;
            Some(WsMessage::Trades(env.data))
        }
        "pm-tickers" => {
            let env: WsPushEnvelope<WsPmTicker> = serde_json::from_str(payload).ok()?;
            Some(WsMessage::Tickers(env.data))
        }
        "game-status" => {
            let env: WsPushEnvelope<WsGameStatus> = serde_json::from_str(payload).ok()?;
            Some(WsMessage::Game(env.data))
        }
        "pm-event-status" => {
            let env: WsPushEnvelope<WsEventStatus> = serde_json::from_str(payload).ok()?;
            Some(WsMessage::EventStatus(env.data))
        }
        "pm-order" => {
            let env: WsPushEnvelope<WsOrder> = serde_json::from_str(payload).ok()?;
            Some(WsMessage::Orders(env.data))
        }
        "pm-position" => {
            let env: WsPushEnvelope<WsPosition> = serde_json::from_str(payload).ok()?;
            Some(WsMessage::Positions(env.data))
        }
        "pm-user-trade" => {
            let env: WsPushEnvelope<WsUserTrade> = serde_json::from_str(payload).ok()?;
            Some(WsMessage::UserTrades(env.data))
        }
        "pm-balance" => {
            let env: WsPushEnvelope<WsBalance> = serde_json::from_str(payload).ok()?;
            Some(WsMessage::Balance(env.data))
        }
        "pm-pnl" => {
            let env: WsPushEnvelope<WsPnl> = serde_json::from_str(payload).ok()?;
            Some(WsMessage::Pnl(env.data))
        }
        ch if ch.starts_with("pm-candle") || ch.starts_with("candle") => {
            let env: WsPushEnvelope<crate::models::price::Candle> =
                serde_json::from_str(payload).ok()?;
            let items = env.data;
            Some(WsMessage::Candle(items))
        }
        _ => {
            let val: serde_json::Value = serde_json::from_str(payload).ok()?;
            Some(WsMessage::Unknown {
                channel: channel.to_string(),
                raw: val,
            })
        }
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn price_tick_roundtrip() {
        let json = r#"{
            "yesAssetId": "asset_001",
            "lastTradePrice": "0.64",
            "bestBid": "0.63",
            "bestAsk": "0.66",
            "probability": "6500",
            "marketVolume": "1234.56",
            "eventVolume": "5678.90",
            "eventId": "evt_001",
            "timestamp": "1672290687"
        }"#;
        let tick: WsPriceTick = serde_json::from_str(json).expect("deserialize");
        assert_eq!(tick.yes_asset_id, "asset_001");
        assert_eq!(tick.best_bid, "0.63");
        assert_eq!(tick.probability, "6500");
    }

    #[test]
    fn game_status_roundtrip() {
        let json = r#"{
            "gameId": "1317359",
            "homeTeam": "Lakers",
            "awayTeam": "Celtics",
            "status": "live",
            "homeTeamScore": "101",
            "awayTeamScore": "100",
            "period": "Q4",
            "scheduleTime": "1773632746",
            "timestamp": "1672290687"
        }"#;
        let game: WsGameStatus = serde_json::from_str(json).expect("deserialize");
        assert_eq!(game.home_team_score, "101");
    }

    #[test]
    fn pm_book_with_integrity_fields() {
        let json = r#"{
            "asks": [["0.551", "1200", "15"]],
            "bids": [["0.549", "1500", "20"]],
            "ts": "1711094400000",
            "checksum": -702280706,
            "seqId": 308306650401,
            "prevSeqId": -1
        }"#;
        let book: WsPmBookData = serde_json::from_str(json).expect("deserialize");
        assert_eq!(book.checksum, Some(-702280706));
        assert_eq!(book.seq_id, Some(308306650401));
    }

    #[test]
    fn envelope_price_tick() {
        let json = r#"{
            "arg": {"channel": "prediction-market-prices", "instId": "asset_001"},
            "data": [{"yesAssetId": "asset_001", "lastTradePrice": "0.64", "bestBid": "0.63", "bestAsk": "0.66", "probability": "6500", "marketVolume": "100", "eventVolume": "200", "eventId": "evt_001", "timestamp": "1672290687"}]
        }"#;
        let env: WsPushEnvelope<WsPriceTick> = serde_json::from_str(json).expect("deserialize");
        assert_eq!(env.arg.channel, "prediction-market-prices");
        assert_eq!(env.data[0].last_trade_price, "0.64");
    }

    #[test]
    fn parse_prices_message() {
        let json = r#"{"arg":{"channel":"prediction-market-prices","instId":"118"},"data":[{"yesAssetId":"118","eventId":"evt_001","bestBid":"0.65","bestAsk":"0.78","lastTradePrice":"0.78","probability":"7168","marketVolume":"5721.42","eventVolume":"5721.42","timestamp":"1776305504477"}]}"#;
        let msg = parse_ws_message("prediction-market-prices", json).unwrap();
        match msg {
            WsMessage::Prices(data) => {
                assert_eq!(data.len(), 1);
                assert_eq!(data[0].yes_asset_id, "118");
                assert_eq!(data[0].best_bid, "0.65");
            }
            _ => panic!("expected Prices variant"),
        }
    }

    #[test]
    fn parse_event_confirmation() {
        let json = r#"{"event":"subscribe","arg":{"channel":"prediction-market-prices","instId":"118"},"connId":"abc123"}"#;
        let msg = parse_ws_message("prediction-market-prices", json).unwrap();
        match msg {
            WsMessage::Event {
                event,
                channel,
                inst_id,
                ..
            } => {
                assert_eq!(event, "subscribe");
                assert_eq!(channel, Some("prediction-market-prices".to_string()));
                assert_eq!(inst_id, Some("118".to_string()));
            }
            _ => panic!("expected Event variant"),
        }
    }

    #[test]
    fn parse_books_message() {
        let json = r#"{"arg":{"channel":"pm-books","instId":"118"},"action":"snapshot","data":[{"asks":[["0.78","100","1"]],"bids":[["0.65","200","2"]],"ts":"1776305504477","checksum":-123,"seqId":1,"prevSeqId":-1}]}"#;
        let msg = parse_ws_message("pm-books", json).unwrap();
        match msg {
            WsMessage::Books { data, action } => {
                assert_eq!(action, "snapshot");
                assert_eq!(data[0].asks.len(), 1);
            }
            _ => panic!("expected Books variant"),
        }
    }

    #[test]
    fn parse_aggregated_trades_message() {
        let json = r#"{"arg":{"channel":"pm-trades","instId":"100168000"},"data":[{"instId":"100168000","fId":"54253","lId":"54255","px":"0.57","sz":"1.75","side":"buy","ts":"1777973850853"}]}"#;
        let msg = parse_ws_message("pm-trades", json).unwrap();
        match msg {
            WsMessage::Trades(data) => {
                assert_eq!(data.len(), 1);
                let t = &data[0];
                assert_eq!(t.trade_id, None);
                assert_eq!(t.f_id, Some("54253".to_string()));
                assert_eq!(t.l_id, Some("54255".to_string()));
                assert_eq!(t.px, "0.57");
                assert_eq!(t.side, "buy");
            }
            _ => panic!("expected Trades variant"),
        }
    }

    #[test]
    fn parse_per_trade_message() {
        let json = r#"{"arg":{"channel":"pm-trades","instId":"100168000"},"data":[{"instId":"100168000","tradeId":"54260","px":"0.58","sz":"3","side":"sell","ts":"1777973900000"}]}"#;
        let msg = parse_ws_message("pm-trades", json).unwrap();
        match msg {
            WsMessage::Trades(data) => {
                let t = &data[0];
                assert_eq!(t.trade_id, Some("54260".to_string()));
                assert_eq!(t.f_id, None);
                assert_eq!(t.l_id, None);
            }
            _ => panic!("expected Trades variant"),
        }
    }

    #[test]
    fn pm_trade_serialize_omits_absent_ids() {
        let trade = WsPmTrade {
            inst_id: "100168000".to_string(),
            trade_id: None,
            f_id: Some("54253".to_string()),
            l_id: Some("54255".to_string()),
            px: "0.57".to_string(),
            sz: "1.75".to_string(),
            side: "buy".to_string(),
            ts: "1777973850853".to_string(),
        };
        let json = serde_json::to_string(&trade).expect("serialize");
        assert!(
            !json.contains("tradeId"),
            "absent tradeId should not be serialized: {json}"
        );
        assert!(json.contains("\"fId\":\"54253\""));
        assert!(json.contains("\"lId\":\"54255\""));
    }

    #[test]
    fn parse_unknown_channel() {
        let json = r#"{"arg":{"channel":"new-channel","instId":"1"},"data":[{"foo":"bar"}]}"#;
        let msg = parse_ws_message("new-channel", json).unwrap();
        assert!(matches!(msg, WsMessage::Unknown { .. }));
    }

    #[test]
    fn order_with_missing_optional_fields() {
        // Minimal `ACTIVE` push per the spec's required-fields table:
        // orderId + marketId + status + side + orderSize + limitPrice
        // are the only fields the server is contractually required to
        // populate for this status. Everything else should default to
        // empty.
        let json = r#"{
            "orderId": "123",
            "marketId": "456",
            "status": "ACTIVE",
            "side": "BUY",
            "size": "100",
            "price": "0.65"
        }"#;
        let order: WsOrder = serde_json::from_str(json).expect("deserialize");
        assert_eq!(order.order_id, "123");
        assert_eq!(order.market_id, "456");
        assert_eq!(order.status, OrderStatus::Active);
        assert_eq!(order.side, OrderSide::Buy);
        // `size` / `price` aliases per the WsOrder serde aliases.
        assert_eq!(order.order_size.as_deref(), Some("100"));
        assert_eq!(order.limit_price.as_deref(), Some("0.65"));
        // All other optional fields default to None when absent.
        assert!(order.tx_hash.is_none());
        assert!(order.asset_id.is_none());
        assert!(order.direction.is_none());
        assert!(order.filled_size.is_none());
        assert!(order.avg_price.is_none());
        assert!(order.amount.is_none());
        assert!(order.fail_message.is_none());
        assert!(order.odds_type.is_none());
        assert!(order.trade_id.is_none());
        assert!(order.client_order_id.is_none());
    }

    #[test]
    fn order_with_full_spec_payload() {
        // Sample payload from the pm-order spec.
        let json = r#"{
            "orderId": "307173036051017730",
            "clientOrderId": "cli-abc-123",
            "marketId": "100001",
            "status": "FILLED",
            "assetId": "71",
            "side": "BUY",
            "direction": "YES",
            "filledSize": "10",
            "orderSize": "10",
            "avgPrice": "0.57",
            "amount": "5.7",
            "limitPrice": "0.45",
            "failMessage": null,
            "oddsType": "points",
            "txHash": "0xdef",
            "tradeId": "9876543210"
        }"#;
        let order: WsOrder = serde_json::from_str(json).expect("deserialize");
        assert_eq!(order.status, OrderStatus::Filled);
        assert_eq!(order.direction, Some(Direction::Yes));
        assert_eq!(order.filled_size.as_deref(), Some("10"));
        assert_eq!(order.avg_price.as_deref(), Some("0.57"));
        assert_eq!(order.amount.as_deref(), Some("5.7"));
        assert_eq!(order.odds_type, Some(OddsType::Points));
        assert_eq!(order.tx_hash.as_deref(), Some("0xdef"));
        // Explicit `null` deserializes to None for Option<String>.
        assert!(order.fail_message.is_none());
    }

    #[test]
    fn position_type_1_and_type_2_payloads_parse() {
        // Both spec variants use `amount` as a string per the field-type
        // tables. The type-2 example payload shows an unquoted number —
        // that's a doc typo in the example, not the wire format.
        let type_1 = r#"{"marketId":"1","status":"FILL","amount":"10","tokenId":"2","assetId":"3","timestamp":"0"}"#;
        let type_2 = r#"{"marketId":"1","status":"DEPOSIT","amount":"100","txHash":"0xa"}"#;
        let p1: WsPosition = serde_json::from_str(type_1).expect("type-1");
        let p2: WsPosition = serde_json::from_str(type_2).expect("type-2");
        assert_eq!(p1.status, PositionStatus::Fill);
        assert!(p1.status.is_position_snapshot());
        assert_eq!(p2.status, PositionStatus::Deposit);
        assert!(!p2.status.is_position_snapshot());
        assert_eq!(p1.amount, "10");
        assert_eq!(p2.amount, "100");
        assert_eq!(p2.tx_hash.as_deref(), Some("0xa"));
    }

    #[test]
    fn closed_set_enums_route_unknown_values_to_unknown_variant() {
        // Forward-compat: any value the SDK doesn't know about should
        // deserialize to the `Unknown` variant rather than failing the
        // whole push. Keeps the channel resilient if the server adds new
        // statuses/sides before the SDK is updated.
        let json = r#"{
            "orderId": "1",
            "marketId": "2",
            "status": "NEW_STATUS_NOT_IN_SPEC",
            "side": "WAT",
            "direction": "MAYBE",
            "oddsType": "experimental"
        }"#;
        let order: WsOrder = serde_json::from_str(json).expect("deserialize");
        assert_eq!(order.status, OrderStatus::Unknown);
        assert_eq!(order.side, OrderSide::Unknown);
        assert_eq!(order.direction, Some(Direction::Unknown));
        assert_eq!(order.odds_type, Some(OddsType::Unknown));
    }

    #[test]
    fn pnl_overview_variant_parses() {
        let json = r#"{
            "portfolioValue": "1234.56",
            "periods": [
                {"period":"1D","periodPnl":"12.34","pnlPercent":"1.01"}
            ]
        }"#;
        let pnl: WsPnl = serde_json::from_str(json).expect("deserialize");
        match pnl {
            WsPnl::Overview(o) => {
                assert_eq!(o.portfolio_value, "1234.56");
                assert_eq!(o.periods.len(), 1);
                assert_eq!(o.periods[0].period, "1D");
            }
            other => panic!("expected Overview, got {other:?}"),
        }
    }

    #[test]
    fn pnl_timeseries_variant_parses() {
        let json = r#"{
            "period": "0",
            "interval": "600000",
            "points": [{"time":"1","pnl":"1000"}],
            "currentPnl": "1023.75",
            "high": "1030",
            "low": "995"
        }"#;
        let pnl: WsPnl = serde_json::from_str(json).expect("deserialize");
        match pnl {
            WsPnl::Timeseries(t) => {
                assert_eq!(t.period, "0");
                assert_eq!(t.current_pnl, "1023.75");
                assert_eq!(t.points.len(), 1);
            }
            other => panic!("expected Timeseries, got {other:?}"),
        }
    }

    #[test]
    fn user_trade_carries_full_spec_field_set() {
        // outcomes_wss_en.md:249-265 — pm-user-trade payload includes
        // clientOrderId, assetId, and tradeId in addition to the fields
        // we used to model. Pin the field set so a regression here
        // surfaces as a test failure.
        let json = r#"{
            "orderId": "307173036051017730",
            "clientOrderId": "cli-abc-123",
            "marketId": "100001",
            "tokenId": "20000",
            "assetId": "71",
            "side": "BUY",
            "size": "10",
            "price": "0.57",
            "txhash": "0xdef",
            "timestamp": "1712736000000",
            "tradeId": "9876543210"
        }"#;
        let trade: WsUserTrade = serde_json::from_str(json).expect("deserialize");
        assert_eq!(trade.client_order_id.as_deref(), Some("cli-abc-123"));
        assert_eq!(trade.asset_id, "71");
        assert_eq!(trade.trade_id, "9876543210");
    }

    #[test]
    fn ws_arg_carries_uid_on_private_envelopes() {
        // Private channel envelopes include `uid` (the CEX user id);
        // public channels include `instId`. WsArg accepts both shapes.
        let private = r#"{"channel":"pm-order","uid":"cex-user-42"}"#;
        let public = r#"{"channel":"pm-trades","instId":"100888000"}"#;
        let p: WsArg = serde_json::from_str(private).expect("private arg");
        let q: WsArg = serde_json::from_str(public).expect("public arg");
        assert_eq!(p.uid.as_deref(), Some("cex-user-42"));
        assert!(p.inst_id.is_none());
        assert_eq!(q.inst_id.as_deref(), Some("100888000"));
        assert!(q.uid.is_none());
    }
}

//! API path constants.
//!
//! All paths are full absolute paths starting with `/api/v5/...` and are
//! concatenated with the client's `base_url` (default `https://www.okx.com`).

// ── Events, Markets & Search ──────────────────────────────────────────────────
pub const EVENTS: &str = "/api/v5/predictions/events";
pub const MARKETS: &str = "/api/v5/predictions/markets";
pub const SEARCH: &str = "/api/v5/predictions/events/search";

// ── Balance ───────────────────────────────────────────────────────────────────
pub const BALANCE: &str = "/api/v5/predictions/balance";

// ── Orders ───────────────────────────────────────────────────────────────────
pub const ORDERS: &str = "/api/v5/predictions/orders";
pub const ORDERS_CANCEL: &str = "/api/v5/predictions/orders/cancel";
pub const ORDERS_CANCEL_ALL: &str = "/api/v5/predictions/orders/cancel-all";

// ── Heartbeat ────────────────────────────────────────────────────────────────
pub const HEARTBEAT: &str = "/api/v5/predictions/heartbeat";

// ── Token Operations ─────────────────────────────────────────────────────────
pub const SPLIT: &str = "/api/v5/predictions/positions/split";
pub const MERGE: &str = "/api/v5/predictions/positions/merge";
pub const REDEEM: &str = "/api/v5/predictions/positions/redeem";

// ── Positions ────────────────────────────────────────────────────────────────
pub const POSITIONS: &str = "/api/v5/predictions/positions";

// ── Trades ───────────────────────────────────────────────────────────────────
pub const TRADES: &str = "/api/v5/predictions/trades";

// ── Market Data ──────────────────────────────────────────────────────────────
// The OKX market-data API lives under a different path prefix (`/api/v5/market/*`)
// and returns a string-typed `code` envelope. Same okx.com host as outcomes.
/// `GET /api/v5/market/ticker` — latest quote for a single instrument.
pub const OKX_MARKET_TICKER_PATH: &str = "/api/v5/market/ticker";
/// `GET /api/v5/market/candles` — K-line history for a single instrument.
pub const OKX_MARKET_CANDLES_PATH: &str = "/api/v5/market/candles";
/// `GET /api/v5/market/pm-books` -- outcome market order book depth snapshot.
/// Rate limit: 40 requests / 2s.
pub const OKX_MARKET_PM_BOOKS_PATH: &str = "/api/v5/market/pm-books";

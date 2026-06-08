## okx-outcomes-sdk API reference

Complete reference for every public method, request body, response shape, error variant, and WebSocket channel exposed by the `okx-outcomes-sdk` Rust crate. The README is the quick-start; this document is the long form.

Conventions used throughout:

- All Rust types are `pub` re-exports from `okx_outcomes_sdk::*` (the module path is given at the top of each section).
- "Authentication" of every REST call means the OKX REST `OK-ACCESS-*` headers signed via HMAC-SHA256 from `ApiCredentials`. The SDK signs locally; the secret never leaves the process.
- "Write" calls (place / cancel / split / merge / redeem / heartbeat) additionally require an EIP-712 ECDSA signature over a typed action. See **Signing** below.
- All decimal values (prices, sizes, balances, P&L) are exchanged as decimal **strings** to avoid float precision loss.
- All timestamps are Unix milliseconds.
- Field names on the wire are camelCase; Rust struct fields are snake_case via `serde(rename_all = "camelCase")`.

### Client construction

Module: `okx_outcomes_sdk::{OutcomesSdkClient, OutcomesSdkClientBuilder, TradingMode, ApiCredentials}`.

```rust
pub struct ApiCredentials {
    pub api_key:    String, // OK-ACCESS-KEY header value
    pub secret_key: String, // HMAC-SHA256 signing key; never transmitted
    pub passphrase: String, // OK-ACCESS-PASSPHRASE header value
}

pub enum TradingMode { Points } // X-Predictions-Mode header

impl OutcomesSdkClient {
    pub fn builder() -> OutcomesSdkClientBuilder;
    pub fn with_credentials(creds: ApiCredentials) -> Self;            // shortcut
    pub fn with_credentials_and_url(creds: ApiCredentials, base_url: impl Into<String>) -> Self;
}

impl OutcomesSdkClientBuilder {
    pub fn credentials(self, creds: ApiCredentials) -> Self;
    pub fn base_url(self, base_url: impl Into<String>) -> Self;       // default https://www.okx.com
    pub fn mode(self, mode: TradingMode) -> Self;                     // omitted if unset
    pub fn accept_language(self, lang: impl Into<String>) -> Self;    // Accept-Language (BCP-47)
    pub fn timeout_secs(self, secs: u64) -> Self;                     // default 10
    pub fn debug(self, debug: bool) -> Self;                          // debug builds only
    pub fn build(self) -> OutcomesSdkClient;
}
```

All configuration is explicit — the SDK reads no environment variables. Base URL resolution: the explicit `.base_url(..)` builder value (or `with_credentials_and_url` arg), else the compiled-in default `https://www.okx.com`. Endpoint constants are full absolute paths (`/api/v5/predictions/...`, `/api/v5/market/...`) that are concatenated with the base URL, so a single host setting covers both the outcomes and market-data calls.

### Errors

Module: `okx_outcomes_sdk::SdkError`.

Every fallible call returns `Result<T, SdkError>`. The enum is `#[non_exhaustive]`
(so future failure modes can be added without a breaking release — a `match` on
`SdkError` must include a wildcard `_` arm) and currently has eight variants:

```rust
#[non_exhaustive]
pub enum SdkError {
    /// Network failure: connection refused, DNS, timeout, TLS handshake.
    /// Transport-layer; retry is usually safe.
    Http(reqwest::Error),

    /// Server returned a non-zero business error code in the response envelope.
    /// `code` is the upstream OKX business code; inspect to decide retry / backoff / bail.
    /// On the wire OKX sends `code` as either a JSON string (`"50105"`) or number
    /// (`100015`); the SDK accepts both and normalizes to this `i64`. This variant
    /// covers both a non-zero code on an HTTP 200 envelope and a non-2xx response
    /// whose body is the standard `{ code, msg }` error shape.
    Api { code: i64, message: String },

    /// A non-2xx HTTP response whose body did *not* match the `{ code, msg }`
    /// error shape — e.g. an HTML error page from a proxy/gateway, or an empty
    /// body. Distinct from `Api` so a genuine API business code is never confused
    /// with a transport-level HTTP status. `body` is a char-bounded snippet
    /// (max 512 chars) of the raw response.
    UnexpectedStatus { status: u16, body: String },

    /// Response body could not be parsed against the expected schema.
    /// Often signals an SDK / server version mismatch.
    Deserialize(serde_json::Error),

    /// WS connect, send, login, or close failure.
    /// Includes login rejections (`60xxx` codes) and timeout-during-login.
    WebSocket { message: String },

    /// Reserved for callers that bypass the public constructors and end up with
    /// no credentials attached. Not produced by `with_credentials` /
    /// `with_credentials_and_url`, which always carry credentials.
    NotAuthenticated { hint: String },

    /// Invalid URL, missing state, or unexpected internal precondition.
    /// Almost always a programming error in the caller.
    Internal { message: String },

    /// Failure to serialize a request body or a WS payload.
    /// Rare in practice (only triggered by non-finite floats or similar).
    Serialization { message: String },
}
```

Variants implement `Display` via `thiserror`, so `format!("{e}")` produces a readable line for logging.

### Events and Markets

Module: `okx_outcomes_sdk::models::event::*`. API impls in `okx_outcomes_sdk::api::events`.

**`get_events`**

Retrieve a paginated list of outcome market events.

- **Endpoint:** `GET /api/v5/predictions/events`
- **Auth:** Required

```rust
pub async fn get_events(
    &self,
    status:    Option<&str>,   // "active" (default) | "resolved"
    category:  Option<&str>,   // "SPORTS" | "CURRENT_AFFAIRS"
    tag:       Option<&str>,   // sport tag ID
    league_id: Option<&str>,   // sports league ID
    sort:      Option<&str>,   // "volume" | "volume_24h" (default) | "ending_soon" | "newest"
    cursor:    Option<&str>,   // pagination cursor from previous `EventsResponse.pagination.next_cursor`
    page_size: Option<i32>,    // items per page, max 50 (default 10)
) -> Result<EventsResponse, SdkError>;

pub struct EventsResponse {
    events:     Vec<EventObject>,
    pagination: Pagination,    // see Common types
}

pub struct EventObject {
    id:                       String,         // central unique event ID
    event_id:                 String,         // event ID
    category:                 EventCategory,  // Sports / CurrentAffairs / Unknown
    neg_risk:                 bool,           // mutually-exclusive (negRisk) event
    status:                   EventStatus,    // Active / Paused / Resolved / Unknown
    event_title:              String,         // display title
    description:              String,         // long description
    event_icon:               Option<String>, // icon URL
    volume:                   String,         // total volume across all markets
    start_time:               Option<String>, // trading start (ms, as string)
    end_time:                 Option<String>, // trading end (ms, as string)
    created_at:               String,         // creation timestamp (ms, as string)
    total_markets_count:      i32,            // number of markets under this event
    final_outcomes_market_id: Option<String>, // winning market ID after settlement
    sport_event_data:         Option<SportEventData>, // non-None only for sports events
    markets:                  Vec<MarketObject>,      // list endpoints cap this at 2 entries;
                                                      // call `get_event_markets` for the full list
}
```

**`search`**

Search events and markets by keyword.

- **Endpoint:** `GET /api/v5/predictions/events/search`
- **Auth:** Required

```rust
pub async fn search(
    &self,
    keyword:   &str,           // free-text query (required)
    cursor:    Option<&str>,   // pagination cursor
    page_size: Option<i32>,    // default 10
) -> Result<EventsResponse, SdkError>;
```

Response: `EventsResponse` (same shape as `get_events`).

**`get_event`**

Retrieve a single event with its full market list inlined.

- **Endpoint:** `GET /api/v5/predictions/events/{eventId}`
- **Auth:** Required

```rust
pub async fn get_event(&self, event_id: &str) -> Result<EventObject, SdkError>;
```

`Api { code: 40404, ... }` is returned when the event ID does not exist.

**`get_event_markets`**

Retrieve all markets for an event (no pagination, no list cap).

- **Endpoint:** `GET /api/v5/predictions/events/{eventId}/markets`
- **Auth:** Required

```rust
pub async fn get_event_markets(&self, event_id: &str) -> Result<MarketsResponse, SdkError>;

pub struct MarketsResponse {
    markets: Vec<MarketObject>,
}

pub struct MarketObject {
    id:                        String,         // central unique market ID
    market_id:                 String,         // market ID
    neg_risk:                  bool,           // negRisk market flag
    status:                    MarketStatus,   // Active / Paused / Settling / Resolved / Unknown
    settle_stage:              i32,            // 0 = not started, 5 = settled
    question:                  String,         // full market question
    short_question:            Option<String>, // abbreviated question
    description:               String,         // long description
    market_icon:               Option<String>, // icon URL
    start_time:                String,         // trading start (ms, as string)
    end_time:                  String,         // trading end (ms, as string)
    resolve_start_at:          String,         // resolution window start (ms, as string)
    resolve_at:                String,         // resolution timestamp (ms, as string)
    best_bid:                  Option<String>, // decimal in [0, 1]; None when no bids exist
    best_ask:                  Option<String>, // decimal in [0, 1]; None when no asks exist
    last_trade_price:          Option<String>, // None until the first trade
    volume:                    String,         // market volume
    probability:               Option<String>, // YES-outcome probability in [0, 1]
    resolution_sources:        Vec<String>,    // URLs used for resolution
    yes_outcome:               OutcomeObject,
    no_outcome:                OutcomeObject,
}

pub struct OutcomeObject {
    token_id:     Option<String>, // conditional token address; None pre-deployment
    asset_id:     Option<String>, // asset ID; used as `inst_id` for orders / market data
    name:         String,         // "Yes" or "No"
    price:        String,         // decimal in [0, 1]
    final_result: Option<bool>,   // Some(true) = winner, Some(false) = loser, None = unsettled
}
```

**`get_market`**

Retrieve a single market.

- **Endpoint:** `GET /api/v5/predictions/markets/{marketId}`
- **Auth:** Required

```rust
pub async fn get_market(&self, market_id: &str) -> Result<MarketObject, SdkError>;
```

### Account: Balance

Module: `okx_outcomes_sdk::models::balance::*`. API in `okx_outcomes_sdk::api::balance`.

**`get_balance`**

Return the authenticated user's spendable balance per odds type.

- **Endpoint:** `GET /api/v5/predictions/balance`
- **Auth:** Required

```rust
pub async fn get_balance(&self) -> Result<BalanceResponse, SdkError>;

pub type BalanceResponse = Vec<BalanceEntry>;

pub struct BalanceEntry {
    odds_type: OddsType, // Points / Unknown
    balance:   String,   // total balance (units determined by odds_type)
    available: String,   // available balance (balance - frozen by open orders)
}
```

### Account: Orders

Module: `okx_outcomes_sdk::models::order::*`. API in `okx_outcomes_sdk::api::orders`.

**`place_order`**

Submit a signed limit (or trigger) order.

- **Endpoint:** `POST /api/v5/predictions/orders`
- **Auth:** Required (REST credentials) + EIP-712 signature

```rust
pub async fn place_order(&self, req: &PlaceOrderRequest) -> Result<TxHashResponse, SdkError>;

struct PlaceOrderRequest {
    action:    PlaceOrderAction,
    nonce:     i64,                // ms timestamp, anti-replay
    signature: SignatureWrapper,   // { Ecdsa: { r, s, v } }
}

struct PlaceOrderAction {
    action_type: String,           // always "placeOrder"
    grouping:    String,           // always "na"
    orders:      Vec<OrderItem>,
}

struct OrderItem {
    asset_id:        String,           // outcome assetId
    side:            SigningOrderSide, // Buy / Sell (placement-side lowercase wire — bytes feed the EIP-712 hash)
    market_type:     String,           // always "prediction"
    client_order_id: String,           // required; 34-char client order ID; see Signing > Client Order ID
    price:           String,           // decimal in [0, 1]
    reduce_only:     bool,
    size:            String,           // decimal
    size_type:       SizeType,         // Base (default, omitted on wire) / Quote
    order_type:      OrderTypeSpec,    // { limit: { tif } }
}

struct OrderTypeSpec   { limit: LimitOrderType }
struct LimitOrderType  { tif: LimitTif /* Gtc | Gtd { expires_after } | Ioc | Fok | Alo */ }
```

Response: `TxHashResponse { tx_hash: String }`.

Build the typed `signing::types::OrderRequest`, sign with `signing::sign_to_wrapper`, then derive the wire-side `OrderItem` via `OrderItem::from(&OrderRequest)` so signed bytes and JSON body cannot drift. See **Signing**.

**`cancel_order`**

Cancel a single active order (by server ID or by client order ID).

- **Endpoint:** `POST /api/v5/predictions/orders/cancel`
- **Auth:** Required + EIP-712 signature

```rust
pub async fn cancel_order(&self, req: &CancelOrderRequest) -> Result<TxHashResponse, SdkError>;

struct CancelOrderRequest {
    action:    CancelOrderAction,
    nonce:     i64,
    signature: SignatureWrapper,
}

struct CancelOrderAction {
    action_type: String,           // always "cancel"
    cancels:     Vec<CancelItem>,
}

struct CancelItem {
    asset_id:     String,
    market_type:  String,           // "prediction"
    // exactly one of:
    by: CancelBy,                   // serialized flat: { "oid": ... } or { "clientOrderId": ... }
}

enum CancelBy {
    Oid           { oid: String },          // server-assigned, decimal string
    ClientOrderId { client_order_id: String }, // 34-char client order ID, 0x-prefixed hex
}
```

Response: `TxHashResponse { tx_hash: String }`.

**`cancel_all`**

Cancel all active orders, or all active orders for a specific set of asset IDs.

- **Endpoint:** `POST /api/v5/predictions/orders/cancel-all`
- **Auth:** Required + EIP-712 signature

```rust
pub async fn cancel_all(&self, req: &CancelAllRequest) -> Result<TxHashResponse, SdkError>;

struct CancelAllRequest {
    action:        CancelAllAction,
    nonce:         i64,
    expires_after: i64,            // expiry timestamp (ms), required
    signature:     SignatureWrapper,
}

struct CancelAllAction {
    action_type: String,            // always "cancelAll"
    asset_ids:   Vec<String>,       // empty = all markets; non-empty = filter
    market_type: String,            // "prediction"
}
```

Response: `TxHashResponse`.

**`heartbeat`**

Renew the dead-man's switch protecting active orders.

- **Endpoint:** `POST /api/v5/predictions/heartbeat`
- **Auth:** Required + EIP-712 signature

```rust
pub async fn heartbeat(&self, req: &CancelAllRequest) -> Result<HeartbeatResponse, SdkError>;

struct HeartbeatResponse {
    server_timestamp: String, // server's current time (ms, as string)
    expire_at:        String, // when this heartbeat expires (ms, as string)
}
```

Request body is the same `CancelAllRequest` shape: the signed payload **is** the pre-authorized cancel-all that the server executes on your behalf if the heartbeat lapses. Set `nonce` to `now_ms` and `expires_after` to `now_ms + 300_000` (5 minutes). Call this more often than every 5 minutes.

**`get_order`**

Look up a single order by server-assigned ID.

- **Endpoint:** `GET /api/v5/predictions/orders/{orderId}`
- **Auth:** Required

```rust
pub async fn get_order(&self, order_id: &str) -> Result<OrderRecord, SdkError>;

pub struct OrderRecord {
    id:              String,           // server-assigned order ID
    oid:             String,           // order oid (distinct from `id`)
    market_id:       String,
    token_id:        String,           // YES/NO token contract address
    asset_id:        String,           // YES or NO outcome asset ID
    client_order_id: Option<String>,   // client order ID if one was supplied at placement
    side:            OrderSide,        // Buy / Sell / Unknown
    order_type:      TimeInForce,      // Gtc / Gtd / Ioc / Fok / PostOnly / Unknown
    size_type:       OrderSizeType,    // Base / Quote / Unknown
    size:            String,           // decimal
    price:           String,           // decimal
    expiration:      Option<String>,   // GTD expiry (ms, as string); None for non-GTD
    tx_hash:         String,           // submission tx hash
    status:          RestOrderStatus,  // PendingPlace / Active / PendingCancel / Filled /
                                       // PartiallyFilled / Failed / Cancelled / Expired / Unknown
    filled_size:     String,           // decimal
    filled_amount:   String,           // decimal
    fail_reason:     Option<String>,   // present only when status == RestOrderStatus::Failed
    cancel_reason:   Option<String>,   // set when the server cancelled (heartbeat lapse, market resolved, ...)
    odds_type:       OddsType,         // Points / Unknown
    created_at:      String,           // Unix ms (as string)
    updated_at:      String,           // Unix ms (as string)
}
```

**`list_orders`**

List the authenticated user's orders.

- **Endpoint:** `GET /api/v5/predictions/orders`
- **Auth:** Required

```rust
pub async fn list_orders(
    &self,
    market_id: Option<&str>,   // filter by market ID
    status:    Option<&str>,   // "open" (pending + active) | "closed" (filled / cancelled / expired / failed)
    cursor:    Option<&str>,   // pagination cursor
    limit:     Option<i32>,    // max 50, default 20
) -> Result<OrdersResponse, SdkError>;

// Type alias of the shared paged-list response shape.
pub type OrdersResponse = PagedListResponse<OrderRecord>;
// pub struct PagedListResponse<T> { list: Vec<T>, next_cursor: Option<String>, has_next: bool }
```

### Account: Positions

Module: `okx_outcomes_sdk::models::position::*`. API in `okx_outcomes_sdk::api::positions`.

**`get_positions`**

Query the authenticated user's positions.

- **Endpoint:** `GET /api/v5/predictions/positions`
- **Auth:** Required

```rust
pub async fn get_positions(
    &self,
    status:    Option<&str>,   // "open" | "closed"; omit for all
    market_id: Option<&str>,
    cursor:    Option<&str>,   // pagination cursor
    limit:     Option<i32>,    // max 100, default 20
) -> Result<PositionsResponse, SdkError>;

pub type PositionsResponse = PagedListResponse<PositionRecord>;

pub struct PositionRecord {
    id:                         String,         // identifiers
    token_id:                   String,
    market_id:                  String,
    token_index:                String,         // "1" = YES, "2" = NO
    token_name:                 String,         // "Yes" or "No"
    size:                       String,         // current remaining size
    available_size:             String,         // size − amount frozen by SELL orders
    value:                      String,         // cur_price * size
    avg_price:                  String,         // weighted average entry cost
    un_realized_pnl:            String,         // unrealized P&L
    un_realized_pnl_percentage: String,
    title:                      String,         // display string
    icon:                       String,         // display string
    event_id:                   String,
    winning_token:              Option<String>, // winning token ID after settlement; None until settled
    position_status:            i32,            // position status code (see API reference for the full enum)
    cur_price:                  String,         // current token price
    realized_pnl:               String,         // realized P&L
    realized_pnl_percentage:    String,
    odds_type:                  OddsType,       // Points / Unknown
                                                 // (verified live: wire value is "points")
}
```

### Account: Trades

Module: `okx_outcomes_sdk::models::trade::*`. API in `okx_outcomes_sdk::api::trades`.

**`get_trades`**

Query the authenticated user's fill history.

- **Endpoint:** `GET /api/v5/predictions/trades`
- **Auth:** Required

```rust
pub async fn get_trades(
    &self,
    market_id:  Option<&str>,   // filter by market ID
    side:       Option<&str>,   // "BUY" | "SELL"
    start_time: Option<i64>,    // inclusive start (ms)
    end_time:   Option<i64>,    // exclusive end (ms)
    cursor:     Option<&str>,   // pagination cursor
    limit:      Option<i32>,    // max 100, default 20
) -> Result<TradesResponse, SdkError>;

type TradesResponse = PagedListResponse<TradeRecord>;

struct TradeRecord {
    trade_id:   String,      // empty for TAKER rows and pre-onchain MAKER rows
    order_id:   String,
    market_id:  String,
    token_id:   String,
    side:       OrderSide, // Buy / Sell / Unknown
    size:       String, // tokens filled
    amount:     String, // filled
    price:      String,
    fee:        String,
    role:       Role,      // Maker / Taker / Unknown
    tx_hash:    String,
    created_at: String,      // Unix ms (as string)
}
```

`trade_id` is `None` for TAKER rows and for legacy MAKER rows that predate on-chain trade-id assignment.

### Conditional Tokens

Module: `okx_outcomes_sdk::models::position::*` (shared with positions). API in `okx_outcomes_sdk::api::positions`.

All three are write operations requiring an EIP-712 signature. Each takes a request body of the same outer shape: `{ action, nonce, signature }` with `action` differing per operation. Each returns `TxHashResponse { tx_hash: String }`.

**`split`**

Split into equal YES + NO tokens for a market (the inverse of `merge`).

- **Endpoint:** `POST /api/v5/predictions/positions/split`
- **Auth:** Required + EIP-712 signature

```rust
pub async fn split(&self, req: &SplitRequest) -> Result<TxHashResponse, SdkError>;

struct SplitRequest { action: SplitAction, nonce: i64, signature: SignatureWrapper }
struct SplitAction {
    action_type: String, // "predictionSplit"
    market_id:   String,
    size:        String, // minimum units
}
```

**`merge`**

Combine equal YES + NO tokens.

- **Endpoint:** `POST /api/v5/predictions/positions/merge`
- **Auth:** Required + EIP-712 signature

```rust
pub async fn merge(&self, req: &MergeRequest) -> Result<TxHashResponse, SdkError>;

struct MergeRequest { action: MergeAction, nonce: i64, signature: SignatureWrapper }
struct MergeAction {
    action_type: String, // "predictionMerge"
    market_id:   String,
    size:        String,
}
```

**`redeem`**

Redeem the caller's full winning token balance after market settlement. There is no `size` field; the server redeems whatever the caller holds.

- **Endpoint:** `POST /api/v5/predictions/positions/redeem`
- **Auth:** Required + EIP-712 signature

```rust
pub async fn redeem(&self, req: &RedeemRequest) -> Result<TxHashResponse, SdkError>;

struct RedeemRequest { action: RedeemAction, nonce: i64, signature: SignatureWrapper }
struct RedeemAction {
    action_type: String, // "predictionRedeem"
    market_id:   String,
}
```

`Api { code: 51020, ... }` is returned when the market is not yet resolved.

### Market Data

Module: `okx_outcomes_sdk::models::price::*`. API in `okx_outcomes_sdk::api::prices`.

These calls hit OKX's market-data API at `https://www.okx.com/api/v5/market/*` — same host as the outcomes API but a different path prefix and a different response envelope. The market-data envelope wraps `code` as a JSON string, so the `code` carried by `SdkError::Api { code }` is the parsed integer of that string field.

**`get_ticker`**

Latest quote for a single instrument. `inst_id` is the market's `yes_outcome.asset_id`.

- **Endpoint:** `GET /api/v5/market/ticker`
- **Auth:** Required

```rust
pub async fn get_ticker(&self, inst_id: &str) -> Result<Ticker, SdkError>;

pub struct Ticker {
    inst_type:  String,
    inst_id:    String,
    last:       String, // last trade price
    last_sz:    String, // last trade size
    ask_px:     String, // top-of-book ask price
    ask_sz:     String, // top-of-book ask size
    bid_px:     String, // top-of-book bid price
    bid_sz:     String, // top-of-book bid size
    open24h:    String, // 24h open
    high24h:    String, // 24h high
    low24h:     String, // 24h low
    vol24h:     String, // 24h volume (base)
    vol_ccy24h: String, // 24h volume (quote)
    sod_utc0:   String, // UTC 0  opening price
    sod_utc8:   String, // UTC+8 opening price
    ts:         String, // update timestamp (Unix ms as decimal string)
}
```

The server returns a 1-element array; the SDK unwraps it. `Api { code: -1, message: "ticker not found" }` if the inst ID is unknown.

**`get_candles`**

K-line history.

- **Endpoint:** `GET /api/v5/market/candles`
- **Auth:** Required

```rust
pub async fn get_candles(
    &self,
    inst_id: &str,
    bar:     Option<&str>,   // "1m" / "5m" / "15m" / "30m" / "1H" / "4H" / "1D" / ... ; default "1m"
    after:   Option<&str>,   // return candles with timestamp BEFORE this value (ms)
    before:  Option<&str>,   // return candles with timestamp AFTER  this value (ms)
    limit:   Option<i32>,    // max 100, default 100
) -> Result<Vec<Candle>, SdkError>;

pub struct Candle(pub Vec<String>);

impl Candle {
    pub fn ts(&self)        -> &str;   // index 0: open time (Unix ms as string)
    pub fn open(&self)      -> &str;   // index 1: open price
    pub fn high(&self)      -> &str;   // index 2: high price
    pub fn low(&self)       -> &str;   // index 3: low price
    pub fn close(&self)     -> &str;   // index 4: close price
    pub fn vol(&self)       -> &str;   // index 5: volume (contracts)
    // index 6: volume in pricing currency (no helper)
    // index 7: volume in quote currency   (no helper)
    pub fn confirmed(&self) -> bool;   // index 8: true when "1" (bar closed)
}
```

**`get_pm_books`**

Outcome-market order book depth snapshot.

- **Endpoint:** `GET /api/v5/market/pm-books`
- **Auth:** Required
- **Rate limit:** 40 requests / 2s

```rust
pub async fn get_pm_books(
    &self,
    inst_id: &str,             // YES-outcome asset ID
    sz:      Option<i32>,      // depth levels per side; max 400 (up to 800 total).
                               // Defaults to 1 (BBO only) when omitted.
) -> Result<PmBookDepth, SdkError>;

pub struct PmBookDepth {
    asks:   Vec<Vec<String>>,  // ask levels, ascending by price. Each entry is [price, size, order_count].
    bids:   Vec<Vec<String>>,  // bid levels, descending by price. Each entry is [price, size, order_count].
    ts:     String,            // snapshot timestamp (Unix ms as decimal string)
    seq_id: i64,               // order book version sequence; opaque to most callers,
                               // exposed for parity with the API response
}
```

The server returns a 1-element `data` array; the SDK unwraps it. `Api { code: -1, message: "pm-books snapshot not found" }` if the response is empty.

### WebSocket

Module: `okx_outcomes_sdk::ws::*`. Requires the `websocket` Cargo feature.

**Connection model**

The Open API uses a single endpoint for both public and private channels: `wss://<host>/ws/v5/business`. Public channels work anonymously. Private channels require a one-time `op: "login"` after the WS handshake.

Hosts:

```rust
pub mod ws::endpoints {
    pub const DEFAULT_WS_HOST: &str = "wss://ws.okx.com:8443";
    pub const EU_WS_HOST:      &str = "wss://wseea.okx.com";
    pub const US_WS_HOST:      &str = "wss://wsus.okx.com";
    pub const BUSINESS_PATH:   &str = "/ws/v5/business";
}
```

`OutcomesWsClient` defaults to `DEFAULT_WS_HOST`; override with `OutcomesWsClient::builder().host(...).build()` (or the `with_host(...)` shortcut). Debug logging is set via `.debug(true)`. The SDK reads no environment variables.

Lifecycle and resilience:

- 25-second ping keepalive (OKX requires < 30 s).
- Auto-reconnect with exponential backoff (3 s -> 6 s -> 12 s -> capped at 30 s).
- On reconnect, the client replays login (if credentials are stored) and re-subscribes to every channel that was active when the connection dropped.
- `connection_state_callback("public" | "private", connected: bool)` fires on every transition.

**Public API**

```rust
pub struct OutcomesWsClient { /* ... */ }

impl OutcomesWsClient {
    pub fn new() -> Self;
    pub fn with_host(host: &str) -> Self;

    pub async fn connect(&self, path: &str) -> Result<(), SdkError>;
    pub async fn login(&self, creds: &ApiCredentials) -> Result<(), SdkError>;
    pub async fn subscribe(&self, channel: &str, params: Vec<HashMap<String, String>>) -> Result<(), SdkError>;
    pub async fn unsubscribe(&self, channel: &str, params: Vec<HashMap<String, String>>) -> Result<(), SdkError>;
    pub async fn disconnect(&self);

    pub fn set_on_data(&self, callback: WsDataCallback);
    pub fn set_on_connection_state(&self, callback: WsConnectionStateCallback);
}

pub type WsDataCallback             = Arc<dyn Fn(&WsMessage) + Send + Sync>;
pub type WsConnectionStateCallback  = Arc<dyn Fn(&str, bool) + Send + Sync>;
```

`subscribe` is idempotent: calling it with the same `(channel, params)` pair twice does not double-subscribe and does not duplicate the replay entry.

Login signing (handled internally by `login`): the SDK computes `sign = Base64(HMAC-SHA256(secret_key, timestamp + "GET" + "/users/self/verify"))` and sends:

```json
{"op": "login", "args": [{"apiKey": "...", "passphrase": "...", "timestamp": "...", "sign": "..."}]}
```

The future returned by `login` resolves only after the server responds:

- `{"event":"login","code":"0"}` -> `Ok(())`.
- `{"event":"error","code":"600xx",...}` -> `Err(SdkError::WebSocket { message: "Login rejected: [60xxx] ..." })`.
- No response within 30 s -> `Err(SdkError::WebSocket { message: "Login timed out (30s)" })`.

**Message dispatch**

Every incoming JSON frame is parsed once into a `WsMessage` enum and handed to `on_data`. Consumers never see raw JSON.

```rust
pub enum WsMessage {
    Event {
        event:   String,
        channel: Option<String>,
        inst_id: Option<String>,
        msg:     Option<String>,
    },
    Prices(Vec<WsPriceTick>),
    Books { data: Vec<WsPmBookData>, action: String }, // action = "snapshot" | "update"
    Trades(Vec<WsPmTrade>),
    Tickers(Vec<WsPmTicker>),
    EventStatus(Vec<WsEventStatus>),
    Candle(Vec<Candle>),                   // typed wrapper over the 9-column OHLCV array
    Orders(Vec<WsOrder>),
    Positions(Vec<WsPosition>),
    UserTrades(Vec<WsUserTrade>),
    Balance(Vec<WsBalance>),
    Pnl(Vec<WsPnl>),
    Unknown { channel: String, raw: serde_json::Value },
}
```

`WsMessage::Event` carries `event:"subscribe"|"unsubscribe"|"login"|"error"` confirmations and is independent of any data channel.

**Public channels**

**`prediction-market-prices`**

Per-market price ticks.

- **Subscribe params:** `[{"instId": "<asset_id>"}]` (one entry per market).
- **Message variant:** `WsMessage::Prices(Vec<WsPriceTick>)`.

```rust
struct WsPriceTick {
    yes_asset_id:     String,
    last_trade_price: String,         // last trade price
    best_bid:         String,         // best bid price
    best_ask:         String,         // best ask price
    timestamp:        String,         // Unix ms as decimal string
    probability:      String,         // basis points * 100, e.g. "6500" = 65.00%
    market_volume:    String,
    event_volume:     String,
    event_id:         String,
}
```

**`pm-books`**

Order book snapshots and incremental updates.

- **Subscribe params:** `[{"instId": "<asset_id>"}]`.
- **Message variant:** `WsMessage::Books { data, action }` where `action` is `"snapshot"` (full book on subscribe / reconnect) or `"update"` (incremental delta).

```rust
struct WsPmBookData {
    asks:        Vec<Vec<String>>, // [[price, size, ...], ...]
    bids:        Vec<Vec<String>>, // [[price, size, ...], ...]
    ts:          String,
    checksum:    Option<i64>,      // CRC32 of canonical book for integrity check
    seq_id:      Option<i64>,      // monotonic sequence; gaps = drop
    prev_seq_id: Option<i64>,      // -1 for first snapshot
}
```

When `prev_seq_id` does not match the previous frame's `seq_id`, drop the current local book and wait for the next `snapshot`.

**`pm-trades`**

Public trade prints.

- **Subscribe params:** `[{"instId": "<asset_id>"}]`.
- **Message variant:** `WsMessage::Trades(Vec<WsPmTrade>)`.

```rust
struct WsPmTrade {
    inst_id:  String,
    trade_id: Option<String>, // per-trade push: Some; aggregated: None
    f_id:     Option<String>, // aggregated push: first trade id; per-trade: None
    l_id:     Option<String>, // aggregated push: last trade id;  per-trade: None
    px:       String,         // price
    sz:       String,         // size
    side:     String,         // "buy" / "sell" (the taker side)
    ts:       String,
}
```

Distinguish single-trade vs window-aggregated messages by which of `trade_id` vs (`f_id`,`l_id`) is `Some`.

**`pm-tickers`**

OKX-style per-instrument ticker push.

- **Subscribe params:** `[{"instId": "<asset_id>"}]`.
- **Message variant:** `WsMessage::Tickers(Vec<WsPmTicker>)`.

```rust
struct WsPmTicker {
    inst_type: String, inst_id: String,
    last:       String, last_sz: String,
    ask_px:     String, ask_sz:  String,
    bid_px:     String, bid_sz:  String,
    open24h:    String, high24h: String, low24h: String,
    vol24h:     String, vol_ccy24h: String,
    sod_utc0:   String, sod_utc8: String,
    ts:         String,
}
```

**`pm-event-status`**

Event settlement push.

- **Subscribe params:** `[{"instId": "event-<event_id>"}]`. The SDK does **not** auto-prefix `event-` on the WS path; pass it explicitly.
- **Message variant:** `WsMessage::EventStatus(Vec<WsEventStatus>)`.

```rust
struct WsEventStatus {
    event_id:       String,
    status:         String,  // e.g. "resolved"
    market_id:      String,  // winning market ID
    outcome_option: String,  // "yes" / "no" / "others" / team name / "draw"
    timestamp:      String,
}
```

**`pm-candle*`**

Candlestick stream. The channel name encodes the bar: `pm-candle1m`, `pm-candle5m`, `pm-candle1H`, `pm-candle1D`, etc.

- **Subscribe params:** `[{"instId": "<asset_id>"}]`.
- **Message variant:** `WsMessage::Candle(Vec<Candle>)` where each `Candle` is a typed wrapper over the 9-column OHLCV array. Accessors: `ts()`, `open()`, `high()`, `low()`, `close()`, `vol()`, `vol_ccy()`, `vol_ccy_quote()`, `confirmed()`.

**Private channels (require `login`)**

Subscribe with empty params (the server scopes the stream to the logged-in account).

**`pm-order`**

Order status changes.

- **Subscribe params:** `[]`.
- **Message variant:** `WsMessage::Orders(WsOrder)`.

```rust
struct WsOrder {
    order_id:        String,
    market_id:       String,
    status:          OrderStatus,       // Active / Filled / PartiallyFilled / PlaceFailed /
                                        // CancelFailed / Cancelled / Expired / Unknown
    side:            OrderSide,         // Buy / Sell / Unknown
    // All fields below are variant-specific per the spec's status →
    // required-fields table. Modelled as Option so missing keys
    // deserialize to None.
    client_order_id: Option<String>,
    asset_id:        Option<String>,    // YES asset id or NO asset id
    direction:       Option<Direction>, // Yes / No / Unknown — which outcome this order takes
    filled_size:     Option<String>,
    order_size:      Option<String>,    // serde alias = "size"
    avg_price:       Option<String>,
    amount:          Option<String>,    // BUY = spent, SELL = received (pts)
    limit_price:     Option<String>,    // serde alias = "price"
    fail_message:    Option<String>,    // only on PLACE_FAILED / CANCEL_FAILED
    odds_type:       Option<OddsType>,
    tx_hash:         Option<String>,    // serde rename = "txHash"
    trade_id:        Option<String>,
}
```

**`pm-position`**

Position updates.

- **Subscribe params:** `[]`.
- **Message variant:** `WsMessage::Positions(WsPosition)`.

The spec defines two payload variants on this channel; `WsPosition` is
a single flat struct with variant-specific fields behind `Option`. Branch
on `status` (use `PositionStatus::is_position_snapshot()` /
`is_failed()`) to know which fields are meaningful.

```rust
struct WsPosition {
    // Common across both variants
    market_id: String,
    status:    PositionStatus,    // Fill / FillFailed / Redeem / RedeemFailed /
                                  // Split / SplitFailed / Merge / MergeFailed /
                                  // Deposit / DepositFailed / Withdraw / WithdrawFailed / Unknown
    amount:    String,            // Type 1: position `remain` ("0" for REDEEM)
                                  // Type 2: split/merge/deposit/withdraw amount
    odds_type: Option<OddsType>,

    // Variant 1 (FILL / REDEEM / *_FAILED) — full position snapshot
    id:                          Option<String>,
    token_id:                    Option<String>,
    asset_id:                    Option<String>,
    timestamp:                   Option<String>,
    un_realized_pnl:             Option<String>,
    un_realized_pnl_percentage:  Option<String>,
    value:                       Option<String>,
    avg_price:                   Option<String>,
    trade_id:                    Option<String>,

    // Variant 2 (SPLIT / MERGE / DEPOSIT / WITHDRAW / *_FAILED)
    tx_hash: Option<String>,           // serde rename = "txHash"
    ext:     Option<WsPositionExt>,    // populated for DEPOSIT
}

struct WsPositionExt {
    to_tx_hash: Option<String>,        // serde rename = "toTxHash"; spec: String | null
}
```

**`pm-user-trade`**

User-specific fill stream.

- **Subscribe params:** `[]`.
- **Message variant:** `WsMessage::UserTrades(WsUserTrade)`.

```rust
struct WsUserTrade {
    order_id:        String,
    client_order_id: Option<String>, // None when client didn't supply one (spec: string | null)
    market_id:       String,
    token_id:        String,
    asset_id:        String,    // yesAssetId or noAssetId
    side:            OrderSide, // Buy / Sell / Unknown
    size:            String,
    price:           String,
    txhash:          String,
    timestamp:       String,
    trade_id:        String,    // Trade ID
}
```

**`pm-balance`**

Balance changes.

- **Subscribe params:** `[]`.
- **Message variant:** `WsMessage::Balance(WsBalance)`.

```rust
struct WsBalance {
    wallet_address: String,
    available:      String,
    total:          String,
    frozen:         String,
    token_id:       String,                  // on-chain Point token id
    change_type:    BalanceChangeType,       // Place / Cancel / Fill / Split / Merge /
                                              // Redeem / Deposit / Withdraw / Unknown
    change_amount:  Option<String>,          // spec: "may be null"
    update_time:    String,
    odds_type:      Option<OddsType>,
}
```

**`pm-pnl`**

Floating P&L stream — pushes **two distinct payload shapes**; modelled
as a serde `untagged` enum so the right variant is picked automatically
based on which discriminating fields are present.

- **Subscribe params:** `[]`.
- **Message variant:** `WsMessage::Pnl(WsPnl)`.

```rust
enum WsPnl {
    Overview(WsPnlOverview),     // portfolioValue + per-period summary
    Timeseries(WsPnlTimeseries), // chart points with high/low/current
}

struct WsPnlOverview {
    portfolio_value: String,                 // point balance + position market value
    periods:         Vec<WsPnlPeriodSummary>,
}

struct WsPnlPeriodSummary {
    period:      String, // "1D" / "1W" / "1M" / "6M" / "1Y"
    period_pnl:  String,
    pnl_percent: String,
}

struct WsPnlTimeseries {
    period:      String,         // "0"=1D / "1"=1W / "2"=1M / "3"=6M / "4"=1Y
    interval:    String,         // ms: 600000 / 1800000 / 3600000 / 86400000
    points:      Vec<WsPnlPoint>,
    current_pnl: String,
    high:        String,
    low:         String,
}
```

**WS error codes**

WS-level errors are surfaced via `WsMessage::Event { event: "error", msg, .. }` and (for login) via the `Err` returned by `login()`. Common codes:

| Code | Meaning |
| --- | --- |
| `60004` | Invalid timestamp on login (clock drift, expired). |
| `60005` | Invalid API key. |
| `60006` | Timestamp expired (30 s window). |
| `60007` | Invalid signature. |
| `60009` | Login failed (generic). |
| `60011` | Login required for this private channel. |
| `60012` | Invalid `op` value. |
| `60018` | Subscribe failed (channel name or params). |

### Signing

Module: `okx_outcomes_sdk::signing::*`. Requires the `signing` Cargo feature.

The full pipeline for any write call: build a typed `Action`, run it through `sign_to_wrapper` with your `k256::ecdsa::SigningKey`, then drop the resulting `SignatureWrapper` into the request body.

```rust
pub fn parse_private_key(hex_key: &str) -> Result<SigningKey, String>;
pub fn now_millis() -> u64;

pub fn sign_to_wrapper(
    action:        &Action,
    nonce:         u64,
    expires_after: Option<u64>,
    chain:         ChainType,   // Mainnet / Testnet — Agent `source`; passed explicitly
    key:           &SigningKey,
) -> Result<SignatureWrapper, String>;
```

Action constructors:

```rust
pub fn action_place_order(orders: Vec<OrderRequest>) -> Action;
pub fn action_cancel(cancels: Vec<CancelRequest>) -> Action;
pub fn action_cancel_all(asset_ids: Vec<String>, market_type: &str) -> Action;
pub fn action_prediction_split (market_id: &str, size: &str) -> Action;
pub fn action_prediction_merge (market_id: &str, size: &str) -> Action;
pub fn action_prediction_redeem(market_id: &str) -> Action;
```

Typed inputs:

```rust
struct OrderRequest {
    asset_id:        String,
    side:            SigningOrderSide,  // Buy / Sell (placement-side lowercase wire)
    market_type:     String,            // "prediction"
    client_order_id: Option<String>,    // 34-char client order ID
    price:           String,
    reduce_only:     bool,
    size:            String,
    size_type:       SizeType,          // Base (default) / Quote
    order_type:      OrderType,
}

enum OrderType {
    Limit(LimitOrderType),
}
struct LimitOrderType { tif: LimitTif }
enum LimitTif {
    Gtc, Ioc, Fok, Alo,                             // serialize as "gtc" / "ioc" / "fok" / "alo"
    Gtd { expires_after: u64 },                     // { "gtd": { "expiresAfter": <ms> } }
}

struct CancelRequest {
    asset_id:    String,
    market_type: String,                            // "prediction"
    target:      CancelTarget,
}
enum CancelTarget { Oid(String), ClientOrderId(String) }
```

The wire-side counterparts (`OrderItem`, `CancelItem`) implement `TryFrom<&OrderRequest>` and `From<&CancelRequest>` so the JSON body and the signed bytes are built from the same source struct.

Client order IDs:

```rust
pub fn generate_client_order_id_default() -> Result<String, String>;
pub fn generate_client_order_id(region: Region, env: Env) -> Result<String, String>;
pub fn validate_client_order_id(s: &str) -> bool;
pub fn parse_client_order_id_prefix(client_order_id: Option<&str>) -> ClientOrderIdPrefix;
pub fn register_client_order_id_context(region: Region, env: Env);
```

Client order IDs are 34-character hex strings of the shape `0x{region}{env}{30-hex random}`. `generate_client_order_id_default()` uses the registered global context, or the compiled-in HK / PROD default (the SDK reads no environment variables). Call `register_client_order_id_context(region, env)` once at startup to override, or pass values explicitly to `generate_client_order_id(region, env)`.

Low-level helpers:

```rust
pub fn signer_address(key: &SigningKey) -> String;
pub fn ecrecover(signing_hash: &str, signature: &str) -> Result<String, String>;
pub fn sign_action(...) -> Result<String, String>;        // returns "0x..." hex
pub fn sign_action_full(...) -> Result<(String, String, String, u8), String>; // (txhash, r, s, v)
pub fn sign_action_debug(...) -> Result<SigningDebug, String>; // returns all intermediate hashes
```

Use `sign_to_wrapper` for normal flows. The lower-level functions exist for debugging and for callers that need access to the txhash (e.g. to display "view on explorer" links).

### Common types

Module: `okx_outcomes_sdk::models::common::*`.

```rust
struct Pagination {
    next_cursor: Option<String>, // None on last page
    has_more:    bool,
    page_size:   i32,            // items in the current page
}

struct EcdsaSignature {
    r: String, // hex, 0x-prefixed
    s: String, // hex, 0x-prefixed
    v: u8,     // recovery id: 0 or 1
}

struct SignatureWrapper {
    // serialized as { "Ecdsa": { r, s, v } }
    ecdsa: EcdsaSignature,
}
```

Two API envelopes are wrapped transparently by the SDK:

- Outcomes REST: `{ "code": <int>, "message": "...", "data": <T> }` where `code == 0` means success.
- OKX market data: `{ "code": "<int>", "msg": "...", "data": <T> }` (note the string code). `code == "0"` means success.

A non-zero code from either envelope becomes `SdkError::Api { code, message }`.

## okx-outcomes-sdk

A typed Rust client for the OKX Outcomes Market Developer API, covering the full REST surface (events, markets, balance, orders, positions, trades), EIP-712 transaction signing, and a real-time WebSocket transport with auto-reconnect.

## Installation

Add the SDK and the Tokio runtime to your `Cargo.toml`.

Add it from the public Git repository:

```toml
[dependencies]
okx-outcomes-sdk = { git = "https://github.com/okx/outcomes-sdk.git", features = ["signing", "websocket"] }
tokio = { version = "1", features = ["full"] }
```

To pin a specific release, add `tag = "v0.1.0"` (or `branch` / `rev`) to the Git dependency.

That is the complete dependency list. `tokio-tungstenite`, `k256`, `alloy-*`, `rmp-serde`, `futures-util`, etc. are pulled in transitively by the SDK's feature gates. Do **not** add them yourself, or you risk version mismatches with what the SDK was built against.

### Feature flags

| Feature | What it enables | When to turn it on |
| --- | --- | --- |
| _default_ | REST client only (events, markets, orders, positions, balance, trades, prices). | Read-only integrations. |
| `signing` | EIP-712 + ECDSA action signing helpers. | Any write call: `place_order`, `cancel_order`, `cancel_all`, `split`, `merge`, `redeem`, `heartbeat`. |
| `websocket` | `OutcomesWsClient` (tokio-tungstenite based) and the typed `WsMessage` parser. | Real-time prices, trades, order books, user-order/position/balance streams. |

## Authentication

All REST endpoints require OKX API credentials. Construct the client with `with_credentials`:

```rust
use okx_outcomes_sdk::{ApiCredentials, OutcomesSdkClient};

let creds = ApiCredentials {
    api_key:    std::env::var("OUTCOMES_API_KEY")?,
    secret_key: std::env::var("OUTCOMES_API_SECRET")?,
    passphrase: std::env::var("OUTCOMES_API_PASSPHRASE")?,
};
let client = OutcomesSdkClient::with_credentials(creds);
```

Notes:

- The `secret_key` is never sent over the wire. The SDK signs every request locally with HMAC-SHA256 per OKX's REST authentication spec (`OK-ACCESS-SIGN = Base64(HMAC-SHA256(secret_key, timestamp + METHOD + path + body))`).
- Place-order / cancel / split / merge / redeem additionally require a **private key** for EIP-712 signing. This is independent of your REST API key.

## Querying data

### Listing events

`get_events` returns a paginated page of events. All filters are optional:

```rust
let page = client
    .get_events(
        Some("active"),      // status: "active" | "resolved"
        None,                // category: "SPORTS" | "CURRENT_AFFAIRS"
        None,                // tag: sport tag ID
        None,                // league_id
        Some("volume_24h"),  // sort: "volume" | "volume_24h" | "ending_soon" | "newest"
        None,                // cursor (from previous page)
        Some(20),            // page_size (max 50)
    )
    .await?;

for event in &page.events {
    println!("{} - {} markets ({} pts volume)",
        event.event_title, event.total_markets_count, event.volume);
}

// Pagination: pass `page.pagination.next_cursor` back as the `cursor` arg.
```

Related read methods:

| Method | Purpose |
| --- | --- |
| `client.search(keyword, cursor, page_size)` | Search events and markets by keyword. |
| `client.get_event(event_id)` | Single event with its full market list. |
| `client.get_event_markets(event_id)` | All markets for an event (no pagination). |
| `client.get_market(market_id)` | Single market by market ID. |
| `client.get_ticker(inst_id)` | Latest quote for a single market instrument. |
| `client.get_candles(...)` | K-line history. |
| `client.get_trades(...)` | Recent public trade history. |

### Account balance, orders, and positions

```rust
let entries = client.get_balance().await?; // Vec<BalanceEntry>
for entry in &entries {
    println!("[{}] available={} total={}", entry.odds_type, entry.available, entry.balance);
}

let orders = client.list_orders(
    None,         // market_id filter
    Some("open"), // "open" (pending+active) or "closed"
    None,         // cursor
    Some(50),     // limit (max 50, default 20)
).await?;
println!("{} open orders, has_next={}", orders.list.len(), orders.has_next);

let positions = client.get_positions(
    Some("open"), // "open" | "closed"
    None,         // market_id
    None,         // cursor
    Some(50),     // limit
).await?;
println!("{} positions", positions.list.len());
```

Both `list_orders` and `get_positions` paginate via `cursor` + `limit`: the previous response's `next_cursor` field becomes the next call's `cursor` argument, and `has_next` is `false` once the cursor is exhausted.

### Checking open orders

`list_orders` with `status = Some("open")` returns every non-terminal order on the account, paginated. Pass the prior response's `next_cursor` back through the `cursor` argument until `has_next` becomes `false`:

```rust
use okx_outcomes_sdk::models::order::OrderRecord;

let mut cursor: Option<String> = None;
let mut all_open: Vec<OrderRecord> = Vec::new();

loop {
    let page = client
        .list_orders(
            None,             // market_id: None = all markets, Some("123") to scope to one
            Some("open"),     // "open" = pending + active; "closed" = filled / cancelled / expired / failed
            cursor.as_deref(),
            Some(50),         // max items per page
        )
        .await?;
    all_open.extend(page.list);
    if !page.has_next {
        break;
    }
    cursor = page.next_cursor;
}

println!("{} open orders", all_open.len());
for o in &all_open {
    println!(
        "#{} [{}] {} {}/{} @ {}  market={} asset={} client_order_id={:?}",
        o.id, o.status, o.side, o.filled_size, o.size, o.price,
        o.market_id, o.asset_id, o.client_order_id,
    );
}
```

Each `OrderRecord` exposes the placement parameters (`side: OrderSide`, `size`, `price`, `order_type: TimeInForce`, `size_type: OrderSizeType`, `expiration`), placement metadata (`market_id`, `asset_id`, `client_order_id`, `tx_hash`), and the current lifecycle (`status: RestOrderStatus`, `filled_size`, `filled_amount`). For a strictly-resting view filter to `o.status == RestOrderStatus::Active`; that excludes orders the matching engine is still ingesting and orders mid-cancel.

Tips:

- `market_id` is a decimal-string ID; pass it as `Some("123456789")`.
- `client_order_id` (the client order ID supplied at placement) can be used with `cancel_order` to cancel by client ID rather than server ID.

## Placing an order

Place-order has three steps: build the typed `OrderRequest`, sign it with EIP-712, then post the wire-format `PlaceOrderRequest`. The SDK provides matching types for each step so the signed bytes and the JSON body cannot drift.

```rust
use okx_outcomes_sdk::{ApiCredentials, OutcomesSdkClient};
use okx_outcomes_sdk::models::order::{OrderItem, PlaceOrderAction, PlaceOrderRequest};
use okx_outcomes_sdk::signing::{
    action_place_order, generate_client_order_id_default, now_millis, parse_private_key, sign_to_wrapper,
    ChainType, LimitOrderType, LimitTif, OrderRequest, OrderType, SigningOrderSide, SizeType,
};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let creds = ApiCredentials {
        api_key:    std::env::var("OUTCOMES_API_KEY")?,
        secret_key: std::env::var("OUTCOMES_API_SECRET")?,
        passphrase: std::env::var("OUTCOMES_API_PASSPHRASE")?,
    };
    let client = OutcomesSdkClient::with_credentials(creds);

    // 1. Load the on-chain signing key (hex, with or without 0x prefix).
    let key = parse_private_key(&std::env::var("OUTCOMES_SIGNING_KEY")?)?;

    // 2. Build the typed order. `asset_id` is the outcome asset ID.
    let order_request = OrderRequest {
        asset_id:         "100170100".into(),
        side:             SigningOrderSide::Buy,
        market_type:      "prediction".into(),
        client_order_id:  generate_client_order_id_default()?,
        price:            "0.65".into(),
        reduce_only:      false,
        size:             "100".into(),
        size_type:        SizeType::Base, // Quote = quote-denominated size
        order_type:       OrderType::Limit(LimitOrderType { tif: LimitTif::Gtc }),
    };

    // 3. Derive the wire-format OrderItem from the same OrderRequest so the
    //    JSON body cannot drift from the signed msgpack bytes.
    let order_item = OrderItem::from(&order_request);
    let action = action_place_order(vec![order_request]);

    // 4. Sign and assemble the SignatureWrapper expected by the wire format.
    let nonce = now_millis();
    let signature = sign_to_wrapper(&action, nonce, None, ChainType::Mainnet, &key)?;

    // 5. Submit.
    let req = PlaceOrderRequest {
        action: PlaceOrderAction {
            action_type: "placeOrder".into(),
            grouping:    "na".into(),
            orders:      vec![order_item],
        },
        nonce: nonce as i64,
        signature,
    };
    let resp = client.place_order(&req).await?;
    println!("tx_hash: {}", resp.tx_hash);
    Ok(())
}
```

### Other write operations

`cancel_order`, `cancel_all`, `split`, `merge`, and `redeem` follow the same three-step shape: build a typed action with the matching `action_*` constructor, call `sign_to_wrapper`, then post the wire-format request. The signing-side and wire-side structs implement `From` / `TryFrom` for the relevant pairs (e.g. `CancelItem::from(&CancelRequest)`), so use those rather than constructing both sides by hand.

### Heartbeat for long-running clients

Each `cancel_all` action signed with `expires_after = nonce + 300_000` doubles as a dead-man's switch. Re-send it via `client.heartbeat(&req).await?` more frequently than every 5 minutes; if the server stops seeing heartbeats it cancels every active order using the pre-signed calldata.

## Cancelling an order

Cancellation follows the same three-step shape as `place_order`: build a typed `CancelRequest`, derive the matching wire-format `CancelItem` from it, sign the action, and post the wire request. Identify the order by either its server `oid` (the value returned in `OrderRecord.id`) or its client order ID (the `client_order_id` you supplied at placement).

The snippet below assumes `client` and `key` are already constructed as in the [Placing an order](#placing-an-order) example.

```rust
use okx_outcomes_sdk::models::order::{CancelItem, CancelOrderAction, CancelOrderRequest};
use okx_outcomes_sdk::signing::{
    action_cancel, now_millis, sign_to_wrapper, ChainType, CancelRequest, CancelTarget,
};

// 1. Build the typed cancel request. Pick exactly one CancelTarget variant:
//    Oid   = server-assigned order ID, as a decimal string (OrderRecord.id).
//    ClientOrderId = client-assigned order ID, hex-encoded with the 0x prefix.
let cancel_request = CancelRequest {
    asset_id:    "100170100".into(),
    market_type: "prediction".into(),
    target:      CancelTarget::Oid("578840".into()),
    // target:   CancelTarget::ClientOrderId("0xabc...".into()),
};

// 2. Derive the wire item from the same request so the signed bytes and the
//    JSON body cannot drift.
let cancel_item = CancelItem::from(&cancel_request);
let action      = action_cancel(vec![cancel_request]);

// 3. Sign and submit.
let nonce     = now_millis();
let signature = sign_to_wrapper(&action, nonce, None, ChainType::Mainnet, &key)?;

let resp = client
    .cancel_order(&CancelOrderRequest {
        action: CancelOrderAction {
            action_type: "cancel".into(),
            cancels:     vec![cancel_item],
        },
        nonce:     nonce as i64,
        signature,
    })
    .await?;
println!("cancel tx_hash: {}", resp.tx_hash);
```

Notes:

- One `CancelOrderRequest` can carry multiple `CancelItem`s; build every `CancelRequest` you want to cancel and pass them all to a single `action_cancel(vec![...])` call so they share one signature and one on-chain transaction.
- Cancellation is asynchronous: `cancel_order` returns once the transaction is accepted, and the order moves through `PENDING_CANCEL` to `CANCELLED` as the chain confirms. Poll `get_order(id)` or subscribe to the private `pm-order` channel if you need to wait for the terminal state.

### Cancelling every active order

`cancel_all` cancels across markets in a single signed call. The signed bytes include both `asset_ids` and `market_type`, so the wire-format `CancelAllAction` must mirror the values passed to `action_cancel_all` exactly; building both sides from the same locals (as below) prevents drift.

```rust
use okx_outcomes_sdk::models::order::{CancelAllAction, CancelAllRequest};
use okx_outcomes_sdk::signing::{action_cancel_all, now_millis, sign_to_wrapper, ChainType};

let asset_ids: Vec<String> = vec![];          // empty = every market; or specific asset IDs
let market_type            = "prediction";

let action    = action_cancel_all(asset_ids.clone(), market_type);
let nonce     = now_millis();
let signature = sign_to_wrapper(&action, nonce, None, ChainType::Mainnet, &key)?;

let resp = client
    .cancel_all(&CancelAllRequest {
        action: CancelAllAction {
            action_type: "cancelAll".into(),
            asset_ids,
            market_type: market_type.into(),
        },
        nonce:     nonce as i64,
        signature,
    })
    .await?;
println!("cancel-all tx_hash: {}", resp.tx_hash);
```

The same signed `CancelAllRequest` doubles as the dead-man's-switch heartbeat described in [Heartbeat for long-running clients](#heartbeat-for-long-running-clients); to use it that way, pass `Some(nonce + 300_000)` as `expires_after` to `sign_to_wrapper` and re-send the request via `client.heartbeat(&req)` more frequently than every 5 minutes.

## WebSocket subscriptions

Both public and private channels share a single endpoint: `wss://ws.okx.com:8443/ws/v5/business`. Public channels work anonymously; private channels require `login` after `connect` and before `subscribe`.

The reader task parses each incoming JSON payload once into a typed `WsMessage` enum, so callbacks never see raw JSON. The client auto-reconnects with exponential backoff (3 s -> 30 s), replays subscriptions, and re-runs login on every successful reconnect.

### Public channels (no login)

```rust
use okx_outcomes_sdk::ws::OutcomesWsClient;
use okx_outcomes_sdk::ws::models::WsMessage;
use std::collections::HashMap;
use std::sync::Arc;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let ws = OutcomesWsClient::new();

    ws.set_on_data(Arc::new(|msg: &WsMessage| match msg {
        WsMessage::Prices(ticks)          => println!("{} price ticks", ticks.len()),
        WsMessage::Books { data, action } => println!("books ({action}): {} levels", data.len()),
        WsMessage::Trades(trades)         => println!("{} trades", trades.len()),
        WsMessage::Event { event, .. }    => println!("server event: {event}"),
        _ => {}
    }));
    ws.set_on_connection_state(Arc::new(|channel_type, connected| {
        println!("[{channel_type}] connected={connected}");
    }));

    ws.connect("/ws/v5/business").await?;

    // Subscribe to one market's price ticks. Use multiple HashMap entries in
    // `params` to subscribe to multiple instruments in a single call.
    let mut filter = HashMap::new();
    filter.insert("instId".into(), "100170100".into());
    ws.subscribe("prediction-market-prices", vec![filter]).await?;

    // Keep the runtime alive while the reader task delivers callbacks.
    tokio::signal::ctrl_c().await?;
    ws.disconnect().await;
    Ok(())
}
```

Public channels and the `WsMessage` variant each one produces:

| Channel | Variant |
| --- | --- |
| `prediction-market-prices` | `WsMessage::Prices` |
| `pm-books` | `WsMessage::Books` |
| `pm-trades` | `WsMessage::Trades` |
| `pm-tickers` | `WsMessage::Tickers` |
| `pm-event-status` | `WsMessage::EventStatus` |
| `game-status` | `WsMessage::Game` |
| `pm-candle*` (any timeframe) | `WsMessage::Candle` |

### Private channels (login required)

Private channels stream the authenticated account's own order, position, balance, fill, and PnL updates. Sequence: `connect`, then `login`, then `subscribe`.

```rust
let ws = OutcomesWsClient::new();
// Private channels push a single object per message (not an array), so each
// variant carries one item.
ws.set_on_data(Arc::new(|msg: &WsMessage| match msg {
    WsMessage::Orders(order)      => println!("order {} -> {:?}", order.order_id, order.status),
    WsMessage::Positions(position) => println!("position update on market {}", position.market_id),
    WsMessage::Balance(b)         => println!("balance update: {:?}", b.change_type),
    WsMessage::UserTrades(fill)   => println!("fill on order {}", fill.order_id),
    WsMessage::Pnl(pnl)           => println!("pnl update: {pnl:?}"),
    _ => {}
}));

ws.connect("/ws/v5/business").await?;
ws.login(&creds).await?;              // ApiCredentials from earlier
ws.subscribe("pm-order",      vec![]).await?;
ws.subscribe("pm-position",   vec![]).await?;
ws.subscribe("pm-balance",    vec![]).await?;
```

Notes:

- `login` blocks until the server confirms (success) or rejects (returns an `SdkError::WebSocket` carrying the OKX error code). It times out after 30 s, matching OKX's documented login expiry.
- The SDK computes the login signature internally (`Base64(HMAC-SHA256(secret_key, timestamp + "GET" + "/users/self/verify"))`); you only supply the same `ApiCredentials` used for REST.
- Credentials are cached on the client and replayed automatically on reconnect, so a transient disconnect does not require user code to re-authenticate.
- Unlike public channels (whose `data` is an array), private channels push a **single object** per message, so each private `WsMessage` variant carries one item (e.g. `WsMessage::Orders(WsOrder)`).

Private channels and their `WsMessage` variants:

| Channel | Variant |
| --- | --- |
| `pm-order` | `WsMessage::Orders` |
| `pm-position` | `WsMessage::Positions` |
| `pm-user-trade` | `WsMessage::UserTrades` |
| `pm-balance` | `WsMessage::Balance` |
| `pm-pnl` | `WsMessage::Pnl` |

## Error handling

Every fallible call returns `Result<T, SdkError>`:

```rust
use okx_outcomes_sdk::SdkError;

match client.list_orders(None, None, None, None).await {
    Ok(page) => { /* ... */ }
    Err(SdkError::NotAuthenticated { hint })       => eprintln!("auth: {hint}"),
    Err(SdkError::Api { code, message })           => eprintln!("api error {code}: {message}"),
    Err(SdkError::UnexpectedStatus { status, body }) => eprintln!("http {status}: {body}"),
    Err(SdkError::Http(e))                         => eprintln!("network: {e}"),
    Err(SdkError::WebSocket { message })           => eprintln!("ws: {message}"),
    Err(e)                                         => eprintln!("other: {e}"),
}
```

`SdkError` is `#[non_exhaustive]`, so a `match` on it must include a wildcard (`_` / `Err(e)`) arm.

`SdkError::Api { code, message }` carries the server's business error code, so you can match on specific codes (rate limit, insufficient balance, signature mismatch, etc.) without parsing strings. OKX sends `code` as either a JSON string (`"50105"`) or a number (`100015`); the SDK accepts both and normalizes to `i64`. When a non-2xx response body isn't the standard `{ code, msg }` shape (e.g. an HTML error page from a gateway), you get `SdkError::UnexpectedStatus { status, body }` instead, which preserves the raw HTTP status.

## Configuration

The SDK reads **no environment variables** — all configuration is passed explicitly. Construct the REST client with the builder:

```rust
use okx_outcomes_sdk::{OutcomesSdkClient, TradingMode};

let client = OutcomesSdkClient::builder()
    .credentials(creds)
    .base_url("https://www.okx.com")     // default; must be https (loopback http ok)
    .mode(TradingMode::Points)           // X-Predictions-Mode header (Points)
    .accept_language("en-US")            // Accept-Language (BCP-47)
    .timeout_secs(20)                    // per-request HTTP timeout (default 10)
    .debug(true)                         // request/response logging — debug builds only
    .build();
```

| Setting | Builder method | Default |
| --- | --- | --- |
| REST base URL | `.base_url(..)` | `https://www.okx.com` (must be https; loopback http allowed) |
| Per-request timeout | `.timeout_secs(..)` | `10` (REST only; WS uses a fixed 25 s ping / 3 s→30 s reconnect backoff) |
| Debug logging | `.debug(true)` | off — **honored in debug builds only**, so credentials are never logged in release |
| Trading mode | `.mode(TradingMode::..)` | unset (no `X-Predictions-Mode` header) |
| Accept-Language | `.accept_language(..)` | unset |

`with_credentials` and `with_credentials_and_url` remain as shortcuts over the builder.

**WebSocket** is configured the same way:

```rust
use okx_outcomes_sdk::ws::OutcomesWsClient;

let ws = OutcomesWsClient::builder()
    .host(okx_outcomes_sdk::ws::endpoints::EU_WS_HOST) // EU_WS_HOST / US_WS_HOST also exported
    .debug(true)
    .build();
```

**Signing** takes the chain explicitly: pass a `ChainType` (`Mainnet` / `Testnet`) to `sign_to_wrapper` / `sign_action*`. For client order IDs, set region/env once at startup with `register_client_order_id_context(region, env)` or pass them to `generate_client_order_id(region, env)` (defaults to HK / PROD).

## License

The Outcomes SDK is open-source software licensed under the [MIT license](LICENSE).

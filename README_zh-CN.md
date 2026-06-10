## okx-outcomes-sdk

OKX Outcomes Market Developer API 的类型化 Rust 客户端，覆盖完整的 REST 接口（事件、市场、余额、订单、持仓、成交）、EIP-712 交易签名，以及带自动重连的实时 WebSocket 传输。

## 安装

将本 SDK 与 Tokio 运行时一并加入 `Cargo.toml`。

从公开 Git 仓库安装：

```toml
[dependencies]
okx-outcomes-sdk = { git = "https://github.com/okx/outcomes-sdk.git", features = ["signing", "websocket"] }
tokio = { version = "1", features = ["full"] }
```

如需锁定到某个发布版本，可在 Git 依赖项中加上 `tag = "v0.1.0"`（或 `branch` / `rev`）。

这就是完整的依赖列表。`tokio-tungstenite`、`k256`、`alloy-*`、`rmp-serde`、`futures-util` 等都会通过 SDK 的 feature 间接引入。**不要**自行添加这些依赖，否则可能与 SDK 构建时使用的版本不一致。

### Feature 开关

| Feature | 启用内容 | 何时启用 |
| --- | --- | --- |
| _default_ | 仅 REST 客户端（事件、市场、订单、持仓、余额、成交、价格）。 | 只读集成。 |
| `signing` | EIP-712 + ECDSA 动作签名辅助函数。 | 任何写操作：`place_order`、`cancel_order`、`cancel_all`、`split`、`merge`、`redeem`、`heartbeat`。 |
| `websocket` | 基于 tokio-tungstenite 的 `OutcomesWsClient`，以及类型化的 `WsMessage` 解析器。 | 实时价格、成交、订单簿，以及用户订单 / 持仓 / 余额流。 |

## 鉴权

所有 REST 接口都需要 OKX API 凭证。使用 `with_credentials` 构造客户端：

```rust
use okx_outcomes_sdk::{ApiCredentials, OutcomesSdkClient};

// 按你喜欢的方式提供凭证即可 —— SDK 不会替你加载。
let creds = ApiCredentials {
    api_key:    "your-api-key".into(),
    secret_key: "your-secret-key".into(),
    passphrase: "your-passphrase".into(),
};
let client = OutcomesSdkClient::with_credentials(creds);
```

说明：

- `secret_key` 不会被发送到网络上。SDK 会按 OKX 的 REST 鉴权规范，在本地用 HMAC-SHA256 对每个请求签名（`OK-ACCESS-SIGN = Base64(HMAC-SHA256(secret_key, timestamp + METHOD + path + body))`）。
- 下单 / 撤单 / split / merge / redeem 还需要一把用于 EIP-712 签名的**私钥**。这把私钥与你的 REST API key 互相独立。

## 数据查询

### 列出事件

`get_events` 返回分页的事件列表。所有筛选参数都是可选的：

```rust
let page = client
    .get_events(
        Some("active"),      // status: "active" | "resolved"
        None,                // category: "SPORTS" | "CURRENT_AFFAIRS"
        None,                // tag: 体育标签 ID
        None,                // league_id
        Some("volume_24h"),  // sort: "volume" | "volume_24h" | "ending_soon" | "newest"
        None,                // cursor （来自上一页）
        Some(20),            // page_size （最大 50）
    )
    .await?;

for event in &page.events {
    println!("{} - {} markets ({} pts volume)",
        event.event_title, event.total_markets_count, event.volume);
}

// 分页：把 `page.pagination.next_cursor` 作为下一次调用的 `cursor` 传回去。
```

其他读取方法：

| 方法 | 用途 |
| --- | --- |
| `client.search(keyword, cursor, page_size)` | 按关键字搜索事件与市场。 |
| `client.get_event(event_id)` | 单个事件及其完整市场列表。 |
| `client.get_event_markets(event_id)` | 单个事件下的全部市场（不分页）。 |
| `client.get_market(market_id)` | 通过市场 ID 获取单个市场。 |
| `client.get_ticker(inst_id)` | 单个市场标的的最新行情。 |
| `client.get_candles(...)` | K 线历史。 |
| `client.get_trades(...)` | 最近的公开成交历史。 |

### 账户余额、订单与持仓

```rust
let entries = client.get_balance().await?; // Vec<BalanceEntry>
for entry in &entries {
    println!("[{}] available={} total={}", entry.odds_type, entry.available, entry.balance);
}

let orders = client.list_orders(
    None,         // market_id 过滤
    Some("open"), // "open" （pending+active）或 "closed"
    None,         // cursor
    Some(50),     // limit （最大 50，默认 20）
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

`list_orders` 和 `get_positions` 都通过 `cursor` + `limit` 分页：上一次响应中的 `next_cursor` 字段会作为下一次调用的 `cursor` 参数；当游标耗尽时 `has_next` 为 `false`。

### 查询未结订单

`list_orders` 在 `status = Some("open")` 时按页返回账户上所有非终态订单。把上一次响应中的 `next_cursor` 通过 `cursor` 参数传回，直到 `has_next` 为 `false`：

```rust
use okx_outcomes_sdk::models::order::OrderRecord;

let mut cursor: Option<String> = None;
let mut all_open: Vec<OrderRecord> = Vec::new();

loop {
    let page = client
        .list_orders(
            None,             // market_id: None 表示所有市场，Some("123") 表示限定某个市场
            Some("open"),     // "open" = pending + active；"closed" = filled / cancelled / expired / failed
            cursor.as_deref(),
            Some(50),         // 每页最大条数
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

每个 `OrderRecord` 暴露下单参数（`side: OrderSide`、`size`、`price`、`order_type: TimeInForce`、`size_type: OrderSizeType`、`expiration`）、下单元数据（`market_id`、`asset_id`、`client_order_id`、`tx_hash`），以及当前生命周期状态（`status: RestOrderStatus`、`filled_size`、`filled_amount`）。如果只想看挂单状态的订单，过滤 `o.status == RestOrderStatus::Active` 即可，这会排除撮合引擎仍在处理以及处于撤单中的订单。

提示：

- `market_id` 是字符串形式的十进制 ID；请用 `Some("123456789")` 传入。
- `client_order_id`（下单时传入的客户端订单 ID）可以传给 `cancel_order`，以使用客户端 ID 撤单而不是服务端 ID。

## 下单

下单分为三步：构造类型化的 `OrderRequest`，使用 EIP-712 对它签名，然后提交 wire 格式的 `PlaceOrderRequest`。SDK 为每一步提供配套类型，保证签名字节与 JSON 请求体不会发散。

```rust
use okx_outcomes_sdk::{ApiCredentials, OutcomesSdkClient};
use okx_outcomes_sdk::models::order::{OrderItem, PlaceOrderAction, PlaceOrderRequest};
use okx_outcomes_sdk::signing::{
    action_place_order, generate_client_order_id_default, now_millis, parse_private_key, sign_to_wrapper,
    ChainType, LimitOrderType, LimitTif, OrderRequest, OrderType, SigningOrderSide, SizeType,
};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // 按你喜欢的方式提供凭证即可 —— SDK 不会替你加载。
    let creds = ApiCredentials {
        api_key:    "your-api-key".into(),
        secret_key: "your-secret-key".into(),
        passphrase: "your-passphrase".into(),
    };
    let client = OutcomesSdkClient::with_credentials(creds);

    // 1. 加载链上签名私钥（hex 格式，可带可不带 0x 前缀）。
    let key = parse_private_key("your-signing-key-hex")?;

    // 2. 构造类型化的订单。`asset_id` 是 outcome asset ID。
    let order_request = OrderRequest {
        asset_id:         "100170100".into(),
        side:             SigningOrderSide::Buy,
        market_type:      "prediction".into(),
        client_order_id:  generate_client_order_id_default()?,
        price:            "0.65".into(),
        reduce_only:      false,
        size:             "100".into(),
        size_type:        SizeType::Base, // Quote 表示按报价货币下单
        order_type:       OrderType::Limit(LimitOrderType { tif: LimitTif::Gtc }),
    };

    // 3. 从同一份 OrderRequest 派生 wire 格式的 OrderItem，确保
    //    JSON 请求体不会与签名的 msgpack 字节发散。
    let order_item = OrderItem::from(&order_request);
    let action = action_place_order(vec![order_request]);

    // 4. 签名并组装 wire 格式要求的 SignatureWrapper。
    let nonce = now_millis();
    let signature = sign_to_wrapper(&action, nonce, None, ChainType::Mainnet, &key)?;

    // 5. 提交。
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

### 其他写操作

`cancel_order`、`cancel_all`、`split`、`merge`、`redeem` 都遵循同样的三步模式：用对应的 `action_*` 构造函数构造类型化动作，调用 `sign_to_wrapper`，然后提交 wire 格式请求。签名侧与 wire 侧的结构体之间实现了相应的 `From` / `TryFrom`（例如 `CancelItem::from(&CancelRequest)`），优先使用这些转换，而不是手工构造两侧。

### 长连接客户端的心跳

每个 `cancel_all` 动作在签名时若设置 `expires_after = nonce + 300_000`，就同时充当 dead-man's switch（失联保护）。通过 `client.heartbeat(&req).await?` 以小于 5 分钟的间隔重复发送；一旦服务端不再收到心跳，就会使用预签名的 calldata 自动撤销所有活跃订单。

## 撤单

撤单流程与 `place_order` 一致，同样分三步：构造类型化的 `CancelRequest`，从中派生对应的 wire 格式 `CancelItem`，对动作签名，最后提交 wire 请求。可以通过订单的服务端 `oid`（`OrderRecord.id` 的返回值）或客户端订单 ID（下单时传入的 `client_order_id`）来定位目标订单。

下面的片段假设 `client` 和 `key` 已按[下单](#下单)一节构造完成。

```rust
use okx_outcomes_sdk::models::order::{CancelItem, CancelOrderAction, CancelOrderRequest};
use okx_outcomes_sdk::signing::{
    action_cancel, now_millis, sign_to_wrapper, ChainType, CancelRequest, CancelTarget,
};

// 1. 构造类型化的撤单请求。CancelTarget 必须选其一：
//    Oid   = 服务端分配的订单 ID，十进制字符串（OrderRecord.id）。
//    ClientOrderId = 客户端分配的订单 ID，带 0x 前缀的 hex 字符串。
let cancel_request = CancelRequest {
    asset_id:    "100170100".into(),
    market_type: "prediction".into(),
    target:      CancelTarget::Oid("578840".into()),
    // target:   CancelTarget::ClientOrderId("0xabc...".into()),
};

// 2. 从同一份请求派生 wire 项，确保签名字节与 JSON 请求体不会发散。
let cancel_item = CancelItem::from(&cancel_request);
let action      = action_cancel(vec![cancel_request]);

// 3. 签名并提交。
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

说明：

- 一个 `CancelOrderRequest` 可以携带多个 `CancelItem`：把所有需要撤销的 `CancelRequest` 都放进同一次 `action_cancel(vec![...])` 调用，它们就会共享一份签名和一笔链上交易。
- 撤单是异步的：`cancel_order` 在交易被接收后即返回，订单会随链上确认从 `PENDING_CANCEL` 进入 `CANCELLED`。如果需要等待终态，可轮询 `get_order(id)` 或订阅私有的 `pm-order` 频道。

### 撤销所有活跃订单

`cancel_all` 在一次签名调用里跨市场撤单。签名字节中包含 `asset_ids` 与 `market_type`，因此 wire 格式的 `CancelAllAction` 必须与传给 `action_cancel_all` 的值完全一致；像下面这样让两侧使用同一组局部变量，可以避免发散。

```rust
use okx_outcomes_sdk::models::order::{CancelAllAction, CancelAllRequest};
use okx_outcomes_sdk::signing::{action_cancel_all, now_millis, sign_to_wrapper, ChainType};

let asset_ids: Vec<String> = vec![];          // 空数组 = 所有市场；也可指定具体 asset ID
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

同一份签名好的 `CancelAllRequest` 也可以充当[长连接客户端的心跳](#长连接客户端的心跳)一节所述的 dead-man's switch：把 `Some(nonce + 300_000)` 作为 `expires_after` 传给 `sign_to_wrapper`，并通过 `client.heartbeat(&req)` 以小于 5 分钟的间隔重复发送即可。

## WebSocket 订阅

公共频道与私有频道共用同一端点：`wss://ws.okx.com:8443/ws/v5/business`。公共频道无需登录；私有频道需要在 `connect` 之后、`subscribe` 之前先调用 `login`。

读取任务会把每条传入的 JSON 一次解析为类型化的 `WsMessage` 枚举，因此回调中不会出现原始 JSON。客户端在 3 s -> 30 s 的指数退避下自动重连，并在每次成功重连后重放订阅、重新执行登录。

### 公共频道（无需登录）

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

    // 订阅单一市场的价格行情。在 `params` 中加入多个 HashMap 条目，
    // 即可在一次调用中同时订阅多个标的。
    let mut filter = HashMap::new();
    filter.insert("instId".into(), "100170100".into());
    ws.subscribe("prediction-market-prices", vec![filter]).await?;

    // 保持运行时存活，让读取任务可以持续派发回调。
    tokio::signal::ctrl_c().await?;
    ws.disconnect().await;
    Ok(())
}
```

各公共频道对应的 `WsMessage` 变体：

| 频道 | 变体 |
| --- | --- |
| `prediction-market-prices` | `WsMessage::Prices` |
| `pm-books` | `WsMessage::Books` |
| `pm-trades` | `WsMessage::Trades` |
| `pm-tickers` | `WsMessage::Tickers` |
| `pm-event-status` | `WsMessage::EventStatus` |
| `pm-candle*` （任意周期） | `WsMessage::Candle` |

### 私有频道（需要登录）

私有频道推送已登录账户自身的订单、持仓、余额、成交以及 PnL 更新。顺序：先 `connect`，再 `login`，最后 `subscribe`。

```rust
let ws = OutcomesWsClient::new();
// 私有频道每条消息推送的是单个对象（不是数组），因此每个变体只携带一个条目。
ws.set_on_data(Arc::new(|msg: &WsMessage| match msg {
    WsMessage::Orders(order)       => println!("order {} -> {:?}", order.order_id, order.status),
    WsMessage::Positions(position) => println!("position update on market {}", position.market_id),
    WsMessage::Balance(b)          => println!("balance update: {:?}", b.change_type),
    WsMessage::UserTrades(fill)    => println!("fill on order {}", fill.order_id),
    WsMessage::Pnl(pnl)            => println!("pnl update: {pnl:?}"),
    _ => {}
}));

ws.connect("/ws/v5/business").await?;
ws.login(&creds).await?;              // 上文构造的 ApiCredentials
ws.subscribe("pm-order",      vec![]).await?;
ws.subscribe("pm-position",   vec![]).await?;
ws.subscribe("pm-balance",    vec![]).await?;
```

说明：

- `login` 会阻塞，直到服务端确认（成功）或拒绝（返回携带 OKX 错误码的 `SdkError::WebSocket`）。30 s 后超时，与 OKX 文档中的登录有效期一致。
- 登录签名由 SDK 内部计算（`Base64(HMAC-SHA256(secret_key, timestamp + "GET" + "/users/self/verify"))`）；你只需提供与 REST 相同的 `ApiCredentials`。
- 凭证会缓存在客户端，并在重连时自动重放，因此偶发的断连不需要用户代码再次鉴权。
- 与公共频道（`data` 为数组）不同，私有频道每条消息推送的是**单个对象**，因此每个私有 `WsMessage` 变体只携带一个条目（例如 `WsMessage::Orders(WsOrder)`）。

各私有频道对应的 `WsMessage` 变体：

| 频道 | 变体 |
| --- | --- |
| `pm-order` | `WsMessage::Orders` |
| `pm-position` | `WsMessage::Positions` |
| `pm-user-trade` | `WsMessage::UserTrades` |
| `pm-balance` | `WsMessage::Balance` |
| `pm-pnl` | `WsMessage::Pnl` |

## 错误处理

每个可失败调用都返回 `Result<T, SdkError>`：

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

`SdkError` 标注了 `#[non_exhaustive]`，因此对它进行 `match` 时必须包含通配（`_` / `Err(e)`）分支。

`SdkError::Api { code, message }` 携带服务端返回的业务错误码，因此可以对具体错误码（限流、余额不足、签名不匹配等）进行匹配，而不需要解析字符串。OKX 在传输层会将 `code` 发送为 JSON 字符串（`"50105"`）或数字（`100015`），SDK 两者都接受并统一归一化为 `i64`。当非 2xx 响应体不符合标准的 `{ code, msg }` 结构时（例如网关返回的 HTML 错误页），则会得到 `SdkError::UnexpectedStatus { status, body }`，其中保留了原始 HTTP 状态码。

## 配置

SDK **不读取任何环境变量** —— 所有配置都显式传入。使用 builder 构造 REST 客户端：

```rust
use okx_outcomes_sdk::{OutcomesSdkClient, TradingMode};

let client = OutcomesSdkClient::builder()
    .credentials(creds)
    .base_url("https://www.okx.com")     // 默认；必须为 https（loopback http 可）
    .mode(TradingMode::Points)           // X-Predictions-Mode 请求头（Points）
    .accept_language("en-US")            // Accept-Language（BCP-47）
    .timeout_secs(20)                    // 每次请求的 HTTP 超时（默认 10）
    .debug(true)                         // 请求/响应日志 —— 仅 debug 构建生效
    .build();
```

| 配置项 | builder 方法 | 默认值 |
| --- | --- | --- |
| REST base URL | `.base_url(..)` | `https://www.okx.com`（必须 https；允许 loopback http） |
| 每次请求超时 | `.timeout_secs(..)` | `10`（仅 REST；WS 固定 25 s 心跳 / 3 s→30 s 重连退避） |
| 调试日志 | `.debug(true)` | 关闭 —— **仅 debug 构建生效**，因此 release 绝不记录凭据 |
| 交易模式 | `.mode(TradingMode::..)` | 未设置（不发送 `X-Predictions-Mode`） |
| Accept-Language | `.accept_language(..)` | 未设置 |

`with_credentials`、`with_credentials_and_url` 仍作为 builder 的快捷方式保留。

**WebSocket** 同样通过 builder 配置：

```rust
use okx_outcomes_sdk::ws::OutcomesWsClient;

let ws = OutcomesWsClient::builder()
    .host(okx_outcomes_sdk::ws::endpoints::EU_WS_HOST) // 也导出 EU_WS_HOST / US_WS_HOST
    .debug(true)
    .build();
```

**签名**需显式传入链：把 `ChainType`（`Mainnet` / `Testnet`）传给 `sign_to_wrapper` / `sign_action*`。客户端订单 ID 的 region/env 可在启动时用 `register_client_order_id_context(region, env)` 注册一次，或传给 `generate_client_order_id(region, env)`（默认 HK / PROD）。

## 许可证

[MIT license](LICENSE).

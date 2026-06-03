//! OutcomesWsClient — WsTransport implementation using tokio-tungstenite.
//!
//! Features:
//! - Direct WS connection — no host-app networking dependency
//! - 25s ping heartbeat (OKX requires < 30s keepalive)
//! - Auto-reconnect with exponential backoff (3s → 6s → 12s → max 30s)
//! - Subscription replay after reconnect

use std::collections::HashMap;
use std::sync::Arc;

use futures_util::{SinkExt, StreamExt};
use tokio::sync::Mutex;
use tokio_tungstenite::tungstenite::Message;

use base64::Engine as _;
use hmac::{Hmac, Mac};
use sha2::Sha256;

use super::endpoints::DEFAULT_WS_HOST;
use super::transport::{WsConnectionStateCallback, WsDataCallback};
use crate::client::ApiCredentials;
use crate::error::SdkError;

const PING_INTERVAL_SECS: u64 = 25;
const RECONNECT_BASE_MS: u64 = 3000;
const RECONNECT_MAX_MS: u64 = 30000;

fn ws_debug_enabled() -> bool {
    // Debug logging is only available in debug builds. In release builds this
    // is always false, so credential-bearing frames (the WS `login` op carries
    // apiKey/passphrase/sign) can never be printed regardless of the env var.
    cfg!(debug_assertions) && std::env::var("OUTCOMES_DEBUG").is_ok_and(|v| v == "1")
}

/// Print a debug line with explicit `\r\n` so it renders correctly
/// even when the terminal is in raw mode (crossterm::enable_raw_mode).
macro_rules! ws_debug {
    ($($arg:tt)*) => {
        eprint!("\r[WS DEBUG] ");
        eprint!($($arg)*);
        eprint!("\r\n");
    };
}

type WsSender = Arc<
    Mutex<
        Option<
            futures_util::stream::SplitSink<
                tokio_tungstenite::WebSocketStream<
                    tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>,
                >,
                Message,
            >,
        >,
    >,
>;

#[derive(Clone)]
struct Subscription {
    channel: String,
    params: Vec<HashMap<String, String>>,
}

/// Shared reconnection state.
struct SharedState {
    host: String,
    path: Mutex<String>,
    sender: WsSender,
    on_data: Arc<std::sync::Mutex<Option<WsDataCallback>>>,
    on_state: Arc<std::sync::Mutex<Option<WsConnectionStateCallback>>>,
    subscriptions: Arc<Mutex<Vec<Subscription>>>,
    /// Credentials for private channel login; replayed on reconnect.
    credentials: Mutex<Option<ApiCredentials>>,
    /// Oneshot sender to signal login result to the `login()` caller.
    /// Set before sending login, consumed by the reader when it sees a login response.
    #[allow(clippy::type_complexity)]
    login_tx: Arc<std::sync::Mutex<Option<tokio::sync::oneshot::Sender<Result<(), String>>>>>,
    auto_reconnect: Arc<std::sync::atomic::AtomicBool>,
    reader_handle: Mutex<Option<tokio::task::JoinHandle<()>>>,
    ping_handle: Mutex<Option<tokio::task::JoinHandle<()>>>,
}

/// WebSocket client using `tokio-tungstenite`.
///
/// Configurable host via [`OutcomesWsClient::with_host`] or the `OUTCOMES_WS_HOST` env var.
pub struct OutcomesWsClient {
    shared: Arc<SharedState>,
}

impl Default for OutcomesWsClient {
    fn default() -> Self {
        let host =
            std::env::var("OUTCOMES_WS_HOST").unwrap_or_else(|_| DEFAULT_WS_HOST.to_string());
        Self::with_host(&host)
    }
}

impl OutcomesWsClient {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_host(host: &str) -> Self {
        Self {
            shared: Arc::new(SharedState {
                host: host.to_string(),
                path: Mutex::new(String::new()),
                sender: Arc::new(Mutex::new(None)),
                on_data: Arc::new(std::sync::Mutex::new(None)),
                on_state: Arc::new(std::sync::Mutex::new(None)),
                subscriptions: Arc::new(Mutex::new(Vec::new())),
                credentials: Mutex::new(None),
                login_tx: Arc::new(std::sync::Mutex::new(None)),
                reader_handle: Mutex::new(None),
                ping_handle: Mutex::new(None),
                auto_reconnect: Arc::new(std::sync::atomic::AtomicBool::new(true)),
            }),
        }
    }
}

type ConnectResult = Result<(tokio::task::JoinHandle<()>, tokio::task::JoinHandle<()>), SdkError>;
type BoxFutureSend<T> = std::pin::Pin<Box<dyn std::future::Future<Output = T> + Send>>;

/// Store write half into sender.
async fn store_sender(
    sender: &WsSender,
    write: futures_util::stream::SplitSink<
        tokio_tungstenite::WebSocketStream<
            tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>,
        >,
        Message,
    >,
) {
    *sender.lock().await = Some(write);
}

/// Connect, spawn reader + ping. Returns handles.
fn do_connect(ss: Arc<SharedState>) -> BoxFutureSend<ConnectResult> {
    Box::pin(async move {
        let debug = ws_debug_enabled();
        let path = { ss.path.lock().await.clone() };
        let url = format!("{}{}", ss.host, path);
        if debug {
            ws_debug!("Connecting to {url}");
        }
        let parsed = url::Url::parse(&url).map_err(|e| SdkError::Internal {
            message: format!("Invalid WS URL: {e}"),
        })?;

        let (ws_stream, _) = tokio_tungstenite::connect_async(parsed)
            .await
            .map_err(|e| SdkError::WebSocket {
                message: format!("WS connect failed: {e}"),
            })?;
        if debug {
            ws_debug!("Connected OK");
        }

        let (write, read) = ws_stream.split();
        store_sender(&ss.sender, write).await;

        // Notify connected.
        {
            if let Some(ref cb) = *ss.on_state.lock().unwrap_or_else(|e| e.into_inner()) {
                cb("public", true);
            }
        }

        // Reader task.
        let data_cb = ss.on_data.lock().unwrap_or_else(|e| e.into_inner()).clone();
        let state_cb = ss
            .on_state
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .clone();
        let ss_for_reader = Arc::clone(&ss);

        let reader_debug = debug;
        let login_tx = Arc::clone(&ss.login_tx);
        let reader = tokio::spawn(async move {
            let mut read = read;
            while let Some(msg_result) = read.next().await {
                match msg_result {
                    Ok(Message::Text(text)) => {
                        if text == "pong" {
                            continue;
                        }
                        if reader_debug {
                            let preview = if text.len() > 500 {
                                format!("{}...({}B)", &text[..500], text.len())
                            } else {
                                text.clone()
                            };
                            ws_debug!("<< {preview}");
                        }
                        // Intercept login responses before normal dispatch.
                        // Success: {"event":"login","code":"0",...}
                        // Failure: {"event":"error","code":"60009","msg":"Login failed.",...}
                        if text.contains("\"event\"") {
                            if let Ok(val) = serde_json::from_str::<serde_json::Value>(&text) {
                                let event = val.get("event").and_then(|e| e.as_str()).unwrap_or("");
                                if event == "login" {
                                    // Login success.
                                    if let Some(tx) =
                                        login_tx.lock().unwrap_or_else(|e| e.into_inner()).take()
                                    {
                                        let _ = tx.send(Ok(()));
                                    }
                                    continue;
                                }
                                // Check for login-related errors (60009, 60011, etc.)
                                let code = val.get("code").and_then(|c| c.as_str()).unwrap_or("");
                                if code.starts_with("600") {
                                    let msg = val
                                        .get("msg")
                                        .and_then(|m| m.as_str())
                                        .unwrap_or("Login failed");
                                    if let Some(tx) =
                                        login_tx.lock().unwrap_or_else(|e| e.into_inner()).take()
                                    {
                                        let _ = tx.send(Err(format!("[{code}] {msg}")));
                                    }
                                    // Still pass to callback so the consumer sees the error.
                                }
                            }
                        }
                        // Normal dispatch: parse into typed WsMessage.
                        let channel = extract_channel(&text);
                        if let Some(msg) = super::models::parse_ws_message(&channel, &text) {
                            if let Some(ref cb) = data_cb {
                                cb(&msg);
                            }
                        }
                    }
                    Ok(Message::Ping(_)) => {}
                    Ok(Message::Close(_)) | Err(_) => {
                        if let Some(ref cb) = state_cb {
                            cb("public", false);
                        }
                        if ss_for_reader
                            .auto_reconnect
                            .load(std::sync::atomic::Ordering::Relaxed)
                        {
                            let ss2 = Arc::clone(&ss_for_reader);
                            tokio::spawn(reconnect_loop(ss2));
                        }
                        break;
                    }
                    _ => {}
                }
            }
        });

        // Ping task.
        let ping_sender = Arc::clone(&ss.sender);
        let ping = tokio::spawn(async move {
            let mut interval =
                tokio::time::interval(std::time::Duration::from_secs(PING_INTERVAL_SECS));
            interval.tick().await;
            loop {
                interval.tick().await;
                let mut guard = ping_sender.lock().await;
                if let Some(ref mut s) = *guard {
                    if s.send(Message::Text("ping".to_string())).await.is_err() {
                        break;
                    }
                } else {
                    break;
                }
            }
        });

        Ok((reader, ping))
    })
}

/// Reconnect loop with exponential backoff + subscription replay.
async fn reconnect_loop(ss: Arc<SharedState>) {
    let debug = ws_debug_enabled();
    let mut delay_ms = RECONNECT_BASE_MS;

    loop {
        if !ss.auto_reconnect.load(std::sync::atomic::Ordering::Relaxed) {
            return;
        }

        if debug {
            ws_debug!("Reconnecting in {delay_ms}ms...");
        }
        tokio::time::sleep(std::time::Duration::from_millis(delay_ms)).await;

        if !ss.auto_reconnect.load(std::sync::atomic::Ordering::Relaxed) {
            return;
        }

        match do_connect(Arc::clone(&ss)).await {
            Ok((reader, ping)) => {
                *ss.reader_handle.lock().await = Some(reader);
                *ss.ping_handle.lock().await = Some(ping);
                // Replay login (if authenticated) and wait for response.
                if let Some(ref creds) = *ss.credentials.lock().await {
                    let (tx, rx) = tokio::sync::oneshot::channel();
                    *ss.login_tx.lock().unwrap_or_else(|e| e.into_inner()) = Some(tx);

                    let timestamp = chrono::Utc::now().timestamp().to_string();
                    let sign = ws_sign(&creds.secret_key, &timestamp);
                    let login_msg = serde_json::json!({
                        "op": "login",
                        "args": [{
                            "apiKey": creds.api_key,
                            "passphrase": creds.passphrase,
                            "timestamp": timestamp,
                            "sign": sign,
                        }]
                    });
                    if let Ok(text) = serde_json::to_string(&login_msg) {
                        let mut guard = ss.sender.lock().await;
                        if let Some(ref mut s) = *guard {
                            let _ = s.send(Message::Text(text)).await;
                        }
                    }
                    // Wait for login response (timeout 30s per OKX docs).
                    let _ = tokio::time::timeout(std::time::Duration::from_secs(30), rx).await;
                }
                // Replay subscriptions.
                let subs = ss.subscriptions.lock().await.clone();
                for sub in &subs {
                    let args = build_args(&sub.channel, sub.params.clone());
                    let msg = serde_json::json!({ "op": "subscribe", "args": args });
                    if let Ok(text) = serde_json::to_string(&msg) {
                        let mut guard = ss.sender.lock().await;
                        if let Some(ref mut s) = *guard {
                            let _ = s.send(Message::Text(text)).await;
                        }
                    }
                }
                return;
            }
            Err(_) => {
                delay_ms = (delay_ms * 2).min(RECONNECT_MAX_MS);
            }
        }
    }
}

/// Build subscribe/unsubscribe args JSON.
fn build_args(channel: &str, params: Vec<HashMap<String, String>>) -> Vec<serde_json::Value> {
    if params.is_empty() {
        vec![serde_json::json!({"channel": channel})]
    } else {
        params
            .into_iter()
            .map(|mut fields| {
                fields.insert("channel".to_string(), channel.to_string());
                serde_json::json!(fields)
            })
            .collect()
    }
}

/// Extract the channel name from a raw WS JSON payload using a lightweight string search.
///
/// Avoids a full `serde_json::Value` parse — just finds `"channel":"<name>"` in the text.
/// Falls back to `"unknown"` if not found.
fn extract_channel(text: &str) -> String {
    // Look for "channel":"<value>" pattern in the raw JSON.
    const NEEDLE: &str = "\"channel\":\"";
    if let Some(start) = text.find(NEEDLE) {
        let rest = &text[start + NEEDLE.len()..];
        if let Some(end) = rest.find('"') {
            return rest[..end].to_string();
        }
    }
    "unknown".to_string()
}

/// Compute the WS login signature.
///
/// `sign = Base64(HMAC-SHA256(secret_key, timestamp + "GET" + "/users/self/verify"))`
#[allow(clippy::expect_used)] // HMAC-SHA256 accepts any key length; infallible.
fn ws_sign(secret_key: &str, timestamp: &str) -> String {
    let pre_hash = format!("{timestamp}GET/users/self/verify");
    let mut mac =
        Hmac::<Sha256>::new_from_slice(secret_key.as_bytes()).expect("HMAC accepts any key length");
    mac.update(pre_hash.as_bytes());
    base64::engine::general_purpose::STANDARD.encode(mac.finalize().into_bytes())
}

impl OutcomesWsClient {
    pub async fn connect(&self, path: &str) -> Result<(), SdkError> {
        *self.shared.path.lock().await = path.to_string();
        self.shared
            .auto_reconnect
            .store(true, std::sync::atomic::Ordering::Relaxed);

        let (reader, ping) = do_connect(Arc::clone(&self.shared)).await?;
        *self.shared.reader_handle.lock().await = Some(reader);
        *self.shared.ping_handle.lock().await = Some(ping);
        Ok(())
    }

    /// Authenticate on the private WS channel using OKX API credentials.
    ///
    /// Must be called after [`connect`] with the business path (`/ws/v5/business`).
    /// Credentials are stored and replayed automatically on reconnect.
    ///
    /// Sends the login message and **waits** for the server's response:
    /// - `{"event":"login","code":"0"}` → success
    /// - `{"event":"error","code":"600xx"}` → returns `SdkError`
    ///
    /// Times out after 30 seconds if no response is received (matching OKX login expiry).
    pub async fn login(&self, creds: &ApiCredentials) -> Result<(), SdkError> {
        // Store for reconnect replay.
        *self.shared.credentials.lock().await = Some(creds.clone());

        // Set up a oneshot channel to receive the login result from the reader task.
        let (tx, rx) = tokio::sync::oneshot::channel();
        *self
            .shared
            .login_tx
            .lock()
            .unwrap_or_else(|e| e.into_inner()) = Some(tx);

        // Send the login message.
        self.send_login(creds).await?;

        // Wait for the reader task to signal the login result.
        // OKX docs: login requests expire 30s after the timestamp.
        match tokio::time::timeout(std::time::Duration::from_secs(30), rx).await {
            Ok(Ok(Ok(()))) => Ok(()),
            Ok(Ok(Err(msg))) => Err(SdkError::WebSocket {
                message: format!("Login rejected: {msg}"),
            }),
            Ok(Err(_)) => Err(SdkError::WebSocket {
                message: "Login response channel dropped".to_string(),
            }),
            Err(_) => Err(SdkError::WebSocket {
                message: "Login timed out (30s)".to_string(),
            }),
        }
    }

    /// Send the login message (used by both `login` and reconnect replay).
    async fn send_login(&self, creds: &ApiCredentials) -> Result<(), SdkError> {
        let timestamp = chrono::Utc::now().timestamp().to_string();
        let sign = ws_sign(&creds.secret_key, &timestamp);
        let msg = serde_json::json!({
            "op": "login",
            "args": [{
                "apiKey": creds.api_key,
                "passphrase": creds.passphrase,
                "timestamp": timestamp,
                "sign": sign,
            }]
        });
        self.send_json(&msg).await
    }

    pub async fn subscribe(
        &self,
        channel: &str,
        params: Vec<HashMap<String, String>>,
    ) -> Result<(), SdkError> {
        {
            let mut subs = self.shared.subscriptions.lock().await;
            subs.retain(|s| s.channel != channel || s.params != params);
            subs.push(Subscription {
                channel: channel.to_string(),
                params: params.clone(),
            });
        }
        let msg = serde_json::json!({ "op": "subscribe", "args": build_args(channel, params) });
        self.send_json(&msg).await
    }

    pub async fn unsubscribe(
        &self,
        channel: &str,
        params: Vec<HashMap<String, String>>,
    ) -> Result<(), SdkError> {
        {
            let mut subs = self.shared.subscriptions.lock().await;
            subs.retain(|s| s.channel != channel || s.params != params);
        }
        let msg = serde_json::json!({ "op": "unsubscribe", "args": build_args(channel, params) });
        self.send_json(&msg).await
    }

    pub fn set_on_data(&self, callback: WsDataCallback) {
        *self
            .shared
            .on_data
            .lock()
            .unwrap_or_else(|e| e.into_inner()) = Some(callback);
    }

    pub fn set_on_connection_state(&self, callback: WsConnectionStateCallback) {
        *self
            .shared
            .on_state
            .lock()
            .unwrap_or_else(|e| e.into_inner()) = Some(callback);
    }

    pub async fn disconnect(&self) {
        if ws_debug_enabled() {
            ws_debug!("Disconnecting");
        }
        self.shared
            .auto_reconnect
            .store(false, std::sync::atomic::Ordering::Relaxed);
        if let Some(h) = self.shared.ping_handle.lock().await.take() {
            h.abort();
        }
        if let Some(h) = self.shared.reader_handle.lock().await.take() {
            h.abort();
        }
        if let Some(mut s) = self.shared.sender.lock().await.take() {
            let _ = s.close().await;
        }
    }

    async fn send_json(&self, msg: &serde_json::Value) -> Result<(), SdkError> {
        let text = serde_json::to_string(msg).map_err(|e| SdkError::Serialization {
            message: e.to_string(),
        })?;
        if ws_debug_enabled() {
            // The `login` frame carries apiKey/passphrase/sign in plaintext;
            // never print its body, even in a debug build. Redact to a fixed
            // placeholder as defense-in-depth on top of the debug-build gate.
            if msg.get("op").and_then(|v| v.as_str()) == Some("login") {
                ws_debug!(">> {{\"op\":\"login\",\"args\":[<redacted>]}}");
            } else {
                ws_debug!(">> {text}");
            }
        }
        let mut guard = self.shared.sender.lock().await;
        let sender = guard.as_mut().ok_or_else(|| SdkError::WebSocket {
            message: "WS not connected".to_string(),
        })?;
        sender
            .send(Message::Text(text))
            .await
            .map_err(|e| SdkError::WebSocket {
                message: format!("WS send failed: {e}"),
            })
    }
}

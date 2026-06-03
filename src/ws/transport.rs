//! WebSocket transport trait and callback types.
//!
//! Abstracts over different WebSocket implementations:
//! - `OutcomesWsClient` (tokio-tungstenite, for CLI/server)
//! - Mobile platforms can provide their own `WsTransport` impl

use std::collections::HashMap;
use std::sync::Arc;

use super::models::WsMessage;
use crate::error::SdkError;

/// Callback for incoming push data — receives a typed [`WsMessage`].
///
/// The SDK parses each WS JSON payload once and delivers a typed enum variant.
/// Consumers never need to parse JSON themselves.
pub type WsDataCallback = Arc<dyn Fn(&WsMessage) + Send + Sync>;

/// Callback for connection state changes: `(channel_type, connected)`.
pub type WsConnectionStateCallback = Arc<dyn Fn(&str, bool) + Send + Sync>;

/// Abstraction over WebSocket transport.
#[async_trait::async_trait]
pub trait WsTransport: Send + Sync {
    /// Connect to a WS endpoint path (e.g., "/ws/v5/business").
    async fn connect(&self, path: &str) -> Result<(), SdkError>;

    /// Subscribe to a channel with given parameters.
    ///
    /// Sends `{"op":"subscribe","args":[{"channel":"...","instId":"..."}]}`.
    /// `params` is a list of key-value maps, e.g. `[{"instId": "121"}]`.
    async fn subscribe(
        &self,
        channel: &str,
        params: Vec<HashMap<String, String>>,
    ) -> Result<(), SdkError>;

    /// Unsubscribe from a channel.
    async fn unsubscribe(
        &self,
        channel: &str,
        params: Vec<HashMap<String, String>>,
    ) -> Result<(), SdkError>;

    /// Register a callback for incoming push data.
    fn set_on_data(&self, callback: WsDataCallback);

    /// Register a callback for connection state changes.
    fn set_on_connection_state(&self, callback: WsConnectionStateCallback);

    /// Disconnect and clean up.
    async fn disconnect(&self);
}

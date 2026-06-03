//! WebSocket support for the OKX Outcomes.
//!
//! Enabled by the `websocket` Cargo feature. Provides real-time streaming
//! via the OKX WebSocket API with auto-reconnect and subscription replay.

pub mod endpoints;
pub mod models;
pub mod transport;
pub mod tungstenite;

pub use transport::{WsConnectionStateCallback, WsDataCallback, WsTransport};
pub use tungstenite::OutcomesWsClient;

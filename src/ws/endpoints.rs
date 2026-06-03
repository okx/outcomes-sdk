//! WebSocket endpoint constants.
//!
//! Open API uses a unified `/ws/v5/business` path for both public and private
//! channels — same connection, login required for private channels.

/// Default WS host (HK production).
pub const DEFAULT_WS_HOST: &str = "wss://ws.okx.com:8443";

/// EU production WS host.
pub const EU_WS_HOST: &str = "wss://wseea.okx.com";

/// US production WS host.
pub const US_WS_HOST: &str = "wss://wsus.okx.com";

/// Unified Open API business channel path — public and private channels share this path.
/// Private channels require `op: "login"` after connecting.
pub const BUSINESS_PATH: &str = "/ws/v5/business";

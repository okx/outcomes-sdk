//! # Outcomes SDK
//!
//! Rust client for the [OKX Outcomes Developer API](https://www.okx.com/api/v5/predictions).
//!
//! ## Quick Start
//!
//! All REST endpoints require OKX API credentials (HMAC). The signing key
//! (EIP-712) is an independent secret used only for write operations.
//!
//! ```no_run
//! use okx_outcomes_sdk::{ApiCredentials, OutcomesSdkClient};
//!
//! #[tokio::main]
//! async fn main() -> Result<(), Box<dyn std::error::Error>> {
//!     // Supply your OKX API credentials however you prefer (config, secrets
//!     // manager, env vars, etc.) — the SDK does not load them for you.
//!     let creds = ApiCredentials {
//!         api_key:    "your-api-key".into(),
//!         secret_key: "your-secret-key".into(),
//!         passphrase: "your-passphrase".into(),
//!     };
//!     let client = OutcomesSdkClient::with_credentials(creds);
//!
//!     let events = client.get_events(None, None, None, None, None, None, None).await?;
//!     println!("{} events", events.events.len());
//!
//!     let orders = client.list_orders(None, None, None, None).await?;
//!     println!("{} orders", orders.list.len());
//!     Ok(())
//! }
//! ```

mod api;
mod client;
pub mod endpoints;
pub mod error;
pub mod models;

#[cfg(feature = "signing")]
pub mod signing;

#[cfg(feature = "websocket")]
pub mod ws;

pub use client::{ApiCredentials, OutcomesSdkClient, OutcomesSdkClientBuilder, TradingMode};
pub use error::SdkError;
pub use models::common::EcdsaSignature;

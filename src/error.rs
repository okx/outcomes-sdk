//! SDK error type.

/// Errors returned by the Outcomes SDK.
///
/// Marked `#[non_exhaustive]` so new failure modes can be added in future
/// minor releases without breaking downstream code. Consumers matching on
/// `SdkError` must include a wildcard (`_`) arm.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum SdkError {
    /// HTTP transport error (network failure, timeout, etc.).
    #[error("HTTP error: {0}")]
    Http(#[from] reqwest::Error),

    /// The server returned a non-zero business error code.
    #[error("API error ({code}): {message}")]
    Api { code: i64, message: String },

    /// A non-2xx HTTP response whose body did not match the API's
    /// `{ code, msg }` error envelope — e.g. an HTML error page from a proxy or
    /// gateway, or an empty body. Carries the HTTP status and a snippet of the
    /// raw body. Distinct from [`SdkError::Api`] so callers can tell a genuine
    /// API business code apart from a transport-level HTTP failure.
    #[error("unexpected HTTP {status} response: {body}")]
    UnexpectedStatus { status: u16, body: String },

    /// Response body could not be deserialized.
    #[error("Deserialization error: {0}")]
    Deserialize(#[from] serde_json::Error),

    /// WebSocket connection or transport error.
    #[error("WebSocket error: {message}")]
    WebSocket { message: String },

    /// The client was constructed without API credentials but attempted an
    /// HTTP request. All Outcomes REST endpoints require auth; only the
    /// WebSocket public channels work without credentials.
    #[error("not authenticated: {hint}")]
    NotAuthenticated { hint: String },

    /// Internal error (invalid URL, missing state, etc.).
    #[error("Internal error: {message}")]
    Internal { message: String },

    /// Serialization error.
    #[error("Serialization error: {message}")]
    Serialization { message: String },
}

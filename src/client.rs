//! HTTP client core - request helpers and OKX REST authentication.

use crate::error::SdkError;
use crate::models::common::{ApiEnvelope, ApiErrorBody, OkxMarketEnvelope};
use base64::Engine as _;
use chrono::Utc;
use hmac::{Hmac, Mac};
use serde::{de::DeserializeOwned, Serialize};
use sha2::Sha256;

const DEFAULT_BASE_URL: &str = "https://www.okx.com";

/// Auth headers whose values are credentials and must never be printed, even in
/// debug builds with `OUTCOMES_DEBUG=1`. Defense-in-depth on top of the
/// debug-build gate: a captured debug log (CI artifact, shared screen, pasted
/// ticket) still cannot leak a reusable credential.
const SENSITIVE_HEADERS: [&str; 4] = [
    "ok-access-sign",
    "ok-access-passphrase",
    "ok-access-key",
    "ok-access-timestamp",
];

/// Render a request header value for debug output, replacing the value of any
/// credential-bearing header with `<redacted>`. Matching is case-insensitive.
fn redact_header_value(name: &str, value: &str) -> String {
    if SENSITIVE_HEADERS.contains(&name.to_ascii_lowercase().as_str()) {
        "<redacted>".to_string()
    } else {
        value.to_string()
    }
}

/// Whether `raw` is a base URL we are willing to attach signed `OK-ACCESS-*`
/// headers to. Requires `https`, except that plain `http` is permitted for an
/// explicit localhost loopback host (local mocks / integration tests).
fn is_acceptable_base_url(raw: &str) -> bool {
    let Ok(url) = reqwest::Url::parse(raw) else {
        return false;
    };
    match url.scheme() {
        "https" => true,
        "http" => matches!(url.host_str(), Some("localhost" | "127.0.0.1" | "[::1]")),
        _ => false,
    }
}

/// OKX REST API credentials.
///
/// Obtain these from the OKX developer portal when creating an API key.
/// The `secret_key` is never sent over the wire - it is used locally to sign
/// each request via HMAC-SHA256.
#[derive(Clone)]
pub struct ApiCredentials {
    /// `OK-ACCESS-KEY` header value.
    pub api_key: String,
    /// Used to compute `OK-ACCESS-SIGN`; never transmitted.
    pub secret_key: String,
    /// `OK-ACCESS-PASSPHRASE` header value.
    pub passphrase: String,
}

/// Client for the OKX Outcomes Developer API.
///
/// # Authentication
///
/// Every REST endpoint requires OKX API-key credentials — construct the client
/// with [`OutcomesSdkClient::with_credentials`].
///
/// Each request is signed per the OKX REST authentication spec:
/// `OK-ACCESS-SIGN = Base64(HMAC-SHA256(secret_key, timestamp + METHOD + path + body))`
///
/// # Example
///
/// ```no_run
/// use okx_outcomes_sdk::{OutcomesSdkClient, ApiCredentials};
///
/// #[tokio::main]
/// async fn main() {
///     let creds = ApiCredentials {
///         api_key:    "your-api-key".into(),
///         secret_key: "your-secret-key".into(),
///         passphrase: "your-passphrase".into(),
///     };
///     let client = OutcomesSdkClient::with_credentials(creds);
/// }
/// ```
pub struct OutcomesSdkClient {
    pub(crate) http: reqwest::Client,
    pub(crate) base_url: String,
    pub(crate) credentials: Option<ApiCredentials>,
    /// Headers attached to every request via [`attach_auth`] (mode +
    /// language). Stored once at construction so [`OUTCOMES_DEBUG=1`]
    /// can surface them in the per-request log alongside `OK-ACCESS-*`,
    /// rather than having them silently merged at send time by
    /// `reqwest::ClientBuilder::default_headers`.
    pub(crate) extra_headers: reqwest::header::HeaderMap,
}

impl OutcomesSdkClient {
    /// Create an authenticated client for user-specific and write endpoints.
    ///
    /// Every request will carry the four `OK-ACCESS-*` headers signed with
    /// HMAC-SHA256 as specified in the OKX REST authentication documentation.
    pub fn with_credentials(credentials: ApiCredentials) -> Self {
        Self::build(None, Some(credentials))
    }

    /// Create an authenticated client pointing at a custom base URL.
    pub fn with_credentials_and_url(
        credentials: ApiCredentials,
        base_url: impl Into<String>,
    ) -> Self {
        Self::build(Some(base_url.into()), Some(credentials))
    }

    fn build(base_url: Option<String>, credentials: Option<ApiCredentials>) -> Self {
        // Resolution order for the outcomes base URL:
        //   1. explicit arg from `with_credentials_and_url`
        //   2. `OUTCOMES_API_BASE` env var
        //   3. compiled-in `DEFAULT_BASE_URL`
        let base_url = base_url
            .or_else(|| std::env::var("OUTCOMES_API_BASE").ok())
            .unwrap_or_else(|| DEFAULT_BASE_URL.to_string());

        // The `OK-ACCESS-*` signing headers are attached to every request to
        // `base_url`, so an attacker who can set `OUTCOMES_API_BASE` could
        // exfiltrate a valid passphrase + signature to an arbitrary host.
        // Refuse any non-`https` base URL (plain `http` is allowed only for an
        // explicit localhost loopback, which is used by tests/local mocks) and
        // fall back to the safe compiled-in default instead.
        let base_url = if is_acceptable_base_url(&base_url) {
            base_url
        } else {
            eprintln!(
                "Warning: refusing insecure OUTCOMES base URL {base_url:?} (must be https); \
                 falling back to {DEFAULT_BASE_URL}"
            );
            DEFAULT_BASE_URL.to_string()
        };

        let mut extra_headers = reqwest::header::HeaderMap::new();
        // Trading mode header from env var (CLI usage).
        // Mobile path uses OutcomesApiClient which reads the global config instead.
        if let Ok(mode) = std::env::var("OUTCOMES_MODE") {
            match mode.as_str() {
                "spots" | "points" => {
                    if let Ok(val) = reqwest::header::HeaderValue::from_str(&mode) {
                        extra_headers.insert("X-Predictions-Mode", val);
                    }
                }
                other => {
                    eprintln!("Warning: OUTCOMES_MODE must be \"spots\" or \"points\", got \"{other}\", header omitted");
                }
            }
        }

        // Accept-Language header for response localization. Standard HTTP
        // semantics: BCP 47 tag (e.g. "en-US", "zh-CN"). Backends translate
        // event titles / market questions when the value is recognized; an
        // unknown value is harmless (the server falls back to its default).
        if let Ok(lang) = std::env::var("OUTCOMES_LANG") {
            let trimmed = lang.trim();
            if !trimmed.is_empty() {
                if let Ok(val) = reqwest::header::HeaderValue::from_str(trimmed) {
                    extra_headers.insert(reqwest::header::ACCEPT_LANGUAGE, val);
                }
            }
        }

        let timeout_secs = std::env::var("OUTCOMES_TIMEOUT")
            .ok()
            .and_then(|v| v.parse::<u64>().ok())
            .unwrap_or(10);

        // Authenticated OKX endpoints never legitimately return a 3xx redirect.
        // Disable redirect following so the `OK-ACCESS-*` headers can never be
        // replayed to a redirect target (reqwest does not strip custom headers
        // across hosts). TLS certificate verification is always on - there is
        // no debug escape hatch; local-proxy debugging must trust the proxy CA
        // at the OS level.
        let builder = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(timeout_secs))
            .redirect(reqwest::redirect::Policy::none());

        Self {
            http: builder.build().unwrap_or_else(|_| reqwest::Client::new()),
            base_url,
            credentials,
            extra_headers,
        }
    }

    // -- Signing ----------------------------------------

    /// Current UTC timestamp in the OKX format: `2020-12-08T09:08:57.715Z`.
    fn timestamp() -> String {
        Utc::now().format("%Y-%m-%dT%H:%M:%S%.3fZ").to_string()
    }

    /// Compute `OK-ACCESS-SIGN` for a request.
    ///
    /// `pre_hash = timestamp + METHOD + request_path + body`
    /// For GET requests `body` is an empty string; query params are part of
    /// `request_path` (e.g. `/orders?status=open&limit=20`).
    #[allow(clippy::expect_used)] // HMAC-SHA256 accepts any key length; infallible in practice.
    fn sign(secret_key: &str, pre_hash: &str) -> String {
        let mut mac = Hmac::<Sha256>::new_from_slice(secret_key.as_bytes())
            .expect("HMAC accepts any key length");
        mac.update(pre_hash.as_bytes());
        base64::engine::general_purpose::STANDARD.encode(mac.finalize().into_bytes())
    }

    /// Attach the four `OK-ACCESS-*` auth headers to `builder`.
    ///
    /// All Outcomes REST endpoints require authentication; this method
    /// returns `SdkError::NotAuthenticated` when the client was constructed
    /// without credentials. Only `OutcomesWsClient` supports anonymous
    /// (public-channel) usage.
    fn attach_auth(
        &self,
        builder: reqwest::RequestBuilder,
        method: &str,
        request_path: &str,
        body: &str,
    ) -> Result<reqwest::RequestBuilder, SdkError> {
        let Some(creds) = &self.credentials else {
            return Err(SdkError::NotAuthenticated {
                hint: "build the client via OutcomesSdkClient::with_credentials \
                       or with_credentials_and_url; all REST endpoints require auth"
                    .to_string(),
            });
        };
        let ts = Self::timestamp();
        let pre_hash = format!("{ts}{method}{request_path}{body}");
        let sign = Self::sign(&creds.secret_key, &pre_hash);
        let mut builder = builder
            .header("OK-ACCESS-KEY", &creds.api_key)
            .header("OK-ACCESS-SIGN", sign)
            .header("OK-ACCESS-TIMESTAMP", ts)
            .header("OK-ACCESS-PASSPHRASE", &creds.passphrase);
        for (name, value) in self.extra_headers.iter() {
            builder = builder.header(name, value);
        }
        Ok(builder)
    }

    // -- HTTP helpers ----------------------------------------

    pub(crate) async fn http_get<T: DeserializeOwned>(
        &self,
        path: &str,
        params: &[(&str, &str)],
    ) -> Result<T, SdkError> {
        let url = format!("{}{}", self.base_url, path);
        let mut builder = self.http.get(&url);
        if !params.is_empty() {
            builder = builder.query(params);
        }

        // Build request_path for signing: full URL path per OKX docs,
        // e.g. /api/v5/predictions/events?status=active. `path` is already the
        // absolute path; just append the query string when present.
        let request_path: String = if params.is_empty() {
            path.to_string()
        } else {
            let qs: String = params
                .iter()
                .map(|(k, v)| format!("{k}={v}"))
                .collect::<Vec<_>>()
                .join("&");
            format!("{path}?{qs}")
        };

        builder = self.attach_auth(builder, "GET", &request_path, "")?;

        // Debug logging (which prints OK-ACCESS-* auth headers) is only honored
        // in debug builds; `debug_assertions` is off in release, so a released
        // binary never emits credentials even if OUTCOMES_DEBUG=1 is set.
        let debug =
            cfg!(debug_assertions) && std::env::var("OUTCOMES_DEBUG").is_ok_and(|v| v == "1");

        let request = builder.build()?;

        if debug {
            eprintln!(">>> GET {}", request.url());
            for (k, v) in request.headers() {
                eprintln!(
                    ">>> {}: {}",
                    k,
                    redact_header_value(k.as_str(), v.to_str().unwrap_or("?"))
                );
            }
        }

        let response = self.http.execute(request).await?;
        let status = response.status();

        if debug {
            eprintln!("<<< status: {status}");
            for (k, v) in response.headers() {
                eprintln!("<<< {}: {}", k, v.to_str().unwrap_or("?"));
            }
        }

        let raw = response.text().await.map_err(SdkError::Http)?;

        if debug {
            eprintln!("<<< body: {raw}");
        }

        Self::parse_response(status, &raw)
    }

    pub(crate) async fn http_post<T: DeserializeOwned>(
        &self,
        path: &str,
        body: &impl Serialize,
    ) -> Result<T, SdkError> {
        let url = format!("{}{}", self.base_url, path);
        let body_str = serde_json::to_string(body).unwrap_or_default();

        // Debug logging (which prints OK-ACCESS-* auth headers) is only honored
        // in debug builds; `debug_assertions` is off in release, so a released
        // binary never emits credentials even if OUTCOMES_DEBUG=1 is set.
        let debug =
            cfg!(debug_assertions) && std::env::var("OUTCOMES_DEBUG").is_ok_and(|v| v == "1");

        let builder = self.http.post(&url);
        let builder = self.attach_auth(builder, "POST", path, &body_str)?;
        let builder = builder
            .header("Content-Type", "application/json")
            .body(body_str.clone());

        let request = builder.build()?;

        if debug {
            eprintln!(">>> POST {url}");
            for (k, v) in request.headers() {
                eprintln!(
                    ">>> {}: {}",
                    k,
                    redact_header_value(k.as_str(), v.to_str().unwrap_or("?"))
                );
            }
            eprintln!(">>> body: {body_str}");
        }

        let response = self.http.execute(request).await?;
        let status = response.status();

        if debug {
            eprintln!("<<< status: {status}");
            for (k, v) in response.headers() {
                eprintln!("<<< {}: {}", k, v.to_str().unwrap_or("?"));
            }
        }

        let raw = response.text().await.map_err(SdkError::Http)?;

        if debug {
            eprintln!("<<< body: {raw}");
        }

        Self::parse_response(status, &raw)
    }

    /// GET to an absolute URL using the OKX market data envelope (`code` is a string).
    pub(crate) async fn http_get_abs<T: DeserializeOwned>(
        &self,
        url: &str,
        params: &[(&str, &str)],
    ) -> Result<T, SdkError> {
        let mut builder = self.http.get(url);
        if !params.is_empty() {
            builder = builder.query(params);
        }
        let envelope: OkxMarketEnvelope<T> = builder.send().await?.json().await?;
        if envelope.code != "0" {
            return Err(SdkError::Api {
                code: envelope.code.parse().unwrap_or(-1),
                message: envelope.msg,
            });
        }
        envelope.data.ok_or_else(|| SdkError::Api {
            code: -1,
            message: "server returned success but data was null".to_string(),
        })
    }

    /// Turn a raw response body into `T`, honoring the HTTP status.
    ///
    /// On a non-2xx status the backend does not return the usual data envelope —
    /// it returns a bare `{ "code": ..., "msg": ... }` error body (with `code`
    /// as either a string or a number). Parse that shape directly into an
    /// [`SdkError::Api`]; only fall through to the data envelope on success.
    /// If an error body is itself unparseable (e.g. an HTML 502 from a proxy),
    /// synthesize an error from the HTTP status and raw body so the caller still
    /// gets an actionable message instead of a deserialize error.
    fn parse_response<T: DeserializeOwned>(
        status: reqwest::StatusCode,
        raw: &str,
    ) -> Result<T, SdkError> {
        if !status.is_success() {
            return Err(match serde_json::from_str::<ApiErrorBody>(raw) {
                Ok(err) => SdkError::Api {
                    code: err.code,
                    message: err.msg,
                },
                // Body wasn't the `{ code, msg }` shape (HTML gateway page,
                // empty body, etc.). Fall back to a transport-level error that
                // preserves the HTTP status. Take a char-bounded snippet so a
                // large error page doesn't flood the message.
                Err(_) => SdkError::UnexpectedStatus {
                    status: status.as_u16(),
                    body: raw.trim().chars().take(512).collect(),
                },
            });
        }

        let envelope: ApiEnvelope<T> = serde_json::from_str(raw).map_err(SdkError::Deserialize)?;
        Self::unwrap_envelope(envelope)
    }

    fn unwrap_envelope<T>(envelope: ApiEnvelope<T>) -> Result<T, SdkError> {
        if envelope.code != 0 {
            return Err(SdkError::Api {
                code: envelope.code,
                message: envelope.message,
            });
        }
        envelope.data.ok_or_else(|| SdkError::Api {
            code: -1,
            message: "server returned success but data was null".to_string(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use reqwest::StatusCode;

    fn api_err(res: Result<serde_json::Value, SdkError>) -> (i64, String) {
        match res {
            Err(SdkError::Api { code, message }) => (code, message),
            other => panic!("expected SdkError::Api, got {other:?}"),
        }
    }

    #[test]
    fn non_2xx_parses_bare_error_body_with_numeric_code() {
        let raw = r#"{"code":100015,"msg":"Invalid calldata or malformed fields"}"#;
        let (code, message) = api_err(OutcomesSdkClient::parse_response(
            StatusCode::BAD_REQUEST,
            raw,
        ));
        assert_eq!(code, 100015);
        assert_eq!(message, "Invalid calldata or malformed fields");
    }

    #[test]
    fn non_2xx_parses_bare_error_body_with_string_code() {
        let raw = r#"{"code":"50105","msg":"Request header OK-ACCESS-PASSPHRASE incorrect."}"#;
        let (code, message) = api_err(OutcomesSdkClient::parse_response(
            StatusCode::UNAUTHORIZED,
            raw,
        ));
        assert_eq!(code, 50105);
        assert_eq!(message, "Request header OK-ACCESS-PASSPHRASE incorrect.");
    }

    #[test]
    fn non_2xx_with_unparseable_body_falls_back_to_unexpected_status() {
        let raw = "<html><body>502 Bad Gateway</body></html>";
        let res: Result<serde_json::Value, _> =
            OutcomesSdkClient::parse_response(StatusCode::BAD_GATEWAY, raw);
        match res {
            Err(SdkError::UnexpectedStatus { status, body }) => {
                assert_eq!(status, 502);
                assert!(body.contains("Bad Gateway"), "body was: {body}");
            }
            other => panic!("expected SdkError::UnexpectedStatus, got {other:?}"),
        }
    }

    #[test]
    fn unexpected_status_snippet_is_char_bounded() {
        // A large HTML error page is truncated to a bounded snippet.
        let raw = "x".repeat(5000);
        let res: Result<serde_json::Value, _> =
            OutcomesSdkClient::parse_response(StatusCode::BAD_GATEWAY, &raw);
        match res {
            Err(SdkError::UnexpectedStatus { status, body }) => {
                assert_eq!(status, 502);
                assert_eq!(body.chars().count(), 512);
            }
            other => panic!("expected SdkError::UnexpectedStatus, got {other:?}"),
        }
    }

    #[test]
    fn success_status_unwraps_data_envelope() {
        let raw = r#"{"code":"0","msg":"","data":{"x":1}}"#;
        let value: serde_json::Value =
            OutcomesSdkClient::parse_response(StatusCode::OK, raw).expect("success");
        assert_eq!(value["x"], 1);
    }

    #[test]
    fn success_status_with_business_error_code_is_api_error() {
        // A 200 OK carrying a non-zero business code still maps to an API error.
        let raw = r#"{"code":51000,"msg":"Parameter error","data":null}"#;
        let (code, message) = api_err(OutcomesSdkClient::parse_response(StatusCode::OK, raw));
        assert_eq!(code, 51000);
        assert_eq!(message, "Parameter error");
    }
}

//! `clientOrderId` generation and parsing per OKX spec v1.
//!
//! Format: `0x{region:1hex}{env:1hex}{random:30hex}` - fixed 34 chars, all
//! lowercase hex. The 30-hex random tail provides 120 bits of entropy from a
//! cryptographically secure RNG.
//!
//! Spec doc: <https://okg-block.sg.larksuite.com/wiki/KWWFwf3cbimvtzkmb8UlQxPAg2c>

use super::hex::{hex_char, parse_hex_nibble};

/// Region prefix.
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Region {
    Hk = 0,
    Us = 1,
    Eu = 2,
}

/// Environment prefix.
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Env {
    Prod = 1,
}

/// Decoded region/env nibbles from a client order ID. Region/env are kept as
/// raw u8 because consumers may encounter values outside the currently-defined
/// enums (the spec reserves the full 0x0..0xf range for future expansion).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ClientOrderIdPrefix {
    pub region: u8,
    pub env: u8,
}

/// Total client order ID length (incl. `0x` prefix).
pub const CLIENT_ORDER_ID_LEN: usize = 34;

const RANDOM_BYTES: usize = 15;

/// Generate a spec-compliant client order ID from a CSPRNG with explicit region/env.
///
/// Most callers should prefer [`generate_client_order_id_default`], which resolves the
/// `(region, env)` tuple from the registered context or the compiled-in default.
pub fn generate_client_order_id(region: Region, env: Env) -> Result<String, String> {
    let mut bytes = [0u8; RANDOM_BYTES];
    getrandom::getrandom(&mut bytes).map_err(|e| format!("CSPRNG failure: {e}"))?;
    Ok(format_client_order_id(region as u8, env as u8, &bytes))
}

/// Generate a spec-compliant client order ID using the resolved `(region, env)` tuple.
///
/// Resolution order:
/// 1. Context registered via [`register_client_order_id_context`] (intended for
///    native iOS/Android startup hooks or a one-time host-app init).
/// 2. Default: `Region::Hk` / `Env::Prod`.
///
/// The SDK reads no environment variables. To drive region/env from your own
/// configuration, either call [`register_client_order_id_context`] once at
/// startup or pass the values explicitly to [`generate_client_order_id`]
/// (parse them yourself with [`parse_region_str`] / [`parse_env_str`]).
pub fn generate_client_order_id_default() -> Result<String, String> {
    let (region, env) = resolve_context();
    generate_client_order_id(region, env)
}

// -- Context resolution ----------------------------------------

/// Register the process-wide `(region, env)` for [`generate_client_order_id_default`].
///
/// Intended to be called once at SDK init from the host application's startup
/// code. Subsequent calls overwrite the previous value, which is useful for
/// tests but should be avoided in production.
///
/// If the internal lock has been poisoned by a previous panic, this function
/// emits a warning to stderr and proceeds with the registration anyway -
/// the protected value is just `Option<(Region, Env)>`, so there's no
/// half-written state to recover.
pub fn register_client_order_id_context(region: Region, env: Env) {
    let mut guard = client_order_id_context_slot()
        .write()
        .unwrap_or_else(|poisoned| {
            eprintln!(
                "warn: okx_outcomes_sdk::signing::client_order_id: register_client_order_id_context recovered from poisoned lock"
            );
            poisoned.into_inner()
        });
    *guard = Some((region, env));
}

/// Clear the registered context, falling back to the compiled-in default.
///
/// Test-only - `#[cfg(test)]` keeps this symbol out of non-test builds so it
/// cannot be called from production code or external crates. Tests inside this
/// module use it to reset global state between assertions.
#[cfg(test)]
fn clear_client_order_id_context() {
    let mut guard = client_order_id_context_slot()
        .write()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    *guard = None;
}

fn client_order_id_context_slot() -> &'static std::sync::RwLock<Option<(Region, Env)>> {
    static SLOT: std::sync::OnceLock<std::sync::RwLock<Option<(Region, Env)>>> =
        std::sync::OnceLock::new();
    SLOT.get_or_init(|| std::sync::RwLock::new(None))
}

fn resolve_context() -> (Region, Env) {
    let guard = client_order_id_context_slot()
        .read()
        .unwrap_or_else(|poisoned| {
            eprintln!(
                "warn: okx_outcomes_sdk::signing::client_order_id: resolve_context recovered from poisoned lock"
            );
            poisoned.into_inner()
        });
    // Registered context if present, else the compiled-in HK-PROD default. The
    // SDK never reads environment variables.
    guard.unwrap_or((Region::Hk, Env::Prod))
}

/// Parse a region string (case-insensitive, trim whitespace). Accepts only
/// the canonical codes `HK` / `US` / `EU`.
pub fn parse_region_str(s: &str) -> Option<Region> {
    match s.trim().to_ascii_uppercase().as_str() {
        "HK" => Some(Region::Hk),
        "US" => Some(Region::Us),
        "EU" => Some(Region::Eu),
        _ => None,
    }
}

/// Parse an env string (case-insensitive, trim whitespace).
///
/// Spec-strict: only "PROD" is accepted. Aliases like "STAGING", "LIVE",
/// "PRODUCTION" are intentionally rejected to keep client order ID prefixes
/// consistent across services.
pub fn parse_env_str(s: &str) -> Option<Env> {
    match s.trim().to_ascii_uppercase().as_str() {
        "PROD" => Some(Env::Prod),
        _ => None,
    }
}

fn format_client_order_id(region: u8, env: u8, random: &[u8]) -> String {
    let mut s = String::with_capacity(CLIENT_ORDER_ID_LEN);
    s.push_str("0x");
    s.push(hex_char(region));
    s.push(hex_char(env));
    for b in random.iter() {
        s.push(hex_char(b >> 4));
        s.push(hex_char(b & 0x0f));
    }
    s
}

/// Validate a client order ID against the spec: 34 chars, `0x` prefix, lowercase hex.
///
/// This is stricter than [`super::hex::hex_decode`], which accepts mixed case.
/// Client order IDs are required to be all-lowercase by the spec for canonical
/// equality comparison across services - do not relax this check.
pub fn validate_client_order_id(s: &str) -> bool {
    if s.len() != CLIENT_ORDER_ID_LEN || !s.starts_with("0x") {
        return false;
    }
    s.as_bytes()[2..]
        .iter()
        .all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f'))
}

/// Parse the (region, env) prefix from a client order ID.
///
/// Any invalid input (null/empty, missing `0x`, length < 4, or non-hex
/// region/env nibble) falls back to the `(0, 0)` sentinel. `(0, 0)` is distinct
/// from any value [`generate_client_order_id`] emits (env nibble is always `1`
/// for `Prod`), so an unparseable ID is never mistaken for a generated one.
pub fn parse_client_order_id_prefix(client_order_id: Option<&str>) -> ClientOrderIdPrefix {
    let fallback = ClientOrderIdPrefix { region: 0, env: 0 };
    let s = match client_order_id {
        Some(s) if !s.is_empty() => s,
        _ => return fallback,
    };
    if s.len() < 4 || !s.starts_with("0x") {
        return fallback;
    }
    let bytes = s.as_bytes();
    match (parse_hex_nibble(bytes[2]), parse_hex_nibble(bytes[3])) {
        (Some(r), Some(e)) => ClientOrderIdPrefix { region: r, env: e },
        _ => fallback,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generated_client_order_id_matches_spec_shape() {
        let client_order_id =
            generate_client_order_id(Region::Hk, Env::Prod).unwrap_or_else(|_| unreachable!());
        assert_eq!(client_order_id.len(), CLIENT_ORDER_ID_LEN);
        assert!(client_order_id.starts_with("0x01"));
        assert!(validate_client_order_id(&client_order_id));
    }

    #[test]
    fn region_env_combinations_round_trip() {
        for (region, env, want_prefix) in [
            (Region::Hk, Env::Prod, "0x01"),
            (Region::Us, Env::Prod, "0x11"),
            (Region::Eu, Env::Prod, "0x21"),
        ] {
            let client_order_id =
                generate_client_order_id(region, env).unwrap_or_else(|_| unreachable!());
            assert!(
                client_order_id.starts_with(want_prefix),
                "expected prefix {want_prefix} got {client_order_id}"
            );
            let parsed = parse_client_order_id_prefix(Some(&client_order_id));
            assert_eq!(parsed.region, region as u8);
            assert_eq!(parsed.env, env as u8);
        }
    }

    #[test]
    fn ten_thousand_generations_are_unique() {
        let mut seen = std::collections::HashSet::new();
        for _ in 0..10_000 {
            let client_order_id =
                generate_client_order_id(Region::Hk, Env::Prod).unwrap_or_else(|_| unreachable!());
            assert!(
                seen.insert(client_order_id),
                "duplicate within 10k generations"
            );
        }
    }

    // -- Spec test vectors ----------------------------------------
    //
    // NOTE: The Lark spec (section 8) lists 36-char examples like
    //   "0x00a3f1b2c4d5e6789012345abcdef01234"
    // which contradict the format table (`0x` + 2 + 30 = 34) and all four
    // reference implementations (`secure_random_bytes(15)` -> 30 hex chars).
    // We follow the format table + reference impls (34 chars), which is what
    // callers actually receive from `generate_client_order_id`.

    #[test]
    fn vector_region0_env0_is_valid() {
        let s = "0x00a3f1b2c4d5e6789012345abcdef012";
        assert_eq!(s.len(), 34);
        assert!(validate_client_order_id(s));
        let p = parse_client_order_id_prefix(Some(s));
        assert_eq!((p.region, p.env), (0, 0));
    }

    #[test]
    fn vector_us_prod_is_valid() {
        let s = "0x117e8d9c0b1a2f3e4d5c6b7a8f9e0d1c";
        assert_eq!(s.len(), 34);
        assert!(validate_client_order_id(s));
        let p = parse_client_order_id_prefix(Some(s));
        assert_eq!((p.region, p.env), (1, 1));
    }

    #[test]
    fn vector_eu_env0_is_valid() {
        let s = "0x205fedcba9876543210abcdef0123456";
        assert_eq!(s.len(), 34);
        assert!(validate_client_order_id(s));
        let p = parse_client_order_id_prefix(Some(s));
        assert_eq!((p.region, p.env), (2, 0));
    }

    #[test]
    fn vector_short_is_invalid() {
        let s = "0x20fedcba9876543210abcdef012345"; // 32 chars
        assert_eq!(s.len(), 32);
        assert!(!validate_client_order_id(s));
    }

    #[test]
    fn vector_missing_0x_is_invalid() {
        let s = "00a3f1b2c4d5e6789012345abcdef0123ab"; // no 0x prefix
        assert!(!validate_client_order_id(s));
    }

    // -- Parser fallback (section 7) ---------------------------------

    #[test]
    fn parse_falls_back_to_zero_sentinel_on_invalid_input() {
        for input in [
            None,
            Some(""),
            Some("0x"),
            Some("0xz0aaaa"), // non-hex region nibble
            Some("0x0zaaaa"), // non-hex env nibble
            Some("nope"),
            Some("0"),
        ] {
            let p = parse_client_order_id_prefix(input);
            assert_eq!(
                (p.region, p.env),
                (0, 0),
                "expected (0, 0) sentinel for {input:?}"
            );
        }
    }

    #[test]
    fn parse_accepts_unknown_region_env_values() {
        // Unknown values must be returned verbatim (not folded into the
        // fallback) so consumers can decide they don't belong here.
        let p = parse_client_order_id_prefix(Some("0xfe000000000000000000000000000000ab"));
        assert_eq!((p.region, p.env), (0xf, 0xe));
    }

    #[test]
    fn validate_rejects_uppercase() {
        let s = "0x00A3F1B2C4D5E6789012345ABCDEF012"; // 34 chars, mixed case
        assert_eq!(s.len(), 34);
        assert!(!validate_client_order_id(s));
    }

    // -- Region/Env string parsers -----------------------------------

    #[test]
    fn region_str_accepts_canonical_codes_case_insensitively() {
        assert_eq!(parse_region_str("HK"), Some(Region::Hk));
        assert_eq!(parse_region_str("hk"), Some(Region::Hk));
        assert_eq!(parse_region_str("  Us  "), Some(Region::Us));
        assert_eq!(parse_region_str("eu"), Some(Region::Eu));
        assert_eq!(parse_region_str(""), None);
        assert_eq!(parse_region_str("APAC"), None);
    }

    #[test]
    fn env_str_only_accepts_prod() {
        assert_eq!(parse_env_str("PROD"), Some(Env::Prod));
        assert_eq!(parse_env_str("prod"), Some(Env::Prod));
        assert_eq!(parse_env_str(" Prod "), Some(Env::Prod));
        // Everything else is rejected - keep client order ID prefixes consistent.
        assert_eq!(parse_env_str("pre"), None);
        assert_eq!(parse_env_str("staging"), None);
        assert_eq!(parse_env_str("preprod"), None);
        assert_eq!(parse_env_str("Production"), None);
        assert_eq!(parse_env_str("live"), None);
        assert_eq!(parse_env_str("dev"), None);
        assert_eq!(parse_env_str(""), None);
    }

    // -- register / clear precedence ---------------------------------
    //
    // These mutate process-global state, so we serialize all global-context
    // assertions in a single test to avoid races between parallel test runs.

    #[test]
    fn registered_context_overrides_default() {
        clear_client_order_id_context();

        // 1. After clear, resolve_context returns the compiled-in HK-PROD
        //    default (the SDK reads no environment variables).
        assert_eq!(resolve_context(), (Region::Hk, Env::Prod));

        // 2. Register an override and confirm it wins.
        register_client_order_id_context(Region::Us, Env::Prod);
        let (r, e) = resolve_context();
        assert_eq!((r, e), (Region::Us, Env::Prod));

        // 3. Re-register overwrites.
        register_client_order_id_context(Region::Eu, Env::Prod);
        let (r, e) = resolve_context();
        assert_eq!((r, e), (Region::Eu, Env::Prod));

        // 4. generate_client_order_id_default uses the registered context.
        let client_order_id = generate_client_order_id_default().unwrap_or_else(|_| unreachable!());
        assert!(
            client_order_id.starts_with("0x21"),
            "expected EU-PROD prefix: {client_order_id}"
        );
        assert!(validate_client_order_id(&client_order_id));

        // 5. Clear restores fallthrough behavior.
        clear_client_order_id_context();
        let client_order_id = generate_client_order_id_default().unwrap_or_else(|_| unreachable!());
        assert!(validate_client_order_id(&client_order_id));
    }
}

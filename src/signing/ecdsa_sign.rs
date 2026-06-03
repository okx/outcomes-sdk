//! ECDSA signing, key parsing, address derivation, and low-level crypto utilities.
//!
//! Backed by alloy: hashing via [`alloy_primitives::keccak256`], signing via
//! [`alloy_signer_local::PrivateKeySigner`], and EIP-712 digest assembly via
//! [`super::eip712`]. We keep [`k256::ecdsa::SigningKey`] as the public key
//! type so call sites stay stable; the alloy signer wraps it internally.

use alloy_primitives::B256;
use alloy_signer::SignerSync;
use alloy_signer_local::PrivateKeySigner;
use k256::ecdsa::SigningKey;

use super::action::Action;
use super::eip712::{
    agent_struct_hash, chain_env, domain_separator, eip712_signing_hash, DOMAIN_CHAIN_TYPE,
};
use super::hex::{hex_decode, hex_encode};
use super::msgpack::{build_unsigned_tx_msgpack, connection_id};

// -- Core helpers ----------------------------------------

/// Thin re-export of alloy's keccak as our preferred `[u8; 32]` shape.
pub(crate) fn keccak256(data: &[u8]) -> [u8; 32] {
    alloy_primitives::keccak256(data).into()
}

/// Build an alloy signer that shares the underlying k256 key. Used as the
/// last step in every sign function; centralizes the conversion so a future
/// `parse_private_key -> PrivateKeySigner` refactor has one call site.
fn to_alloy_signer(key: &SigningKey) -> PrivateKeySigner {
    PrivateKeySigner::from_signing_key(key.clone())
}

/// Sign a 32-byte digest. Returns `(r, s, v)` where `v ∈ {0, 1}` matches
/// the OKX wire format (not Ethereum's 27/28).
fn sign_digest_parts(
    digest: &[u8; 32],
    key: &SigningKey,
) -> Result<([u8; 32], [u8; 32], u8), String> {
    let signer = to_alloy_signer(key);
    let sig = signer
        .sign_hash_sync(&B256::from_slice(digest))
        .map_err(|e| format!("ECDSA sign failed: {e}"))?;
    let r: [u8; 32] = sig.r().to_be_bytes();
    let s: [u8; 32] = sig.s().to_be_bytes();
    // alloy emits parity (false=0, true=1) — exactly the OKX wire format.
    let v: u8 = u8::from(sig.v());
    Ok((r, s, v))
}

/// Derive the Ethereum address from a signing key. Lowercase, `0x`-prefixed.
pub fn signer_address(key: &SigningKey) -> String {
    let addr = to_alloy_signer(key).address();
    format!("{addr:#x}")
}

// -- Unified sign_action ----------------------------------------

/// Sign any action, returning all components needed to submit the request to OKX.
///
/// Returns `(txhash, r, s, v)` where:
/// - `txhash` = EIP-712 signing hash (used as `?txhash=` query parameter)
/// - `r`, `s`  = hex strings with `0x` prefix
/// - `v`       = recovery id: `0` or `1` (NOT Ethereum's 27/28)
pub fn sign_action_full(
    action: &Action,
    nonce: u64,
    expires_after: Option<u64>,
    user: Option<&str>,
    key: &SigningKey,
) -> Result<(String, String, String, u8), String> {
    let source_chain = chain_env();
    let conn_id = connection_id(action, nonce, expires_after, user)?;
    let struct_hash = agent_struct_hash(source_chain.source(), &conn_id);
    let digest = eip712_signing_hash(DOMAIN_CHAIN_TYPE.chain_id(), struct_hash);
    let (r, s, v) = sign_digest_parts(&digest, key)?;
    Ok((
        format!("0x{}", hex_encode(&digest)),
        format!("0x{}", hex_encode(&r)),
        format!("0x{}", hex_encode(&s)),
        v,
    ))
}

/// Sign any action. Returns the 65-byte (r || s || v) signature hex string.
pub fn sign_action(
    action: &Action,
    nonce: u64,
    expires_after: Option<u64>,
    user: Option<&str>,
    key: &SigningKey,
) -> Result<String, String> {
    let source_chain = chain_env();
    let conn_id = connection_id(action, nonce, expires_after, user)?;
    let struct_hash = agent_struct_hash(source_chain.source(), &conn_id);
    let digest = eip712_signing_hash(DOMAIN_CHAIN_TYPE.chain_id(), struct_hash);
    let (r, s, v) = sign_digest_parts(&digest, key)?;
    // Reassemble the 65-byte hex blob the OKX `sign_action` consumers expect.
    // v is encoded as 27/28 in this string form to match legacy Ethereum
    // hex sigs; the `sign_action_full` path returns the raw 0/1 byte
    // separately for callers that need it.
    let v_eth = v + 27;
    let mut out = String::with_capacity(132);
    out.push_str("0x");
    out.push_str(&hex_encode(&r));
    out.push_str(&hex_encode(&s));
    out.push_str(&format!("{v_eth:02x}"));
    Ok(out)
}

/// Sign any action with full debug output.
pub fn sign_action_debug(
    action: &Action,
    nonce: u64,
    expires_after: Option<u64>,
    user: Option<&str>,
    key: &SigningKey,
) -> Result<SigningDebug, String> {
    let source_chain = chain_env();
    let domain_chain_id = DOMAIN_CHAIN_TYPE.chain_id();
    let conn_id = connection_id(action, nonce, expires_after, user)?;
    let ds = domain_separator(domain_chain_id);
    let sh = agent_struct_hash(source_chain.source(), &conn_id);
    let digest = eip712_signing_hash(domain_chain_id, sh);
    let sig = sign_action(action, nonce, expires_after, user, key)?;
    let addr = signer_address(key);

    let serialized_bytes = build_unsigned_tx_msgpack(action, nonce, expires_after, user);

    Ok(SigningDebug {
        domain_separator: format!("0x{}", hex_encode(&ds)),
        connection_id: format!("0x{}", hex_encode(&conn_id)),
        struct_hash: format!("0x{}", hex_encode(&sh)),
        signing_hash: format!("0x{}", hex_encode(&digest)),
        signature: sig,
        signer_address: addr,
        serialized_hex: format!("0x{}", hex_encode(&serialized_bytes)),
    })
}

/// Debug info from a signing operation.
pub struct SigningDebug {
    pub domain_separator: String,
    pub connection_id: String,
    pub struct_hash: String,
    pub signing_hash: String,
    pub signature: String,
    pub signer_address: String,
    /// Hex of MessagePack-serialized UnsignedTransaction.
    pub serialized_hex: String,
}

// -- ecrecover ----------------------------------------

/// Recover signer address from signing hash + 65-byte (r || s || v) signature
/// hex string. Accepts `v` as either `0/1` or `27/28`.
pub fn ecrecover(signing_hash: &str, signature: &str) -> Result<String, String> {
    let hash_bytes = hex_decode(signing_hash.strip_prefix("0x").unwrap_or(signing_hash))?;
    let sig_bytes = hex_decode(signature.strip_prefix("0x").unwrap_or(signature))?;

    if hash_bytes.len() != 32 {
        return Err(format!(
            "signing hash must be 32 bytes, got {}",
            hash_bytes.len()
        ));
    }
    if sig_bytes.len() != 65 {
        return Err(format!(
            "signature must be 65 bytes (r+s+v), got {}",
            sig_bytes.len()
        ));
    }

    let mut sig_array = [0u8; 65];
    sig_array.copy_from_slice(&sig_bytes);
    // alloy expects v ∈ {27, 28} or {0, 1}; normalize to 27/28.
    if sig_array[64] < 27 {
        sig_array[64] += 27;
    }

    let sig = alloy_primitives::Signature::try_from(&sig_array[..])
        .map_err(|e| format!("invalid signature bytes: {e}"))?;

    // Reject non-canonical (high-s) signatures (EIP-2). ECDSA is malleable:
    // for any valid (r, s, v) the variant (r, n-s, v^1) recovers the same
    // address, so a consumer that treats the signature bytes / recovery success
    // as a unique identifier (dedup, idempotency key, replay cache) could be
    // fed a second valid signature. `normalize_s` returns `Some` only when `s`
    // was in the upper half of the curve order, i.e. the input was high-s.
    if sig.normalize_s().is_some() {
        return Err("non-canonical (high-s) signature".to_string());
    }

    let hash: [u8; 32] = hash_bytes
        .try_into()
        .map_err(|_| "hash conversion failed".to_string())?;
    let addr = sig
        .recover_address_from_prehash(&B256::from(hash))
        .map_err(|e| format!("ecrecover failed: {e}"))?;
    Ok(format!("{addr:#x}"))
}

// -- Key parsing ----------------------------------------

/// Parse a hex private key string ("0x..." or raw hex) into a SigningKey.
pub fn parse_private_key(hex_key: &str) -> Result<SigningKey, String> {
    let clean = hex_key.strip_prefix("0x").unwrap_or(hex_key);
    let bytes = hex_decode(clean)?;
    SigningKey::from_bytes(bytes.as_slice().into()).map_err(|e| format!("invalid private key: {e}"))
}

// -- Nonce helper ----------------------------------------

/// Generate a nonce from current time (milliseconds since epoch).
pub fn now_millis() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloy_primitives::U256;

    // secp256k1 group order n.
    const SECP256K1_N_HEX: &str =
        "fffffffffffffffffffffffffffffffebaaedce6af48a03bbfd25e8cd0364141";

    #[test]
    fn ecrecover_accepts_low_s_and_rejects_high_s() {
        // Deterministic, obviously-synthetic test scalar (bytes 0x01..=0x20);
        // not a real or secret key.
        let seed: [u8; 32] = std::array::from_fn(|i| (i + 1) as u8);
        let key = parse_private_key(&format!("0x{}", hex_encode(&seed))).unwrap();
        let expected = signer_address(&key);

        let digest = [0x11u8; 32];
        let digest_hex = format!("0x{}", hex_encode(&digest));
        // alloy always emits canonical low-s signatures.
        let (r, s, v) = sign_digest_parts(&digest, &key).unwrap();

        // Canonical signature recovers the signer.
        let mut canonical = Vec::with_capacity(65);
        canonical.extend_from_slice(&r);
        canonical.extend_from_slice(&s);
        canonical.push(v);
        let recovered = ecrecover(&digest_hex, &format!("0x{}", hex_encode(&canonical))).unwrap();
        assert_eq!(recovered, expected);

        // Malleated variant (r, n-s, v^1) is a second valid signature for the
        // same digest; it must now be rejected.
        let n = U256::from_str_radix(SECP256K1_N_HEX, 16).unwrap();
        let s_high = (n - U256::from_be_bytes(s)).to_be_bytes::<32>();
        let mut malleated = Vec::with_capacity(65);
        malleated.extend_from_slice(&r);
        malleated.extend_from_slice(&s_high);
        malleated.push(v ^ 1);
        let err = ecrecover(&digest_hex, &format!("0x{}", hex_encode(&malleated))).unwrap_err();
        assert!(err.contains("high-s"), "unexpected error: {err}");
    }
}

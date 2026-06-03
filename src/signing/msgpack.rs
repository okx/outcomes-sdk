//! MessagePack serialization for UnsignedTransaction via `rmp-serde`.
//!
//! The wire shape is whatever `rmp_serde::to_vec_named` emits for a serde-
//! derived `UnsignedTransaction { action, nonce, expires_after?, user? }`.
//! Field declaration order on the inner types (`Action`, `OrderRequest`,
//! etc.) determines byte order; the `*_msgpack_has_correct_field_order`
//! tests in [`super`] gate this end-to-end.

use serde::Serialize;

use super::action::Action;
use super::ecdsa_sign::keccak256;

/// Wire shape of the bytes hashed to produce `connectionId`. Field
/// declaration order here is load-bearing — `action` first, `nonce`
/// second, then optional fields skipped when `None`.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct UnsignedTransaction<'a> {
    action: &'a Action,
    nonce: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    expires_after: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    user: Option<&'a str>,
}

/// Build MessagePack bytes for UnsignedTransaction. Shared by
/// [`connection_id`] and `sign_action_debug`.
#[allow(clippy::expect_used)] // rmp-serde never fails for these statically-typed in-memory structures.
pub(crate) fn build_unsigned_tx_msgpack(
    action: &Action,
    nonce: u64,
    expires_after: Option<u64>,
    user: Option<&str>,
) -> Vec<u8> {
    let tx = UnsignedTransaction {
        action,
        nonce,
        expires_after,
        user,
    };
    rmp_serde::to_vec_named(&tx)
        .expect("rmp-serde to_vec_named on UnsignedTransaction is infallible")
}

/// Compute connectionId = keccak256(msgpack(UnsignedTransaction)).
pub(crate) fn connection_id(
    action: &Action,
    nonce: u64,
    expires_after: Option<u64>,
    user: Option<&str>,
) -> Result<[u8; 32], String> {
    let buf = build_unsigned_tx_msgpack(action, nonce, expires_after, user);
    Ok(keccak256(&buf))
}

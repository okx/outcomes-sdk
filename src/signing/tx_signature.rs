//! Canonical transaction-signature envelope produced by the signing pipeline.
//!
//! The ECDSA variant holds raw `r` / `s` bytes and a `v` recovery id; the
//! `From<TxSignature> for SignatureWrapper` impl converts these into the
//! OKX wire-format `{ Ecdsa: { r, s, v } }` shape.
//!
//! A Passkey variant is intentionally omitted - OKX mobile does Passkey
//! signing on the native side, not in this crate.

use crate::models::common::{EcdsaSignature, SignatureWrapper};

use super::hex::hex_encode;

/// ECDSA signature components produced by `k256` signing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EcdsaParts {
    /// 32-byte `r` component.
    pub r: [u8; 32],
    /// 32-byte `s` component.
    pub s: [u8; 32],
    /// Recovery id: 0 or 1 (NOT 27/28).
    pub v: u8,
}

/// Action-signature envelope. Today only the ECDSA variant is constructible.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TxSignature {
    Ecdsa(EcdsaParts),
}

impl From<TxSignature> for SignatureWrapper {
    fn from(sig: TxSignature) -> Self {
        match sig {
            TxSignature::Ecdsa(EcdsaParts { r, s, v }) => SignatureWrapper {
                ecdsa: EcdsaSignature {
                    r: format!("0x{}", hex_encode(&r)),
                    s: format!("0x{}", hex_encode(&s)),
                    v,
                },
            },
        }
    }
}

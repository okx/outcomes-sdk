//! `ActionSigner` trait and standalone hash/sign/verify functions.
//!
//! The implementation routes through `ecdsa_sign` / `eip712` / `msgpack`,
//! with the crypto plumbing delegated to alloy. Wire format is unchanged.
//!
//! Today only the Agent signing path is wired up (all 6 of our action
//! variants take it). When TypedData actions land (Transfer / Withdraw /
//! ApproveAgent), [`signing_method`] grows a second branch and [`hash_action`]
//! dispatches accordingly.

use alloy_primitives::B256;
use alloy_signer::SignerSync;
use alloy_signer_local::PrivateKeySigner;
use k256::ecdsa::SigningKey;

use super::action::Action;
use super::chain_type::ChainType;
use super::ecdsa_sign;
use super::eip712::{agent_struct_hash, eip712_signing_hash, DOMAIN_CHAIN_TYPE};
use super::msgpack::connection_id;
use super::tx_signature::{EcdsaParts, TxSignature};

/// Which EIP-712 path an action takes when hashed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SigningMethod {
    /// msgpack(action) -> keccak -> wrap in Agent struct -> EIP-712.
    Agent,
}

/// Dispatch table from action variant to its signing path.
///
/// Today every variant goes through the Agent path. When TypedData actions
/// are added, this is where the per-variant routing lives.
pub fn signing_method(action: &Action) -> SigningMethod {
    match action {
        Action::PredictionSplit { .. }
        | Action::PredictionMerge { .. }
        | Action::PredictionRedeem { .. }
        | Action::PlaceOrder { .. }
        | Action::Cancel { .. }
        | Action::CancelAll { .. } => SigningMethod::Agent,
    }
}

/// Anything that can sign a 32-byte EIP-712 digest.
///
/// Mirrors tradezone's pattern: blanket impl over alloy's [`SignerSync`].
/// We additionally provide an explicit impl for [`SigningKey`] so existing
/// k256-typed call sites need no rewrite — internally we wrap into a
/// [`PrivateKeySigner`].
pub trait ActionSigner {
    type Error: std::fmt::Display;

    fn sign_hash(&self, hash: &[u8; 32]) -> Result<TxSignature, Self::Error>;
}

impl ActionSigner for SigningKey {
    type Error = String;

    fn sign_hash(&self, hash: &[u8; 32]) -> Result<TxSignature, String> {
        let signer = PrivateKeySigner::from_signing_key(self.clone());
        let sig = signer
            .sign_hash_sync(&B256::from_slice(hash))
            .map_err(|e| format!("ECDSA sign failed: {e}"))?;
        Ok(TxSignature::Ecdsa(EcdsaParts {
            r: sig.r().to_be_bytes(),
            s: sig.s().to_be_bytes(),
            v: u8::from(sig.v()),
        }))
    }
}

/// Compute the EIP-712 signing hash for an action.
///
/// `source_chain` is the Agent-source ChainType (env-dependent - see
/// [`super::eip712::chain_env`]). `nonce` / `expires_after` / `user` are the
/// fields embedded in the msgpack envelope.
pub fn hash_action(
    action: &Action,
    nonce: u64,
    expires_after: Option<u64>,
    user: Option<&str>,
    source_chain: ChainType,
) -> Result<[u8; 32], String> {
    match signing_method(action) {
        SigningMethod::Agent => {
            let conn_id = connection_id(action, nonce, expires_after, user)?;
            let struct_hash = agent_struct_hash(source_chain.source(), &conn_id);
            Ok(eip712_signing_hash(
                DOMAIN_CHAIN_TYPE.chain_id(),
                struct_hash,
            ))
        }
    }
}

/// Sign an action using an [`ActionSigner`], returning the canonical
/// [`TxSignature`] envelope.
pub fn sign_action<S: ActionSigner>(
    action: &Action,
    signer: &S,
    nonce: u64,
    expires_after: Option<u64>,
    user: Option<&str>,
    source_chain: ChainType,
) -> Result<TxSignature, String> {
    let digest = hash_action(action, nonce, expires_after, user, source_chain)?;
    signer.sign_hash(&digest).map_err(|e| e.to_string())
}

/// Verify a [`TxSignature`] against an action by recovering the signer
/// address. Returns the recovered Ethereum address as a lowercase
/// `"0x..."` string.
pub fn verify_action(
    sig: &TxSignature,
    action: &Action,
    nonce: u64,
    expires_after: Option<u64>,
    user: Option<&str>,
    source_chain: ChainType,
) -> Result<String, String> {
    let digest = hash_action(action, nonce, expires_after, user, source_chain)?;
    let TxSignature::Ecdsa(EcdsaParts { r, s, v }) = sig;
    let mut sig_bytes = [0u8; 65];
    sig_bytes[..32].copy_from_slice(r);
    sig_bytes[32..64].copy_from_slice(s);
    sig_bytes[64] = *v;
    let sig_hex = format!("0x{}", super::hex::hex_encode(&sig_bytes));
    let digest_hex = format!("0x{}", super::hex::hex_encode(&digest));
    ecdsa_sign::ecrecover(&digest_hex, &sig_hex)
}

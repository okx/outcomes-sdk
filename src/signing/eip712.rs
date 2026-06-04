//! EIP-712 domain separator, struct hash, and signing hash via the alloy
//! crates. The encoding follows the canonical EIP-712 spec — alloy enforces
//! it via [`alloy_sol_types::SolStruct`] and the [`alloy_sol_types::eip712_domain!`]
//! macro, so this module is now a thin wrapper. Hand-rolled byte assembly
//! lives in git history if you need to compare.

use alloy_primitives::{Address, B256};
use alloy_sol_types::{eip712_domain, sol, Eip712Domain, SolStruct};

use super::chain_type::ChainType;

/// Domain name. Source: `EXCHANGE_NAME` in the on-chain Solidity.
const DOMAIN_NAME: &str = "Exchange";

/// Domain version. Source: `VERSION` in the on-chain Solidity.
const DOMAIN_VERSION: &str = "1";

/// EIP-712 domain ChainType. Always Mainnet for outcomes — only the Agent
/// `source` string varies by env (see [`chain_env`]).
pub(crate) const DOMAIN_CHAIN_TYPE: ChainType = ChainType::Mainnet;

sol! {
    /// EIP-712 Agent struct that wraps the msgpack-derived `connectionId`.
    /// This is the only typed-data shape signed by this SDK; per-action
    /// disambiguation happens via the msgpack bytes hashed into
    /// `connectionId`, not via separate Solidity structs.
    #[derive(Debug)]
    struct Agent {
        string source;
        bytes32 connectionId;
    }
}

/// Build the alloy [`Eip712Domain`] for a given chain id. Kept as a private
/// helper because the only place a domain is needed is inside this module.
///
/// # Trust assumption (replay protection)
///
/// This domain is deliberately weak as an app/deployment binding: the
/// `verifying_contract` is [`Address::ZERO`] and `chain_id` is always the
/// compiled-in [`DOMAIN_CHAIN_TYPE`] (Mainnet), not the caller-supplied signing
/// chain. It therefore does **not** by itself prevent a signed payload from
/// being replayed across deployments or chains. Replay/uniqueness protection
/// instead relies on the msgpack-derived `connectionId` (which hashes the full
/// action bytes) together with the Agent `source` string. Any change to that
/// assumption — e.g. binding `chain_id` to the caller's chain and setting a
/// real `verifying_contract` — is a signing-compatibility change and must be
/// coordinated with the backend.
fn domain(chain_id: u64) -> Eip712Domain {
    eip712_domain! {
        name: DOMAIN_NAME,
        version: DOMAIN_VERSION,
        chain_id: chain_id,
        verifying_contract: Address::ZERO,
    }
}

/// Compute the EIP-712 domain separator. Public for snapshot tests; internal
/// callers should prefer [`eip712_signing_hash`].
pub(crate) fn domain_separator(chain_id: u64) -> [u8; 32] {
    domain(chain_id).hash_struct().into()
}

/// Compute the EIP-712 struct hash for `Agent { source, connectionId }`.
pub(crate) fn agent_struct_hash(source: &str, connection_id: &[u8; 32]) -> [u8; 32] {
    let agent = Agent {
        source: source.to_string(),
        connectionId: B256::from_slice(connection_id),
    };
    agent.eip712_hash_struct().into()
}

/// Compute the full EIP-712 signing hash:
/// `keccak256("\x19\x01" || domainSeparator || structHash)`.
pub(crate) fn eip712_signing_hash(chain_id: u64, struct_hash: [u8; 32]) -> [u8; 32] {
    let mut buf = Vec::with_capacity(66);
    buf.extend_from_slice(&[0x19, 0x01]);
    buf.extend_from_slice(&domain_separator(chain_id));
    buf.extend_from_slice(&struct_hash);
    alloy_primitives::keccak256(buf).into()
}

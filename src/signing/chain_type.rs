//! Chain identifier used in the EIP-712 domain.
//!
//! The numeric value is the EIP-712 domain chain ID; the `source()` method
//! returns the Agent-source string embedded in the EIP-712 struct.
//!
//! NOTE: outcomes always uses [`ChainType::Mainnet`] as the EIP-712 domain
//! chain ID (see [`crate::signing::eip712`] for the invariant). Only the Agent
//! `source` string varies by env - that's what [`chain_env`] returns.
//!
//! [`chain_env`]: super::eip712::chain_env

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u64)]
pub enum ChainType {
    Dev = 0,
    Testnet = 70_000_195,
    Mainnet = 70_000_196,
}

impl ChainType {
    /// Numeric chain ID used in EIP-712 domain separators.
    pub fn chain_id(self) -> u64 {
        self as u64
    }

    /// Agent-source string baked into the EIP-712 struct.
    pub fn source(self) -> &'static str {
        match self {
            ChainType::Dev => "Dev",
            ChainType::Testnet => "Testnet",
            ChainType::Mainnet => "Mainnet",
        }
    }
}

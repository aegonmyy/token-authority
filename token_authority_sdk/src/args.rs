//! Typed instruction-argument structs matching the guest program's signatures.
//!
//! Each struct derives `BorshSerialize` so callers can produce the raw payload
//! expected by the LEZ SPEL executor without hand-rolling byte serialisation.

use borsh::{BorshDeserialize, BorshSerialize};
use serde::{Deserialize, Serialize};

// ── new_fungible_token ────────────────────────────────────────────────────────

/// Arguments for the `new_fungible_token` instruction.
#[derive(Debug, Clone, BorshSerialize, BorshDeserialize, Serialize, Deserialize)]
pub struct NewFungibleTokenArgs {
    pub name: String,
    pub decimals: u8,
    /// Total tokens credited to the creator's holding on creation.
    pub initial_supply: u128,
    /// 32-byte account id of the initial mint authority, or `None` for
    /// fixed-supply (no future minting allowed).
    pub mint_authority: Option<[u8; 32]>,
}

impl NewFungibleTokenArgs {
    /// Encode `mint_authority` as the `Vec<u8>` the guest expects
    /// (empty = no authority, 32 bytes = authority account id).
    pub fn mint_authority_bytes(&self) -> Vec<u8> {
        match self.mint_authority {
            None     => vec![],
            Some(id) => id.to_vec(),
        }
    }
}

// ── mint_tokens ───────────────────────────────────────────────────────────────

/// Arguments for the `mint_tokens` instruction.
#[derive(Debug, Clone, BorshSerialize, BorshDeserialize, Serialize, Deserialize)]
pub struct MintTokensArgs {
    pub amount: u128,
}

// ── transfer_tokens ───────────────────────────────────────────────────────────

/// Arguments for the `transfer_tokens` instruction.
#[derive(Debug, Clone, BorshSerialize, BorshDeserialize, Serialize, Deserialize)]
pub struct TransferTokensArgs {
    pub amount: u128,
}

// ── burn_tokens ───────────────────────────────────────────────────────────────

/// Arguments for the `burn_tokens` instruction.
#[derive(Debug, Clone, BorshSerialize, BorshDeserialize, Serialize, Deserialize)]
pub struct BurnTokensArgs {
    pub amount: u128,
}

// ── rotate_authority ──────────────────────────────────────────────────────────

/// Arguments for the `rotate_authority` instruction.
#[derive(Debug, Clone, BorshSerialize, BorshDeserialize, Serialize, Deserialize)]
pub struct RotateAuthorityArgs {
    /// New 32-byte account id that will become the mint authority.
    pub new_authority: [u8; 32],
}

// ── revoke_authority ──────────────────────────────────────────────────────────

/// `revoke_authority` takes no arguments beyond the signer account.
/// The authority account is passed as an `AccountWithMetadata` at the call site.
#[derive(Debug, Clone, BorshSerialize, BorshDeserialize, Serialize, Deserialize)]
pub struct RevokeAuthorityArgs;

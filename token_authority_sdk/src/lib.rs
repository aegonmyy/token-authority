//! Host-side SDK for the `token_authority` SPEL program.
//!
//! Provides:
//! - Typed instruction argument structs that mirror the guest program's signature.
//! - Borsh serialisation helpers so callers can build raw instruction payloads.
//! - PDA seed constants matching those used inside the guest.
//! - A [`SimulatedLedger`] for unit-testing token flows without a RISC-Zero
//!   prover or a live LEZ network.
//!
//! # On-chain flow (with a real ELF + LEZ)
//! ```text
//! let elf = std::fs::read("methods/guest/target/.../token_authority").unwrap();
//! let args = NewFungibleTokenArgs { name: "LOGOS".into(), decimals: 6,
//!                                   initial_supply: 1_000_000, mint_authority: None };
//! // Pass `elf` + borsh-encoded `args` to the LEZ SPEL executor.
//! ```
//!
//! # Off-chain / test flow (this module's [`SimulatedLedger`])
//! ```rust
//! use token_authority_sdk::SimulatedLedger;
//! let mut ledger    = SimulatedLedger::new();
//! let authority_id  = [1u8; 32];
//! let creator_id    = [2u8; 32];
//! ledger.new_fungible_token("LOGOS", 6, 1_000_000, Some(authority_id), creator_id).unwrap();
//! ```

pub mod args;
pub mod pda;
pub mod sim;

pub use args::*;
pub use pda::*;
pub use sim::SimulatedLedger;

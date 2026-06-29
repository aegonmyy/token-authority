//! Host-side SDK for the token-authority SPEL program.
//!
//! ```rust
//! use token_authority_sdk::SimulatedLedger;
//! let mut ledger   = SimulatedLedger::new();
//! let authority_id = [1u8; 32];
//! let creator_id   = [2u8; 32];
//! ledger.new_fungible_token("LOGOS", 6, 1_000_000, Some(authority_id), creator_id).unwrap();
//! ```

pub mod args;
pub mod pda;
pub mod sim;

pub use args::*;
pub use pda::*;
pub use sim::SimulatedLedger;

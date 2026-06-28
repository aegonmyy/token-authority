//! Pure-Rust simulated ledger for testing token flows without a RISC-Zero
//! prover or a live LEZ network.
//!
//! All state lives in memory. PDA accounts are identified by deterministic
//! hashes. Holdings are keyed by holder account id.
//!
//! This module is intentionally self-contained and dependency-light so it can
//! run in any environment (CI, local tests, Windows, no RISC-Zero toolchain).

use std::collections::HashMap;

use thiserror::Error;

use admin_authority::{AdminConfig, AdminError, require_admin, transfer_admin, revoke_admin};
use token_authority_core::{TokenDef, TokenHolding};

use crate::pda::{TOKEN_DEF_SEED, derive_pda_id};

// ── Errors ────────────────────────────────────────────────────────────────────

#[derive(Debug, Error, PartialEq, Eq)]
pub enum SimError {
    #[error("token not initialised — call new_fungible_token first")]
    NotInitialised,

    #[error("token already initialised")]
    AlreadyInitialised,

    #[error("admin error: {0}")]
    Admin(#[from] AdminError),

    #[error("arithmetic overflow or underflow")]
    Arithmetic,

    #[error("insufficient token balance")]
    InsufficientFunds,

    #[error("invalid argument: {0}")]
    InvalidArg(&'static str),
}

// ── SimulatedLedger ───────────────────────────────────────────────────────────

/// In-memory ledger that mirrors the on-chain token program state.
///
/// One ledger = one token program deployment (one `token_def` + one `mint_auth`).
pub struct SimulatedLedger {
    program_id:  [u8; 32],
    token_def:   Option<TokenDef>,
    mint_auth:   Option<AdminConfig>,
    /// Balances keyed by holder account id.
    holdings:    HashMap<[u8; 32], TokenHolding>,
}

impl SimulatedLedger {
    pub fn new() -> Self {
        Self::with_program_id([0xabu8; 32])
    }

    pub fn with_program_id(program_id: [u8; 32]) -> Self {
        Self {
            program_id,
            token_def: None,
            mint_auth: None,
            holdings:  HashMap::new(),
        }
    }

    fn def_id(&self) -> [u8; 32] {
        derive_pda_id(&self.program_id, &[TOKEN_DEF_SEED])
    }

    // ── Read helpers ──────────────────────────────────────────────────────

    pub fn token_def(&self) -> Option<&TokenDef> {
        self.token_def.as_ref()
    }

    pub fn mint_auth(&self) -> Option<&AdminConfig> {
        self.mint_auth.as_ref()
    }

    pub fn balance_of(&self, holder: &[u8; 32]) -> u128 {
        self.holdings.get(holder).map(|h| h.balance).unwrap_or(0)
    }

    pub fn total_supply(&self) -> u128 {
        self.token_def.as_ref().map(|d| d.total_supply).unwrap_or(0)
    }

    // ── Instructions ──────────────────────────────────────────────────────

    /// `new_fungible_token` — mirrors the guest instruction.
    ///
    /// `creator` receives the entire `initial_supply`.
    pub fn new_fungible_token(
        &mut self,
        name: impl Into<String>,
        decimals: u8,
        initial_supply: u128,
        mint_authority: Option<[u8; 32]>,
        creator: [u8; 32],
    ) -> Result<(), SimError> {
        if self.token_def.is_some() {
            return Err(SimError::AlreadyInitialised);
        }
        let name = name.into();
        if name.trim().is_empty() {
            return Err(SimError::InvalidArg("token name cannot be empty"));
        }
        if decimals > 18 {
            return Err(SimError::InvalidArg("decimals must be ≤ 18"));
        }
        if let Some(id) = mint_authority {
            if id == [0u8; 32] {
                return Err(SimError::InvalidArg(
                    "mint_authority cannot be the null account id",
                ));
            }
        }

        let def = TokenDef { name, decimals, total_supply: initial_supply, mint_authority };
        let auth_cfg = match mint_authority {
            None     => AdminConfig { admin: None },
            Some(id) => AdminConfig::new(id)?,
        };

        // Credit initial supply to creator.
        let holding = TokenHolding { definition_id: self.def_id(), balance: initial_supply };
        self.holdings.insert(creator, holding);

        self.token_def = Some(def);
        self.mint_auth = Some(auth_cfg);
        Ok(())
    }

    /// `mint_tokens` — only the current mint authority may call this.
    pub fn mint_tokens(
        &mut self,
        authority: &[u8; 32],
        recipient: [u8; 32],
        amount: u128,
    ) -> Result<(), SimError> {
        if amount == 0 {
            return Err(SimError::InvalidArg("amount must be > 0"));
        }
        let auth_cfg = self.mint_auth.as_ref().ok_or(SimError::NotInitialised)?;
        require_admin(auth_cfg, authority)?;

        let def = self.token_def.as_mut().ok_or(SimError::NotInitialised)?;
        def.total_supply = def.total_supply.checked_add(amount).ok_or(SimError::Arithmetic)?;

        let def_id = self.def_id();
        let h = self.holdings.entry(recipient).or_insert_with(|| TokenHolding::new_zero(def_id));
        h.balance = h.balance.checked_add(amount).ok_or(SimError::Arithmetic)?;
        Ok(())
    }

    /// `transfer_tokens` — any holder may transfer their own tokens.
    pub fn transfer_tokens(
        &mut self,
        sender: [u8; 32],
        recipient: [u8; 32],
        amount: u128,
    ) -> Result<(), SimError> {
        if amount == 0 {
            return Err(SimError::InvalidArg("amount must be > 0"));
        }
        let sender_balance = self
            .holdings
            .get(&sender)
            .map(|h| h.balance)
            .unwrap_or(0);
        if sender_balance < amount {
            return Err(SimError::InsufficientFunds);
        }
        let def_id = self.def_id();
        self.holdings.entry(sender).and_modify(|h| h.balance -= amount);
        let h = self.holdings.entry(recipient).or_insert_with(|| TokenHolding::new_zero(def_id));
        h.balance = h.balance.checked_add(amount).ok_or(SimError::Arithmetic)?;
        Ok(())
    }

    /// `burn_tokens` — any holder may burn their own tokens.
    pub fn burn_tokens(
        &mut self,
        holder: [u8; 32],
        amount: u128,
    ) -> Result<(), SimError> {
        if amount == 0 {
            return Err(SimError::InvalidArg("amount must be > 0"));
        }
        let h = self.holdings.get_mut(&holder).ok_or(SimError::InsufficientFunds)?;
        if h.balance < amount {
            return Err(SimError::InsufficientFunds);
        }
        h.balance -= amount;

        let def = self.token_def.as_mut().ok_or(SimError::NotInitialised)?;
        def.total_supply = def.total_supply.checked_sub(amount).ok_or(SimError::Arithmetic)?;
        Ok(())
    }

    /// `rotate_authority` — transfer mint authority to a new account.
    pub fn rotate_authority(
        &mut self,
        current_authority: &[u8; 32],
        new_authority: [u8; 32],
    ) -> Result<(), SimError> {
        let auth_cfg = self.mint_auth.as_ref().ok_or(SimError::NotInitialised)?;
        let new_cfg = transfer_admin(auth_cfg, current_authority, new_authority)?;

        let def = self.token_def.as_mut().ok_or(SimError::NotInitialised)?;
        def.mint_authority = new_cfg.admin;
        self.mint_auth = Some(new_cfg);
        Ok(())
    }

    /// `revoke_authority` — permanently fix the supply (irreversible).
    pub fn revoke_authority(&mut self, current_authority: &[u8; 32]) -> Result<(), SimError> {
        let auth_cfg = self.mint_auth.as_ref().ok_or(SimError::NotInitialised)?;
        let new_cfg = revoke_admin(auth_cfg, current_authority)?;

        let def = self.token_def.as_mut().ok_or(SimError::NotInitialised)?;
        def.mint_authority = None;
        self.mint_auth = Some(new_cfg);
        Ok(())
    }
}

impl Default for SimulatedLedger {
    fn default() -> Self {
        Self::new()
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    const ALICE: [u8; 32] = [1u8; 32];
    const BOB:   [u8; 32] = [2u8; 32];
    const CAROL: [u8; 32] = [3u8; 32];

    fn fixed_supply_ledger() -> SimulatedLedger {
        let mut l = SimulatedLedger::new();
        l.new_fungible_token("FIXED", 6, 1_000, None, ALICE).unwrap();
        l
    }

    fn variable_supply_ledger() -> SimulatedLedger {
        let mut l = SimulatedLedger::new();
        l.new_fungible_token("VAR", 6, 500, Some(ALICE), ALICE).unwrap();
        l
    }

    // ── new_fungible_token ────────────────────────────────────────────────

    #[test]
    fn creates_token_and_credits_creator() {
        let l = fixed_supply_ledger();
        assert_eq!(l.total_supply(), 1_000);
        assert_eq!(l.balance_of(&ALICE), 1_000);
        assert!(l.token_def().unwrap().mint_authority.is_none());
    }

    #[test]
    fn double_init_fails() {
        let mut l = fixed_supply_ledger();
        assert_eq!(
            l.new_fungible_token("DUP", 6, 0, None, ALICE),
            Err(SimError::AlreadyInitialised)
        );
    }

    #[test]
    fn empty_name_rejected() {
        let mut l = SimulatedLedger::new();
        assert!(matches!(
            l.new_fungible_token("  ", 6, 0, None, ALICE),
            Err(SimError::InvalidArg(_))
        ));
    }

    #[test]
    fn decimals_over_18_rejected() {
        let mut l = SimulatedLedger::new();
        assert!(matches!(
            l.new_fungible_token("X", 19, 0, None, ALICE),
            Err(SimError::InvalidArg(_))
        ));
    }

    // ── mint_tokens ───────────────────────────────────────────────────────

    #[test]
    fn authority_can_mint() {
        let mut l = variable_supply_ledger();
        l.mint_tokens(&ALICE, BOB, 200).unwrap();
        assert_eq!(l.balance_of(&BOB), 200);
        assert_eq!(l.total_supply(), 700);
    }

    #[test]
    fn non_authority_cannot_mint() {
        let mut l = variable_supply_ledger();
        assert!(matches!(l.mint_tokens(&BOB, BOB, 1), Err(SimError::Admin(_))));
    }

    #[test]
    fn fixed_supply_mint_fails() {
        let mut l = fixed_supply_ledger();
        assert!(matches!(l.mint_tokens(&ALICE, ALICE, 1), Err(SimError::Admin(_))));
    }

    // ── transfer_tokens ───────────────────────────────────────────────────

    #[test]
    fn transfer_moves_balance() {
        let mut l = fixed_supply_ledger();
        l.transfer_tokens(ALICE, BOB, 300).unwrap();
        assert_eq!(l.balance_of(&ALICE), 700);
        assert_eq!(l.balance_of(&BOB), 300);
        assert_eq!(l.total_supply(), 1_000); // unchanged
    }

    #[test]
    fn transfer_insufficient_balance_fails() {
        let mut l = fixed_supply_ledger();
        assert_eq!(l.transfer_tokens(BOB, ALICE, 1), Err(SimError::InsufficientFunds));
    }

    // ── burn_tokens ───────────────────────────────────────────────────────

    #[test]
    fn burn_reduces_supply_and_balance() {
        let mut l = fixed_supply_ledger();
        l.burn_tokens(ALICE, 400).unwrap();
        assert_eq!(l.balance_of(&ALICE), 600);
        assert_eq!(l.total_supply(), 600);
    }

    #[test]
    fn burn_insufficient_balance_fails() {
        let mut l = fixed_supply_ledger();
        assert_eq!(l.burn_tokens(ALICE, 9_999), Err(SimError::InsufficientFunds));
    }

    // ── rotate_authority ──────────────────────────────────────────────────

    #[test]
    fn rotate_transfers_mint_right() {
        let mut l = variable_supply_ledger();
        l.rotate_authority(&ALICE, BOB).unwrap();

        assert_eq!(l.token_def().unwrap().mint_authority, Some(BOB));
        // old authority can no longer mint
        assert!(matches!(l.mint_tokens(&ALICE, CAROL, 1), Err(SimError::Admin(_))));
        // new authority can mint
        l.mint_tokens(&BOB, CAROL, 50).unwrap();
        assert_eq!(l.balance_of(&CAROL), 50);
    }

    // ── revoke_authority ──────────────────────────────────────────────────

    #[test]
    fn revoke_fixes_supply() {
        let mut l = variable_supply_ledger();
        l.revoke_authority(&ALICE).unwrap();

        assert!(l.token_def().unwrap().mint_authority.is_none());
        assert!(matches!(l.mint_tokens(&ALICE, BOB, 1), Err(SimError::Admin(_))));
    }

    #[test]
    fn revoke_twice_fails() {
        let mut l = variable_supply_ledger();
        l.revoke_authority(&ALICE).unwrap();
        assert!(matches!(l.revoke_authority(&ALICE), Err(SimError::Admin(_))));
    }

    // ── full lifecycle ────────────────────────────────────────────────────

    #[test]
    fn variable_supply_full_lifecycle() {
        let mut l = SimulatedLedger::new();

        // 1. Create token with ALICE as authority; all supply goes to ALICE.
        l.new_fungible_token("LOGOS", 6, 1_000_000, Some(ALICE), ALICE).unwrap();
        assert_eq!(l.total_supply(), 1_000_000);

        // 2. ALICE distributes via transfer.
        l.transfer_tokens(ALICE, BOB,   300_000).unwrap();
        l.transfer_tokens(ALICE, CAROL, 200_000).unwrap();

        // 3. ALICE mints a bonus round.
        l.mint_tokens(&ALICE, BOB, 50_000).unwrap();
        assert_eq!(l.total_supply(), 1_050_000);

        // 4. BOB burns some tokens.
        l.burn_tokens(BOB, 25_000).unwrap();
        assert_eq!(l.total_supply(), 1_025_000);

        // 5. ALICE hands authority to CAROL.
        l.rotate_authority(&ALICE, CAROL).unwrap();
        assert!(matches!(l.mint_tokens(&ALICE, BOB, 1), Err(SimError::Admin(_))));

        // 6. CAROL mints a little, then permanently locks supply.
        l.mint_tokens(&CAROL, CAROL, 5_000).unwrap();
        l.revoke_authority(&CAROL).unwrap();
        assert!(matches!(l.mint_tokens(&CAROL, CAROL, 1), Err(SimError::Admin(_))));

        // Final supply is deterministic.
        assert_eq!(l.total_supply(), 1_030_000);
    }
}

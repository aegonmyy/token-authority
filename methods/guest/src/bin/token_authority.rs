#![no_main]

use spel_framework::prelude::*;
use nssa_core::account::Data;

use admin_authority::{AdminConfig, AdminError, transfer_admin, revoke_admin, require_admin};
use token_authority_core::{TokenDef, TokenHolding};

risc0_zkvm::guest::entry!(main);

// ── Error helpers ─────────────────────────────────────────────────────────────

fn admin_err(e: AdminError) -> SpelError {
    SpelError::custom(e.code(), e.message().to_string())
}

fn log_cycles(label: &str, start: u64) {
    let end = risc0_zkvm::guest::env::cycle_count();
    risc0_zkvm::guest::env::log(&format!("CU {} cycles={}", label, end - start));
}

// ── Shared account-data helpers ───────────────────────────────────────────────

fn read_token_def(acc: &AccountWithMetadata) -> Result<TokenDef, SpelError> {
    TokenDef::from_bytes(acc.account.data.as_ref())
        .map_err(|e| SpelError::SerializationError { message: e.to_string() })
}

fn read_admin_config(acc: &AccountWithMetadata) -> Result<AdminConfig, SpelError> {
    AdminConfig::from_bytes(acc.account.data.as_ref())
        .map_err(|e| SpelError::SerializationError { message: e.to_string() })
}

fn read_holding(acc: &AccountWithMetadata) -> Result<TokenHolding, SpelError> {
    TokenHolding::from_bytes(acc.account.data.as_ref())
        .map_err(|e| SpelError::SerializationError { message: e.to_string() })
}

fn write_def(acc: &mut AccountWithMetadata, def: &TokenDef) -> Result<(), SpelError> {
    acc.account.data = Data::try_from(def.to_bytes())
        .map_err(|_| SpelError::custom(0, String::from("token_def data overflow")))?;
    Ok(())
}

fn write_auth(acc: &mut AccountWithMetadata, cfg: &AdminConfig) -> Result<(), SpelError> {
    acc.account.data = Data::try_from(cfg.to_bytes())
        .map_err(|_| SpelError::custom(0, String::from("mint_auth data overflow")))?;
    Ok(())
}

fn write_holding(acc: &mut AccountWithMetadata, h: &TokenHolding) -> Result<(), SpelError> {
    acc.account.data = Data::try_from(h.to_bytes())
        .map_err(|_| SpelError::custom(0, String::from("holding data overflow")))?;
    Ok(())
}

// ── Program ───────────────────────────────────────────────────────────────────

#[lez_program]
mod token_authority {
    #[allow(unused_imports)]
    use super::*;

    /// Create a new fungible token.
    ///
    /// Initialises the token definition PDA and the mint-authority config PDA.
    /// The entire `initial_supply` is credited to `creator_holding`.
    ///
    /// `mint_authority_id`: 32-byte account id of the initial mint authority.
    ///   Pass all-zeros to launch with no mint authority (fixed supply from day one).
    #[instruction]
    pub fn new_fungible_token(
        #[account(init, pda = [literal("token_def")])]
        mut def_acc: AccountWithMetadata,
        #[account(init, pda = [literal("mint_auth")])]
        mut auth_acc: AccountWithMetadata,
        #[account(mut)]
        mut creator_holding: AccountWithMetadata,
        #[account(signer)]
        creator: AccountWithMetadata,
        name: String,
        decimals: u8,
        initial_supply: u128,
        mint_authority_id: Vec<u8>,
    ) -> SpelResult {
        let cu_start = risc0_zkvm::guest::env::cycle_count();

        if name.trim().is_empty() {
            return Err(SpelError::custom(3001, String::from("token name cannot be empty")));
        }
        if decimals > 18 {
            return Err(SpelError::custom(3002, String::from("decimals must be ≤ 18")));
        }

        // Resolve mint authority — empty or all-zeros → no authority.
        let authority_opt: Option<[u8; 32]> = if mint_authority_id.is_empty()
            || mint_authority_id == vec![0u8; 32]
        {
            None
        } else {
            if mint_authority_id.len() != 32 {
                return Err(SpelError::custom(
                    3003,
                    String::from("mint_authority_id must be 32 bytes or empty"),
                ));
            }
            let mut arr = [0u8; 32];
            arr.copy_from_slice(&mint_authority_id);
            Some(arr)
        };

        // Build token definition.
        let def = TokenDef {
            name,
            decimals,
            total_supply: initial_supply,
            mint_authority: authority_opt,
        };
        write_def(&mut def_acc, &def)?;

        // Build mint-authority config.
        let admin_cfg = match authority_opt {
            None => AdminConfig { admin: None },
            Some(id) => AdminConfig::new(id).map_err(admin_err)?,
        };
        write_auth(&mut auth_acc, &admin_cfg)?;

        // Credit initial supply to creator holding.
        let holding = if creator_holding.account.data.is_empty() {
            TokenHolding {
                definition_id: *def_acc.account_id.value(),
                balance:       initial_supply,
            }
        } else {
            let mut h = read_holding(&creator_holding)?;
            h.balance = h
                .balance
                .checked_add(initial_supply)
                .ok_or_else(|| SpelError::custom(2002, String::from("balance overflow")))?;
            h
        };
        write_holding(&mut creator_holding, &holding)?;

        log_cycles("new_fungible_token", cu_start);
        Ok(SpelOutput::execute(
            vec![def_acc, auth_acc, creator_holding, creator],
            vec![],
        ))
    }

    /// Mint additional tokens to `recipient_holding`.
    ///
    /// Only callable by the current mint authority. Fails if authority is revoked.
    #[instruction]
    pub fn mint_tokens(
        #[account(mut, pda = [literal("token_def")])]
        mut def_acc: AccountWithMetadata,
        #[account(pda = [literal("mint_auth")])]
        auth_acc: AccountWithMetadata,
        #[account(mut)]
        mut recipient_holding: AccountWithMetadata,
        #[account(signer)]
        authority: AccountWithMetadata,
        amount: u128,
    ) -> SpelResult {
        let cu_start = risc0_zkvm::guest::env::cycle_count();

        let admin_cfg = read_admin_config(&auth_acc)?;
        require_admin(&admin_cfg, authority.account_id.value()).map_err(admin_err)?;

        let def     = read_token_def(&def_acc)?;
        let holding = if recipient_holding.account.data.is_empty() {
            TokenHolding::new_zero(*def_acc.account_id.value())
        } else {
            read_holding(&recipient_holding)?
        };

        // Delegate arithmetic to token_authority_core.
        // The core uses TEST_DEF_ID as a placeholder — in the guest we bypass
        // definition_id validation and do it here explicitly.
        if !recipient_holding.account.data.is_empty()
            && holding.definition_id != *def_acc.account_id.value()
        {
            return Err(SpelError::custom(2005, String::from("holding belongs to a different token")));
        }

        if amount == 0 {
            return Err(SpelError::custom(2006, String::from("amount must be greater than zero")));
        }

        let new_supply = def
            .total_supply
            .checked_add(amount)
            .ok_or_else(|| SpelError::custom(2001, String::from("total supply overflow")))?;
        let new_balance = holding
            .balance
            .checked_add(amount)
            .ok_or_else(|| SpelError::custom(2002, String::from("balance overflow")))?;

        let mut new_def = def;
        new_def.total_supply = new_supply;
        write_def(&mut def_acc, &new_def)?;

        let new_holding = TokenHolding {
            definition_id: *def_acc.account_id.value(),
            balance: new_balance,
        };
        write_holding(&mut recipient_holding, &new_holding)?;

        log_cycles("mint_tokens", cu_start);
        Ok(SpelOutput::execute(
            vec![def_acc, auth_acc, recipient_holding, authority],
            vec![],
        ))
    }

    /// Transfer `amount` tokens from `sender_holding` to `recipient_holding`.
    ///
    /// Anyone can transfer from their own holding. No authority required.
    #[instruction]
    pub fn transfer_tokens(
        #[account(pda = [literal("token_def")])]
        def_acc: AccountWithMetadata,
        #[account(mut)]
        mut sender_holding: AccountWithMetadata,
        #[account(mut)]
        mut recipient_holding: AccountWithMetadata,
        #[account(signer)]
        sender: AccountWithMetadata,
        amount: u128,
    ) -> SpelResult {
        let cu_start = risc0_zkvm::guest::env::cycle_count();

        if amount == 0 {
            return Err(SpelError::custom(2006, String::from("amount must be greater than zero")));
        }

        let def_id = *def_acc.account_id.value();

        let sender_h = read_holding(&sender_holding)?;
        if sender_h.definition_id != def_id {
            return Err(SpelError::custom(2005, String::from("sender holding: wrong token")));
        }

        let recipient_h = if recipient_holding.account.data.is_empty() {
            TokenHolding::new_zero(def_id)
        } else {
            let h = read_holding(&recipient_holding)?;
            if h.definition_id != def_id {
                return Err(SpelError::custom(2005, String::from("recipient holding: wrong token")));
            }
            h
        };

        let new_sender_balance = sender_h
            .balance
            .checked_sub(amount)
            .ok_or_else(|| SpelError::custom(2007, String::from("insufficient token balance")))?;
        let new_recipient_balance = recipient_h
            .balance
            .checked_add(amount)
            .ok_or_else(|| SpelError::custom(2002, String::from("recipient balance overflow")))?;

        let mut new_sender_h = sender_h;
        new_sender_h.balance = new_sender_balance;
        write_holding(&mut sender_holding, &new_sender_h)?;

        let mut new_recipient_h = recipient_h;
        new_recipient_h.balance = new_recipient_balance;
        write_holding(&mut recipient_holding, &new_recipient_h)?;

        log_cycles("transfer_tokens", cu_start);
        Ok(SpelOutput::execute(
            vec![def_acc, sender_holding, recipient_holding, sender],
            vec![],
        ))
    }

    /// Burn `amount` tokens from `holder_holding`, reducing total supply.
    ///
    /// Anyone can burn their own tokens. No authority required.
    #[instruction]
    pub fn burn_tokens(
        #[account(mut, pda = [literal("token_def")])]
        mut def_acc: AccountWithMetadata,
        #[account(mut)]
        mut holder_holding: AccountWithMetadata,
        #[account(signer)]
        holder: AccountWithMetadata,
        amount: u128,
    ) -> SpelResult {
        let cu_start = risc0_zkvm::guest::env::cycle_count();

        if amount == 0 {
            return Err(SpelError::custom(2006, String::from("amount must be greater than zero")));
        }

        let def   = read_token_def(&def_acc)?;
        let h     = read_holding(&holder_holding)?;

        if h.definition_id != *def_acc.account_id.value() {
            return Err(SpelError::custom(2005, String::from("holding belongs to a different token")));
        }

        let new_balance = h
            .balance
            .checked_sub(amount)
            .ok_or_else(|| SpelError::custom(2007, String::from("insufficient token balance")))?;
        let new_supply = def
            .total_supply
            .checked_sub(amount)
            .ok_or_else(|| SpelError::custom(2004, String::from("supply underflow")))?;

        let mut new_def = def;
        new_def.total_supply = new_supply;
        write_def(&mut def_acc, &new_def)?;

        let mut new_h = h;
        new_h.balance = new_balance;
        write_holding(&mut holder_holding, &new_h)?;

        log_cycles("burn_tokens", cu_start);
        Ok(SpelOutput::execute(
            vec![def_acc, holder_holding, holder],
            vec![],
        ))
    }

    /// Rotate mint authority to `new_authority_id`.
    ///
    /// Only callable by the current mint authority.
    /// To permanently fix supply, call `revoke_authority` instead.
    #[instruction]
    pub fn rotate_authority(
        #[account(mut, pda = [literal("token_def")])]
        mut def_acc: AccountWithMetadata,
        #[account(mut, pda = [literal("mint_auth")])]
        mut auth_acc: AccountWithMetadata,
        #[account(signer)]
        authority: AccountWithMetadata,
        new_authority_id: Vec<u8>,
    ) -> SpelResult {
        let cu_start = risc0_zkvm::guest::env::cycle_count();

        if new_authority_id.len() != 32 {
            return Err(SpelError::custom(
                3003,
                String::from("new_authority_id must be exactly 32 bytes"),
            ));
        }
        let mut new_id = [0u8; 32];
        new_id.copy_from_slice(&new_authority_id);

        let admin_cfg = read_admin_config(&auth_acc)?;
        let new_cfg = transfer_admin(&admin_cfg, authority.account_id.value(), new_id)
            .map_err(admin_err)?;

        // Keep TokenDef.mint_authority in sync.
        let mut def = read_token_def(&def_acc)?;
        def.mint_authority = new_cfg.admin;
        write_def(&mut def_acc, &def)?;
        write_auth(&mut auth_acc, &new_cfg)?;

        log_cycles("rotate_authority", cu_start);
        Ok(SpelOutput::execute(
            vec![def_acc, auth_acc, authority],
            vec![],
        ))
    }

    /// Permanently revoke mint authority — supply becomes fixed forever.
    ///
    /// Only callable by the current mint authority. This action is irreversible.
    #[instruction]
    pub fn revoke_authority(
        #[account(mut, pda = [literal("token_def")])]
        mut def_acc: AccountWithMetadata,
        #[account(mut, pda = [literal("mint_auth")])]
        mut auth_acc: AccountWithMetadata,
        #[account(signer)]
        authority: AccountWithMetadata,
    ) -> SpelResult {
        let cu_start = risc0_zkvm::guest::env::cycle_count();

        let admin_cfg = read_admin_config(&auth_acc)?;
        let new_cfg   = revoke_admin(&admin_cfg, authority.account_id.value())
            .map_err(admin_err)?;

        // Mirror revocation into TokenDef.mint_authority.
        let mut def = read_token_def(&def_acc)?;
        def.mint_authority = None;
        write_def(&mut def_acc, &def)?;
        write_auth(&mut auth_acc, &new_cfg)?;

        log_cycles("revoke_authority", cu_start);
        Ok(SpelOutput::execute(
            vec![def_acc, auth_acc, authority],
            vec![],
        ))
    }
}

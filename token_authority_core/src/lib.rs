use borsh::{BorshDeserialize, BorshSerialize};
use serde::{Deserialize, Serialize};

pub const E_SUPPLY_OVERFLOW:    u32 = 2001;
pub const E_BALANCE_OVERFLOW:   u32 = 2002;
pub const E_BALANCE_UNDERFLOW:  u32 = 2003;
pub const E_SUPPLY_UNDERFLOW:   u32 = 2004;
pub const E_WRONG_DEFINITION:   u32 = 2005;
pub const E_ZERO_AMOUNT:        u32 = 2006;
pub const E_INSUFFICIENT_FUNDS: u32 = 2007;

/// On-chain fungible token definition. `mint_authority == None` means supply is fixed.
#[derive(
    Debug, Clone, PartialEq, Eq,
    Serialize, Deserialize,
    BorshSerialize, BorshDeserialize,
)]
pub struct TokenDef {
    pub name:           String,
    pub decimals:       u8,
    pub total_supply:   u128,
    pub mint_authority: Option<[u8; 32]>,
}

impl TokenDef {
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, std::io::Error> {
        borsh::from_slice(bytes)
    }

    pub fn to_bytes(&self) -> Vec<u8> {
        borsh::to_vec(self).expect("TokenDef serialisation cannot fail")
    }
}

/// Per-holder balance account for a fungible token.
#[derive(
    Debug, Clone, PartialEq, Eq,
    Serialize, Deserialize,
    BorshSerialize, BorshDeserialize,
)]
pub struct TokenHolding {
    pub definition_id: [u8; 32],
    pub balance:       u128,
}

impl TokenHolding {
    pub fn new_zero(definition_id: [u8; 32]) -> Self {
        Self { definition_id, balance: 0 }
    }

    pub fn from_bytes(bytes: &[u8]) -> Result<Self, std::io::Error> {
        borsh::from_slice(bytes)
    }

    pub fn to_bytes(&self) -> Vec<u8> {
        borsh::to_vec(self).expect("TokenHolding serialisation cannot fail")
    }
}

pub fn apply_mint(
    def: &TokenDef,
    holding: &TokenHolding,
    amount: u128,
) -> Result<(TokenDef, TokenHolding), TokenError> {
    if amount == 0 {
        return Err(TokenError::ZeroAmount);
    }
    if holding.definition_id != def_id_placeholder(def) {
        return Err(TokenError::WrongDefinition);
    }
    let new_supply = def
        .total_supply
        .checked_add(amount)
        .ok_or(TokenError::SupplyOverflow)?;
    let new_balance = holding
        .balance
        .checked_add(amount)
        .ok_or(TokenError::BalanceOverflow)?;

    let mut new_def = def.clone();
    new_def.total_supply = new_supply;

    let mut new_holding = holding.clone();
    new_holding.balance = new_balance;

    Ok((new_def, new_holding))
}

pub fn apply_transfer(
    def: &TokenDef,
    sender: &TokenHolding,
    recipient: &TokenHolding,
    amount: u128,
) -> Result<(TokenHolding, TokenHolding), TokenError> {
    if amount == 0 {
        return Err(TokenError::ZeroAmount);
    }
    if sender.definition_id != def_id_placeholder(def)
        || recipient.definition_id != def_id_placeholder(def)
    {
        return Err(TokenError::WrongDefinition);
    }
    let new_sender_balance = sender
        .balance
        .checked_sub(amount)
        .ok_or(TokenError::InsufficientFunds)?;
    let new_recipient_balance = recipient
        .balance
        .checked_add(amount)
        .ok_or(TokenError::BalanceOverflow)?;

    let mut new_sender = sender.clone();
    new_sender.balance = new_sender_balance;

    let mut new_recipient = recipient.clone();
    new_recipient.balance = new_recipient_balance;

    Ok((new_sender, new_recipient))
}

pub fn apply_burn(
    def: &TokenDef,
    holding: &TokenHolding,
    amount: u128,
) -> Result<(TokenDef, TokenHolding), TokenError> {
    if amount == 0 {
        return Err(TokenError::ZeroAmount);
    }
    if holding.definition_id != def_id_placeholder(def) {
        return Err(TokenError::WrongDefinition);
    }
    let new_balance = holding
        .balance
        .checked_sub(amount)
        .ok_or(TokenError::InsufficientFunds)?;
    let new_supply = def
        .total_supply
        .checked_sub(amount)
        .ok_or(TokenError::SupplyUnderflow)?;

    let mut new_def = def.clone();
    new_def.total_supply = new_supply;

    let mut new_holding = holding.clone();
    new_holding.balance = new_balance;

    Ok((new_def, new_holding))
}

// In the guest, definition_id comes from AccountWithMetadata.
// Tests use this sentinel so helpers are testable without SPEL deps.
fn def_id_placeholder(_def: &TokenDef) -> [u8; 32] {
    TEST_DEF_ID
}

pub const TEST_DEF_ID: [u8; 32] = [0xdeu8; 32];

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TokenError {
    SupplyOverflow,
    BalanceOverflow,
    BalanceUnderflow,
    SupplyUnderflow,
    WrongDefinition,
    ZeroAmount,
    InsufficientFunds,
}

impl TokenError {
    pub fn code(&self) -> u32 {
        match self {
            Self::SupplyOverflow    => E_SUPPLY_OVERFLOW,
            Self::BalanceOverflow   => E_BALANCE_OVERFLOW,
            Self::BalanceUnderflow  => E_BALANCE_UNDERFLOW,
            Self::SupplyUnderflow   => E_SUPPLY_UNDERFLOW,
            Self::WrongDefinition   => E_WRONG_DEFINITION,
            Self::ZeroAmount        => E_ZERO_AMOUNT,
            Self::InsufficientFunds => E_INSUFFICIENT_FUNDS,
        }
    }

    pub fn message(&self) -> &'static str {
        match self {
            Self::SupplyOverflow    => "total supply overflow",
            Self::BalanceOverflow   => "balance overflow",
            Self::BalanceUnderflow  => "balance underflow",
            Self::SupplyUnderflow   => "total supply underflow",
            Self::WrongDefinition   => "token holding belongs to a different definition",
            Self::ZeroAmount        => "amount must be greater than zero",
            Self::InsufficientFunds => "insufficient token balance",
        }
    }
}

impl std::fmt::Display for TokenError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "[{}] {}", self.code(), self.message())
    }
}

impl std::error::Error for TokenError {}

#[cfg(test)]
mod tests {
    use super::*;

    fn def(supply: u128, authority: Option<[u8; 32]>) -> TokenDef {
        TokenDef {
            name:           "TEST".into(),
            decimals:       6,
            total_supply:   supply,
            mint_authority: authority,
        }
    }

    fn holding(balance: u128) -> TokenHolding {
        TokenHolding { definition_id: TEST_DEF_ID, balance }
    }

    #[test]
    fn mint_increases_balance_and_supply() {
        let (new_def, new_h) = apply_mint(&def(100, Some([1u8; 32])), &holding(50), 25).unwrap();
        assert_eq!(new_def.total_supply, 125);
        assert_eq!(new_h.balance, 75);
    }

    #[test]
    fn mint_rejects_zero() {
        assert_eq!(
            apply_mint(&def(100, Some([1u8; 32])), &holding(50), 0),
            Err(TokenError::ZeroAmount)
        );
    }

    #[test]
    fn mint_rejects_supply_overflow() {
        assert_eq!(
            apply_mint(&def(u128::MAX, Some([1u8; 32])), &holding(0), 1),
            Err(TokenError::SupplyOverflow)
        );
    }

    #[test]
    fn transfer_moves_balance() {
        let (s, r) = apply_transfer(&def(100, None), &holding(80), &holding(20), 30).unwrap();
        assert_eq!(s.balance, 50);
        assert_eq!(r.balance, 50);
    }

    #[test]
    fn transfer_rejects_insufficient_funds() {
        assert_eq!(
            apply_transfer(&def(100, None), &holding(10), &holding(0), 11),
            Err(TokenError::InsufficientFunds)
        );
    }

    #[test]
    fn transfer_rejects_zero() {
        assert_eq!(
            apply_transfer(&def(100, None), &holding(10), &holding(0), 0),
            Err(TokenError::ZeroAmount)
        );
    }

    #[test]
    fn burn_reduces_balance_and_supply() {
        let (new_def, new_h) = apply_burn(&def(100, None), &holding(60), 40).unwrap();
        assert_eq!(new_def.total_supply, 60);
        assert_eq!(new_h.balance, 20);
    }

    #[test]
    fn burn_rejects_insufficient_balance() {
        assert_eq!(
            apply_burn(&def(100, None), &holding(5), 10),
            Err(TokenError::InsufficientFunds)
        );
    }

    #[test]
    fn burn_rejects_zero() {
        assert_eq!(
            apply_burn(&def(100, None), &holding(50), 0),
            Err(TokenError::ZeroAmount)
        );
    }

    #[test]
    fn tokendef_roundtrip() {
        let d = def(999, Some([7u8; 32]));
        let decoded = TokenDef::from_bytes(&d.to_bytes()).unwrap();
        assert_eq!(d, decoded);
    }

    #[test]
    fn tokenholding_roundtrip() {
        let h = holding(42);
        let decoded = TokenHolding::from_bytes(&h.to_bytes()).unwrap();
        assert_eq!(h, decoded);
    }
}

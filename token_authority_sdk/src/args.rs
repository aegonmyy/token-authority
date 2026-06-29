use borsh::{BorshDeserialize, BorshSerialize};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, BorshSerialize, BorshDeserialize, Serialize, Deserialize)]
pub struct NewFungibleTokenArgs {
    pub name: String,
    pub decimals: u8,
    pub initial_supply: u128,
    /// 32-byte account id of the initial mint authority, or `None` for fixed supply.
    pub mint_authority: Option<[u8; 32]>,
}

impl NewFungibleTokenArgs {
    /// Encode `mint_authority` as the `Vec<u8>` the guest expects.
    pub fn mint_authority_bytes(&self) -> Vec<u8> {
        match self.mint_authority {
            None     => vec![],
            Some(id) => id.to_vec(),
        }
    }
}

#[derive(Debug, Clone, BorshSerialize, BorshDeserialize, Serialize, Deserialize)]
pub struct MintTokensArgs {
    pub amount: u128,
}

#[derive(Debug, Clone, BorshSerialize, BorshDeserialize, Serialize, Deserialize)]
pub struct TransferTokensArgs {
    pub amount: u128,
}

#[derive(Debug, Clone, BorshSerialize, BorshDeserialize, Serialize, Deserialize)]
pub struct BurnTokensArgs {
    pub amount: u128,
}

#[derive(Debug, Clone, BorshSerialize, BorshDeserialize, Serialize, Deserialize)]
pub struct RotateAuthorityArgs {
    pub new_authority: [u8; 32],
}

#[derive(Debug, Clone, BorshSerialize, BorshDeserialize, Serialize, Deserialize)]
pub struct RevokeAuthorityArgs;

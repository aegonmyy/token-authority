//! PDA seed constants — must stay in sync with the guest program.

/// Seed for the token-definition account (`TokenDef`).
pub const TOKEN_DEF_SEED: &[u8] = b"token_def";

/// Seed for the mint-authority config account (`AdminConfig`).
pub const MINT_AUTH_SEED: &[u8] = b"mint_auth";

/// Derive a fake in-process "account id" by hashing seeds with a program id.
///
/// Real LEZ PDA derivation is done by the runtime; this helper exists only for
/// local simulation tests where we need a stable, deterministic id.
pub fn derive_pda_id(program_id: &[u8; 32], seeds: &[&[u8]]) -> [u8; 32] {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};

    let mut h = DefaultHasher::new();
    program_id.hash(&mut h);
    for s in seeds {
        s.hash(&mut h);
    }
    let v = h.finish();
    let mut out = [0u8; 32];
    out[..8].copy_from_slice(&v.to_le_bytes());
    out[8..16].copy_from_slice(&v.to_be_bytes());
    out
}

//! Seed constants and deterministic id derivation for the simulated ledger.

pub const TOKEN_DEF_SEED:  &[u8] = b"token_def";
pub const MINT_AUTH_SEED:  &[u8] = b"mint_auth";

/// Derive a stable in-process account id for simulation tests.
///
/// The real LEZ runtime handles account id derivation; this exists only for
/// `SimulatedLedger` where we need deterministic ids without a network.
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

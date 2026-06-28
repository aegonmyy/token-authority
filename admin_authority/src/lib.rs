//! RFP-001 admin-authority library.
//!
//! A single shared primitive for SPEL programs that need one privileged
//! administrator. Stores `AdminConfig { admin: Option<[u8; 32]> }` in a PDA
//! and exposes a gate that rejects callers who are not the stored administrator.
//!
//! # Error code namespace
//! All errors use the 10xx range so they compose cleanly alongside other
//! authority libraries without collision.
//!
//! # Usage in a SPEL program
//! ```ignore
//! use admin_authority::{AdminConfig, require_admin, AdminError};
//!
//! #[instruction]
//! pub fn privileged_action(
//!     #[account(mut, pda = [literal("admin_config")])] config_acc: AccountWithMetadata,
//!     #[account(signer)] caller: AccountWithMetadata,
//! ) -> SpelResult {
//!     let config = AdminConfig::from_account(&config_acc)?;
//!     require_admin(&config, caller.account_id.value())?;
//!     // ... do privileged work ...
//! }
//! ```

use borsh::{BorshDeserialize, BorshSerialize};
use serde::{Deserialize, Serialize};

// ── Error codes (10xx namespace) ─────────────────────────────────────────────

/// Caller is not the stored administrator.
pub const E_UNAUTHORIZED: u32 = 1001;
/// Config PDA already initialised — call `initialize_admin` only once.
pub const E_ALREADY_INITIALIZED: u32 = 1002;
/// Admin authority has been permanently revoked (`admin == None`).
pub const E_REVOKED: u32 = 1003;
/// Cannot set admin to the all-zeros account id (null authority guard).
pub const E_NULL_AUTHORITY: u32 = 1004;

// ── State ────────────────────────────────────────────────────────────────────

/// On-chain administrator configuration stored in the program's config PDA.
///
/// `admin == None` means the authority has been permanently revoked; no further
/// privileged instructions can execute.
#[derive(
    Debug, Clone, PartialEq, Eq,
    Serialize, Deserialize,
    BorshSerialize, BorshDeserialize,
)]
pub struct AdminConfig {
    pub admin: Option<[u8; 32]>,
}

impl AdminConfig {
    /// Create a new config with `admin` as the initial administrator.
    ///
    /// Returns `Err(E_NULL_AUTHORITY)` if `admin` is the all-zeros id.
    pub fn new(admin: [u8; 32]) -> Result<Self, AdminError> {
        if admin == [0u8; 32] {
            return Err(AdminError::NullAuthority);
        }
        Ok(Self { admin: Some(admin) })
    }

    /// Deserialise from raw Borsh bytes (account data slice).
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, std::io::Error> {
        borsh::from_slice(bytes)
    }

    /// Serialise to Borsh bytes.
    pub fn to_bytes(&self) -> Vec<u8> {
        borsh::to_vec(self).expect("AdminConfig serialisation cannot fail")
    }
}

// ── Error type ───────────────────────────────────────────────────────────────

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AdminError {
    Unauthorized,
    AlreadyInitialized,
    Revoked,
    NullAuthority,
}

impl AdminError {
    pub fn code(&self) -> u32 {
        match self {
            Self::Unauthorized       => E_UNAUTHORIZED,
            Self::AlreadyInitialized => E_ALREADY_INITIALIZED,
            Self::Revoked            => E_REVOKED,
            Self::NullAuthority      => E_NULL_AUTHORITY,
        }
    }

    pub fn message(&self) -> &'static str {
        match self {
            Self::Unauthorized       => "signer is not the admin authority",
            Self::AlreadyInitialized => "admin authority already initialized",
            Self::Revoked            => "admin authority has been revoked",
            Self::NullAuthority      => "admin cannot be the null (all-zero) account id",
        }
    }
}

impl std::fmt::Display for AdminError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "[{}] {}", self.code(), self.message())
    }
}

impl std::error::Error for AdminError {}

// ── Gate ─────────────────────────────────────────────────────────────────────

/// Assert that `signer` is the current administrator.
///
/// - Returns `Ok(())` if `config.admin == Some(signer)`.
/// - Returns `Err(Revoked)` if `config.admin == None`.
/// - Returns `Err(Unauthorized)` if the signer does not match.
pub fn require_admin(config: &AdminConfig, signer: &[u8; 32]) -> Result<(), AdminError> {
    match &config.admin {
        None        => Err(AdminError::Revoked),
        Some(admin) => {
            if admin != signer {
                Err(AdminError::Unauthorized)
            } else {
                Ok(())
            }
        }
    }
}

// ── State transitions ────────────────────────────────────────────────────────

/// Initialise a fresh `AdminConfig`.
///
/// Fails if `existing` already contains data (already-initialised guard) or if
/// `admin` is the null account id.
pub fn initialize_admin(
    existing_bytes: &[u8],
    admin: [u8; 32],
) -> Result<AdminConfig, AdminError> {
    if !existing_bytes.is_empty() {
        return Err(AdminError::AlreadyInitialized);
    }
    AdminConfig::new(admin)
}

/// Transfer admin authority to `new_admin`.
///
/// Requires the current `signer` to be the administrator.
/// Fails if `new_admin` is the null account id (use `revoke_admin` instead).
pub fn transfer_admin(
    config: &AdminConfig,
    signer: &[u8; 32],
    new_admin: [u8; 32],
) -> Result<AdminConfig, AdminError> {
    require_admin(config, signer)?;
    if new_admin == [0u8; 32] {
        return Err(AdminError::NullAuthority);
    }
    Ok(AdminConfig { admin: Some(new_admin) })
}

/// Permanently revoke admin authority.
///
/// Requires the current `signer` to be the administrator.
/// After revocation `admin == None` and no further privileged calls can proceed.
pub fn revoke_admin(
    config: &AdminConfig,
    signer: &[u8; 32],
) -> Result<AdminConfig, AdminError> {
    require_admin(config, signer)?;
    Ok(AdminConfig { admin: None })
}

// ── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    const ALICE: [u8; 32] = [1u8; 32];
    const BOB:   [u8; 32] = [2u8; 32];
    const NULL:  [u8; 32] = [0u8; 32];

    // ── AdminConfig::new ──────────────────────────────────────────────────

    #[test]
    fn new_accepts_non_null_admin() {
        let cfg = AdminConfig::new(ALICE).unwrap();
        assert_eq!(cfg.admin, Some(ALICE));
    }

    #[test]
    fn new_rejects_null_admin() {
        assert_eq!(AdminConfig::new(NULL), Err(AdminError::NullAuthority));
    }

    // ── require_admin ─────────────────────────────────────────────────────

    #[test]
    fn require_admin_passes_for_admin() {
        let cfg = AdminConfig::new(ALICE).unwrap();
        assert!(require_admin(&cfg, &ALICE).is_ok());
    }

    #[test]
    fn require_admin_rejects_non_admin() {
        let cfg = AdminConfig::new(ALICE).unwrap();
        assert_eq!(require_admin(&cfg, &BOB), Err(AdminError::Unauthorized));
    }

    #[test]
    fn require_admin_rejects_revoked() {
        let cfg = AdminConfig { admin: None };
        assert_eq!(require_admin(&cfg, &ALICE), Err(AdminError::Revoked));
    }

    // ── initialize_admin ──────────────────────────────────────────────────

    #[test]
    fn initialize_sets_admin() {
        let cfg = initialize_admin(&[], ALICE).unwrap();
        assert_eq!(cfg.admin, Some(ALICE));
    }

    #[test]
    fn initialize_rejects_already_initialized() {
        let existing = borsh::to_vec(&AdminConfig::new(ALICE).unwrap()).unwrap();
        assert_eq!(
            initialize_admin(&existing, BOB),
            Err(AdminError::AlreadyInitialized)
        );
    }

    #[test]
    fn initialize_rejects_null_admin() {
        assert_eq!(initialize_admin(&[], NULL), Err(AdminError::NullAuthority));
    }

    // ── transfer_admin ────────────────────────────────────────────────────

    #[test]
    fn transfer_changes_admin() {
        let cfg = AdminConfig::new(ALICE).unwrap();
        let updated = transfer_admin(&cfg, &ALICE, BOB).unwrap();
        assert_eq!(updated.admin, Some(BOB));
    }

    #[test]
    fn transfer_rejects_non_admin_signer() {
        let cfg = AdminConfig::new(ALICE).unwrap();
        assert_eq!(transfer_admin(&cfg, &BOB, BOB), Err(AdminError::Unauthorized));
    }

    #[test]
    fn transfer_rejects_null_new_admin() {
        let cfg = AdminConfig::new(ALICE).unwrap();
        assert_eq!(transfer_admin(&cfg, &ALICE, NULL), Err(AdminError::NullAuthority));
    }

    #[test]
    fn transfer_rejects_revoked_config() {
        let cfg = AdminConfig { admin: None };
        assert_eq!(transfer_admin(&cfg, &ALICE, BOB), Err(AdminError::Revoked));
    }

    // ── revoke_admin ──────────────────────────────────────────────────────

    #[test]
    fn revoke_sets_admin_to_none() {
        let cfg = AdminConfig::new(ALICE).unwrap();
        let revoked = revoke_admin(&cfg, &ALICE).unwrap();
        assert_eq!(revoked.admin, None);
    }

    #[test]
    fn revoke_rejects_non_admin_signer() {
        let cfg = AdminConfig::new(ALICE).unwrap();
        assert_eq!(revoke_admin(&cfg, &BOB), Err(AdminError::Unauthorized));
    }

    #[test]
    fn revoke_rejects_already_revoked() {
        let cfg = AdminConfig { admin: None };
        assert_eq!(revoke_admin(&cfg, &ALICE), Err(AdminError::Revoked));
    }

    // ── Borsh round-trip ──────────────────────────────────────────────────

    #[test]
    fn borsh_roundtrip_some() {
        let cfg = AdminConfig::new(ALICE).unwrap();
        let bytes = cfg.to_bytes();
        let decoded = AdminConfig::from_bytes(&bytes).unwrap();
        assert_eq!(cfg, decoded);
    }

    #[test]
    fn borsh_roundtrip_none() {
        let cfg = AdminConfig { admin: None };
        let bytes = cfg.to_bytes();
        let decoded = AdminConfig::from_bytes(&bytes).unwrap();
        assert_eq!(cfg, decoded);
    }

    // ── Full lifecycle ────────────────────────────────────────────────────

    #[test]
    fn full_admin_lifecycle() {
        // 1. initialize with ALICE
        let cfg = initialize_admin(&[], ALICE).unwrap();
        assert_eq!(cfg.admin, Some(ALICE));

        // 2. ALICE transfers to BOB
        let cfg = transfer_admin(&cfg, &ALICE, BOB).unwrap();
        assert_eq!(cfg.admin, Some(BOB));

        // 3. ALICE can no longer act
        assert_eq!(require_admin(&cfg, &ALICE), Err(AdminError::Unauthorized));

        // 4. BOB revokes
        let cfg = revoke_admin(&cfg, &BOB).unwrap();
        assert_eq!(cfg.admin, None);

        // 5. nobody can act after revocation
        assert_eq!(require_admin(&cfg, &BOB), Err(AdminError::Revoked));
        assert_eq!(require_admin(&cfg, &ALICE), Err(AdminError::Revoked));
    }
}

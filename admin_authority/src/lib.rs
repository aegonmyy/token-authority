use borsh::{BorshDeserialize, BorshSerialize};
use serde::{Deserialize, Serialize};

pub const E_UNAUTHORIZED: u32 = 1001;
pub const E_ALREADY_INITIALIZED: u32 = 1002;
pub const E_REVOKED: u32 = 1003;
pub const E_NULL_AUTHORITY: u32 = 1004;

/// Stores the current mint authority. `admin == None` means permanently revoked.
#[derive(
    Debug, Clone, PartialEq, Eq,
    Serialize, Deserialize,
    BorshSerialize, BorshDeserialize,
)]
pub struct AdminConfig {
    pub admin: Option<[u8; 32]>,
}

impl AdminConfig {
    pub fn new(admin: [u8; 32]) -> Result<Self, AdminError> {
        if admin == [0u8; 32] {
            return Err(AdminError::NullAuthority);
        }
        Ok(Self { admin: Some(admin) })
    }

    pub fn from_bytes(bytes: &[u8]) -> Result<Self, std::io::Error> {
        borsh::from_slice(bytes)
    }

    pub fn to_bytes(&self) -> Vec<u8> {
        borsh::to_vec(self).expect("AdminConfig serialisation cannot fail")
    }
}

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

/// Returns `Err(Revoked)` if admin is None, `Err(Unauthorized)` if signer doesn't match.
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

/// Initialise a fresh `AdminConfig`. Fails if `existing` is non-empty or `admin` is null.
pub fn initialize_admin(
    existing_bytes: &[u8],
    admin: [u8; 32],
) -> Result<AdminConfig, AdminError> {
    if !existing_bytes.is_empty() {
        return Err(AdminError::AlreadyInitialized);
    }
    AdminConfig::new(admin)
}

/// Transfer admin authority to `new_admin`. Signer must be current admin.
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

/// Permanently revoke admin authority. Signer must be current admin.
pub fn revoke_admin(
    config: &AdminConfig,
    signer: &[u8; 32],
) -> Result<AdminConfig, AdminError> {
    require_admin(config, signer)?;
    Ok(AdminConfig { admin: None })
}

#[cfg(test)]
mod tests {
    use super::*;

    const ALICE: [u8; 32] = [1u8; 32];
    const BOB:   [u8; 32] = [2u8; 32];
    const NULL:  [u8; 32] = [0u8; 32];

    #[test]
    fn new_accepts_non_null_admin() {
        let cfg = AdminConfig::new(ALICE).unwrap();
        assert_eq!(cfg.admin, Some(ALICE));
    }

    #[test]
    fn new_rejects_null_admin() {
        assert_eq!(AdminConfig::new(NULL), Err(AdminError::NullAuthority));
    }

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

    #[test]
    fn full_admin_lifecycle() {
        let cfg = initialize_admin(&[], ALICE).unwrap();
        assert_eq!(cfg.admin, Some(ALICE));

        let cfg = transfer_admin(&cfg, &ALICE, BOB).unwrap();
        assert_eq!(cfg.admin, Some(BOB));

        assert_eq!(require_admin(&cfg, &ALICE), Err(AdminError::Unauthorized));

        let cfg = revoke_admin(&cfg, &BOB).unwrap();
        assert_eq!(cfg.admin, None);

        assert_eq!(require_admin(&cfg, &BOB), Err(AdminError::Revoked));
        assert_eq!(require_admin(&cfg, &ALICE), Err(AdminError::Revoked));
    }
}

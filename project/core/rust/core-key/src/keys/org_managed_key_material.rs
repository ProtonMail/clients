use lattice::core::LtCoreUnprivActivationToken;
use proton_crypto_account::keys::LocalUserKey;

/// Org-managed `POST /keys/setup` extras: activation token + org-primary user key.
pub struct OrgManagedKeyMaterial {
    pub activation_token: LtCoreUnprivActivationToken,
    pub primary_user_key: LocalUserKey,
}

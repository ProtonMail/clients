use lattice::Sensitive;
use proton_crypto_account::keys::LocalUserKey;

/// Org-managed `POST /keys/setup` extras: activation token + org-primary user key.
pub struct OrgManagedKeyMaterial {
    pub activation_token: Sensitive<String>,
    pub primary_user_key: LocalUserKey,
}

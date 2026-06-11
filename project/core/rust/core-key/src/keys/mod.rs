mod locked_keys;
mod new_addr_key;
mod new_user_key;
mod org_managed_key_material;

pub use locked_keys::LockedKeysExt;
pub use new_addr_key::NewAddrKey;
pub use new_user_key::AddressMetadata;
pub use new_user_key::NewUserKey;
pub use org_managed_key_material::OrgManagedKeyMaterial;

/// Address key for the primary address (lowest `order`), if any.
pub fn primary_addr_key(addr_keys: &[NewAddrKey]) -> Option<&NewAddrKey> {
    addr_keys.iter().min_by_key(|key| key.address.order)
}

pub(crate) fn new_key_id() -> proton_crypto_account::keys::KeyId {
    proton_crypto_account::keys::KeyId(String::default())
}

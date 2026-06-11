use proton_crypto_account::keys::{KeyId, LockedKey};

pub trait LockedKeysExt {
    fn primary_key(&self) -> Option<&LockedKey>;
    fn primary_key_id(&self) -> Option<&KeyId>;
}

impl<T: AsRef<[LockedKey]>> LockedKeysExt for T {
    fn primary_key(&self) -> Option<&LockedKey> {
        self.as_ref().iter().find(|key| key.primary)
    }

    fn primary_key_id(&self) -> Option<&KeyId> {
        self.primary_key().map(|key| &key.id)
    }
}

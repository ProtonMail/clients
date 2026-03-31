use crate::{
    error::KeyHandlingError,
    ids::{AddressId, UserId},
};
use parking_lot::RwLock;
use proton_crypto_account::{
    keys::{
        DecryptedAddressKey, DecryptedUserKey, KeyFlag, KeyId, UnlockedAddressKey,
        UnlockedAddressKeys, UnlockedUserKey, UnlockedUserKeys,
    },
    proton_crypto::crypto::{DataEncoding, PGPProviderSync},
};
use secrecy::{ExposeSecret, SecretSlice};
use std::{
    collections::HashMap,
    sync::Arc,
    time::{Duration, Instant},
};

/// The default lifetime of user keys in the cache.
pub const USER_KEY_LIFETIME: Duration = Duration::from_secs(600);

/// The default lifetime of address keys in the cache.
pub const ADDRESS_KEY_LIFETIME: Duration = Duration::from_secs(300);

type CachedUserKeys = Vec<CachedUserKey>;
type CachedAddressKeys = Vec<CachedAddressKey>;

/// Represents a cached user key independent of the PGP provider.
pub(crate) struct CachedUserKey {
    id: KeyId,
    private_key: SecretSlice<u8>,
}

impl CachedUserKey {
    /// Tries to create a [`CachedUserKey`] from an [`UnlockedUserKey`].
    pub fn new<P>(pgp: &P, key: &UnlockedUserKey<P>) -> Result<CachedUserKey, KeyHandlingError>
    where
        P: PGPProviderSync,
    {
        let exported_key =
            pgp.private_key_export_unlocked(&key.private_key, DataEncoding::Bytes)?;

        Ok(CachedUserKey {
            id: key.id.clone(),
            private_key: SecretSlice::new(exported_key.as_ref().into()),
        })
    }

    /// Tries to transform a [`CachedUserKey`] into a [`UnlockedUserKey`].
    pub fn to_unlocked_key<P>(&self, pgp: &P) -> Result<UnlockedUserKey<P>, KeyHandlingError>
    where
        P: PGPProviderSync,
    {
        let imported_key =
            pgp.private_key_import_unlocked(self.private_key.expose_secret(), DataEncoding::Bytes)?;

        let public_key = pgp.private_key_to_public_key(&imported_key)?;

        Ok(DecryptedUserKey {
            id: self.id.clone(),
            private_key: imported_key,
            public_key,
        })
    }
}

/// Represents a cached address key independent of the PGP provider.
pub(crate) struct CachedAddressKey {
    id: KeyId,
    flags: KeyFlag,
    primary: bool,
    is_v6: bool,
    private_key: SecretSlice<u8>,
}

impl CachedAddressKey {
    /// Tries to create a [`CachedAddressKey`] from an [`UnlockedAddressKey`].
    pub fn new<P>(
        pgp: &P,
        key: &UnlockedAddressKey<P>,
    ) -> Result<CachedAddressKey, KeyHandlingError>
    where
        P: PGPProviderSync,
    {
        let exported_key =
            pgp.private_key_export_unlocked(&key.private_key, DataEncoding::Bytes)?;

        Ok(CachedAddressKey {
            id: key.id.clone(),
            flags: key.flags,
            primary: key.primary,
            is_v6: key.is_v6,
            private_key: SecretSlice::new(exported_key.as_ref().into()),
        })
    }

    /// Tries to transform a [`CachedAddressKey`] into a [`UnlockedAddressKey`].
    pub fn to_unlocked_key<P>(&self, pgp: &P) -> Result<UnlockedAddressKey<P>, KeyHandlingError>
    where
        P: PGPProviderSync,
    {
        let imported_key =
            pgp.private_key_import_unlocked(self.private_key.expose_secret(), DataEncoding::Bytes)?;

        let public_key = pgp.private_key_to_public_key(&imported_key)?;

        Ok(DecryptedAddressKey {
            id: self.id.clone(),
            flags: self.flags,
            primary: self.primary,
            is_v6: self.is_v6,
            public_key,
            private_key: imported_key,
        })
    }
}

pub(crate) struct CacheOption<T>(Option<(Instant, Arc<T>)>);

impl<T> CacheOption<T> {
    pub fn new(item: T) -> Self {
        Self(Some((Instant::now(), Arc::new(item))))
    }

    pub fn get(&self, lifetime: Duration) -> Option<Arc<T>> {
        match &self.0 {
            Some((instant, value)) if instant.elapsed() <= lifetime => Some(Arc::clone(value)),
            _ => None,
        }
    }
}

/// Caches unlocked user and address keys for one account in memory.
///
/// # Security
///
/// Keys are only removed from memory if replaced or an exteranl caller calls [`Self::clear`] on the cache.
pub struct MemoryKeyCache {
    user_key_lifetime: Duration,
    address_key_lifetime: Duration,
    user_keys: RwLock<HashMap<UserId, CacheOption<CachedUserKeys>>>,
    address_keys: RwLock<HashMap<AddressId, CacheOption<CachedAddressKeys>>>,
}

impl MemoryKeyCache {
    #[must_use]
    pub fn new(user_key_lifetime: Duration, address_key_lifetime: Duration) -> Self {
        Self {
            user_key_lifetime,
            address_key_lifetime,
            user_keys: RwLock::new(HashMap::new()),
            address_keys: RwLock::new(HashMap::new()),
        }
    }
}

impl Default for MemoryKeyCache {
    fn default() -> Self {
        Self::new(USER_KEY_LIFETIME, ADDRESS_KEY_LIFETIME)
    }
}

impl MemoryKeyCache {
    pub(crate) fn get_user_keys(&self, user_id: &UserId) -> Option<Arc<CachedUserKeys>> {
        self.user_keys
            .read()
            .get(user_id)
            .and_then(|opt| opt.get(self.user_key_lifetime))
    }

    pub(crate) fn store_user_keys(&self, user_id: &UserId, keys: CachedUserKeys) {
        self.user_keys
            .write()
            .insert(user_id.clone(), CacheOption::new(keys));
    }

    pub(crate) fn get_address_keys(
        &self,
        address_id: &AddressId,
    ) -> Option<Arc<CachedAddressKeys>> {
        self.address_keys
            .read()
            .get(address_id)
            .and_then(|opt| opt.get(self.address_key_lifetime))
    }

    pub(crate) fn store_address_keys(&self, address_id: &AddressId, keys: CachedAddressKeys) {
        self.address_keys
            .write()
            .insert(address_id.clone(), CacheOption::new(keys));
    }

    pub fn clear(&self) {
        self.user_keys.write().clear();
        self.address_keys.write().clear();
    }
}

/// Caches unlocked user and address keys for one or more accounts.
#[derive(Clone)]
pub(crate) struct WrappedKeyCache<'a> {
    storage: &'a MemoryKeyCache,
}

impl<'a> WrappedKeyCache<'a> {
    pub fn new(storage: &'a MemoryKeyCache) -> Self {
        Self { storage }
    }

    /// Returns cached user keys, or `None` on a cache miss.
    pub fn get_user_keys<P>(
        &self,
        pgp: &P,
        user_id: &UserId,
    ) -> crate::Result<Option<UnlockedUserKeys<P>>>
    where
        P: PGPProviderSync,
    {
        let Some(cached) = self.storage.get_user_keys(user_id) else {
            return Ok(None);
        };
        Ok(Some(to_unlocked_user_keys(pgp, cached.as_ref())?))
    }

    /// Stores unlocked user keys in the cache.
    pub fn store_user_keys<P>(
        &self,
        pgp: &P,
        user_id: &UserId,
        keys: &[UnlockedUserKey<P>],
    ) -> crate::Result<()>
    where
        P: PGPProviderSync,
    {
        let cached = keys
            .iter()
            .map(|k| CachedUserKey::new(pgp, k))
            .collect::<crate::Result<_>>()?;
        self.storage.store_user_keys(user_id, cached);
        Ok(())
    }

    /// Returns cached address keys for `address_id`, or `None` on a cache miss.
    pub fn get_address_keys<P>(
        &self,
        pgp: &P,
        address_id: &AddressId,
    ) -> crate::Result<Option<UnlockedAddressKeys<P>>>
    where
        P: PGPProviderSync,
    {
        let Some(cached) = self.storage.get_address_keys(address_id) else {
            return Ok(None);
        };
        Ok(Some(to_unlocked_address_keys(pgp, cached.as_ref())?))
    }

    /// Stores unlocked address keys in the cache.
    pub fn store_address_keys<P>(
        &self,
        pgp: &P,
        address_id: &AddressId,
        keys: &[UnlockedAddressKey<P>],
    ) -> crate::Result<()>
    where
        P: PGPProviderSync,
    {
        let cached = keys
            .iter()
            .map(|k| CachedAddressKey::new(pgp, k))
            .collect::<crate::Result<_>>()?;
        self.storage.store_address_keys(address_id, cached);
        Ok(())
    }
}

impl<'a> From<&'a MemoryKeyCache> for WrappedKeyCache<'a> {
    fn from(storage: &'a MemoryKeyCache) -> Self {
        Self::new(storage)
    }
}

impl<'a> From<&'a Arc<MemoryKeyCache>> for WrappedKeyCache<'a> {
    fn from(storage: &'a Arc<MemoryKeyCache>) -> Self {
        Self::new(storage.as_ref())
    }
}

fn to_unlocked_user_keys<P>(pgp: &P, cached: &CachedUserKeys) -> crate::Result<UnlockedUserKeys<P>>
where
    P: PGPProviderSync,
{
    cached
        .iter()
        .map(|k| k.to_unlocked_key(pgp))
        .collect::<crate::Result<Vec<_>>>()
        .map(Into::into)
}

fn to_unlocked_address_keys<P>(
    pgp: &P,
    cached: &CachedAddressKeys,
) -> crate::Result<UnlockedAddressKeys<P>>
where
    P: PGPProviderSync,
{
    cached
        .iter()
        .map(|k| k.to_unlocked_key(pgp))
        .collect::<crate::Result<Vec<_>>>()
        .map(Into::into)
}

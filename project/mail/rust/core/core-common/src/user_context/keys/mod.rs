//! Contains the logic to access PGP keys from the `UserContext`.
mod cache;
mod manager;

use crate::{CoreContextError, CoreContextResult, UserContext};
use cache::{CachedAddressKey, CachedUserKey};
use mail_core_api::auth::UserKeySecret;
use mail_core_api::services::proton::PrivateEmailRef;
use mail_core_api::{services::proton::AddressId, session::Session};
use mail_stash::{UserDb, stash::RunTransaction, stash::Tether};
pub use manager::*;
use proton_crypto_account::{
    keys::{PinnedPublicKeys, PublicAddressKeys, UnlockedAddressKeys, UnlockedUserKeys},
    proton_crypto::{CryptoError, crypto::PGPProviderSync},
};
use thiserror::Error;

#[allow(clippy::module_name_repetitions)]
type CachedUserKeys = Vec<CachedUserKey>;
#[allow(clippy::module_name_repetitions)]
type CachedAddressKeys = Vec<CachedAddressKey>;

/// Result for key handling operations.
pub type KeyHandlingResult<T> = Result<T, KeyHandlingError>;

/// An error type that is thrown when loading keys
/// via the [`CryptoKeyManager`].
#[derive(Debug, Error)]
pub enum KeyHandlingError {
    #[error("No user found")]
    NoUser,
    #[error("No user secret found")]
    NoUserSecret,
    #[error("No user keys unlocked but has {0} user keys")]
    UserKeyUnlock(usize),
    #[error("Failed to store user keys in the cache {0}")]
    UserKeyCacheStore(#[from] CryptoError),
    #[error("No address found for id {0}")]
    NoAddress(AddressId),
    #[error("Failed to unlock at least one address key, but the user has {0} address keys")]
    AddressKeyUnlock(usize),
}

/// A trait that loads the user secret to unlock the user keys.
#[allow(async_fn_in_trait)]
pub trait LoadKeySecret {
    /// Loads the user secret to unlock the user keys.
    async fn key_secret(&self) -> Option<UserKeySecret>;
}

impl LoadKeySecret for Session {
    async fn key_secret(&self) -> Option<UserKeySecret> {
        self.expose_key_secret().await
    }
}

impl UserContext {
    /// Returns the unlocked user keys of this user.
    ///
    /// First tries to retrieve them from the cache else
    /// it loads and unlocks them from the database.
    pub async fn unlocked_user_keys<P, S>(
        &self,
        pgp: &P,
        conn: &Tether,
        secret_loader: &S,
    ) -> CoreContextResult<UnlockedUserKeys<P>>
    where
        P: PGPProviderSync,
        S: LoadKeySecret,
    {
        self.key_manager
            .user_keys(pgp, conn, secret_loader, &self.user_id)
            .await
    }

    /// Returns the unlocked address keys of this user for the given address.
    ///
    /// Loads the address keys from the database and unlocks them with the user keys.
    pub async fn unlocked_address_keys<P, S>(
        &self,
        pgp: &P,
        conn: &Tether,
        secret_loader: &S,
        address_id: &AddressId,
    ) -> CoreContextResult<UnlockedAddressKeys<P>>
    where
        P: PGPProviderSync,
        S: LoadKeySecret,
    {
        self.key_manager
            .address_keys(pgp, conn, secret_loader, &self.user_id, address_id)
            .await
    }

    /// Loads the public address keys for an email address from the backend API.
    ///
    /// Imports the keys with the PGP provider. In the future, this function will also
    /// verify the keys with key transparency.
    pub async fn public_address_keys<P>(
        &self,
        pgp: &P,
        email: PrivateEmailRef<'_>,
        internal_only: bool,
        fetch_policy: PublicAddressKeyFetchPolicy,
    ) -> CoreContextResult<PublicAddressKeys<<P>::PublicKey>>
    where
        P: PGPProviderSync,
    {
        self.key_manager
            .public_address_keys(pgp, email, internal_only, fetch_policy, self)
            .await
    }

    /// Loads the public address keys pinned to a user's contact, if any.
    ///
    /// Delegates to [`contacts_common::public_address_keys_from_contacts`].
    pub async fn public_address_keys_from_contacts<P>(
        &self,
        pgp: &P,
        tx: &mut impl RunTransaction<UserDb>,
        unlocked_user_keys: &UnlockedUserKeys<P>,
        email: PrivateEmailRef<'_>,
        fetch_policy: AddressKeysContactFetchPolicy,
    ) -> CoreContextResult<Option<PinnedPublicKeys<<P>::PublicKey>>>
    where
        P: PGPProviderSync,
    {
        contacts_common::public_address_keys_from_contacts(
            pgp,
            self.session(),
            tx,
            unlocked_user_keys,
            email,
            fetch_policy,
        )
        .await
        .map_err(CoreContextError::from)
    }
}

pub use contacts_common::{AddressKeysContactFetchPolicy, ContactCryptoError};

impl From<AddressKeysContactFetchPolicy> for PublicAddressKeyFetchPolicy {
    fn from(value: AddressKeysContactFetchPolicy) -> Self {
        match value {
            AddressKeysContactFetchPolicy::RequireSync => Self::RequireSync,
            AddressKeysContactFetchPolicy::AllowCachedFallback => Self::AllowCachedFallback,
        }
    }
}

use std::sync::Arc;

use async_trait::async_trait;
use proton_crypto_account::contacts::{ContactCardType, DecryptableVerifiableCard};
use proton_crypto_account::keys::{
    APIPublicAddressKeyGroup, APIPublicAddressKeys, AddressKeys, UserKeys,
};
use proton_crypto_account::salts::KeySecret;

use crate::cache::MemoryKeyCache;
use crate::error::LoadingResult;
use crate::ids::{AddressId, UserId};
use crate::policy::{PublicAddressKeyApiFetchPolicy, PublicAddressKeyContactFetchPolicy};

/// Local model with the information needed to load address keys.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AddressWithKeys {
    /// The email address of the address.
    pub email: String,

    /// The address id of the address.
    pub address_id: AddressId,

    /// Whether the address is active.
    pub is_active: bool,

    /// Whether the address is external.
    pub is_external: bool,

    /// The keys of the address.
    pub keys: AddressKeys,
}

/// Local model with the information needed to load keys from a signed vCard.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SignedVCard {
    /// The vCard data to parse.
    pub data: String,

    /// The cryptographic signature over the vCard in armored format.
    pub signature: String,
}

impl DecryptableVerifiableCard for SignedVCard {
    fn card_type(&self) -> ContactCardType {
        ContactCardType::Signed
    }

    fn card_data(&self) -> &[u8] {
        self.data.as_bytes()
    }

    fn card_signature(&self) -> Option<&[u8]> {
        self.signature.as_bytes().into()
    }
}

/// Loads the own account's user and address key material.
#[async_trait]
pub trait LockedPrivateKeyLoader: Send + Sync {
    /// Loads the user keys from the User model by the user identifier.
    ///
    /// If no user keys are found, the loader returns a None.
    async fn load_user_keys(&self, user_id: &UserId) -> LoadingResult<Option<UserKeys>>;

    /// Loads the address keys from the Address model by the address identifier.
    ///
    /// If no address keys are found, the loader returns `None`.
    async fn load_address_keys(
        &self,
        address_id: &AddressId,
    ) -> LoadingResult<Option<AddressWithKeys>>;

    /// Loads the address keys from the Address model by the email address.
    ///
    /// If no address is found for the email address, the loader returns `None`.
    async fn load_address_keys_by_email(
        &self,
        email: &str,
    ) -> LoadingResult<Option<AddressWithKeys>>;
}

/// Load public address keys from the `keys/all` route.
#[async_trait]
pub trait PublicKeyLoader: Send + Sync {
    /// Loads the public address keys from the `keys/all` route.
    ///
    /// Note if internal only is true, the API will return an error for external addresses
    /// If `internal_only` is true, the API will return an error for external addresses.
    /// So, the app might need to handle this case and return an empty response.
    /// ```skip
    /// if internal_only
    ///     && (error.code == 33102
    ///         || error.code == 33103)
    /// {
    ///     return Ok(APIPublicAddressKeys::default());
    /// }
    /// ```
    async fn load_public_address_keys(
        &self,
        email: &str,
        internal_only: bool,
        fetch_policy: PublicAddressKeyApiFetchPolicy,
    ) -> LoadingResult<APIPublicAddressKeys>;
}

/// Loads the signed vCard for a contact, optionally syncing it from the server first.
#[async_trait]
pub trait ContactPublicKeyLoader: Send + Sync {
    /// Loads the contact for the given email address and extracts the signed vCard from it.
    ///
    /// If no contact is found or there is no signed vCard in the contact, the loader returns `None`.
    async fn load_signed_contact_card(
        &self,
        email: &str,
        fetch_policy: PublicAddressKeyContactFetchPolicy,
    ) -> LoadingResult<Option<SignedVCard>>;
}

/// Loads the secret to unlock the user keys.
#[async_trait]
pub trait KeySecretLoader: Send + Sync {
    /// Loads the secret to unlock the user keys.
    ///
    /// If no secret is found, the loader returns `None`.
    async fn key_secret(&self) -> LoadingResult<Option<KeySecret>>;
}

pub trait CacheAccess: Send + Sync {
    /// Gets the cache for the keys.
    ///
    /// If no cache is used, the loader returns `None`.
    fn key_cache(&self) -> Option<&MemoryKeyCache>;
}

#[derive(Default, Clone)]
pub struct DefaultKeySecretLoader {}

impl DefaultKeySecretLoader {
    #[must_use]
    pub fn dyn_loader() -> Arc<dyn KeySecretLoader> {
        Arc::new(Self::default())
    }
}

#[async_trait]
impl KeySecretLoader for DefaultKeySecretLoader {
    /// Loads the key secret to unlock the user keys.
    async fn key_secret(&self) -> LoadingResult<Option<KeySecret>> {
        Ok(None)
    }
}

#[derive(Default, Clone)]
pub struct DefaultContactPublicKeyLoader {}

impl DefaultContactPublicKeyLoader {
    #[must_use]
    pub fn dyn_loader() -> Arc<dyn ContactPublicKeyLoader> {
        Arc::new(Self::default())
    }
}

#[async_trait]
impl ContactPublicKeyLoader for DefaultContactPublicKeyLoader {
    async fn load_signed_contact_card(
        &self,
        _email: &str,
        _fetch_policy: PublicAddressKeyContactFetchPolicy,
    ) -> LoadingResult<Option<SignedVCard>> {
        Ok(None)
    }
}

#[derive(Default, Clone)]
pub struct DefaultLockedPrivateKeyLoader {}

impl DefaultLockedPrivateKeyLoader {
    #[must_use]
    pub fn dyn_loader() -> Arc<dyn LockedPrivateKeyLoader> {
        Arc::new(Self::default())
    }
}

#[async_trait]
impl LockedPrivateKeyLoader for DefaultLockedPrivateKeyLoader {
    async fn load_user_keys(&self, _user_id: &UserId) -> LoadingResult<Option<UserKeys>> {
        Ok(None)
    }

    async fn load_address_keys(&self, _id: &AddressId) -> LoadingResult<Option<AddressWithKeys>> {
        Ok(None)
    }

    async fn load_address_keys_by_email(
        &self,
        _email: &str,
    ) -> LoadingResult<Option<AddressWithKeys>> {
        Ok(None)
    }
}

#[derive(Default, Clone)]
pub struct DefaultPublicKeyLoader {}

impl DefaultPublicKeyLoader {
    #[must_use]
    pub fn dyn_loader() -> Arc<dyn PublicKeyLoader> {
        Arc::new(Self::default())
    }
}

#[async_trait]
impl PublicKeyLoader for DefaultPublicKeyLoader {
    async fn load_public_address_keys(
        &self,
        _email: &str,
        _internal_only: bool,
        _fetch_policy: PublicAddressKeyApiFetchPolicy,
    ) -> LoadingResult<APIPublicAddressKeys> {
        Ok(APIPublicAddressKeys {
            address_keys: APIPublicAddressKeyGroup::default(),
            catch_all_keys: None,
            unverified_keys: None,
            warnings: Vec::default(),
            proton_mx: false,
            is_proton: false,
        })
    }
}

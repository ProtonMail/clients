use std::sync::Arc;

use proton_crypto_account::{
    keys::{
        AddressKeyForEmailSelector, AddressKeySelector, PinnedPublicKeys, PublicAddressKeys,
        UnlockedAddressKeys, UnlockedUserKeys, UserKeySelector,
    },
    proton_crypto::crypto::PGPProviderSync,
};
use tracing::{debug, error};

use crate::{
    Result,
    cache::{MemoryKeyCache, WrappedKeyCache},
    error::{KeyHandlingError, KeyManagerBuilderError},
    ids::{AddressId, UserId},
    policy::{PublicAddressKeyApiFetchPolicy, PublicAddressKeyContactFetchPolicy},
    traits::{
        AddressWithKeys, CacheAccess, ContactPublicKeyLoader, KeySecretLoader,
        LockedPrivateKeyLoader, PublicKeyLoader,
    },
};

use futures::join;

/// Allows to access and select `OpenPGP` keys for a givien context.
#[derive(Clone)]
pub struct KeySelector<'a, Ctx> {
    /// The user id of the account to select keys for.
    user_id: &'a UserId,

    /// The context that allows the selector to access the raw keys.
    ctx: Ctx,
}

impl<'a, Ctx> KeySelector<'a, Ctx>
where
    Ctx: KeySecretLoader + LockedPrivateKeyLoader + CacheAccess + 'a,
{
    /// Creates a new key selector for a given user id and context.
    ///
    /// Only allow to access private keys of the user.
    pub fn new_partial(user_id: &'a UserId, ctx: Ctx) -> Self {
        Self { user_id, ctx }
    }

    /// Returns a selector for the unlocked user keys of the account.
    pub async fn user_keys<P>(&self, pgp: &P) -> Result<UserKeySelector<'static, P>>
    where
        P: PGPProviderSync,
    {
        Ok(UserKeySelector::new(self.raw_user_keys(pgp).await?))
    }

    /// Returns a selector for the unlocked address keys of the account for a specific address with the provided address id.
    pub async fn address_keys<P>(
        &self,
        pgp: &P,
        address_id: &AddressId,
    ) -> Result<AddressKeySelector<'static, P>>
    where
        P: PGPProviderSync,
    {
        Ok(AddressKeySelector::new(
            self.raw_address_keys(pgp, address_id).await?,
        ))
    }

    async fn raw_user_keys<P>(&self, pgp: &P) -> Result<UnlockedUserKeys<P>>
    where
        P: PGPProviderSync,
    {
        load_user_keys(
            pgp,
            self.user_id,
            self.ctx.key_cache(),
            &self.ctx,
            &self.ctx,
        )
        .await
    }

    async fn raw_address_keys<P>(
        &self,
        pgp: &P,
        address_id: &AddressId,
    ) -> Result<UnlockedAddressKeys<P>>
    where
        P: PGPProviderSync,
    {
        load_address_keys(
            pgp,
            self.user_id,
            address_id,
            self.ctx.key_cache(),
            &self.ctx,
            &self.ctx,
        )
        .await
    }
}

impl<'a, Ctx> KeySelector<'a, Ctx>
where
    Ctx: KeySecretLoader
        + LockedPrivateKeyLoader
        + CacheAccess
        + PublicKeyLoader
        + ContactPublicKeyLoader
        + 'a,
{
    /// Creates a new key selector for a given user id and context.
    ///
    /// Allow to access public and private keys of the user.
    pub fn new(user_id: &'a UserId, ctx: Ctx) -> Self {
        Self { user_id, ctx }
    }

    /// Returns a selector for the public address keys of an email address.
    ///
    /// The selector will contain the public address keys for the email address, if any.
    /// If the public key loader is not set in the manager, and the account does not own the input email address,
    /// the function will return [`KeyHandlingError::NoPublicKeyLoader`].
    pub async fn address_keys_for_email<P>(
        &self,
        pgp: &P,
        email: &str,
        internal_only: bool,
        api_fetch_policy: PublicAddressKeyApiFetchPolicy,
        contact_fetch_policy: PublicAddressKeyContactFetchPolicy,
    ) -> Result<AddressKeyForEmailSelector<P>>
    where
        Ctx: KeySecretLoader
            + LockedPrivateKeyLoader
            + CacheAccess
            + PublicKeyLoader
            + ContactPublicKeyLoader,
        P: PGPProviderSync,
    {
        let secret: &dyn KeySecretLoader = &self.ctx;
        let private_keys: &dyn LockedPrivateKeyLoader = &self.ctx;
        let public_keys: &dyn PublicKeyLoader = &self.ctx;
        let contact_keys: &dyn ContactPublicKeyLoader = &self.ctx;
        address_keys_for_email_impl(
            pgp,
            self.user_id,
            email,
            internal_only,
            api_fetch_policy,
            contact_fetch_policy,
            self.ctx.key_cache(),
            secret,
            private_keys,
            Some(public_keys),
            Some(contact_keys),
        )
        .await
    }
}

/// Implements a key manager that can be used to load Proton
/// `OpenPGP` keys from its key infrastructure.
#[derive(Clone)]
#[allow(dead_code)]
pub struct KeyManager {
    /// The user id of the account.
    user_id: UserId,

    /// The service that load the user secret to unlock keys.
    secret_loader: Arc<dyn KeySecretLoader>,

    /// The service that load the user/address private keys from the user and address model.
    private_key_loader: Arc<dyn LockedPrivateKeyLoader>,

    /// The service that loads the public address keys for a specific email address from the API.
    public_key_loader: Option<Arc<dyn PublicKeyLoader>>,

    /// The service that loads the pinned public address keys from the contact model.
    public_contact_key_loader: Option<Arc<dyn ContactPublicKeyLoader>>,

    /// Optional internal cache to store unlocked user/address keys.
    cache: Option<Arc<MemoryKeyCache>>,
}

/// Builder for [`KeyManager`].
#[derive(Clone)]
pub struct KeyManagerBuilder {
    user_id: UserId,
    secret_loader: Option<Arc<dyn KeySecretLoader>>,
    private_key_loader: Option<Arc<dyn LockedPrivateKeyLoader>>,
    public_key_loader: Option<Arc<dyn PublicKeyLoader>>,
    public_contact_key_loader: Option<Arc<dyn ContactPublicKeyLoader>>,
    cache_storage: Option<Arc<MemoryKeyCache>>,
}

impl KeyManagerBuilder {
    /// Sets the service that loads the user secret to unlock keys.
    ///
    /// The default implementation is [`DefaultKeySecretLoader`].
    pub fn with_key_secret_loader(self, secret_loader: Arc<dyn KeySecretLoader>) -> Self {
        Self {
            secret_loader: Some(secret_loader),
            ..self
        }
    }

    /// Sets the service that loads the user/address private keys from the user and address model.
    ///
    /// The default implementation is [`DefaultLockedPrivateKeyLoader`].
    pub fn with_private_key_loader(
        self,
        private_key_loader: Arc<dyn LockedPrivateKeyLoader>,
    ) -> Self {
        Self {
            private_key_loader: Some(private_key_loader),
            ..self
        }
    }

    /// Sets the service that loads the public address keys for a specific email address from the API.
    ///
    /// The default implementation is [`DefaultPublicKeyLoader`].
    pub fn with_public_key_loader(self, public_key_loader: Arc<dyn PublicKeyLoader>) -> Self {
        Self {
            public_key_loader: Some(public_key_loader),
            ..self
        }
    }

    /// Sets the service that loads the pinned public address keys from the contact model.
    ///
    /// The default implementation is [`DefaultContactPublicKeyLoader`].
    pub fn with_public_contact_key_loader(
        self,
        public_contact_key_loader: Arc<dyn ContactPublicKeyLoader>,
    ) -> Self {
        Self {
            public_contact_key_loader: Some(public_contact_key_loader),
            ..self
        }
    }

    /// Sets the internal cache to store unlocked user/address keys.
    ///
    /// The default implementation is [`MemoryKeyCache`].
    pub fn with_key_cache(self, key_cache: Arc<MemoryKeyCache>) -> Self {
        Self {
            cache_storage: Some(key_cache),
            ..self
        }
    }

    /// Enables the in-memory key cache with the default settings.
    pub fn with_default_key_cache(self) -> Self {
        Self {
            cache_storage: Some(Arc::new(MemoryKeyCache::default())),
            ..self
        }
    }

    /// Builds the [`KeyManager`].
    pub fn build(self) -> Result<KeyManager> {
        Ok(KeyManager {
            user_id: self.user_id,
            secret_loader: self
                .secret_loader
                .ok_or(KeyManagerBuilderError::MissingSecretLoader)?,
            private_key_loader: self
                .private_key_loader
                .ok_or(KeyManagerBuilderError::MissingPrivateKeyLoader)?,
            public_key_loader: self.public_key_loader,
            public_contact_key_loader: self.public_contact_key_loader,
            cache: self.cache_storage,
        })
    }
}

impl KeyManager {
    pub fn builder(user_id: UserId) -> KeyManagerBuilder {
        KeyManagerBuilder {
            user_id,
            secret_loader: None,
            private_key_loader: None,
            public_contact_key_loader: None,
            public_key_loader: None,
            cache_storage: None,
        }
    }

    /// Returns a selector for the unlocked user keys of the account.
    pub async fn user_keys<P: PGPProviderSync>(&self, pgp: &P) -> Result<UserKeySelector<'_, P>> {
        Ok(UserKeySelector::new(self.raw_user_keys(pgp).await?))
    }

    /// Returns a selector for the unlocked address keys of the account for a specific address with the provided address id.
    pub async fn address_keys<P: PGPProviderSync>(
        &self,
        pgp: &P,
        address_id: &AddressId,
    ) -> Result<AddressKeySelector<'_, P>> {
        Ok(AddressKeySelector::new(
            self.raw_address_keys(pgp, address_id).await?,
        ))
    }

    /// Returns a selector for the public address keys of an email address.
    ///
    /// The selector will contain the public address keys for the email address, if any.
    /// If the public key loader is not set in the manager, and the account does not own the input email address,
    /// the function will return [`KeyHandlingError::NoPublicKeyLoader`].
    pub async fn address_keys_for_email<P: PGPProviderSync>(
        &self,
        pgp: &P,
        email: &str,
        internal_only: bool,
        api_fetch_policy: PublicAddressKeyApiFetchPolicy,
        contact_fetch_policy: PublicAddressKeyContactFetchPolicy,
    ) -> Result<AddressKeyForEmailSelector<P>> {
        address_keys_for_email_impl(
            pgp,
            &self.user_id,
            email,
            internal_only,
            api_fetch_policy,
            contact_fetch_policy,
            self.cache.as_deref(),
            self.secret_loader.as_ref(),
            self.private_key_loader.as_ref(),
            self.public_key_loader.as_deref(),
            self.public_contact_key_loader.as_deref(),
        )
        .await
    }

    /// Clears the internal unlocked user and address keys cache.
    pub fn clear_cache(&self) {
        if let Some(cache) = &self.cache {
            cache.clear();
        }
    }

    async fn raw_user_keys<P: PGPProviderSync>(&self, pgp: &P) -> Result<UnlockedUserKeys<P>> {
        load_user_keys(
            pgp,
            &self.user_id,
            self.cache.as_deref(),
            self.secret_loader.as_ref(),
            self.private_key_loader.as_ref(),
        )
        .await
    }

    async fn raw_address_keys<P: PGPProviderSync>(
        &self,
        pgp: &P,
        address_id: &AddressId,
    ) -> Result<UnlockedAddressKeys<P>> {
        load_address_keys(
            pgp,
            &self.user_id,
            address_id,
            self.cache.as_deref(),
            self.secret_loader.as_ref(),
            self.private_key_loader.as_ref(),
        )
        .await
    }
}

#[allow(clippy::too_many_arguments)]
async fn address_keys_for_email_impl<P: PGPProviderSync>(
    pgp: &P,
    user_id: &UserId,
    email: &str,
    internal_only: bool,
    api_fetch_policy: PublicAddressKeyApiFetchPolicy,
    contact_fetch_policy: PublicAddressKeyContactFetchPolicy,
    cache: Option<&MemoryKeyCache>,
    secret_loader: &dyn KeySecretLoader,
    private_key_loader: &dyn LockedPrivateKeyLoader,
    public_key_loader: Option<&dyn PublicKeyLoader>,
    contact_key_loader: Option<&dyn ContactPublicKeyLoader>,
) -> Result<AddressKeyForEmailSelector<P>> {
    // If the email address is owned by the user, we return the owned address keys.
    if let Some((address, address_keys)) = resolve_self_owned_address_keys(
        pgp,
        user_id,
        email,
        cache,
        secret_loader,
        private_key_loader,
    )
    .await?
    {
        return Ok(AddressKeyForEmailSelector::Owned {
            is_external_address: address.is_external,
            address_keys,
        });
    }

    // If the email address is not owned by the user, we resolve the keys via the API and contact keys.
    let (api_keys, vcard_keys) = resolve_api_and_contact_keys(
        pgp,
        user_id,
        email,
        internal_only,
        api_fetch_policy,
        contact_fetch_policy,
        secret_loader,
        private_key_loader,
        public_key_loader,
        contact_key_loader,
        cache,
    )
    .await?;
    Ok(AddressKeyForEmailSelector::Other {
        api_keys,
        vcard_keys,
    })
}

async fn load_user_keys<P: PGPProviderSync>(
    pgp: &P,
    user_id: &UserId,
    cache: Option<&MemoryKeyCache>,
    secret_loader: &dyn KeySecretLoader,
    private_key_loader: &dyn LockedPrivateKeyLoader,
) -> Result<UnlockedUserKeys<P>> {
    // If the user keys are cached, we return them.
    if let Some(cache) = cache
        && let Some(keys) = WrappedKeyCache::from(cache).get_user_keys(pgp, user_id)?
    {
        return Ok(keys);
    }

    // Load and unlock the user keys via the key secret.
    let unlocked =
        load_and_unlock_user_keys(pgp, user_id, secret_loader, private_key_loader).await?;

    if let Some(cache) = cache {
        WrappedKeyCache::from(cache).store_user_keys(pgp, user_id, unlocked.as_ref())?;
    }
    Ok(unlocked)
}

async fn load_address_keys<P: PGPProviderSync>(
    pgp: &P,
    user_id: &UserId,
    address_id: &AddressId,
    cache: Option<&MemoryKeyCache>,
    secret_loader: &dyn KeySecretLoader,
    private_key_loader: &dyn LockedPrivateKeyLoader,
) -> Result<UnlockedAddressKeys<P>> {
    // If the address keys are cached, we return them.
    if let Some(cache) = cache
        && let Some(keys) = WrappedKeyCache::from(cache).get_address_keys(pgp, address_id)?
    {
        debug!("address keys are cached, returning them");
        return Ok(keys);
    }

    debug!("address keys are not cached, loading them");

    // Load and unlock them via the user keys.
    let user_keys = load_user_keys(pgp, user_id, cache, secret_loader, private_key_loader).await?;
    let unlocked = load_and_unlock_address_keys(
        pgp,
        address_id,
        private_key_loader,
        secret_loader,
        &user_keys,
    )
    .await?;

    if let Some(cache) = cache {
        WrappedKeyCache::from(cache).store_address_keys(pgp, address_id, unlocked.as_ref())?;
    }
    Ok(unlocked)
}

async fn fetch_public_address_keys<P: PGPProviderSync>(
    pgp: &P,
    email: &str,
    internal_only: bool,
    fetch_policy: PublicAddressKeyApiFetchPolicy,
    public_key_loader: Option<&dyn PublicKeyLoader>,
) -> Result<PublicAddressKeys<P::PublicKey>> {
    let Some(public_key_loader) = public_key_loader else {
        return Err(KeyHandlingError::NoPublicKeyLoader);
    };
    let api_keys = public_key_loader
        .load_public_address_keys(email, internal_only, fetch_policy)
        .await?;
    api_keys
        .import(pgp)
        .map_err(KeyHandlingError::PublicKeyImport)
}

#[allow(clippy::unused_async)]
#[allow(unused_variables)]
async fn fetch_pinned_keys_from_contacts<P: PGPProviderSync>(
    pgp: &P,
    email: &str,
    fetch_policy: PublicAddressKeyContactFetchPolicy,
    user_keys: &UnlockedUserKeys<P>,
    contact_key_loader: Option<&dyn ContactPublicKeyLoader>,
) -> Result<Option<PinnedPublicKeys<P::PublicKey>>> {
    #[cfg(feature = "contacts")]
    {
        let Some(contact_key_loader) = contact_key_loader else {
            return Ok(None);
        };
        let signed_card = contact_key_loader
            .load_signed_contact_card(email, fetch_policy)
            .await?;
        mail_crypto_contact_keys::extract_pinned_keys(pgp, user_keys, signed_card, email)
            .map_err(KeyHandlingError::VCardKeyExtraction)
    }
    #[cfg(not(feature = "contacts"))]
    {
        Ok(None)
    }
}

async fn resolve_self_owned_address_keys<P: PGPProviderSync>(
    pgp: &P,
    user_id: &UserId,
    email: &str,
    cache: Option<&MemoryKeyCache>,
    secret_loader: &dyn KeySecretLoader,
    private_key_loader: &dyn LockedPrivateKeyLoader,
) -> Result<Option<(AddressWithKeys, UnlockedAddressKeys<P>)>> {
    if let Some(address) = private_key_loader.load_address_keys_by_email(email).await?
        && address.is_active
    {
        let address_keys = load_address_keys(
            pgp,
            user_id,
            &address.address_id,
            cache,
            secret_loader,
            private_key_loader,
        )
        .await?;
        Ok(Some((address, address_keys)))
    } else {
        Ok(None)
    }
}

#[allow(clippy::too_many_arguments)]
async fn resolve_api_and_contact_keys<P: PGPProviderSync>(
    pgp: &P,
    user_id: &UserId,
    email: &str,
    internal_only: bool,
    api_fetch_policy: PublicAddressKeyApiFetchPolicy,
    contact_fetch_policy: PublicAddressKeyContactFetchPolicy,
    secret_loader: &dyn KeySecretLoader,
    private_key_loader: &dyn LockedPrivateKeyLoader,
    public_key_loader: Option<&dyn PublicKeyLoader>,
    contact_key_loader: Option<&dyn ContactPublicKeyLoader>,
    cache: Option<&MemoryKeyCache>,
) -> Result<(
    PublicAddressKeys<P::PublicKey>,
    Option<PinnedPublicKeys<P::PublicKey>>,
)> {
    let load_contacts = async {
        let user_keys =
            load_user_keys(pgp, user_id, cache, secret_loader, private_key_loader).await?;
        fetch_pinned_keys_from_contacts(
            pgp,
            email,
            contact_fetch_policy,
            &user_keys,
            contact_key_loader,
        )
        .await
    };

    let (api_keys_res, vcard_keys_res) = join!(
        fetch_public_address_keys(
            pgp,
            email,
            internal_only,
            api_fetch_policy,
            public_key_loader
        ),
        load_contacts
    );

    // Log an error if the contact pinned keys fail to load.
    if let Err(err) = &vcard_keys_res {
        error!("cryptographic key fetch: failed to load contact pinned keys: {err}");
    }

    // We ignore the error if the contact pinned keys fail to load.
    // Contact keys are non critical and we will fallback to the API keys.
    let vcard_keys = vcard_keys_res.ok().flatten();
    let api_keys = api_keys_res?;

    Ok((api_keys, vcard_keys))
}

async fn load_and_unlock_user_keys<P>(
    pgp: &P,
    user_id: &UserId,
    secret_loader: &dyn KeySecretLoader,
    private_key_loader: &dyn LockedPrivateKeyLoader,
) -> Result<UnlockedUserKeys<P>>
where
    P: PGPProviderSync,
{
    let Some(user_keys) = private_key_loader.load_user_keys(user_id).await? else {
        return Err(KeyHandlingError::NoUser(user_id.clone()));
    };
    let pw = secret_loader
        .key_secret()
        .await?
        .ok_or(KeyHandlingError::NoUserSecret)?;

    let unlock_result = user_keys.unlock(pgp, &pw);

    // If no user key unlocks, we return the unlock errors.
    if unlock_result.unlocked_keys.is_empty() {
        return Err(KeyHandlingError::UserKeyUnlock(unlock_result.failed));
    }

    Ok(unlock_result.unlocked_keys.into())
}

async fn load_and_unlock_address_keys<P>(
    pgp: &P,
    address_id: &AddressId,
    private_key_loader: &dyn LockedPrivateKeyLoader,
    secret_loader: &dyn KeySecretLoader,
    user_keys: &UnlockedUserKeys<P>,
) -> Result<UnlockedAddressKeys<P>>
where
    P: PGPProviderSync,
{
    let address_with_keys = private_key_loader
        .load_address_keys(address_id)
        .await?
        .ok_or(KeyHandlingError::NoAddress(address_id.clone()))?;

    // For legacy address keys, we need to load the user secret to unlock the address keys.
    let passphrase = if address_with_keys.keys.contains_legacy_keys() {
        secret_loader.key_secret().await?
    } else {
        None
    };

    let unlock_result = address_with_keys
        .keys
        .unlock(pgp, user_keys, passphrase.as_ref());

    // If no address key unlocks, we return the unlock errors.
    if unlock_result.unlocked_keys.is_empty() {
        return Err(KeyHandlingError::AddressKeyUnlock(unlock_result.failed));
    }

    Ok(unlock_result.unlocked_keys.into())
}

use async_trait::async_trait;
use mail_contacts_common::contact::Contact;
use mail_contacts_common::contact_card::ContactCard;
use mail_contacts_common::contact_email::ContactEmail;
use mail_contacts_common::error::ContactError;
use mail_core_api::consts::CoreBundle;
use mail_core_api::service::ApiServiceError;
use mail_core_api::services::proton::{
    AddressId, GetKeysAllOptions, PrivateEmailRef, ProtonAccount, UserId,
};
use mail_core_key_manager::cache::MemoryKeyCache;
use mail_core_key_manager::error::{ApiError, KeyHandlingError, LoadingError, LoadingResult};
use mail_core_key_manager::proton_crypto_account::salts::KeySecret;
use mail_core_key_manager::traits::{
    AddressWithKeys, CacheAccess, ContactPublicKeyLoader, KeySecretLoader, LockedPrivateKeyLoader,
    PublicKeyLoader, SignedVCard,
};
use mail_core_key_manager::{
    AddressId as KeyManagerAddressId, KeySelector, PublicAddressKeyApiFetchPolicy,
    PublicAddressKeyContactFetchPolicy, UserId as KeyManagerUserId,
};
use mail_shared_types::{ModelIdExtension, UnixTimestamp};
use mail_stash::macros::DbRecord;
use mail_stash::sql_using_serde;
use mail_stash::stash::{StashError, Tether, WriteTx};
use proton_crypto_account::contacts::ContactCardType;
use proton_crypto_account::errors::EncryptionPreferencesError;
use proton_crypto_account::keys::{APIPublicAddressKeyGroup, APIPublicAddressKeys, UserKeys};
use serde::{Deserialize, Serialize};
use thiserror::Error;
use tracing::{debug, error};

use crate::UserContext;
use crate::datatypes::AddressStatus;
use crate::models::{Address, User};
use indoc::indoc;
use mail_stash::orm::Model;
use mail_stash::params;

/// Re-export the `mail_core_key_manager` crate.
pub use mail_core_key_manager;

#[derive(Debug, Error)]
pub enum CryptoKeyLoadingError {
    #[error("No user found")]
    NoUser,

    #[error("No user secret found")]
    NoUserSecret,

    #[error("No address found for id {0}")]
    NoAddress(AddressId),

    #[error("Failed to unlock at least one address key, but the user has {0} address keys")]
    AddressKeyUnlock(usize),

    #[error("Database Error: {0}")]
    DB(#[from] StashError),

    #[error("Address has no remote id")]
    AddressHasNoRemoteId,

    #[error("No key secret found")]
    NoKeySecretFound,

    #[error("Public API key cache error: {0}")]
    PublicApiKeyCache(StashError),

    #[error("Public API key fetch error: {0}")]
    PublicApiKeyFetch(#[from] ApiServiceError),

    #[error("Contact error: {0}")]
    ContactError(#[from] ContactError),

    #[error("Contact fetch error: {0}")]
    ContactFetchError(ApiServiceError),

    #[error("User context not found")]
    NoTransactionProvided,

    #[error("Failed to select keys: ")]
    KeyHandlingError(#[from] KeyHandlingError),

    #[error("Failed to access OpenPGP keys: {0}")]
    EncryptionPreferences(#[from] EncryptionPreferencesError),
}

impl From<CryptoKeyLoadingError> for LoadingError {
    fn from(error: CryptoKeyLoadingError) -> Self {
        match &error {
            CryptoKeyLoadingError::NoUser
            | CryptoKeyLoadingError::NoUserSecret
            | CryptoKeyLoadingError::NoAddress(_)
            | CryptoKeyLoadingError::AddressKeyUnlock(_)
            | CryptoKeyLoadingError::AddressHasNoRemoteId
            | CryptoKeyLoadingError::NoKeySecretFound
            | CryptoKeyLoadingError::ContactError(_)
            | CryptoKeyLoadingError::NoTransactionProvided
            | CryptoKeyLoadingError::KeyHandlingError(_)
            | CryptoKeyLoadingError::EncryptionPreferences(_) => LoadingError::Other(error.into()),
            CryptoKeyLoadingError::PublicApiKeyCache(_) | CryptoKeyLoadingError::DB(_) => {
                LoadingError::Database(error.into())
            }
            CryptoKeyLoadingError::PublicApiKeyFetch(api_service_error)
            | CryptoKeyLoadingError::ContactFetchError(api_service_error) => {
                convert_to_api_error(api_service_error)
            }
        }
    }
}

fn convert_to_api_error(error: &ApiServiceError) -> LoadingError {
    if error.is_network_failure() {
        return ApiError::Network(error.to_string().into()).into();
    }

    match error.to_proton_error() {
        Some(info) => ApiError::Api {
            code: info.code,
            error: info.error,
            details: info.details.map(|d| d.to_string()),
        }
        .into(),
        None => LoadingError::Other(error.to_string().into()),
    }
}

impl TryFrom<Address> for AddressWithKeys {
    type Error = CryptoKeyLoadingError;

    fn try_from(address: Address) -> Result<Self, Self::Error> {
        let remote_id = address
            .remote_id
            .ok_or(CryptoKeyLoadingError::AddressHasNoRemoteId)?;
        Ok(AddressWithKeys {
            is_external: address.address_type.is_external(),
            email: address.email,
            address_id: KeyManagerAddressId::from(remote_id.as_str().to_string()),
            is_active: address.status == AddressStatus::Enabled,
            keys: address.keys.0,
        })
    }
}

pub struct CryptoKeyService {
    user_id: UserId,
    cache: MemoryKeyCache,
}

impl CryptoKeyService {
    #[must_use]
    pub fn new(user_id: UserId) -> Self {
        Self {
            user_id,
            cache: MemoryKeyCache::default(),
        }
    }

    pub async fn load<'a>(
        &'a self,
        ctx: &'a UserContext,
    ) -> Result<KeySelector<'a, KeyLoader<'a>>, StashError> {
        let tether = ctx.mail_stash().connection();
        let key_loader = KeyLoader::new(ctx, Connection::CtxTether(tether), &self.cache);
        Ok(KeySelector::new(&self.user_id, key_loader))
    }

    #[must_use]
    pub fn load_with_tether<'a>(
        &'a self,
        ctx: &'a UserContext,
        tether: &'a Tether,
    ) -> KeySelector<'a, KeyLoader<'a>> {
        let key_loader = KeyLoader::new(ctx, Connection::Tether(tether), &self.cache);
        KeySelector::new(&self.user_id, key_loader)
    }

    pub fn clear_cache(&self) {
        self.cache.clear();
    }
}

enum Connection<'a> {
    Tether(&'a Tether),
    CtxTether(Tether),
}

impl Connection<'_> {
    fn tether(&self) -> &Tether {
        match self {
            Connection::Tether(tether) => tether,
            Connection::CtxTether(tether) => tether,
        }
    }
}

pub struct KeyLoader<'a> {
    ctx: &'a UserContext,
    db_conn: Connection<'a>,
    cache: &'a MemoryKeyCache,
}

impl<'a> KeyLoader<'a> {
    fn new(ctx: &'a UserContext, db_conn: Connection<'a>, cache: &'a MemoryKeyCache) -> Self {
        Self {
            ctx,
            db_conn,
            cache,
        }
    }

    // Helper for storing a PublicAddressKeys API response in the cache
    async fn store_public_key_request(
        ctx: &UserContext,
        email: &PrivateEmailRef<'_>,
        internal_only: bool,
        response: &APIPublicAddressKeys,
    ) {
        let mut tether = ctx.mail_stash().connection();

        match tether
            .write_tx(async |tx| {
                PublicAddressKeysResponseCache::store(
                    email.as_clear_text_str().to_owned(),
                    internal_only,
                    response.clone(),
                    tx,
                )
                .await
            })
            .await
        {
            Ok(()) => (),
            Err(e) => tracing::error!("Failed to store response in cache: {e}"),
        }
    }
}

impl CacheAccess for KeyLoader<'_> {
    fn key_cache(&self) -> Option<&MemoryKeyCache> {
        Some(self.cache)
    }
}

#[async_trait]
impl KeySecretLoader for KeyLoader<'_> {
    async fn key_secret(&self) -> LoadingResult<Option<KeySecret>> {
        let session = self.ctx.session();
        let key_secret = session.expose_key_secret().await;
        Ok(key_secret.map(|secret| secret.0))
    }
}

#[async_trait]
impl LockedPrivateKeyLoader for KeyLoader<'_> {
    async fn load_user_keys(&self, user_id: &KeyManagerUserId) -> LoadingResult<Option<UserKeys>> {
        let conn = self.db_conn.tether();

        // Load the user from the DB.
        let user_id = UserId::from(user_id.to_string());
        let user = User::load(user_id, conn)
            .await
            .map_err(CryptoKeyLoadingError::DB)?;

        Ok(user.map(|u| u.keys.0))
    }

    async fn load_address_keys(
        &self,
        address_id: &KeyManagerAddressId,
    ) -> LoadingResult<Option<AddressWithKeys>> {
        let conn = self.db_conn.tether();

        // Load the address from the DB.
        let address_id = AddressId::from(address_id.to_string());
        let address = Address::find_by_remote_id(address_id, conn)
            .await
            .map_err(CryptoKeyLoadingError::DB)?;

        Ok(address.map(TryInto::try_into).transpose()?)
    }

    async fn load_address_keys_by_email(
        &self,
        email: &str,
    ) -> LoadingResult<Option<AddressWithKeys>> {
        let conn = self.db_conn.tether();

        let address = Address::by_email(email, conn)
            .await
            .map_err(CryptoKeyLoadingError::DB)?;

        Ok(address.map(TryInto::try_into).transpose()?)
    }
}

#[async_trait]
impl PublicKeyLoader for KeyLoader<'_> {
    async fn load_public_address_keys(
        &self,
        email: &str,
        internal_only: bool,
        fetch_policy: PublicAddressKeyApiFetchPolicy,
    ) -> LoadingResult<APIPublicAddressKeys> {
        let tether = self.db_conn.tether();

        let email = PrivateEmailRef::new(email);

        let cached_value = PublicAddressKeysResponseCache::get(
            email.as_clear_text_str().into(),
            internal_only,
            tether,
        )
        .await
        .map_err(CryptoKeyLoadingError::PublicApiKeyCache)?;

        let api_keys = match (cached_value, fetch_policy) {
            (Some(cached_response), PublicAddressKeyApiFetchPolicy::AllowCachedFallback)
                if cached_response.is_valid() =>
            {
                cached_response.into_response()
            }
            (v, policy) =>
            // Invalid or does not exist, we need to fetch
            {
                match self
                    .ctx
                    .session()
                    .get_keys_all(GetKeysAllOptions {
                        email: email.to_owned(),
                        internal_only: Some(internal_only),
                    })
                    .await
                {
                    Ok(api_keys) => {
                        Self::store_public_key_request(self.ctx, &email, internal_only, &api_keys)
                            .await;
                        api_keys
                    }
                    Err(e)
                        if matches!(
                            policy,
                            PublicAddressKeyApiFetchPolicy::AllowCachedFallback
                        ) && v.is_some()
                            && e.is_network_failure() =>
                    {
                        tracing::debug!("Using cached value due to network failure");
                        v.expect("validated as some").into_response()
                    }
                    // We need treat these specific errors when doing internal only queries as
                    // if they have no keys.
                    Err(ApiServiceError::UnprocessableEntity(_, Some(error)))
                        if internal_only
                            && (error.code == CoreBundle::KeyGetAddressMissing as u32
                                || error.code == CoreBundle::KeyGetDomainExternal as u32) =>
                    {
                        let response = APIPublicAddressKeys {
                            address_keys: APIPublicAddressKeyGroup::default(),
                            catch_all_keys: None,
                            unverified_keys: None,
                            warnings: vec![],
                            proton_mx: false,
                            is_proton: false,
                        };
                        Self::store_public_key_request(self.ctx, &email, internal_only, &response)
                            .await;
                        response
                    }
                    Err(e) => {
                        return Err(CryptoKeyLoadingError::PublicApiKeyFetch(e).into());
                    }
                }
            }
        };

        Ok(api_keys)
    }
}

#[async_trait]
impl ContactPublicKeyLoader for KeyLoader<'_> {
    async fn load_signed_contact_card(
        &self,
        email: &str,
        fetch_policy: PublicAddressKeyContactFetchPolicy,
    ) -> LoadingResult<Option<SignedVCard>> {
        let email = PrivateEmailRef::new(email);
        debug!("Try loading signed contact card for email: {email}");

        let tether = self.db_conn.tether();

        let contact_email =
            ContactEmail::find_first("WHERE email = ?", params![email.to_owned()], tether)
                .await
                .map_err(CryptoKeyLoadingError::DB)?
                .ok_or(ContactError::CardNotFound(email.to_owned()))
                .map_err(CryptoKeyLoadingError::ContactError)?;

        let local_contact_id = contact_email
            .local_contact_id
            .ok_or(ContactError::ContactCardRemoteIdNotPresent(
                email.to_owned(),
            ))
            .map_err(CryptoKeyLoadingError::ContactError)?;

        // If a contact exists and has linked vCards, attempt to extract pinned keys from them.
        // vCards should be current if they were synced at least once
        // since they would be updated via update events.
        match fetch_policy {
            PublicAddressKeyContactFetchPolicy::AllowCachedFallback => {
                match Contact::load(local_contact_id, tether).await {
                    Ok(Some(mut contact)) => {
                        if let Ok(cards) = contact.cards(tether).await
                            && !cards.is_empty()
                        {
                            debug!(
                                "Use local contact {local_contact_id} for pinned keys extraction"
                            );

                            return Ok(signed_vcard_from_cards(cards));
                        }
                    }
                    Err(e) => {
                        error!("Failed to load contact for pinned keys extraction: {e}");
                    }
                    _ => {}
                }
            }
            PublicAddressKeyContactFetchPolicy::RequireSync => {} // continue
        }

        // Sync the most recent full contact including its v-cards from the backend (one DB tx).
        let session = self.ctx.session();
        let mut tether_for_tx = self.ctx.mail_stash().connection();
        if let Err(e) = Contact::force_sync_with_card(local_contact_id, session, &mut tether_for_tx)
            .await
            .inspect_err(|e| error!("Failed to force sync contact: {e}"))
        {
            match e {
                ContactError::Api(api_err) if api_err.is_network_failure() => match fetch_policy {
                    PublicAddressKeyContactFetchPolicy::RequireSync => {
                        return Err(CryptoKeyLoadingError::ContactFetchError(api_err).into());
                    }
                    PublicAddressKeyContactFetchPolicy::AllowCachedFallback => {} // continue
                },
                e => return Err(CryptoKeyLoadingError::ContactError(e).into()),
            }
        }
        drop(tether_for_tx);

        let mut contact = Contact::load(local_contact_id, tether)
            .await
            .map_err(CryptoKeyLoadingError::DB)?
            .ok_or(ContactError::FullContactNotFound(email.to_owned()))
            .map_err(CryptoKeyLoadingError::ContactError)?;

        let cards = contact
            .cards(tether)
            .await
            .map_err(CryptoKeyLoadingError::DB)?;

        let signed_vcard = signed_vcard_from_cards(cards);
        if signed_vcard.is_some() {
            debug!("Found signed vcard for email: {email}");
        } else {
            debug!("No signed vcard found for email: {email}");
        }
        Ok(signed_vcard)
    }
}

fn signed_vcard_from_cards(cards: &[ContactCard]) -> Option<SignedVCard> {
    cards.iter().find_map(|card| {
        if card.card_type == ContactCardType::Signed {
            card.signature.as_ref().map(|sig| SignedVCard {
                data: card.data.clone(),
                signature: sig.clone(),
            })
        } else {
            None
        }
    })
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
struct PublicKeyResponseCacheValue(APIPublicAddressKeys);

sql_using_serde!(PublicKeyResponseCacheValue);

pub struct PublicAddressKeysResponseCache;

#[derive(Debug, DbRecord, PartialEq, Clone)]
pub struct PublicKeyResponseCacheEntry {
    #[DbField]
    response: PublicKeyResponseCacheValue,
    #[DbField]
    timestamp: UnixTimestamp,
}

const PUBLIC_RESPONSE_KEY_RESPONSE_TTL_SEC: u64 = 60 * 60; //1h
impl PublicKeyResponseCacheEntry {
    #[must_use]
    pub fn is_valid(&self) -> bool {
        self.timestamp
            .saturating_add(PUBLIC_RESPONSE_KEY_RESPONSE_TTL_SEC)
            >= UnixTimestamp::now()
    }

    #[must_use]
    pub fn into_response(self) -> APIPublicAddressKeys {
        self.response.0
    }
}

impl PublicAddressKeysResponseCache {
    pub async fn store(
        email: String,
        internal_only: bool,
        response: APIPublicAddressKeys,
        tx: &WriteTx<'_>,
    ) -> Result<(), StashError> {
        tx.execute(
            indoc! {"
            INSERT INTO public_address_key_response_cache (email, internal_only, response, timestamp)
            VALUES (?,?,?, ?)
            ON CONFLICT (email, internal_only) DO UPDATE SET
                response=excluded.response,
                timestamp=excluded.timestamp
        "},
            params![email, internal_only, PublicKeyResponseCacheValue(response), UnixTimestamp::now()],
        )
        .await?;
        Ok(())
    }

    pub async fn get(
        email: String,
        internal_only: bool,
        tether: &Tether,
    ) -> Result<Option<PublicKeyResponseCacheEntry>, StashError> {
        Ok(tether.query::<_,PublicKeyResponseCacheEntry>("SELECT response, timestamp FROM public_address_key_response_cache WHERE email=? AND internal_only=?",
       params![email, internal_only]).await?.into_iter().next())
    }
}

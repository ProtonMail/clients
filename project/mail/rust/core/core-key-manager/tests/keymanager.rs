use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use async_trait::async_trait;
use mail_core_key_manager::cache::MemoryKeyCache;
use mail_core_key_manager::error::{
    KeyHandlingError, KeyManagerBuilderError, LoadingError, LoadingResult,
};
use mail_core_key_manager::traits::{
    AddressWithKeys, ContactPublicKeyLoader, KeySecretLoader, LockedPrivateKeyLoader,
    PublicKeyLoader, SignedVCard,
};
use mail_core_key_manager::{
    AddressId, KeyManager, KeyManagerBuilder, PublicAddressKeyApiFetchPolicy,
    PublicAddressKeyContactFetchPolicy, UserId,
};
use proton_crypto_account::keys::{
    APIPublicAddressKeyGroup, APIPublicAddressKeys, APIPublicKey, APIPublicKeySource,
    AddressKeyForEmailSelector, AddressKeys, CryptoMailSettings, KeyFlag, KeyId, LocalAddressKey,
    LocalUserKey, LockedKey, UserKeys,
};
use proton_crypto_account::proton_crypto::ProtonPGP;
use proton_crypto_account::proton_crypto::crypto::{
    DataEncoding, KeyGenerator, KeyGeneratorAlgorithm, KeyGeneratorSync, PGPProviderSync,
    UnixTimestamp,
};
use proton_crypto_account::salts::KeySecret;

struct TestAccount {
    user_id: UserId,
    address_id: AddressId,
    address_email: String,
    user_keys: UserKeys,
    address_keys: AddressKeys,
    key_secret_bytes: Vec<u8>,
}

fn setup_account<P: PGPProviderSync>(provider: &P) -> TestAccount {
    let key_secret_bytes = b"password".to_vec();
    let key_secret = KeySecret::new(key_secret_bytes.clone());

    let local_user_key =
        LocalUserKey::generate(provider, KeyGeneratorAlgorithm::ECC, &key_secret).unwrap();

    let user_key_id = KeyId("test_user_key_id".to_string());
    let unlocked_user_keys = local_user_key
        .unlock_and_assign_key_id(provider, user_key_id.clone(), &key_secret)
        .unwrap();

    let address_email = "test@example.com".to_string();
    let local_address_key = LocalAddressKey::generate(
        provider,
        &address_email,
        KeyGeneratorAlgorithm::ECC,
        KeyFlag::default(),
        true,
        &unlocked_user_keys,
    )
    .unwrap();

    let locked_user_key = LockedKey {
        id: user_key_id,
        version: 4,
        private_key: local_user_key.private_key,
        token: None,
        signature: None,
        activation: None,
        primary: true,
        active: true,
        flags: None,
        recovery_secret: None,
        recovery_secret_signature: None,
        address_forwarding_id: None,
    };

    let locked_address_key = LockedKey {
        id: KeyId("test_address_key_id".to_string()),
        version: 4,
        private_key: local_address_key.private_key,
        token: local_address_key.token,
        signature: local_address_key.signature,
        activation: None,
        primary: local_address_key.primary,
        active: true,
        flags: Some(local_address_key.flags),
        recovery_secret: None,
        recovery_secret_signature: None,
        address_forwarding_id: None,
    };

    TestAccount {
        user_id: UserId::new("test_user_id".to_string()),
        address_id: AddressId::new("test_address_id".to_string()),
        address_email,
        user_keys: UserKeys(vec![locked_user_key]),
        address_keys: AddressKeys(vec![locked_address_key]),
        key_secret_bytes,
    }
}

fn setup_public_address_keys<P: PGPProviderSync>(provider: &P) -> String {
    let key = provider
        .new_key_generator()
        .with_user_id("other@example.com", "other@example.com")
        .generate()
        .unwrap();
    let public_key = provider.private_key_to_public_key(&key).unwrap();
    let armored = provider
        .public_key_export(&public_key, DataEncoding::Armor)
        .unwrap();
    str::from_utf8(armored.as_ref()).unwrap().to_string()
}

#[cfg(feature = "contacts")]
struct CardSigner(String);

#[cfg(feature = "contacts")]
impl proton_crypto_account::contacts::EncryptableAndSignableCard for CardSigner {
    fn plaintext_card_data(&self) -> &[u8] {
        self.0.as_bytes()
    }
}

#[cfg(feature = "contacts")]
fn setup_signed_contact_card<P: PGPProviderSync>(
    provider: &P,
    unlocked_user_key: &proton_crypto_account::keys::UnlockedUserKey<P>,
    email: &str,
) -> SignedVCard {
    use base64::Engine as _;
    use base64::engine::general_purpose::STANDARD;
    use proton_crypto_account::contacts::EncryptableAndSignableCard;

    let key = provider
        .new_key_generator()
        .with_user_id(email, email)
        .generate()
        .unwrap();
    let public_key = provider.private_key_to_public_key(&key).unwrap();
    let public_key_bytes = provider
        .public_key_export(&public_key, DataEncoding::Bytes)
        .unwrap();
    let public_key_base64 = STANDARD.encode(public_key_bytes.as_ref());
    let card = CardSigner(format!(
        "BEGIN:VCARD\nVERSION:4.0\nFN:Test User\nITEM1.EMAIL:{email}\nITEM1.KEY;PREF=1:data:application/pgp-keys;base64,{public_key_base64}\nEND:VCARD"
    ));
    let signature = card.sign_sync(provider, unlocked_user_key).unwrap();
    SignedVCard {
        data: card.0,
        signature: signature.0,
    }
}

#[derive(Default)]
struct TestCounts {
    loader_user: Arc<AtomicUsize>,
    loader_address: Arc<AtomicUsize>,
}

impl TestCounts {
    fn loader_user(&self) -> usize {
        self.loader_user.load(Ordering::SeqCst)
    }

    fn loader_address(&self) -> usize {
        self.loader_address.load(Ordering::SeqCst)
    }
}

struct MockPrivateKeyLoader {
    user_keys: UserKeys,
    address_keys: AddressKeys,
    user_load_count: Arc<AtomicUsize>,
    address_load_count: Arc<AtomicUsize>,
}

#[async_trait]
impl LockedPrivateKeyLoader for MockPrivateKeyLoader {
    async fn load_user_keys(&self, _: &UserId) -> LoadingResult<Option<UserKeys>> {
        self.user_load_count.fetch_add(1, Ordering::SeqCst);
        Ok(Some(self.user_keys.clone()))
    }

    async fn load_address_keys(&self, id: &AddressId) -> LoadingResult<Option<AddressWithKeys>> {
        self.address_load_count.fetch_add(1, Ordering::SeqCst);
        Ok(Some(AddressWithKeys {
            is_external: false,
            email: String::default(),
            address_id: id.clone(),
            is_active: true,
            keys: self.address_keys.clone(),
        }))
    }

    async fn load_address_keys_by_email(
        &self,
        _email: &str,
    ) -> LoadingResult<Option<AddressWithKeys>> {
        Ok(None)
    }
}

/// Mock that claims ownership of an email address (active).
struct MockPrivateKeyLoaderWithOwnedAddress {
    inner: MockPrivateKeyLoader,
    owned_address_id: AddressId,
    owned_email: String,
    is_active: bool,
}

#[async_trait]
impl LockedPrivateKeyLoader for MockPrivateKeyLoaderWithOwnedAddress {
    async fn load_user_keys(&self, user_id: &UserId) -> LoadingResult<Option<UserKeys>> {
        self.inner.load_user_keys(user_id).await
    }

    async fn load_address_keys(&self, id: &AddressId) -> LoadingResult<Option<AddressWithKeys>> {
        self.inner.load_address_keys(id).await
    }

    async fn load_address_keys_by_email(
        &self,
        email: &str,
    ) -> LoadingResult<Option<AddressWithKeys>> {
        if email == self.owned_email {
            Ok(Some(AddressWithKeys {
                is_external: false,
                email: email.to_string(),
                address_id: self.owned_address_id.clone(),
                is_active: self.is_active,
                keys: self.inner.address_keys.clone(),
            }))
        } else {
            Ok(None)
        }
    }
}

struct MockFailingUserKeyLoader;

#[async_trait]
impl LockedPrivateKeyLoader for MockFailingUserKeyLoader {
    async fn load_user_keys(&self, _: &UserId) -> LoadingResult<Option<UserKeys>> {
        Err(LoadingError::Other("simulated loader error".into()))
    }

    async fn load_address_keys(&self, _: &AddressId) -> LoadingResult<Option<AddressWithKeys>> {
        unreachable!()
    }

    async fn load_address_keys_by_email(&self, _: &str) -> LoadingResult<Option<AddressWithKeys>> {
        Ok(None)
    }
}

/// Mock that returns `None` from `load_address_keys` (address not found).
struct MockAddressNotFoundLoader {
    user_keys: UserKeys,
}

#[async_trait]
impl LockedPrivateKeyLoader for MockAddressNotFoundLoader {
    async fn load_user_keys(&self, _: &UserId) -> LoadingResult<Option<UserKeys>> {
        Ok(Some(self.user_keys.clone()))
    }

    async fn load_address_keys(&self, _: &AddressId) -> LoadingResult<Option<AddressWithKeys>> {
        Ok(None)
    }

    async fn load_address_keys_by_email(&self, _: &str) -> LoadingResult<Option<AddressWithKeys>> {
        Ok(None)
    }
}

struct MockKeySecretLoader {
    key_secret_bytes: Vec<u8>,
}

#[async_trait]
impl KeySecretLoader for MockKeySecretLoader {
    async fn key_secret(&self) -> LoadingResult<Option<KeySecret>> {
        Ok(Some(KeySecret::new(self.key_secret_bytes.clone())))
    }
}

/// Mock that returns no secret (simulates a locked/absent keychain).
struct MockNoSecretLoader;

#[async_trait]
impl KeySecretLoader for MockNoSecretLoader {
    async fn key_secret(&self) -> LoadingResult<Option<KeySecret>> {
        Ok(None)
    }
}

struct MockPublicKeyLoader {
    armored_public_key: String,
}

#[async_trait]
impl PublicKeyLoader for MockPublicKeyLoader {
    async fn load_public_address_keys(
        &self,
        _email: &str,
        _internal_only: bool,
        _fetch_policy: PublicAddressKeyApiFetchPolicy,
    ) -> LoadingResult<APIPublicAddressKeys> {
        Ok(APIPublicAddressKeys {
            address_keys: APIPublicAddressKeyGroup {
                keys: vec![APIPublicKey {
                    source: APIPublicKeySource::Proton,
                    flags: KeyFlag::default(),
                    primary: true,
                    public_key: self.armored_public_key.clone(),
                }],
                signed_key_list: None,
            },
            catch_all_keys: None,
            unverified_keys: None,
            warnings: Vec::default(),
            proton_mx: false,
            is_proton: true,
        })
    }
}

struct MockContactPublicKeyLoader {
    signed_vcard: Option<SignedVCard>,
}

#[async_trait]
impl ContactPublicKeyLoader for MockContactPublicKeyLoader {
    async fn load_signed_contact_card(
        &self,
        _email: &str,
        _fetch_policy: PublicAddressKeyContactFetchPolicy,
    ) -> LoadingResult<Option<SignedVCard>> {
        Ok(self.signed_vcard.clone())
    }
}

fn build_manager(account: &TestAccount, counts: &TestCounts) -> KeyManagerBuilder {
    let key_loader = Arc::new(MockPrivateKeyLoader {
        user_keys: account.user_keys.clone(),
        address_keys: account.address_keys.clone(),
        user_load_count: counts.loader_user.clone(),
        address_load_count: counts.loader_address.clone(),
    });
    let secret_loader = Arc::new(MockKeySecretLoader {
        key_secret_bytes: account.key_secret_bytes.clone(),
    });
    KeyManager::builder(account.user_id.clone())
        .with_private_key_loader(key_loader)
        .with_key_secret_loader(secret_loader)
        .with_key_cache(MemoryKeyCache::default().into())
}

fn build_manager_with_active(
    account: &TestAccount,
    counts: &TestCounts,
    is_active: bool,
) -> KeyManagerBuilder {
    let key_loader = Arc::new(MockPrivateKeyLoaderWithOwnedAddress {
        inner: MockPrivateKeyLoader {
            user_keys: account.user_keys.clone(),
            address_keys: account.address_keys.clone(),
            user_load_count: counts.loader_user.clone(),
            address_load_count: counts.loader_address.clone(),
        },
        owned_address_id: account.address_id.clone(),
        owned_email: account.address_email.clone(),
        is_active,
    });
    let secret_loader = Arc::new(MockKeySecretLoader {
        key_secret_bytes: account.key_secret_bytes.clone(),
    });
    KeyManager::builder(account.user_id.clone())
        .with_private_key_loader(key_loader)
        .with_key_secret_loader(secret_loader)
        .with_key_cache(MemoryKeyCache::default().into())
}

#[test]
fn builder_fails_without_secret_loader() {
    let pgp = ProtonPGP::new_sync();
    let account = setup_account(&pgp);

    let err = KeyManager::builder(account.user_id.clone())
        .with_private_key_loader(Arc::new(MockFailingUserKeyLoader))
        // no secret loader
        .build()
        .map(|_| ())
        .unwrap_err();

    assert!(
        matches!(
            err,
            KeyHandlingError::Build(KeyManagerBuilderError::MissingSecretLoader)
        ),
        "expected MissingSecretLoader, got {err:?}"
    );
}

#[test]
fn builder_fails_without_private_key_loader() {
    let pgp = ProtonPGP::new_sync();
    let account = setup_account(&pgp);

    let err = KeyManager::builder(account.user_id.clone())
        .with_key_secret_loader(Arc::new(MockKeySecretLoader {
            key_secret_bytes: b"pw".to_vec(),
        }))
        // no private key loader
        .build()
        .map(|_| ())
        .unwrap_err();

    assert!(
        matches!(
            err,
            KeyHandlingError::Build(KeyManagerBuilderError::MissingPrivateKeyLoader)
        ),
        "expected MissingPrivateKeyLoader, got {err:?}"
    );
}

#[tokio::test]
async fn user_keys_loads_and_unlocks_successfully() {
    let pgp = ProtonPGP::new_sync();
    let account = setup_account(&pgp);
    let manager = build_manager(&account, &TestCounts::default())
        .build()
        .unwrap();

    let selector = manager.user_keys(&pgp).await.unwrap();
    assert_eq!(selector.for_decryption().len(), account.user_keys.0.len());
}

#[tokio::test]
async fn user_key_selector_provides_encryption_key() {
    let pgp = ProtonPGP::new_sync();
    let account = setup_account(&pgp);
    let manager = build_manager(&account, &TestCounts::default())
        .build()
        .unwrap();

    let selector = manager.user_keys(&pgp).await.unwrap();
    assert!(selector.for_encryption().is_ok());
}

#[tokio::test]
async fn user_key_selector_provides_signing_key() {
    let pgp = ProtonPGP::new_sync();
    let account = setup_account(&pgp);
    let manager = build_manager(&account, &TestCounts::default())
        .build()
        .unwrap();

    let selector = manager.user_keys(&pgp).await.unwrap();
    assert!(selector.for_signing().is_ok());
}

#[tokio::test]
async fn user_key_selector_provides_verification_keys() {
    let pgp = ProtonPGP::new_sync();
    let account = setup_account(&pgp);
    let manager = build_manager(&account, &TestCounts::default())
        .build()
        .unwrap();

    let selector = manager.user_keys(&pgp).await.unwrap();
    assert!(!selector.for_signature_verification().is_empty());
}

#[tokio::test]
async fn user_keys_served_from_cache_on_second_call() {
    let pgp = ProtonPGP::new_sync();
    let account = setup_account(&pgp);
    let counts = TestCounts::default();
    let manager = build_manager(&account, &counts).build().unwrap();

    manager.user_keys(&pgp).await.unwrap();
    manager.user_keys(&pgp).await.unwrap();

    assert_eq!(counts.loader_user(), 1);
}

#[tokio::test]
async fn user_keys_reloaded_on_every_call_without_cache() {
    let pgp = ProtonPGP::new_sync();
    let account = setup_account(&pgp);
    let counts = TestCounts::default();
    // Build without a cache
    let manager = KeyManager::builder(account.user_id.clone())
        .with_private_key_loader(Arc::new(MockPrivateKeyLoader {
            user_keys: account.user_keys.clone(),
            address_keys: account.address_keys.clone(),
            user_load_count: counts.loader_user.clone(),
            address_load_count: counts.loader_address.clone(),
        }))
        .with_key_secret_loader(Arc::new(MockKeySecretLoader {
            key_secret_bytes: account.key_secret_bytes.clone(),
        }))
        .build()
        .unwrap();

    manager.user_keys(&pgp).await.unwrap();
    manager.user_keys(&pgp).await.unwrap();

    assert_eq!(
        counts.loader_user(),
        2,
        "without cache each call must reload"
    );
}

#[tokio::test]
async fn user_keys_reload_after_cache_cleared() {
    let pgp = ProtonPGP::new_sync();
    let account = setup_account(&pgp);
    let counts = TestCounts::default();
    let manager = build_manager(&account, &counts).build().unwrap();

    manager.user_keys(&pgp).await.unwrap();
    manager.clear_cache();
    manager.user_keys(&pgp).await.unwrap();

    assert_eq!(counts.loader_user(), 2);
}

#[tokio::test]
async fn user_keys_fail_when_no_secret() {
    let pgp = ProtonPGP::new_sync();
    let account = setup_account(&pgp);

    let manager = KeyManager::builder(account.user_id.clone())
        .with_private_key_loader(Arc::new(MockPrivateKeyLoader {
            user_keys: account.user_keys.clone(),
            address_keys: account.address_keys.clone(),
            user_load_count: Arc::default(),
            address_load_count: Arc::default(),
        }))
        .with_key_secret_loader(Arc::new(MockNoSecretLoader))
        .build()
        .unwrap();

    let err = manager.user_keys(&pgp).await.map(|_| ()).unwrap_err();
    assert!(
        matches!(err, KeyHandlingError::NoUserSecret),
        "expected NoUserSecret, got {err:?}"
    );
}

#[tokio::test]
async fn user_keys_fail_when_loader_errors() {
    let pgp = ProtonPGP::new_sync();
    let account = setup_account(&pgp);

    let manager = KeyManager::builder(account.user_id.clone())
        .with_private_key_loader(Arc::new(MockFailingUserKeyLoader))
        .with_key_secret_loader(Arc::new(MockKeySecretLoader {
            key_secret_bytes: account.key_secret_bytes.clone(),
        }))
        .build()
        .unwrap();

    let err = manager.user_keys(&pgp).await.map(|_| ()).unwrap_err();
    assert!(
        matches!(err, KeyHandlingError::Loading(_)),
        "expected Loading error, got {err:?}"
    );
}

#[tokio::test]
async fn address_keys_loads_and_unlocks_successfully() {
    let pgp = ProtonPGP::new_sync();
    let account = setup_account(&pgp);
    let manager = build_manager(&account, &TestCounts::default())
        .build()
        .unwrap();

    let selector = manager
        .address_keys(&pgp, &account.address_id)
        .await
        .unwrap();
    assert_eq!(
        selector.for_decryption().len(),
        account.address_keys.0.len()
    );
}

#[tokio::test]
async fn address_key_selector_provides_encryption_key() {
    let pgp = ProtonPGP::new_sync();
    let account = setup_account(&pgp);
    let manager = build_manager(&account, &TestCounts::default())
        .build()
        .unwrap();

    let selector = manager
        .address_keys(&pgp, &account.address_id)
        .await
        .unwrap();
    assert!(selector.for_encryption().is_ok());
}

#[tokio::test]
async fn address_key_selector_provides_signing_key() {
    let pgp = ProtonPGP::new_sync();
    let account = setup_account(&pgp);
    let manager = build_manager(&account, &TestCounts::default())
        .build()
        .unwrap();

    let selector = manager
        .address_keys(&pgp, &account.address_id)
        .await
        .unwrap();
    assert!(selector.for_signing().is_ok());
}

#[tokio::test]
async fn address_keys_served_from_cache_on_second_call() {
    let pgp = ProtonPGP::new_sync();
    let account = setup_account(&pgp);
    let counts = TestCounts::default();
    let manager = build_manager(&account, &counts).build().unwrap();

    manager
        .address_keys(&pgp, &account.address_id)
        .await
        .unwrap();
    manager
        .address_keys(&pgp, &account.address_id)
        .await
        .unwrap();

    assert_eq!(counts.loader_address(), 1);
}

#[tokio::test]
async fn address_keys_reloaded_on_every_call_without_cache() {
    let pgp = ProtonPGP::new_sync();
    let account = setup_account(&pgp);
    let counts = TestCounts::default();
    let manager = KeyManager::builder(account.user_id.clone())
        .with_private_key_loader(Arc::new(MockPrivateKeyLoader {
            user_keys: account.user_keys.clone(),
            address_keys: account.address_keys.clone(),
            user_load_count: counts.loader_user.clone(),
            address_load_count: counts.loader_address.clone(),
        }))
        .with_key_secret_loader(Arc::new(MockKeySecretLoader {
            key_secret_bytes: account.key_secret_bytes.clone(),
        }))
        .build()
        .unwrap();

    manager
        .address_keys(&pgp, &account.address_id)
        .await
        .unwrap();
    manager
        .address_keys(&pgp, &account.address_id)
        .await
        .unwrap();

    assert_eq!(
        counts.loader_address(),
        2,
        "without cache each call must reload"
    );
}

#[tokio::test]
async fn address_keys_reload_after_cache_cleared() {
    let pgp = ProtonPGP::new_sync();
    let account = setup_account(&pgp);
    let counts = TestCounts::default();
    let manager = build_manager(&account, &counts).build().unwrap();

    manager
        .address_keys(&pgp, &account.address_id)
        .await
        .unwrap();
    manager.clear_cache();
    manager
        .address_keys(&pgp, &account.address_id)
        .await
        .unwrap();

    assert_eq!(counts.loader_address(), 2);
}

#[tokio::test]
async fn address_keys_user_keys_loaded_only_once_for_multiple_addresses() {
    let pgp = ProtonPGP::new_sync();
    let account = setup_account(&pgp);
    let counts = TestCounts::default();
    let manager = build_manager(&account, &counts).build().unwrap();

    let address_id_b = AddressId::new("other_address_id".to_string());
    manager
        .address_keys(&pgp, &account.address_id)
        .await
        .unwrap();
    manager.address_keys(&pgp, &address_id_b).await.unwrap();

    assert_eq!(counts.loader_user(), 1, "user keys should be loaded once");
    assert_eq!(counts.loader_address(), 2, "each address loaded separately");
}

#[tokio::test]
async fn address_keys_fail_when_not_found() {
    let pgp = ProtonPGP::new_sync();
    let account = setup_account(&pgp);

    let manager = KeyManager::builder(account.user_id.clone())
        .with_private_key_loader(Arc::new(MockAddressNotFoundLoader {
            user_keys: account.user_keys.clone(),
        }))
        .with_key_secret_loader(Arc::new(MockKeySecretLoader {
            key_secret_bytes: account.key_secret_bytes.clone(),
        }))
        .build()
        .unwrap();

    let err = manager
        .address_keys(&pgp, &account.address_id)
        .await
        .map(|_| ())
        .unwrap_err();
    assert!(
        matches!(err, KeyHandlingError::NoAddress(_)),
        "expected NoAddress, got {err:?}"
    );
}

#[tokio::test]
async fn address_keys_for_email_returns_owned_for_active_self_address() {
    let pgp = ProtonPGP::new_sync();
    let account = setup_account(&pgp);
    let counts = TestCounts::default();
    let manager = build_manager_with_active(&account, &counts, true)
        .build()
        .unwrap();

    let selector = manager
        .address_keys_for_email(
            &pgp,
            &account.address_email,
            false,
            PublicAddressKeyApiFetchPolicy::AllowCachedFallback,
            PublicAddressKeyContactFetchPolicy::AllowCachedFallback,
        )
        .await
        .unwrap();

    assert!(
        matches!(selector, AddressKeyForEmailSelector::Owned { .. }),
        "expected Owned variant for a self-owned active address"
    );
}

#[tokio::test]
async fn address_keys_for_email_returns_other_for_inactive_owned_address() {
    let pgp = ProtonPGP::new_sync();
    let account = setup_account(&pgp);
    let counts = TestCounts::default();
    let armored = setup_public_address_keys(&pgp);
    // Address is owned but inactive → should fall back to API keys.
    let manager = build_manager_with_active(&account, &counts, false)
        .with_public_key_loader(Arc::new(MockPublicKeyLoader {
            armored_public_key: armored,
        }))
        .build()
        .unwrap();

    let selector = manager
        .address_keys_for_email(
            &pgp,
            &account.address_email,
            false,
            PublicAddressKeyApiFetchPolicy::AllowCachedFallback,
            PublicAddressKeyContactFetchPolicy::AllowCachedFallback,
        )
        .await
        .unwrap();

    assert!(
        matches!(selector, AddressKeyForEmailSelector::Other { .. }),
        "expected Other variant when owned address is inactive"
    );
}

#[tokio::test]
async fn address_keys_for_email_fails_without_public_key_loader() {
    let pgp = ProtonPGP::new_sync();
    let account = setup_account(&pgp);
    let manager = build_manager(&account, &TestCounts::default())
        .build()
        .unwrap();

    let err = manager
        .address_keys_for_email(
            &pgp,
            "other@example.com",
            false,
            PublicAddressKeyApiFetchPolicy::AllowCachedFallback,
            PublicAddressKeyContactFetchPolicy::AllowCachedFallback,
        )
        .await
        .map(|_| ())
        .unwrap_err();

    assert!(
        matches!(err, KeyHandlingError::NoPublicKeyLoader),
        "expected NoPublicKeyLoader, got {err:?}"
    );
}

#[tokio::test]
async fn address_keys_for_email_other_provides_encryption_key() {
    let pgp = ProtonPGP::new_sync();
    let account = setup_account(&pgp);
    let armored = setup_public_address_keys(&pgp);
    let manager = build_manager(&account, &TestCounts::default())
        .with_public_key_loader(Arc::new(MockPublicKeyLoader {
            armored_public_key: armored,
        }))
        .build()
        .unwrap();

    let selector = manager
        .address_keys_for_email(
            &pgp,
            "other@example.com",
            false,
            PublicAddressKeyApiFetchPolicy::AllowCachedFallback,
            PublicAddressKeyContactFetchPolicy::AllowCachedFallback,
        )
        .await
        .unwrap();

    assert!(selector.for_encryption().is_ok());
}

#[tokio::test]
async fn address_keys_for_email_owned_provides_encryption_key() {
    let pgp = ProtonPGP::new_sync();
    let account = setup_account(&pgp);
    let manager = build_manager_with_active(&account, &TestCounts::default(), true)
        .build()
        .unwrap();

    let selector = manager
        .address_keys_for_email(
            &pgp,
            &account.address_email,
            false,
            PublicAddressKeyApiFetchPolicy::AllowCachedFallback,
            PublicAddressKeyContactFetchPolicy::AllowCachedFallback,
        )
        .await
        .unwrap();

    assert!(selector.for_encryption().is_ok());
}

#[tokio::test]
async fn address_keys_for_email_owned_provides_inbox_encryption_preferences() {
    let pgp = ProtonPGP::new_sync();
    let account = setup_account(&pgp);
    let manager = build_manager_with_active(&account, &TestCounts::default(), true)
        .build()
        .unwrap();

    let selector = manager
        .address_keys_for_email(
            &pgp,
            &account.address_email,
            false,
            PublicAddressKeyApiFetchPolicy::AllowCachedFallback,
            PublicAddressKeyContactFetchPolicy::AllowCachedFallback,
        )
        .await
        .unwrap();

    let prefs = selector.for_inbox_encryption(
        false,
        CryptoMailSettings::default(),
        UnixTimestamp(9_773_399_837),
    );
    assert!(
        prefs.is_ok(),
        "inbox encryption prefs should succeed for owned address"
    );
}

#[tokio::test]
async fn address_keys_for_email_other_provides_inbox_encryption_preferences() {
    use proton_crypto_account::proton_crypto::crypto::UnixTimestamp;

    let pgp = ProtonPGP::new_sync();
    let account = setup_account(&pgp);
    let armored = setup_public_address_keys(&pgp);
    let manager = build_manager(&account, &TestCounts::default())
        .with_public_key_loader(Arc::new(MockPublicKeyLoader {
            armored_public_key: armored,
        }))
        .build()
        .unwrap();

    let selector = manager
        .address_keys_for_email(
            &pgp,
            "other@example.com",
            false,
            PublicAddressKeyApiFetchPolicy::AllowCachedFallback,
            PublicAddressKeyContactFetchPolicy::AllowCachedFallback,
        )
        .await
        .unwrap();

    let prefs = selector.for_inbox_encryption(
        false,
        CryptoMailSettings::default(),
        UnixTimestamp(9_773_399_837),
    );

    assert!(
        prefs.is_ok(),
        "inbox encryption prefs should succeed for API keys"
    );
}

#[tokio::test]
async fn address_keys_for_email_other_provides_signature_verification() {
    let pgp = ProtonPGP::new_sync();
    let account = setup_account(&pgp);
    let armored = setup_public_address_keys(&pgp);
    let manager = build_manager(&account, &TestCounts::default())
        .with_public_key_loader(Arc::new(MockPublicKeyLoader {
            armored_public_key: armored,
        }))
        .build()
        .unwrap();

    let selector = manager
        .address_keys_for_email(
            &pgp,
            "other@example.com",
            false,
            PublicAddressKeyApiFetchPolicy::AllowCachedFallback,
            PublicAddressKeyContactFetchPolicy::AllowCachedFallback,
        )
        .await
        .unwrap();

    assert!(!selector.for_signature_verification().api_keys.is_empty());
}

#[tokio::test]
async fn address_keys_for_email_owned_provides_signature_verification() {
    let pgp = ProtonPGP::new_sync();
    let account = setup_account(&pgp);
    let manager = build_manager_with_active(&account, &TestCounts::default(), true)
        .build()
        .unwrap();

    let selector = manager
        .address_keys_for_email(
            &pgp,
            &account.address_email,
            false,
            PublicAddressKeyApiFetchPolicy::AllowCachedFallback,
            PublicAddressKeyContactFetchPolicy::AllowCachedFallback,
        )
        .await
        .unwrap();

    assert!(!selector.for_signature_verification().api_keys.is_empty());
}

#[tokio::test]
async fn address_keys_for_email_owned_does_not_call_public_key_loader() {
    struct CountingPublicKeyLoader(Arc<AtomicUsize>);

    #[async_trait]
    impl PublicKeyLoader for CountingPublicKeyLoader {
        async fn load_public_address_keys(
            &self,
            _email: &str,
            _internal_only: bool,
            _fetch_policy: PublicAddressKeyApiFetchPolicy,
        ) -> LoadingResult<APIPublicAddressKeys> {
            self.0.fetch_add(1, Ordering::SeqCst);
            Err(LoadingError::Other("should not be called".into()))
        }
    }

    let pgp = ProtonPGP::new_sync();
    let account = setup_account(&pgp);
    let public_loader_call_count = Arc::new(AtomicUsize::new(0));

    let manager = build_manager_with_active(&account, &TestCounts::default(), true)
        .with_public_key_loader(Arc::new(CountingPublicKeyLoader(
            public_loader_call_count.clone(),
        )))
        .build()
        .unwrap();

    manager
        .address_keys_for_email(
            &pgp,
            &account.address_email,
            false,
            PublicAddressKeyApiFetchPolicy::AllowCachedFallback,
            PublicAddressKeyContactFetchPolicy::AllowCachedFallback,
        )
        .await
        .unwrap();

    assert_eq!(
        public_loader_call_count.load(Ordering::SeqCst),
        0,
        "public key loader must not be called for a self-owned address"
    );
}

#[tokio::test]
async fn public_address_keys_loads_and_imports_successfully() {
    let pgp = ProtonPGP::new_sync();
    let account = setup_account(&pgp);
    let counts = TestCounts::default();
    let armored = setup_public_address_keys(&pgp);
    let public_key_loader = Arc::new(MockPublicKeyLoader {
        armored_public_key: armored,
    });
    let manager = build_manager(&account, &counts)
        .with_public_key_loader(public_key_loader)
        .build()
        .unwrap();

    let selector = manager
        .address_keys_for_email(
            &pgp,
            "other@example.com",
            false,
            PublicAddressKeyApiFetchPolicy::AllowCachedFallback,
            PublicAddressKeyContactFetchPolicy::AllowCachedFallback,
        )
        .await
        .unwrap();
    assert!(selector.for_encryption().is_ok());
}

#[tokio::test]
async fn public_address_keys_from_contacts_returns_none_when_no_card() {
    let pgp = ProtonPGP::new_sync();
    let account = setup_account(&pgp);
    let contact_loader = Arc::new(MockContactPublicKeyLoader { signed_vcard: None });
    let manager = build_manager(&account, &TestCounts::default())
        .with_public_contact_key_loader(contact_loader)
        .with_public_key_loader(Arc::new(MockPublicKeyLoader {
            armored_public_key: setup_public_address_keys(&pgp),
        }))
        .build()
        .unwrap();

    let selector = manager
        .address_keys_for_email(
            &pgp,
            "other@example.com",
            false,
            PublicAddressKeyApiFetchPolicy::AllowCachedFallback,
            PublicAddressKeyContactFetchPolicy::AllowCachedFallback,
        )
        .await
        .unwrap();

    let AddressKeyForEmailSelector::Other { vcard_keys, .. } = selector else {
        panic!("expected Other variant");
    };
    assert!(vcard_keys.is_none());
}

#[tokio::test]
#[cfg(feature = "contacts")]
async fn public_address_keys_from_contacts_returns_pinned_keys() {
    let pgp = ProtonPGP::new_sync();
    let account = setup_account(&pgp);
    let counts = TestCounts::default();

    let manager = build_manager(&account, &counts).build().unwrap();

    let user_key_selector = manager.user_keys(&pgp).await.unwrap();

    let signed_vcard = setup_signed_contact_card(
        &pgp,
        user_key_selector.primary().unwrap(),
        "other@example.com",
    );

    let contact_loader = Arc::new(MockContactPublicKeyLoader {
        signed_vcard: Some(signed_vcard),
    });
    let manager = build_manager(&account, &TestCounts::default())
        .with_public_contact_key_loader(contact_loader)
        .with_public_key_loader(Arc::new(MockPublicKeyLoader {
            armored_public_key: setup_public_address_keys(&pgp),
        }))
        .build()
        .unwrap();

    let selector = manager
        .address_keys_for_email(
            &pgp,
            "other@example.com",
            false,
            PublicAddressKeyApiFetchPolicy::AllowCachedFallback,
            PublicAddressKeyContactFetchPolicy::AllowCachedFallback,
        )
        .await
        .unwrap();

    let AddressKeyForEmailSelector::Other { vcard_keys, .. } = selector else {
        panic!("expected Other variant");
    };
    assert_eq!(vcard_keys.unwrap().pinned_keys.len(), 1);
}

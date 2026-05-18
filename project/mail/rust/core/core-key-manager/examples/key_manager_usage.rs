//! Example: using the key manager to load and work with Proton `OpenPGP` keys.

use std::sync::Arc;

use async_trait::async_trait;
use base64::Engine as _;
use base64::engine::general_purpose::STANDARD;
use mail_core_key_manager::cache::MemoryKeyCache;
use mail_core_key_manager::traits::{
    AddressWithKeys, CacheAccess, ContactPublicKeyLoader, KeySecretLoader, LockedPrivateKeyLoader,
    PublicKeyLoader, SignedVCard,
};
use mail_core_key_manager::{
    AddressId, KeyManager, KeySelector, PublicAddressKeyApiFetchPolicy,
    PublicAddressKeyContactFetchPolicy, UserId,
};
use proton_crypto_account::contacts::EncryptableAndSignableCard;
use proton_crypto_account::keys::{
    APIPublicAddressKeyGroup, APIPublicAddressKeys, APIPublicKey, APIPublicKeySource, AddressKeys,
    CryptoMailSettings, KeyFlag, KeyId, LocalAddressKey, LocalUserKey, LockedKey, UnlockedUserKey,
    UserKeys,
};
use proton_crypto_account::proton_crypto::crypto::{
    DataEncoding, Decryptor, DecryptorSync, Encryptor, EncryptorSync, KeyGenerator,
    KeyGeneratorAlgorithm, KeyGeneratorSync, PGPProviderSync, VerifiedData, Verifier, VerifierSync,
};
use proton_crypto_account::proton_crypto::{ProtonPGP, crypto_clock};
use proton_crypto_account::salts::KeySecret;

#[tokio::main]
#[allow(clippy::print_stdout)]
async fn main() {
    let pgp = ProtonPGP::new_sync();
    let account = create_dummy_account(&pgp);

    // Here we define dummy services. In a real application the app would implement proper services.
    let raw_secret_loader = HardcodedSecretLoader(account.password.clone());
    let secret_loader = Arc::new(raw_secret_loader.clone());

    let key_loader_impl = HardcodedKeyLoader {
        user_keys: account.user_keys.clone(),
        address_id: account.address_id.clone(),
        address_keys: account.address_keys.clone(),
        email: account.email.clone(),
    };
    let key_loader = Arc::new(key_loader_impl.clone());
    let tmp_key_manager = KeyManager::builder(account.user_id.clone())
        .with_private_key_loader(key_loader.clone())
        .with_key_secret_loader(secret_loader.clone())
        .build()
        .unwrap();
    let (external_armored, signed_vcard) =
        create_external_contact_data(&pgp, &tmp_key_manager).await;

    let public_key_loader_impl = HardcodedPublicKeyLoader(external_armored);
    let contact_key_loader_impl = HardcodedContactKeyLoader(signed_vcard);

    let setup = ExampleSetup {
        account,
        raw_secret_loader,
        key_loader_impl,
        public_key_loader_impl,
        contact_key_loader_impl,
        cache: MemoryKeyCache::default(),
    };

    // INTERESTING PART STARTS HERE

    // Run the key manager example
    key_manager_example(&pgp, &setup).await;

    // Run the key selector example
    key_selector_example(&pgp, setup).await;
}

#[allow(clippy::print_stdout)]
async fn key_manager_example<P: PGPProviderSync>(pgp: &P, setup: &ExampleSetup) {
    let ExampleSetup {
        account,
        raw_secret_loader,
        key_loader_impl,
        public_key_loader_impl,
        contact_key_loader_impl,
        cache: _,
    } = setup;

    // [`crate::traits::KeySecretLoader`]
    let secret_loader = Arc::new(raw_secret_loader.clone());
    // [`crate::traits::LockedPrivateKeyLoader`]
    let key_loader = Arc::new(key_loader_impl.clone());
    // [`crate::traits::PublicKeyLoader`]
    let public_key_loader: Arc<dyn PublicKeyLoader> = Arc::new(public_key_loader_impl.clone());
    // [`crate::traits::ContactPublicKeyLoader`]
    let contact_key_loader: Arc<dyn ContactPublicKeyLoader> =
        Arc::new(contact_key_loader_impl.clone());

    // Build the key manager
    // The public key loader and the contact key loader are optional.
    let key_manager = KeyManager::builder(account.user_id.clone())
        .with_private_key_loader(key_loader)
        .with_key_secret_loader(secret_loader)
        .with_public_key_loader(public_key_loader)
        .with_public_contact_key_loader(contact_key_loader)
        .with_default_key_cache()
        .build()
        .unwrap();

    let message = "Hello, world!";

    // Encrypt/decrypt with user keys
    let user_key_selector = key_manager.user_keys(pgp).await.unwrap();
    let encrypted_message = pgp
        .new_encryptor()
        .with_encryption_key(user_key_selector.for_encryption().unwrap())
        .with_signing_key(user_key_selector.for_signing().unwrap())
        .encrypt_raw(message.as_bytes(), DataEncoding::Armor)
        .unwrap();
    let decrypted_message = pgp
        .new_decryptor()
        .with_decryption_key_refs(user_key_selector.for_decryption())
        .with_verification_key_refs(user_key_selector.for_signature_verification())
        .decrypt(encrypted_message, DataEncoding::Armor)
        .unwrap()
        .try_into_verified_vec()
        .unwrap();
    assert_eq!(decrypted_message, message.as_bytes());

    // Select own address keys for a specific address id
    let own_address_key_selector = key_manager
        .address_keys(pgp, &account.address_id)
        .await
        .unwrap();
    let encrypted_message = pgp
        .new_encryptor()
        .with_encryption_key(own_address_key_selector.for_encryption().unwrap())
        .with_signing_key(own_address_key_selector.for_signing().unwrap())
        .encrypt_raw(message.as_bytes(), DataEncoding::Armor)
        .unwrap();
    let decrypted_message = pgp
        .new_decryptor()
        .with_decryption_key_refs(own_address_key_selector.for_decryption())
        .with_verification_key_refs(own_address_key_selector.for_signature_verification())
        .decrypt(encrypted_message, DataEncoding::Armor)
        .unwrap()
        .try_into_verified_vec()
        .unwrap();
    assert_eq!(decrypted_message, message.as_bytes());

    // Select public address keys for a recipient and show send/verification preferences
    // If the email address is owned by the user, the key manager will load the self owned public keys for this specific email address / identity.
    // If the email address is not owned by the user, the key manager will load the public keys from the API and the contact model.
    let public_address_key_selector = key_manager
        .address_keys_for_email(
            pgp,
            "bob@example.com", // In this case we load the public keys for this specific email address / identity.
            false,
            PublicAddressKeyApiFetchPolicy::AllowCachedFallback,
            PublicAddressKeyContactFetchPolicy::AllowCachedFallback,
        )
        .await
        .unwrap();

    // Basic encryption with the public key. Ignores encryption preferences and just uses the primary key.
    let _encrypted_message = pgp
        .new_encryptor()
        .with_encryption_key(public_address_key_selector.for_encryption().unwrap())
        .encrypt_raw(message.as_bytes(), DataEncoding::Armor)
        .unwrap();

    // Load encryption preferences to select the appropriate key for encryption.
    // The recipient might not have a valid encryption key according to its encryption preferences.
    // Mostly used in mail/calendar for sharing or sending emails.
    let encryption_preferences = public_address_key_selector
        .for_inbox_encryption(
            true,
            CryptoMailSettings::default(),
            crypto_clock().unix_time(),
        )
        .unwrap();

    let _encrypted_message = pgp
        .new_encryptor()
        .with_encryption_key(encryption_preferences.selected_key.as_ref().unwrap())
        .encrypt_raw(message.as_bytes(), DataEncoding::Armor)
        .unwrap();

    // Send preferences inbox specific code.
    // let send_preferences =
    //     SendPreferences::from_preferences(encryption_preferences, ComposerPreference::default());
    // println!("Send preferences (KeyManager): {send_preferences}");

    // Load the signature verification preferences to select the appropriate keys for signature verification.
    // The verification preferences are loaded from the public address keys and the pinned public address keys,
    // and handle the logic to select the appropriate keys for signature verification.
    let signature_verification_preferences =
        public_address_key_selector.for_signature_verification();
    println!(
        "Are pinned keys used (KeyManager): {}",
        signature_verification_preferences.uses_pinned_keys()
    );

    pgp.new_verifier()
        .with_verification_keys(signature_verification_preferences.signature_verification_keys())
        .verify_detached(message, b"dummy", DataEncoding::Bytes)
        .expect_err("dummy is not a signature");
}

#[allow(clippy::print_stdout)]
async fn key_selector_example<P: PGPProviderSync>(pgp: &P, setup: ExampleSetup) {
    let ctx = setup;

    // Same as the key manager example, but with a short lived key selector.

    let message = "Hello, world!";
    let user_id = ctx.account.user_id.clone();
    let address_id = ctx.account.address_id.clone();

    let key_selector = KeySelector::new(&user_id, ctx);

    let key_selector_user_keys = key_selector.user_keys(pgp).await.unwrap();
    let encrypted_via_selector = pgp
        .new_encryptor()
        .with_encryption_key(key_selector_user_keys.for_encryption().unwrap())
        .with_signing_key(key_selector_user_keys.for_signing().unwrap())
        .encrypt_raw(message.as_bytes(), DataEncoding::Armor)
        .unwrap();
    let decrypted_via_selector = pgp
        .new_decryptor()
        .with_decryption_key_refs(key_selector_user_keys.for_decryption())
        .with_verification_key_refs(key_selector_user_keys.for_signature_verification())
        .decrypt(encrypted_via_selector, DataEncoding::Armor)
        .unwrap()
        .try_into_verified_vec()
        .unwrap();
    assert_eq!(decrypted_via_selector, message.as_bytes());

    let key_selector_address_keys = key_selector.address_keys(pgp, &address_id).await.unwrap();
    let _encrypted_address = pgp
        .new_encryptor()
        .with_encryption_key(key_selector_address_keys.for_encryption().unwrap())
        .with_signing_key(key_selector_address_keys.for_signing().unwrap())
        .encrypt_raw(message.as_bytes(), DataEncoding::Armor)
        .unwrap();

    let key_selector_public_address = key_selector
        .address_keys_for_email(
            pgp,
            "bob@example.com",
            false,
            PublicAddressKeyApiFetchPolicy::AllowCachedFallback,
            PublicAddressKeyContactFetchPolicy::AllowCachedFallback,
        )
        .await
        .unwrap();
    let _encrypted_to_bob = pgp
        .new_encryptor()
        .with_encryption_key(key_selector_public_address.for_encryption().unwrap())
        .encrypt_raw(message.as_bytes(), DataEncoding::Armor)
        .unwrap();
    let _encryption_preferences = key_selector_public_address
        .for_inbox_encryption(
            true,
            CryptoMailSettings::default(),
            crypto_clock().unix_time(),
        )
        .unwrap();

    // Inbox send preference from encryption preferences.
    // let _send_prefs_via_selector =
    //     SendPreferences::from_preferences(encryption_preferences, ComposerPreference::default());

    let _verification_prefs = key_selector_public_address.for_signature_verification();
}

struct ExampleSetup {
    account: DummyAccount,
    raw_secret_loader: HardcodedSecretLoader,
    key_loader_impl: HardcodedKeyLoader,
    public_key_loader_impl: HardcodedPublicKeyLoader,
    contact_key_loader_impl: HardcodedContactKeyLoader,
    cache: MemoryKeyCache,
}

#[async_trait]
impl KeySecretLoader for ExampleSetup {
    async fn key_secret(&self) -> mail_core_key_manager::error::LoadingResult<Option<KeySecret>> {
        self.raw_secret_loader.key_secret().await
    }
}

#[async_trait]
impl LockedPrivateKeyLoader for ExampleSetup {
    async fn load_user_keys(
        &self,
        id: &UserId,
    ) -> mail_core_key_manager::error::LoadingResult<Option<UserKeys>> {
        self.key_loader_impl.load_user_keys(id).await
    }

    async fn load_address_keys(
        &self,
        id: &AddressId,
    ) -> mail_core_key_manager::error::LoadingResult<Option<AddressWithKeys>> {
        self.key_loader_impl.load_address_keys(id).await
    }

    async fn load_address_keys_by_email(
        &self,
        email: &str,
    ) -> mail_core_key_manager::error::LoadingResult<Option<AddressWithKeys>> {
        self.key_loader_impl.load_address_keys_by_email(email).await
    }
}

#[async_trait]
impl PublicKeyLoader for ExampleSetup {
    async fn load_public_address_keys(
        &self,
        email: &str,
        internal_only: bool,
        fetch_policy: PublicAddressKeyApiFetchPolicy,
    ) -> mail_core_key_manager::error::LoadingResult<APIPublicAddressKeys> {
        self.public_key_loader_impl
            .load_public_address_keys(email, internal_only, fetch_policy)
            .await
    }
}

#[async_trait]
impl ContactPublicKeyLoader for ExampleSetup {
    async fn load_signed_contact_card(
        &self,
        email: &str,
        fetch_policy: PublicAddressKeyContactFetchPolicy,
    ) -> mail_core_key_manager::error::LoadingResult<Option<SignedVCard>> {
        self.contact_key_loader_impl
            .load_signed_contact_card(email, fetch_policy)
            .await
    }
}

impl CacheAccess for ExampleSetup {
    fn key_cache(&self) -> Option<&MemoryKeyCache> {
        Some(&self.cache)
    }
}

struct DummyAccount {
    user_id: UserId,
    address_id: AddressId,
    email: String,
    user_keys: UserKeys,
    address_keys: AddressKeys,
    password: Vec<u8>,
}

fn create_dummy_account<P: PGPProviderSync>(pgp: &P) -> DummyAccount {
    let password = b"password".to_vec();
    let key_secret = KeySecret::new(password.clone());
    let user_key_id = KeyId("user_key_1".to_string());

    let local_user_key =
        LocalUserKey::generate(pgp, KeyGeneratorAlgorithm::ECC, &key_secret).unwrap();
    let unlocked_user_key = local_user_key
        .unlock_and_assign_key_id(pgp, user_key_id.clone(), &key_secret)
        .unwrap();

    let local_address_key = LocalAddressKey::generate(
        pgp,
        "alice@example.com",
        KeyGeneratorAlgorithm::ECC,
        KeyFlag::default(),
        true,
        &unlocked_user_key,
    )
    .unwrap();

    DummyAccount {
        user_id: UserId::new("user_1".to_string()),
        address_id: AddressId::new("address_1".to_string()),
        email: "alice@example.com".to_string(),
        user_keys: UserKeys(vec![LockedKey {
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
        }]),
        address_keys: AddressKeys(vec![LockedKey {
            id: KeyId("address_key_1".to_string()),
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
        }]),
        password,
    }
}

async fn create_external_contact_data<P: PGPProviderSync>(
    pgp: &P,
    key_manager: &KeyManager,
) -> (String, SignedVCard) {
    let signing_key = key_manager.user_keys(pgp).await.unwrap();
    let external_armored = create_external_public_key(pgp);
    let signed_vcard = create_signed_vcard(pgp, &external_armored, signing_key.primary().unwrap());
    (external_armored, signed_vcard)
}

fn create_external_public_key<P: PGPProviderSync>(pgp: &P) -> String {
    let key = pgp
        .new_key_generator()
        .with_user_id("bob@example.com", "bob@example.com")
        .generate()
        .unwrap();
    let public_key = pgp.private_key_to_public_key(&key).unwrap();
    let armored = pgp
        .public_key_export(&public_key, DataEncoding::Armor)
        .unwrap();
    std::str::from_utf8(armored.as_ref()).unwrap().to_string()
}

struct VCardSigner(String);

impl EncryptableAndSignableCard for VCardSigner {
    fn plaintext_card_data(&self) -> &[u8] {
        self.0.as_bytes()
    }
}

fn create_signed_vcard<P: PGPProviderSync>(
    pgp: &P,
    contact_key: &str,
    signing_key: &UnlockedUserKey<P>,
) -> SignedVCard {
    let public_key = pgp
        .public_key_import(contact_key.as_bytes(), DataEncoding::Armor)
        .unwrap();
    let public_key_bytes = pgp
        .public_key_export(&public_key, DataEncoding::Bytes)
        .unwrap();
    let public_key_base64 = STANDARD.encode(public_key_bytes.as_ref());

    let card = VCardSigner(format!(
        "BEGIN:VCARD\nVERSION:4.0\nFN:Bob\nITEM1.EMAIL:bob@example.com\nITEM1.KEY;PREF=1:data:application/pgp-keys;base64,{public_key_base64}\nEND:VCARD"
    ));
    let signature = card.sign_sync(pgp, signing_key).unwrap();

    SignedVCard {
        data: card.0,
        signature: signature.0,
    }
}

#[derive(Clone)]
struct HardcodedKeyLoader {
    user_keys: UserKeys,
    address_id: AddressId,
    address_keys: AddressKeys,
    email: String,
}

#[async_trait]
impl LockedPrivateKeyLoader for HardcodedKeyLoader {
    async fn load_user_keys(
        &self,
        _: &UserId,
    ) -> mail_core_key_manager::error::LoadingResult<Option<UserKeys>> {
        Ok(Some(self.user_keys.clone()))
    }

    async fn load_address_keys(
        &self,
        _: &AddressId,
    ) -> mail_core_key_manager::error::LoadingResult<Option<AddressWithKeys>> {
        Ok(Some(AddressWithKeys {
            is_external: false,
            email: self.email.clone(),
            address_id: self.address_id.clone(),
            is_active: true,
            keys: self.address_keys.clone(),
        }))
    }

    async fn load_address_keys_by_email(
        &self,
        email: &str,
    ) -> mail_core_key_manager::error::LoadingResult<Option<AddressWithKeys>> {
        if email != self.email {
            return Ok(None);
        }
        Ok(Some(AddressWithKeys {
            is_external: false,
            email: email.to_string(),
            address_id: self.address_id.clone(),
            is_active: true,
            keys: self.address_keys.clone(),
        }))
    }
}

#[derive(Clone)]
struct HardcodedSecretLoader(Vec<u8>);

#[async_trait]
impl KeySecretLoader for HardcodedSecretLoader {
    async fn key_secret(&self) -> mail_core_key_manager::error::LoadingResult<Option<KeySecret>> {
        Ok(Some(KeySecret::new(self.0.clone())))
    }
}

#[derive(Clone)]
struct HardcodedPublicKeyLoader(String);

#[async_trait]
impl PublicKeyLoader for HardcodedPublicKeyLoader {
    async fn load_public_address_keys(
        &self,
        _email: &str,
        _internal_only: bool,
        _fetch_policy: mail_core_key_manager::PublicAddressKeyApiFetchPolicy,
    ) -> mail_core_key_manager::error::LoadingResult<APIPublicAddressKeys> {
        Ok(APIPublicAddressKeys {
            address_keys: APIPublicAddressKeyGroup {
                keys: vec![APIPublicKey {
                    source: APIPublicKeySource::Proton,
                    flags: KeyFlag::default(),
                    primary: true,
                    public_key: self.0.clone(),
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

#[derive(Clone)]
struct HardcodedContactKeyLoader(SignedVCard);

#[async_trait]
impl ContactPublicKeyLoader for HardcodedContactKeyLoader {
    async fn load_signed_contact_card(
        &self,
        _email: &str,
        _fetch_policy: PublicAddressKeyContactFetchPolicy,
    ) -> mail_core_key_manager::error::LoadingResult<Option<SignedVCard>> {
        Ok(Some(self.0.clone()))
    }
}

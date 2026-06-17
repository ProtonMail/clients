use crate::error::SharedCryptoError;
use crate::keys::NewAddrKey;
use crate::keys::OrgManagedKeyMaterial;

use crate::keys::new_key_id;
use lattice::Sensitive;
use lattice::auth::LtAuthAddressId;
use lattice::core::LtCoreUnprivActivationToken;
use lattice::core::keys::LtCoreSetupKeysBody;
use lattice::core::user::LtCoreSrpVerifier;
use lattice::core::{LtCoreAddress, LtCoreAddressFlags};
use proton_crypto::crypto::{DataEncoding, Encryptor, EncryptorSync, PGPProviderSync};
use proton_crypto_account::keys::{LocalUserKey, UnlockedUserKey};
use proton_crypto_account::proton_crypto::crypto::KeyGeneratorAlgorithm;
use proton_crypto_account::proton_crypto::srp::SRPProvider;
use proton_crypto_account::salts::{KeySalt, KeySecret};

const ORG_ACTIVATION_TOKEN_SIGNING_CONTEXT: &str = "account.key-token.user-unprivatization";

pub struct NewUserKey {
    pub key: LocalUserKey,
    pub salt: KeySalt,
    pub pass: KeySecret,
}

impl std::fmt::Debug for NewUserKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "NewUserKey {{ key: redacted, salt: redacted, pass: redacted }}"
        )
    }
}

impl NewUserKey {
    pub fn init(
        srp: &impl SRPProvider,
        pgp: &impl PGPProviderSync,
        pass: impl AsRef<[u8]>,
    ) -> Result<Self, SharedCryptoError> {
        let algo = KeyGeneratorAlgorithm::ECC;
        let salt = KeySalt::generate();
        let pass = salt.salted_key_passphrase(srp, pass.as_ref())?;
        let key = LocalUserKey::generate(pgp, algo, &pass)?;

        Ok(Self { key, salt, pass })
    }

    pub fn unlock_user_key<P: PGPProviderSync>(
        &self,
        pgp: &P,
    ) -> Result<UnlockedUserKey<P>, SharedCryptoError> {
        Ok(self
            .key
            .unlock_and_assign_key_id(pgp, new_key_id(), &self.pass)?)
    }

    pub fn init_addr_key(
        &self,
        pgp: &impl PGPProviderSync,
        address: impl Into<AddressMetadata>,
    ) -> Result<NewAddrKey, SharedCryptoError> {
        let address = address.into();
        let key_id = new_key_id();
        let user_key = self.key.unlock_and_assign_key_id(pgp, key_id, &self.pass)?;

        NewAddrKey::init(pgp, &user_key, address)
    }

    pub fn generate_new_addr_keys(
        &self,
        pgp: &impl PGPProviderSync,
        addresses: impl IntoIterator<Item = impl Into<AddressMetadata>>,
    ) -> Result<Vec<NewAddrKey>, SharedCryptoError> {
        addresses
            .into_iter()
            .map(|address| self.init_addr_key(pgp, address))
            .collect()
    }

    pub fn generate_org_managed_key_material<P: PGPProviderSync>(
        &self,
        pgp: &P,
        org_token: &KeySecret,
        org_public_key_armor: &str,
        primary_addr_key: &NewAddrKey,
    ) -> Result<OrgManagedKeyMaterial, SharedCryptoError> {
        let unlocked_user = self.unlock_user_key(pgp)?;
        let unlocked_addr = primary_addr_key.unlock(pgp, &unlocked_user)?;
        let org_public =
            pgp.public_key_import(org_public_key_armor.as_bytes(), DataEncoding::Armor)?;
        let signing_context =
            pgp.new_signing_context(ORG_ACTIVATION_TOKEN_SIGNING_CONTEXT.to_owned(), true);
        let encrypted = pgp
            .new_encryptor()
            .with_encryption_key(&org_public)
            .with_signing_key(&unlocked_addr.private_key)
            .with_signing_context(&signing_context)
            .encrypt_raw(org_token.as_ref(), DataEncoding::Armor)?;
        let activation_token = Sensitive::new(String::from_utf8(encrypted)?);
        let primary_user_key = LocalUserKey::relock_user_key(pgp, &unlocked_user, org_token)?;
        Ok(OrgManagedKeyMaterial {
            activation_token: LtCoreUnprivActivationToken(activation_token),
            primary_user_key,
        })
    }

    pub fn into_setup_keys_body(
        self,
        auth: LtCoreSrpVerifier,
        new_addr_keys: Vec<NewAddrKey>,
        encrypted_secret: Option<Sensitive<String>>,
        org_primary_user_key: Option<Sensitive<String>>,
        org_activation_token: Option<LtCoreUnprivActivationToken>,
    ) -> LtCoreSetupKeysBody {
        LtCoreSetupKeysBody {
            auth,
            primary_key: Sensitive::new(self.key.private_key.0),
            key_salt: Sensitive::new(self.salt.0),
            address_keys: new_addr_keys
                .into_iter()
                .map(|key| key.into_address_key_input())
                .collect(),
            encrypted_secret,
            org_primary_user_key,
            org_activation_token,
        }
    }
}

pub struct AddressMetadata {
    pub(crate) address_id: LtAuthAddressId,
    pub(crate) email: String,
    pub(crate) flags: LtCoreAddressFlags,
    pub(crate) order: u32,
}

impl From<LtCoreAddress> for AddressMetadata {
    fn from(address: LtCoreAddress) -> Self {
        Self {
            address_id: address.id,
            email: address.email,
            flags: address.flags,
            order: address.order,
        }
    }
}

impl AddressMetadata {
    pub fn new(
        address_id: LtAuthAddressId,
        email: impl Into<String>,
        flags: LtCoreAddressFlags,
        order: u32,
    ) -> Self {
        Self {
            address_id,
            email: email.into(),
            flags,
            order,
        }
    }
}

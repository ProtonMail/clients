use crate::error::SharedCryptoError;
use crate::keys::new_key_id;
use crate::keys::new_user_key::AddressMetadata;

use lattice::Sensitive;
use lattice::core::{LtCoreAddressFlags, LtCoreAddressKeyInput};
use proton_crypto_account::keys::{
    KeyFlag, LocalAddressKey, LocalSignedKeyList, UnlockedAddressKey, UnlockedAddressKeys,
    UnlockedUserKey,
};
use proton_crypto_account::proton_crypto::crypto::{KeyGeneratorAlgorithm, PGPProviderSync};

#[allow(missing_debug_implementations)]
pub struct NewAddrKey {
    pub(crate) address: AddressMetadata,
    pub local_address_key: LocalAddressKey,
    pub signed_key_list: LocalSignedKeyList,
}

impl NewAddrKey {
    pub fn init<P: PGPProviderSync>(
        pgp: &P,
        user_key: &UnlockedUserKey<P>,
        address: AddressMetadata,
    ) -> Result<Self, SharedCryptoError> {
        let algo = KeyGeneratorAlgorithm::ECC;

        let flags = new_key_flags(address.flags);
        let primary = true;

        let local_address_key =
            LocalAddressKey::generate(pgp, address.email.as_str(), algo, flags, primary, user_key)?;

        let signed_key_list = create_addr_skl(pgp, user_key, &local_address_key)?;

        Ok(Self {
            local_address_key,
            signed_key_list,
            address,
        })
    }

    pub fn unlock<P: PGPProviderSync>(
        &self,
        pgp: &P,
        user_key: &UnlockedUserKey<P>,
    ) -> Result<UnlockedAddressKey<P>, SharedCryptoError> {
        Ok(self
            .local_address_key
            .unlock_and_assign_key_id(pgp, new_key_id(), user_key)?)
    }

    pub fn into_address_key_input(self) -> LtCoreAddressKeyInput {
        LtCoreAddressKeyInput {
            address_id: self.address.address_id,
            private_key: Sensitive::new(self.local_address_key.private_key.0),
            token: self.local_address_key.token.map(|t| Sensitive::new(t.0)),
            signature: self
                .local_address_key
                .signature
                .map(|t| Sensitive::new(t.0)),
            signed_key_list: self.signed_key_list.into(),
            revision: 0,
            primary: 1,
        }
    }
}

fn create_addr_skl<P: PGPProviderSync>(
    pgp: &P,
    user_key: &UnlockedUserKey<P>,
    addr_key: &LocalAddressKey,
) -> Result<LocalSignedKeyList, SharedCryptoError> {
    let addr_key = addr_key.unlock_and_assign_key_id(pgp, new_key_id(), user_key)?;
    Ok(LocalSignedKeyList::generate(
        pgp,
        &UnlockedAddressKeys(vec![addr_key]),
    )?)
}

/// Generates a key flags from address flags.
fn new_key_flags(address_flags: LtCoreAddressFlags) -> KeyFlag {
    let mut flags = KeyFlag::default();

    if address_flags.contains(LtCoreAddressFlags::DisableE2EE) {
        flags.set_email_no_encryption();
    }

    if address_flags.contains(LtCoreAddressFlags::DisableExpectedSigned) {
        flags.set_email_no_sign();
    }

    flags
}

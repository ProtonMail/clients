//! Org member lookup, address pagination, and shared org-token helpers for integration tests.

use std::num::NonZeroU32;

use data_encoding::HEXLOWER;
use lattice::LtSlimApiPageQuery;
use lattice::core::addresses::{ADDRESSES_LIST_MAX_PAGE_SIZE, LtCoreAddressesListQuery};
use lattice::core::get_members::{LtCoreGetMembersRes, LtCoreMemberInfo};
use lattice::core::keys::LtCoreGetKeySaltsReq;
use lattice::core::members::addresses::LtCoreGetMembersMemberIDAddressesReq;
use lattice::core::user::LtCoreUser;
use lattice::core::user::get_users::LtCoreGetUsersReq;
use lattice::core::{LtCoreAddressesListRes, LtCoreMemberEncId};
use lattice_muon2::LtTransportError;
use proton_crypto::crypto::{
    DataEncoding, Decryptor, DecryptorSync, PGPProviderSync, VerifiedData,
};
use proton_crypto::new_srp_provider;
use proton_crypto_account::keys::KeyId;
use proton_crypto_account::salts::{KeySecret, SaltError, Salts};

use super::Session;
pub use super::org_member_error::OrgMemberError;

pub fn transport_err(msg: impl ToString) -> LtTransportError {
    LtTransportError::from(lattice::LatticeError::Other(msg.to_string()))
}

pub fn primary_key_id(user: &LtCoreUser) -> Result<KeyId, OrgMemberError> {
    user.keys
        .0
        .0
        .iter()
        .find(|k| k.primary)
        .map(|k| k.id.clone())
        .ok_or(OrgMemberError::NoPrimaryUserKey)
}

pub fn derive_key_passphrase(
    key_salts: &Salts,
    primary_key_id: &KeyId,
    password: &[u8],
) -> Result<KeySecret, SaltError> {
    key_salts.salt_for_key(&new_srp_provider(), primary_key_id, password)
}

pub async fn derive_session_key_passphrase(
    session: &Session,
    password: &[u8],
) -> Result<KeySecret, OrgMemberError> {
    let user = session.send_lt(LtCoreGetUsersReq).await?.user;
    let primary_id = primary_key_id(&user)?;
    let salts = session.send_lt(LtCoreGetKeySaltsReq).await?;
    derive_key_passphrase(&salts.key_salts, &primary_id, password)
        .map_err(OrgMemberError::KeyPassphrase)
}

pub fn find_member_by_email<'a>(
    members: &'a LtCoreGetMembersRes,
    email: &str,
) -> Result<&'a LtCoreMemberInfo, OrgMemberError> {
    members
        .members
        .iter()
        .find(|m| m.name == email)
        .ok_or(OrgMemberError::MemberNotFound {
            email: email.to_string(),
            num_members: members.members.len(),
        })
}

pub async fn fetch_member_addresses_paginated(
    admin_session: &Session,
    member_id: &LtCoreMemberEncId,
) -> Result<LtCoreAddressesListRes, LtTransportError> {
    let page_size = NonZeroU32::new(ADDRESSES_LIST_MAX_PAGE_SIZE).expect("valid page size");
    let mut all = Vec::new();
    let mut page = 0u32;
    loop {
        let res = admin_session
            .send_lt(LtCoreGetMembersMemberIDAddressesReq {
                member_id: member_id.clone(),
                query: LtCoreAddressesListQuery {
                    pagination: LtSlimApiPageQuery::new()
                        .with_pagination(Some(page), Some(page_size))
                        .expect("valid pagination"),
                    ..Default::default()
                },
            })
            .await?;
        let count = res.addresses.len();
        all.extend(res.addresses);
        if count < ADDRESSES_LIST_MAX_PAGE_SIZE as usize {
            break;
        }
        page += 1;
    }
    Ok(LtCoreAddressesListRes {
        addresses: all,
        total: None,
    })
}

pub fn organization_token_from_random() -> KeySecret {
    let mut bytes = [0u8; 32];
    rand::Rng::fill(&mut rand::rng(), &mut bytes);
    KeySecret::new(HEXLOWER.encode(&bytes).into_bytes())
}

/// Decrypt org-wrapped armored token with the org private key.
///
/// When `verify` is true, the org public key is used to verify the signature (device approval).
/// When false, only decryption is performed (unprivatization setup tokens).
pub fn decrypt_org_armored_token<P: PGPProviderSync>(
    pgp: &P,
    org_private: &P::PrivateKey,
    org_token_armored: &str,
    verify: bool,
) -> Result<KeySecret, String> {
    if verify {
        let org_public = pgp
            .private_key_to_public_key(org_private)
            .map_err(|e| e.to_string())?;
        let verified = pgp
            .new_decryptor()
            .with_decryption_key(org_private)
            .with_verification_key_refs(&[&org_public])
            .decrypt(org_token_armored.as_bytes(), DataEncoding::Armor)
            .map_err(|e| e.to_string())?;
        Ok(KeySecret::new(verified.to_vec()))
    } else {
        let verified = pgp
            .new_decryptor()
            .with_decryption_key(org_private)
            .decrypt(org_token_armored.as_bytes(), DataEncoding::Armor)
            .map_err(|e| e.to_string())?;
        Ok(KeySecret::new(verified.to_vec()))
    }
}

//! Member `POST /keys/setup` for SSO org-managed unprivatization tests.

use core_key::{NewAddrKey, NewUserKey, build_setup_keys_body, generate_user_and_address_keys};
use lattice::Sensitive;
use lattice::auth::devices::post_auth_devices_create::LtAuthPostDevicesCreateReq;
use lattice::auth::get_auth_modulus::LtAuthGetModulusReq;
use lattice::core::LtCoreAddressesListQuery;
use lattice::core::LtCoreAsyncUserInitialization;
use lattice::core::get_core_addresses::LtCoreGetAddressesReq;
use lattice::core::get_members_me_unprivatize::{
    LtCoreGetMembersMeUnprivatizeReq, LtCoreGetMembersMeUnprivatizeRes,
};
use lattice::core::keys::post_keys_setup::LtCorePostKeysSetupReq;
use lattice::core::unpriv_types::LtCoreUnprivState;
use lattice::core::user::LtCoreSrpVerifier;
use lattice_muon2::LtTransportError;
use proton_crypto::crypto::{DataEncoding, Encryptor, EncryptorSync, PGPProviderSync};
use proton_crypto::new_pgp_provider;
use proton_crypto::new_srp_provider;
use proton_crypto::srp::SRPProvider;
use proton_crypto_account::keys::{KeyId, LocalUserKey};
use proton_crypto_account::salts::KeySecret;

use super::Session;
use super::device_approval::device_secret::DeviceSecret;
use super::org_members::organization_token_from_random;

const ORG_ACTIVATION_TOKEN_SIGNING_CONTEXT: &str = "account.key-token.user-unprivatization";

pub async fn setup_sso_member_keys(
    session: &Session,
    backup_password: &str,
) -> Result<(), LtTransportError> {
    let me: LtCoreGetMembersMeUnprivatizeRes =
        session.send_lt(LtCoreGetMembersMeUnprivatizeReq).await?;
    assert_eq!(
        me.state,
        LtCoreUnprivState::Pending,
        "member unprivatization should be Pending before keys/setup"
    );
    let org_public_key = me.org_public_key.ok_or_else(|| {
        LtTransportError::from(lattice::LatticeError::Other(
            "missing org_public_key on GET /members/me/unprivatize".into(),
        ))
    })?;

    let addresses = session
        .send_lt(LtCoreGetAddressesReq {
            query: LtCoreAddressesListQuery::default(),
        })
        .await?;

    let addr_inputs: Vec<_> = addresses
        .addresses
        .iter()
        .map(|a| (a.id.as_str(), a.email.as_str(), a.flags))
        .collect();

    let (user_key, addr_keys) = generate_user_and_address_keys(backup_password, addr_inputs)
        .map_err(|e| {
            LtTransportError::from(lattice::LatticeError::Other(format!("generate keys: {e}")))
        })?;

    let device_secret = DeviceSecret::random();
    let encrypted_secret = device_secret
        .encrypt_passphrase(user_key.pass.as_ref())
        .map_err(|e| {
            LtTransportError::from(lattice::LatticeError::Other(format!(
                "encrypted secret: {e}"
            )))
        })?;

    // Required by `POST /keys/setup` (EncryptedSecret on the body), not used in device-approval tests.
    let _device_res = session
        .send_lt(LtAuthPostDevicesCreateReq {
            name: "lattice-test-device".to_string(),
            activation_token: None,
        })
        .await?;
    let org_token = organization_token_from_random();
    let org_activation_token = generate_org_activation_token(
        &org_token,
        org_public_key.0.as_str(),
        &user_key,
        &addr_keys
            .first()
            .ok_or_else(|| {
                LtTransportError::from(lattice::LatticeError::Other("no address keys".into()))
            })?
            .1,
    )
    .map_err(|e| {
        LtTransportError::from(lattice::LatticeError::Other(format!(
            "org activation token: {e}"
        )))
    })?;

    let org_primary_user_key =
        generate_org_primary_user_key(&org_token, &user_key).map_err(|e| {
            LtTransportError::from(lattice::LatticeError::Other(format!(
                "org primary user key: {e}"
            )))
        })?;

    let auth = build_srp_verifier(session, backup_password).await?;
    let body = build_setup_keys_body(
        auth,
        &user_key,
        addr_keys.iter().map(|(id, k)| (id.as_str(), k)),
        Some(Sensitive::new(encrypted_secret)),
        Some(Sensitive::new(org_primary_user_key.private_key.to_string())),
        Some(Sensitive::new(org_activation_token.clone())),
    );

    session
        .send_lt(LtCorePostKeysSetupReq {
            user_init_flag: LtCoreAsyncUserInitialization::CalledByClient,
            body,
        })
        .await?;

    Ok(())
}

async fn build_srp_verifier(
    session: &Session,
    password: &str,
) -> Result<LtCoreSrpVerifier, LtTransportError> {
    let res = session.send_lt(LtAuthGetModulusReq).await?;
    let srp = new_srp_provider();
    let ver = srp
        .generate_client_verifier(password, &res.modulus)
        .map_err(|e| {
            LtTransportError::from(lattice::LatticeError::Other(format!("srp verifier: {e}")))
        })?;
    Ok(LtCoreSrpVerifier {
        version: ver.version,
        modulus_id: res.modulus_id,
        salt: ver.salt.into(),
        verifier: ver.verifier.into(),
    })
}

fn generate_org_activation_token(
    token: &KeySecret,
    org_public_key_armor: &str,
    user_key: &NewUserKey,
    primary_addr_key: &NewAddrKey,
) -> Result<String, String> {
    let pgp = new_pgp_provider();
    let unlocked_user_key = user_key
        .key
        .unlock_and_assign_key_id(&pgp, KeyId(String::new()), &user_key.pass)
        .map_err(|e| e.to_string())?;
    let unlocked_addr_key = primary_addr_key
        .key
        .unlock_and_assign_key_id(&pgp, KeyId(String::new()), &unlocked_user_key)
        .map_err(|e| e.to_string())?;
    let org_public = pgp
        .public_key_import(org_public_key_armor.as_bytes(), DataEncoding::Armor)
        .map_err(|e| e.to_string())?;
    encrypt_and_sign_org_token(
        &pgp,
        token,
        &org_public,
        &unlocked_addr_key.private_key,
        ORG_ACTIVATION_TOKEN_SIGNING_CONTEXT,
    )
}

fn generate_org_primary_user_key(
    token: &KeySecret,
    user_key: &NewUserKey,
) -> Result<LocalUserKey, String> {
    let pgp = new_pgp_provider();
    let unlocked_user_key = user_key
        .key
        .unlock_and_assign_key_id(&pgp, KeyId(String::new()), &user_key.pass)
        .map_err(|e| e.to_string())?;
    LocalUserKey::relock_user_key(&pgp, &unlocked_user_key, token).map_err(|e| e.to_string())
}

fn encrypt_and_sign_org_token<P: PGPProviderSync>(
    pgp: &P,
    token: &KeySecret,
    encryption_key: &P::PublicKey,
    signing_key: &P::PrivateKey,
    signing_context_value: &str,
) -> Result<String, String> {
    let signing_context = pgp.new_signing_context(signing_context_value.to_owned(), true);
    let encrypted = pgp
        .new_encryptor()
        .with_encryption_key(encryption_key)
        .with_signing_key(signing_key)
        .with_signing_context(&signing_context)
        .encrypt_raw(token.as_ref(), DataEncoding::Armor)
        .map_err(|e| e.to_string())?;
    String::from_utf8(encrypted).map_err(|e| e.to_string())
}

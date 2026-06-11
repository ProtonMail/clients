//! Member `POST /keys/setup` for SSO org-managed unprivatization tests.

use core_key::NewUserKey;
use core_key::keys::AddressMetadata;
use core_key::primary_addr_key;
use lattice::Sensitive;
use lattice::auth::devices::post_auth_devices_create::LtAuthPostDevicesCreateReq;
use lattice::auth::get_auth_modulus::LtAuthGetModulusReq;
use lattice::core::LtCoreAsyncUserInitialization;
use lattice::core::get_core_addresses::LtCoreGetAddressesReq;
use lattice::core::get_members_me_unprivatize::{
    LtCoreGetMembersMeUnprivatizeReq, LtCoreGetMembersMeUnprivatizeRes,
};
use lattice::core::keys::post_keys_setup::LtCorePostKeysSetupReq;
use lattice::core::unpriv_types::LtCoreUnprivState;
use lattice::core::user::LtCoreSrpVerifier;
use lattice_muon2::LtTransportError;
use proton_crypto::new_pgp_provider;
use proton_crypto::new_srp_provider;
use proton_crypto::srp::SRPProvider;

use super::Session;

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
            query: Default::default(),
        })
        .await?;

    let addr_inputs: Vec<_> = addresses
        .addresses
        .into_iter()
        .map(AddressMetadata::from)
        .collect();
    let srp = new_srp_provider();
    let pgp = new_pgp_provider();

    let user_key = NewUserKey::init(&srp, &pgp, backup_password).map_err(|e| {
        LtTransportError::from(lattice::LatticeError::Other(format!("generate keys: {e}")))
    })?;
    let addr_keys = user_key
        .generate_new_addr_keys(&pgp, addr_inputs)
        .map_err(|e| {
            LtTransportError::from(lattice::LatticeError::Other(format!("generate keys: {e}")))
        })?;

    let device_secret = crate::common::device_approval::pending_device::random_device_secret();
    let encrypted_secret =
        core_key::EncryptedSecret::from_key_secret(&user_key.pass, &device_secret.0).map_err(
            |e| {
                LtTransportError::from(lattice::LatticeError::Other(format!(
                    "encrypted secret: {e}"
                )))
            },
        )?;
    let encrypted_secret = encrypted_secret.as_str().to_string();

    // Required by `POST /keys/setup` (EncryptedSecret on the body), not used in device-approval tests.
    let _device_res = session
        .send_lt(LtAuthPostDevicesCreateReq {
            name: "lattice-test-device".to_string(),
            activation_token: None,
        })
        .await?;
    let org_token = core_key::secure_hex_key_secret_32();
    let primary_addr_key = primary_addr_key(&addr_keys).ok_or_else(|| {
        LtTransportError::from(lattice::LatticeError::Other(
            "no primary address key".into(),
        ))
    })?;
    let org_material = user_key
        .generate_org_managed_key_material(
            &pgp,
            &org_token,
            org_public_key.0.as_str(),
            primary_addr_key,
        )
        .map_err(|e| {
            LtTransportError::from(lattice::LatticeError::Other(format!(
                "org key material: {e}"
            )))
        })?;

    let auth = build_srp_verifier(session, backup_password).await?;
    let body = user_key.into_setup_keys_body(
        auth,
        addr_keys,
        Some(Sensitive::new(encrypted_secret)),
        Some(Sensitive::new(
            org_material.primary_user_key.private_key.to_string(),
        )),
        Some(org_material.activation_token),
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

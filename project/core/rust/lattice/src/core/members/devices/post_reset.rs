//! `POST /core/v4/members/{memberId}/devices/reset` — org admin completes device reset for a member.
//!
//! Source: `Proton\Apps\Account\Controller\Auth\ResetAuthDevicesAction`. Scope: `ORGANIZATION` only.
//! The body matches `ResetAuthDevicesInput` (`AuthDeviceID`, `EncryptedSecret`, `UserKeys` with `ID` + `PrivateKey` per key).

use std::borrow::Cow;

use crate::auth::LtAuthUserKeyId;
use crate::core::ids::{LtCoreAuthDeviceId, LtCoreMemberEncId};
use crate::{AuthReq, LatticeError, LtContract, LtNoQueryParams, LtSlimAPIJSON, Method, Sensitive};

/// Identifies the target member; body carries the new device to activate and re-encrypted key material.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct LtCorePostMembersDevicesResetReq {
    pub member_id: LtCoreMemberEncId,
    pub body: LtCorePostMembersDevicesResetBody,
}

/// `ResetAuthDevicesInput` — all active user keys must be present per server validation.
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", serde(rename_all = "PascalCase"))]
pub struct LtCorePostMembersDevicesResetBody {
    /// Device row to activate for the **target** user; other devices for that user are removed.
    #[cfg_attr(feature = "serde", serde(rename = "AuthDeviceID"))]
    pub auth_device_id: LtCoreAuthDeviceId,
    /// Base64 AES-GCM ciphertext (key agreement is a client concern beyond this DTO).
    pub encrypted_secret: Sensitive<String>,
    pub user_keys: Vec<LtCoreResetAuthDevicesUserKey>,
}

/// One user key in `UserKeys` (`ResetAuthDevicesUserKeyDto`); only used on this route.
/// `ID` is the encrypted user key id, same as [`LtAuthUserKeyId`] (e.g. `PUT /core/v4/keys/{userKeyID}/user`).
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", serde(rename_all = "PascalCase"))]
pub struct LtCoreResetAuthDevicesUserKey {
    #[cfg_attr(feature = "serde", serde(rename = "ID"))]
    pub id: LtAuthUserKeyId,
    /// Armored PGP private key material (encrypted for the new device as per product flow).
    pub private_key: Sensitive<String>,
}

impl LtContract for LtCorePostMembersDevicesResetReq {
    type Response = LtSlimAPIJSON<()>;
    type Body<'a> = LtSlimAPIJSON<&'a LtCorePostMembersDevicesResetBody>;
    type Query<'q> = LtNoQueryParams;

    fn method<'a>(&'a self) -> Result<Method<Self::Body<'a>>, LatticeError> {
        Ok(Method::Post(LtSlimAPIJSON(&self.body)))
    }

    fn path<'a>(&'a self) -> Result<Cow<'a, str>, LatticeError> {
        Ok(Cow::Owned(format!(
            "/core/v4/members/{}/devices/reset",
            self.member_id
        )))
    }
}

impl AuthReq for LtCorePostMembersDevicesResetReq {}

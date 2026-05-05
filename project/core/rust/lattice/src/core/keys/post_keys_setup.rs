use std::borrow::Cow;
use std::collections::HashMap;

use crate::{
    AuthReq, LatticeError, LtContract, LtRequestQueryParams, LtSlimAPIJSON, Method, Sensitive,
    core::{LtCoreAddressKeyInput, LtCoreAsyncUserInitialization, user::LtCoreUser},
};

/// Request body for setting up keys
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", serde(rename_all = "PascalCase"))]
pub struct LtCoreSetupKeysBody {
    /// Authentication details for the setup.
    pub auth: crate::core::user::LtCoreSrpVerifier,
    /// The primary key for the user.
    pub primary_key: Sensitive<String>,
    /// A randomly generated client-side key salt.
    pub key_salt: Sensitive<String>,
    /// The primary key encrypted to the token in `OrgActivationToken` (for magic link setup).
    #[cfg_attr(
        feature = "serde",
        serde(default, skip_serializing_if = "Option::is_none")
    )]
    pub org_primary_user_key: Option<Sensitive<String>>,
    /// A 32-byte random token encoded as hex, encrypted to the organization key and signed.
    #[cfg_attr(
        feature = "serde",
        serde(default, skip_serializing_if = "Option::is_none")
    )]
    pub org_activation_token: Option<Sensitive<String>>,
    /// List of address keys for the account.
    pub address_keys: Vec<LtCoreAddressKeyInput>,
    /// Base64-encoded AES-GCM encrypted secret using the `DeviceSecret` as key.
    #[cfg_attr(
        feature = "serde",
        serde(default, skip_serializing_if = "Option::is_none")
    )]
    pub encrypted_secret: Option<Sensitive<String>>,
}

pub struct LtCorePostKeysSetupReq {
    pub user_init_flag: LtCoreAsyncUserInitialization,
    pub body: LtCoreSetupKeysBody,
}

#[cfg_attr(feature = "facet", derive(facet::Facet))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", serde(rename_all = "PascalCase"))]
pub struct LtCorePostKeysSetupRes {
    pub user: LtCoreUser,
}

pub struct LtCorePostKeysSetupQueryParams {
    flag: i32,
}

impl LtRequestQueryParams for LtCorePostKeysSetupQueryParams {
    fn to_query_params<'a>(
        &'a self,
    ) -> Result<HashMap<Cow<'a, str>, Sensitive<String>>, LatticeError> {
        Ok(HashMap::from([(
            "AsyncUserInitialization".into(),
            Sensitive::new(self.flag.to_string()),
        )]))
    }
}

impl LtContract for LtCorePostKeysSetupReq {
    type Response = LtSlimAPIJSON<LtCorePostKeysSetupRes>;
    type Body<'a> = LtSlimAPIJSON<&'a LtCoreSetupKeysBody>;
    type Query<'q> = LtCorePostKeysSetupQueryParams;

    fn method<'a>(&'a self) -> Result<Method<Self::Body<'a>>, LatticeError> {
        Ok(Method::Post(LtSlimAPIJSON(&self.body)))
    }

    fn path<'a>(&'a self) -> Result<Cow<'a, str>, LatticeError> {
        Ok(Cow::Borrowed("/core/v4/keys/setup"))
    }

    fn query<'a>(&'a self) -> Option<Self::Query<'a>> {
        let flag: i32 = self.user_init_flag.into();
        Some(LtCorePostKeysSetupQueryParams { flag })
    }
}

impl AuthReq for LtCorePostKeysSetupReq {}

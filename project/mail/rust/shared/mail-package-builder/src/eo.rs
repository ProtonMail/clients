use async_trait::async_trait;
use std::convert::Infallible;

/// The SRP modulus pair returned by `auth/v4/modulus`, used to derive the SRP
/// verifier for an Encrypted Outside (EO) recipient.
#[derive(Clone, Debug)]
pub struct EoModulus {
    pub modulus: String,
    pub modulus_id: String,
}

/// Delegated I/O for fetching the SRP modulus required by EO recipients.
#[async_trait]
pub trait EoModulusProvider: Send + Sync {
    type Error: std::error::Error + Send + Sync + 'static;
    async fn get_auth_modulus(&self) -> Result<EoModulus, Self::Error>;
}

#[async_trait]
impl<T: EoModulusProvider + ?Sized> EoModulusProvider for &T {
    type Error = T::Error;
    async fn get_auth_modulus(&self) -> Result<EoModulus, Self::Error> {
        (**self).get_auth_modulus().await
    }
}

/// No-op provider for callers that do not support EO (e.g. calendar). Holding
/// this type signals "this code path will never need an EO modulus"; calling
/// `get_auth_modulus` is a programmer error and panics.
pub struct NoopEoModulusProvider;

#[async_trait]
impl EoModulusProvider for NoopEoModulusProvider {
    type Error = Infallible;
    async fn get_auth_modulus(&self) -> Result<EoModulus, Self::Error> {
        unreachable!(
            "NoopEoModulusProvider::get_auth_modulus called — this caller is not configured for EO recipients"
        );
    }
}

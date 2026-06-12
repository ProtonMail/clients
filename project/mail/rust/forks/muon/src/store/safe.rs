use crate::auth::Auth;
use crate::store::{DynStore, Store};
use derive_more::Display;

/// An auth version.
///
/// This is used to track changes to the auth data.
/// Each time the auth data is updated, the version is incremented.
#[derive(Debug, Display, Default, Clone, Copy, PartialEq, Eq)]
pub struct AuthVersion(usize);

impl AuthVersion {
    fn upgrade(&mut self) {
        self.0 += 1;
    }
}

/// A handle to a store.
///
/// This is the type used to actually interact with the foreign store.
/// It tracks changes to the auth data; each write increments the version.
#[derive(Debug)]
pub struct StoreHandle {
    store: DynStore,
    version: AuthVersion,
}

impl StoreHandle {
    pub fn new(store: impl Store) -> Self {
        Self {
            store: Box::new(store),
            version: AuthVersion::default(),
        }
    }

    /// Get the current auth and its version.
    pub async fn get_auth(&self) -> (AuthVersion, Auth) {
        (self.version, self.store.get_auth().await)
    }

    /// Set the current auth and return the new version.
    pub async fn set_auth(&mut self, auth: Auth) -> AuthVersion {
        if self.store.set_auth(auth).await.is_ok() {
            self.version.upgrade();
        }

        self.version
    }
}

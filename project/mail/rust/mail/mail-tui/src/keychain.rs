use crate::app_model::APP_ID;
use anyhow::anyhow;
use mail_core_common::db::account::SessionEncryptionKey;
use mail_core_common::os::{KeyChain, KeyChainEntryKind, KeyChainError, KeyChainExt};
use secrecy::{ExposeSecret, SecretString};
use std::sync::Arc;

pub struct AppKeyChain {
    session_key: Arc<keyring::Entry>,
    device_key: Arc<keyring::Entry>,
}

impl AppKeyChain {
    pub fn new() -> anyhow::Result<Self> {
        let session_key = keyring::Entry::new(APP_ID, "session_key")?;
        let device_key = keyring::Entry::new(APP_ID, "device_key")?;
        Ok(Self {
            session_key: Arc::new(session_key),
            device_key: Arc::new(device_key),
        })
    }

    pub fn init(&mut self) -> anyhow::Result<()> {
        let v = self.load::<SessionEncryptionKey>()?;
        if v.is_none() {
            let key = SessionEncryptionKey::random();
            self.store(key)?;
        }
        Ok(())
    }

    fn kind_to_entry(&self, kind: KeyChainEntryKind) -> Option<&Arc<keyring::Entry>> {
        match kind {
            KeyChainEntryKind::EncryptionKey => Some(&self.session_key),
            KeyChainEntryKind::DeviceKey => Some(&self.device_key),
            KeyChainEntryKind::PinHash => None,
        }
    }
}

impl KeyChain for AppKeyChain {
    fn store_entry(&self, kind: KeyChainEntryKind, key: SecretString) -> Result<(), KeyChainError> {
        let Some(entry) = self.kind_to_entry(kind) else {
            return Err(KeyChainError::new(
                anyhow!("TUI does not support pin protection yet").into(),
            ));
        };

        entry
            .set_password(key.expose_secret())
            .map_err(|e| KeyChainError::new(anyhow!(e).into()))?;

        Ok(())
    }

    fn delete_entry(&self, kind: KeyChainEntryKind) -> Result<(), KeyChainError> {
        if let Some(entry) = self.kind_to_entry(kind)
            && let Err(e) = entry.delete_credential()
            && !matches!(e, keyring::Error::NoEntry)
        {
            return Err(KeyChainError::new(anyhow!(e).into()));
        }

        Ok(())
    }

    fn load_entry(&self, kind: KeyChainEntryKind) -> Result<Option<SecretString>, KeyChainError> {
        match self.kind_to_entry(kind).map(|e| e.get_password()) {
            None => Ok(None),
            Some(Ok(str)) => Ok(Some(str.into())),
            Some(Err(e)) => match e {
                keyring::Error::NoEntry => Ok(None),
                _ => Err(KeyChainError::new(anyhow!(e).into())),
            },
        }
    }
}

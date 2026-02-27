use super::{RsvpCache, RsvpContacts};
use mail_stash::{UserDb, stash::Stash};

pub struct RsvpService {
    cache: RsvpCache,
    contacts: RsvpContacts,
}

impl RsvpService {
    pub fn new(mail_stash: &Stash<UserDb>) -> Self {
        Self {
            cache: Default::default(),
            contacts: RsvpContacts::new(mail_stash),
        }
    }

    pub(crate) fn cache(&self) -> &RsvpCache {
        &self.cache
    }

    pub(crate) fn contacts(&self) -> &RsvpContacts {
        &self.contacts
    }
}

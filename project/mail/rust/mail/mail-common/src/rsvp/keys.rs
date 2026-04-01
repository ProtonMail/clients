use crate::{MailContextError, MailUserContext};
use mail_calendar_common as cal;
use mail_core_api::services::proton::AddressId;
use mail_crypto_inbox::proton_crypto::crypto::PGPProviderSync;
use mail_stash::stash::Tether;
use proton_crypto_account::keys::{AddressKeySelector, UnlockedAddressKeys};
use tracing::error;

pub struct RsvpKeys<'a> {
    ctx: &'a MailUserContext,
    tether: &'a Tether,
}

impl<'a> RsvpKeys<'a> {
    pub fn new(ctx: &'a MailUserContext, tether: &'a Tether) -> Self {
        Self { ctx, tether }
    }
}

impl cal::RsvpKeys for RsvpKeys<'_> {
    type Error = MailContextError;

    async fn get_address_keys<P>(
        &self,
        pgp: &P,
        id: &AddressId,
    ) -> Result<UnlockedAddressKeys<P>, Self::Error>
    where
        P: PGPProviderSync,
    {
        self.ctx
            .crypto_key_service()
            .load_with_tether(self.ctx.user_context(), self.tether)
            .address_keys(pgp, id)
            .await
            .map(AddressKeySelector::into_raw_keys)
            .inspect_err(|err| error!("Couldn't unlock address keys: {err:?}"))
            .map_err(MailContextError::from)
    }
}

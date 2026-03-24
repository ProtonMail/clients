use bytes::Buf;
use ical::{VcardParser, parser::ParserError};
use mail_core_api::services::proton::PrivateEmailRef;
use mail_core_api::session::Session;
use mail_stash::stash::RunTransaction;
use mail_stash::{UserDb, orm::Model, params, stash::StashError};
use mail_vcard::{VCardError, vcard::VCard};
use proton_crypto_account::{
    contacts::{ContactCardType, DecryptableVerifiableCard},
    errors::CardCryptoError,
    keys::{PinnedPublicKeys, UnlockedUserKeys},
    proton_crypto::crypto::PGPProviderSync,
};
use thiserror::Error;
use tracing::{debug, error};

use crate::{
    contact::Contact, contact_card::ContactCard, contact_email::ContactEmail, error::ContactError,
    vcard_crypto,
};

/// Error type for contact pinned-key extraction operations.
#[derive(Debug, Error)]
pub enum ContactCryptoError {
    #[error("{0}")]
    Contact(#[from] ContactError),
    #[error("Database Error: {0}")]
    DB(#[from] StashError),
    #[error("Problem decrypting and/or verifying the signature of the contact card: {0}")]
    CardDecryptionVerification(#[from] CardCryptoError),
    #[error("Failed to validate contact v-card: {0}")]
    VCard(#[from] VCardError),
    #[error("Failed to parse contact v-card: {0}")]
    VCardParse(#[from] ParserError),
    #[error("No contact v-card found in signed data")]
    NoVCard,
}

/// Controls whether contacts must be synced from the server before extracting pinned keys.
#[derive(Debug, Copy, Clone, Default)]
pub enum AddressKeysContactFetchPolicy {
    #[default]
    /// Always requires the contacts to be synced from the server, and will error out if
    /// the request fails.
    RequireSync,
    /// If the request fails due to lack of network, attempt to load existing cached data.
    AllowCachedFallback,
}

/// Loads the public address keys pinned to a user's contact, if any.
///
/// Performs the following operations:
/// - Searches for the contact email matching the input email in the database.
/// - If no contact matches returns None, else Syncs the full contact with cards
/// - Extracts the pinned keys from the signed `VCard` if any
/// - Verifies the signature of the `VCard` with the unlocked user keys
/// - Returns the pinned keys if any else None
#[tracing::instrument(skip_all, fields(email=%email))]
pub async fn public_address_keys_from_contacts<P>(
    pgp: &P,
    session: &Session,
    tx: &mut impl RunTransaction<UserDb>,
    unlocked_user_keys: &UnlockedUserKeys<P>,
    email: PrivateEmailRef<'_>,
    fetch_policy: AddressKeysContactFetchPolicy,
) -> Result<Option<PinnedPublicKeys<P::PublicKey>>, ContactCryptoError>
where
    P: PGPProviderSync,
{
    let contact_email =
        ContactEmail::find_first("WHERE email = ?", params![email.to_owned()], tx.tether())
            .await?
            .ok_or(ContactError::CardNotFound(email.to_owned()))?;

    let local_contact_id =
        contact_email
            .local_contact_id
            .ok_or(ContactError::ContactCardRemoteIdNotPresent(
                email.to_owned(),
            ))?;

    // If a contact exists and has linked vCards, attempt to extract pinned keys from them.
    // vCards should be current if they were synced at least once
    // since they would be updated via update events.
    match fetch_policy {
        AddressKeysContactFetchPolicy::AllowCachedFallback => {
            match Contact::load(local_contact_id, tx.tether()).await {
                Ok(Some(mut contact)) => {
                    if let Ok(cards) = contact.cards(tx.tether()).await
                        && !cards.is_empty()
                    {
                        debug!("Use local contact {local_contact_id} for pinned keys extraction");
                        return extract_pinned_keys(pgp, unlocked_user_keys, cards, &email);
                    }
                }
                Err(e) => {
                    error!("Failed to load contact for pinned keys extraction: {e}");
                }
                _ => {}
            }
        }
        AddressKeysContactFetchPolicy::RequireSync => {} // continue
    }

    // On success try to sync the most recent full contact including its v-cards from the BE.
    if let Err(e) = Contact::force_sync_with_card(local_contact_id, session, tx)
        .await
        .inspect_err(|e| error!("Failed to force sync contact: {e}"))
    {
        match e {
            ContactError::Api(e) if e.is_network_failure() => match fetch_policy {
                AddressKeysContactFetchPolicy::RequireSync => {
                    return Err(ContactCryptoError::Contact(ContactError::Api(e)));
                }
                AddressKeysContactFetchPolicy::AllowCachedFallback => {} // continue
            },
            e => return Err(ContactCryptoError::Contact(e)),
        }
    }

    let mut contact = Contact::load(local_contact_id, tx.tether())
        .await?
        .ok_or(ContactError::FullContactNotFound(email.to_owned()))?;

    let cards = contact.cards(tx.tether()).await?;

    extract_pinned_keys(pgp, unlocked_user_keys, cards, &email)
}

fn extract_pinned_keys<P>(
    pgp: &P,
    unlocked_user_keys: &UnlockedUserKeys<P>,
    cards: &[ContactCard],
    email: &PrivateEmailRef<'_>,
) -> Result<Option<PinnedPublicKeys<P::PublicKey>>, ContactCryptoError>
where
    P: PGPProviderSync,
{
    // The pinned key information can be found in the signed v-card.
    let signed_card_opt = cards
        .iter()
        .find(|card| card.card_type == ContactCardType::Signed);

    let Some(signed_card) = signed_card_opt else {
        return Ok(None);
    };

    let mut verification_keys = Vec::with_capacity(unlocked_user_keys.len());

    unlocked_user_keys
        .iter()
        .for_each(|uuk| verification_keys.push(uuk.public_key.clone()));

    // Verify the signature of the v-card.
    let card_data =
        signed_card.decrypt_and_verify_sync(pgp, &pgp.empty_private_keys(), &verification_keys)?;

    // Parse the v-card contact, there should be exactly one v-card
    let vcard_contact = VcardParser::new(card_data.reader())
        .next()
        .ok_or(ContactCryptoError::NoVCard)??;

    let vcard = VCard::try_from(vcard_contact)?;

    Ok(vcard_crypto::pinned_keys_for_mail(&vcard, pgp, email))
}

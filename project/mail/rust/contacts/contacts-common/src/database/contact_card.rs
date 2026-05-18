use contact_database::{RoContactCardTable, RwContactCardTable};
use mail_shared_types::ModelExtension;
use mail_stash::orm::Model;
use mail_stash::stash::StashError;

use crate::contact_card::ContactCard;
use crate::database::{ContactReadTx, ContactWriteTx};
use crate::local_ids::LocalContactCardId;

impl RoContactCardTable for ContactReadTx<'_> {
    type Error = StashError;

    async fn find_contact_card_by_id(
        &self,
        id: contact_database::LocalContactCardId,
    ) -> Result<Option<contact_database::ContactCard>, Self::Error> {
        self.0
            .sync_query(move |conn| {
                Ok(ContactCard::load_by_id_sync(id.into(), conn)?.map(Into::into))
            })
            .await
    }

    async fn find_contact_cards_for_contact(
        &self,
        id: contact_database::LocalContactId,
    ) -> Result<Vec<contact_database::ContactCard>, Self::Error> {
        self.0
            .sync_query(move |conn| {
                Ok(
                    ContactCard::find_sync("WHERE local_contact_id = ?", [id.as_u64()], conn)?
                        .into_iter()
                        .map(Into::into)
                        .collect(),
                )
            })
            .await
    }
}

impl RoContactCardTable for ContactWriteTx<'_> {
    type Error = StashError;

    async fn find_contact_card_by_id(
        &self,
        id: contact_database::LocalContactCardId,
    ) -> Result<Option<contact_database::ContactCard>, Self::Error> {
        self.0
            .sync_query(move |conn| {
                Ok(ContactCard::load_by_id_sync(id.into(), conn)?.map(Into::into))
            })
            .await
    }

    async fn find_contact_cards_for_contact(
        &self,
        id: contact_database::LocalContactId,
    ) -> Result<Vec<contact_database::ContactCard>, Self::Error> {
        self.0
            .sync_query(move |conn| {
                Ok(
                    ContactCard::find_sync("WHERE local_contact_id = ?", [id.as_u64()], conn)?
                        .into_iter()
                        .map(Into::into)
                        .collect(),
                )
            })
            .await
    }
}

impl RwContactCardTable for ContactWriteTx<'_> {
    async fn create_contact_card(
        &self,
        contact_card: contact_database::NewContactCard,
    ) -> Result<contact_database::ContactCard, Self::Error> {
        self.0
            .sync_bridge(|tx| {
                let mut contact_card: ContactCard = contact_card.into();
                contact_card.save_sync(tx)?;
                Ok(contact_card.into())
            })
            .await
    }

    async fn create_contact_cards(
        &self,
        contact_cards: impl IntoIterator<Item = contact_database::NewContactCard>,
    ) -> Result<Vec<contact_database::ContactCard>, Self::Error> {
        let mut contact_cards: Vec<ContactCard> =
            contact_cards.into_iter().map(Into::into).collect();
        self.0
            .sync_bridge(move |tx| {
                for contact_card in &mut contact_cards {
                    contact_card.save_sync(tx)?;
                }
                Ok(contact_cards.into_iter().map(Into::into).collect())
            })
            .await
    }

    async fn update_contact_card(
        &self,
        contact_card: &contact_database::ContactCard,
    ) -> Result<(), Self::Error> {
        let mut contact_card: ContactCard = contact_card.into();
        self.0
            .sync_bridge(move |tx| {
                contact_card.save_sync(tx)?;
                Ok(())
            })
            .await
    }

    async fn delete_contact_cards_for_contact(
        &self,
        id: contact_database::LocalContactId,
    ) -> Result<(), Self::Error> {
        self.0
            .sync_bridge(move |tx| {
                let mut stmt =
                    tx.prepare_cached("DELETE FROM contact_cards WHERE local_contact_id=?")?;
                stmt.execute([id.as_u64()])?;
                Ok(())
            })
            .await
    }

    async fn delete_contact_cards(
        &self,
        ids: impl IntoIterator<Item = contact_database::LocalContactCardId>,
    ) -> Result<(), Self::Error> {
        let ids: Vec<LocalContactCardId> = ids.into_iter().map(Into::into).collect();
        ContactCard::delete_by_ids(ids, &self.0).await?;
        Ok(())
    }
}

impl From<contact_database::ContactCard> for ContactCard {
    fn from(value: contact_database::ContactCard) -> Self {
        Self {
            local_id: Some(value.local_id.into()),
            local_contact_id: Some(value.local_contact_id.into()),
            card_type: value.card_type,
            data: value.data,
            signature: value.signature,
            remote_contact_id: None,
        }
    }
}

impl From<&contact_database::ContactCard> for ContactCard {
    fn from(value: &contact_database::ContactCard) -> Self {
        Self {
            local_id: Some(value.local_id.into()),
            local_contact_id: Some(value.local_contact_id.into()),
            card_type: value.card_type,
            data: value.data.clone(),
            signature: value.signature.clone(),
            remote_contact_id: None,
        }
    }
}

impl From<ContactCard> for contact_database::ContactCard {
    fn from(value: ContactCard) -> Self {
        Self {
            local_id: value.id().into(),
            local_contact_id: value.local_contact_id.expect("must be set").into(),
            card_type: value.card_type,
            data: value.data,
            signature: value.signature,
        }
    }
}

impl From<contact_database::NewContactCard> for ContactCard {
    fn from(value: contact_database::NewContactCard) -> Self {
        Self {
            local_id: None,
            local_contact_id: Some(value.contact_id.into()),
            remote_contact_id: None,
            card_type: value.card_type,
            data: value.data,
            signature: value.signature,
        }
    }
}

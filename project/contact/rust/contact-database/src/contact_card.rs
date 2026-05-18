use proton_crypto_account::contacts::ContactCardType;

use crate::LocalContactId;

mail_local_id::declare_local_id!(pub LocalContactCardId);

#[derive(Debug, Clone)]
pub struct ContactCard {
    pub local_id: LocalContactCardId,
    pub local_contact_id: LocalContactId,
    pub card_type: ContactCardType,
    pub data: String,
    pub signature: Option<String>,
}

pub struct NewContactCard {
    pub contact_id: LocalContactId,
    pub card_type: ContactCardType,
    pub data: String,
    pub signature: Option<String>,
}

pub trait RoContactCardTable {
    type Error: std::error::Error + 'static;

    async fn find_contact_card_by_id(
        &self,
        id: LocalContactCardId,
    ) -> Result<Option<ContactCard>, Self::Error>;
    async fn find_contact_cards_for_contact(
        &self,
        id: LocalContactId,
    ) -> Result<Vec<ContactCard>, Self::Error>;
}

pub trait RwContactCardTable: RoContactCardTable {
    async fn create_contact_card(
        &self,
        contact_card: NewContactCard,
    ) -> Result<ContactCard, Self::Error>;

    async fn create_contact_cards(
        &self,
        contact_card: impl IntoIterator<Item = NewContactCard>,
    ) -> Result<Vec<ContactCard>, Self::Error>;

    async fn update_contact_card(&self, contact_card: &ContactCard) -> Result<(), Self::Error>;

    async fn delete_contact_cards_for_contact(&self, id: LocalContactId)
    -> Result<(), Self::Error>;

    async fn delete_contact_cards(
        &self,
        ids: impl IntoIterator<Item = LocalContactCardId>,
    ) -> Result<(), Self::Error>;
}

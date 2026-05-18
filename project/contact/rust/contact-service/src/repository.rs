use contact_database::{
    Contact as DbContact, ContactCard as DbContactCard, ContactEmail as DbContactEmail,
    LocalContactId, NewContactCard, RoContactTable, RwContactCardTable, RwContactEmailTable,
    RwContactTable, UpseratableContactEmail, UpsertableContact,
};
use contact_lattice::ContactFull as ApiContact;

pub struct Contact {
    pub contact: DbContact,
    pub emails: Vec<DbContactEmail>,
    pub cards: Vec<DbContactCard>,
}

pub struct ContactRepository;

impl ContactRepository {
    pub async fn upsert_api_contact<T, E>(tx: &T, api_contact: ApiContact) -> Result<Contact, E>
    where
        E: std::error::Error + 'static,
        T: RwContactCardTable<Error = E>
            + RwContactTable<Error = E>
            + RwContactEmailTable<Error = E>,
    {
        let db_contact = UpsertableContact {
            id: api_contact.id,
            create_time: api_contact.create_time,
            label_ids: api_contact.label_ids,
            modify_time: api_contact.modify_time,
            name: api_contact.name,
            size: api_contact.modify_time,
            uid: api_contact.uid,
        };
        let db_contact_emails =
            api_contact
                .contact_emails
                .into_iter()
                .map(|v| UpseratableContactEmail {
                    id: v.id,
                    contact_id: v.contact_id,
                    canonical_email: v.canonical_email,
                    contact_type: v.contact_type,
                    defaults: v.defaults,
                    display_order: v.order,
                    email: v.email,
                    is_proton: v.is_proton,
                    label_ids: v.label_ids,
                    last_used_time: v.last_used_time,
                    name: v.name,
                });
        let contact = tx.upsert_contact(db_contact).await?;
        let contact_emails = tx.upsert_contact_emails(db_contact_emails).await?;

        let db_contact_cards = api_contact.cards.into_iter().map(|v| NewContactCard {
            contact_id: contact.local_id,
            card_type: v.card_type,
            data: v.data,
            signature: v.signature,
        });
        //TODO: could use replace method to avoid rountrip on delete
        tx.delete_contact_cards_for_contact(contact.local_id)
            .await?;
        let contact_cards = tx.create_contact_cards(db_contact_cards).await?;

        Ok(Contact {
            contact,
            emails: contact_emails,
            cards: contact_cards,
        })
    }

    pub async fn find_contact_by_id<T, E>(tx: &T, id: LocalContactId) -> Result<Option<Contact>, E>
    where
        E: std::error::Error + 'static,
        T: RoContactTable<Error = E>,
    {
        let Some(contact) = tx.find_contact_by_id(id).await? else {
            return Ok(None);
        };

        Ok(Some(Contact {
            contact,
            emails: vec![],
            cards: vec![],
        }))
    }
}

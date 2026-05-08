use contact_database::{
    Contact as DbContact, ContactCard as DbContactCard, ContactEmail as DbContactEmail,
    RwContactCardTable, RwContactEmailTable, RwContactTable, UpseratableContactEmail,
    UpsertableContact, UpsertableContactCard,
};
use contact_lattice::ContactFull as ApiContact;

pub struct Contact {
    pub contact: DbContact,
    pub emails: Vec<DbContactEmail>,
    pub cards: Vec<DbContactCard>,
}

pub struct ContactRepository;

impl ContactRepository {
    pub async fn upsert_api_contact<T, E>(tx: T, contact: ApiContact) -> Result<Contact, E>
    where
        E: std::error::Error + 'static,
        T: RwContactCardTable<Error = E>
            + RwContactTable<Error = E>
            + RwContactEmailTable<Error = E>,
    {
        let contact_id = contact.id.clone();
        let db_contact = UpsertableContact {
            id: contact.id,
            create_time: contact.create_time,
            label_ids: contact.label_ids,
            modify_time: contact.modify_time,
            name: contact.name,
            size: contact.modify_time,
            uid: contact.uid,
        };
        let db_contact_emails =
            contact
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
        let db_contact_cards = contact.cards.into_iter().map(|v| UpsertableContactCard {
            contact_id: contact_id.clone(),
            card_type: v.card_type,
            data: v.data,
            signature: v.signature,
        });
        let contact = tx.upsert_contact(db_contact).await?;
        let contact_emails = tx.upsert_contact_emails(db_contact_emails).await?;
        let contact_cards = tx.upsert_contact_cards(db_contact_cards).await?;

        Ok(Contact {
            contact,
            emails: contact_emails,
            cards: contact_cards,
        })
    }
}

use crate::events::{CONTACT_EVENT_TYPE_ID, ContactEventLoopV6Context, ContactEventStorageContext};
use async_trait::async_trait;
use core_event_loop::store::EventStore;
use mail_stash::{params, stash::StashError};

#[async_trait]
impl<T: ContactEventStorageContext> EventStore<T> for ContactEventLoopV6Context {
    async fn load(&self, ctx: &T) -> anyhow::Result<Option<core_event_loop::EventId>> {
        let tether = ctx.get_contact_stash().connection().await?;

        let result = tether
            .query_value_opt::<String>(
                "SELECT event_id FROM contacts_event_id_store WHERE id=?",
                params![STORE_ID],
            )
            .await
            .map(|v| v.map(core_event_loop::EventId::from))?;

        // If enable try to load the event from the old location
        #[cfg(feature = "mail-compat")]
        if result.is_none() {
            return Ok(tether
                .query_value_opt::<String>(
                    "SELECT value FROM event_id_store WHERE id = ?",
                    params![CONTACT_EVENT_TYPE_ID],
                )
                .await
                .map(|v| v.map(core_event_loop::EventId::from))?);
        }

        return Ok(result);
    }

    async fn store(&self, ctx: &T, id: core_event_loop::EventId) -> anyhow::Result<()> {
        let mut tether = ctx.get_contact_stash().connection().await?;
        tether
            .write_tx::<_, _, StashError>(async |tx| {
                // Also update old mail location in case of v5 revert
                #[cfg(feature = "mail-compat")]
                tx.execute(
                    "INSERT OR REPLACE INTO event_id_store (id, value) VALUES (?,?)",
                    params![CONTACT_EVENT_TYPE_ID, id.clone().into_inner()],
                )
                .await?;
                tx.execute(
                    "INSERT OR REPLACE INTO contacts_event_id_store (id, event_id) VALUES (?,?)",
                    params![STORE_ID, id.into_inner()],
                )
                .await?;

                Ok(())
            })
            .await?;

        Ok(())
    }
}

const STORE_ID: u64 = 1;

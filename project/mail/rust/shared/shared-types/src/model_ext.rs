use indoc::formatdoc;
use itertools::Itertools as _;
use mail_local_id::LocalIdMarker;
use mail_proton_ids::ProtonIdMarker;
use mail_stash::exports::{FromSql, SqliteError, ToSql, Transaction};
use mail_stash::orm::Model;
use mail_stash::params;
use mail_stash::rusqlite::{Connection, OptionalExtension, params_from_iter};
use mail_stash::stash::{StashError, Tether, WriteTx};
use mail_stash::utils::{ConnectionExt, IterMapToSql as _, MapToSql as _, placeholders};

#[allow(async_fn_in_trait)]
pub trait ModelExtension: Model {
    #[must_use]
    async fn all(tether: &Tether<Self::Database>) -> Result<Vec<Self>, StashError> {
        Self::find(String::new(), vec![], tether).await
    }

    async fn find_by_id(
        id: Self::IdType,
        tether: &Tether<Self::Database>,
    ) -> Result<Option<Self>, StashError> {
        Self::find_first(
            format!("WHERE {} =?", Self::id_field_name()),
            params![id],
            tether,
        )
        .await
    }

    async fn find_by_ids(
        ids: impl IntoIterator<Item = Self::IdType>,
        tether: &Tether<Self::Database>,
    ) -> Result<Vec<Self>, StashError> {
        let mut ids = ids.into_iter().peekable();
        let field_name = if ids.peek().is_some() {
            Self::id_field_name()
        } else {
            return Ok(vec![]);
        };
        #[allow(trivial_casts)]
        let parameters = ids
            .map(|i| Box::new(i) as Box<dyn ToSql + Send>)
            .collect_vec();
        let placeholders = placeholders(&parameters);

        let query = format!("WHERE {field_name} IN ({placeholders})");
        Self::find(query, parameters, tether).await
    }

    async fn with_save(mut self, bond: &WriteTx<'_, Self::Database>) -> Result<Self, StashError> {
        self.save(bond).await?;
        Ok(self)
    }

    async fn with_insert(mut self, bond: &WriteTx<'_, Self::Database>) -> Result<Self, StashError> {
        self.insert(bond).await?;
        Ok(self)
    }

    async fn find_ids<Q>(
        query_logic: Q,
        params: Vec<Box<dyn ToSql + Send>>,
        tether: &Tether<Self::Database>,
    ) -> Result<Vec<Self::IdType>, StashError>
    where
        Q: Into<String> + Send,
    {
        tether
            .query_values::<_, Self::IdType>(
                formatdoc!(
                    "
                    SELECT
                        {}
                    FROM
                        {}
                    {}
                    ",
                    Self::id_field_name(),
                    Self::table_name(),
                    query_logic.into(),
                ),
                params,
            )
            .await
    }

    async fn reload(&mut self, tether: &Tether<Self::Database>) -> Result<(), StashError> {
        if let Some(this) = Self::load(self.id_value()?, tether).await? {
            *self = this;
        }

        Ok(())
    }

    async fn exists(&self, tether: &Tether<Self::Database>) -> Result<bool, StashError> {
        tether
            .query_values::<_, Self::IdType>(
                formatdoc!(
                    "SELECT {} FROM {} WHERE {} = ? LIMIT 1",
                    Self::id_field_name(),
                    Self::table_name(),
                    Self::id_field_name(),
                ),
                params![self.id_value()?],
            )
            .await
            .map(|v| !v.is_empty())
    }

    async fn delete(self, bond: &WriteTx<'_, Self::Database>) -> Result<bool, StashError> {
        Self::delete_by_id(self.id_value()?, bond).await
    }

    async fn delete_by_id(
        id: Self::IdType,
        bond: &WriteTx<'_, Self::Database>,
    ) -> Result<bool, StashError> {
        bond.sync_bridge(|tx| Self::delete_by_id_sync(id, tx)).await
    }

    #[allow(trivial_casts)]
    #[must_use]
    async fn delete_by_ids(
        ids: Vec<Self::IdType>,
        bond: &WriteTx<'_, Self::Database>,
    ) -> Result<usize, StashError> {
        bond.sync_bridge(move |tx| Self::delete_by_ids_sync(&ids, tx))
            .await
    }

    fn delete_sync(self, tx: &Transaction<'_>) -> Result<bool, StashError> {
        Self::delete_by_id_sync(self.id_value()?, tx)
    }

    fn delete_by_id_sync(id: Self::IdType, tx: &Transaction<'_>) -> Result<bool, StashError> {
        let mut query = tx.prepare_cached(Self::DELETE_BY_ID_QUERY)?;

        Ok(query.execute((id,))? == 1)
    }

    fn delete_by_ids_sync(ids: &[Self::IdType], tx: &Transaction<'_>) -> Result<usize, StashError> {
        let mut query = tx.prepare(&format!(
            "DELETE FROM {table} WHERE {id} IN ({placeholders})",
            table = Self::table_name(),
            id = Self::id_field_name(),
            placeholders = placeholders(ids)
        ))?;

        Ok(query.execute(params_from_iter(ids))?)
    }

    #[must_use]
    async fn delete_all(bond: &WriteTx<'_, Self::Database>) -> Result<usize, StashError> {
        let table = Self::table_name();
        let query = format!("DELETE FROM {table}");

        bond.execute(query, vec![]).await
    }
}

/// Extension trait for models where there is a relationship between a local id and remote id
/// for a resource.
///
/// This relationship usually exists when the given resource can be created locally without it
/// existing on the remote server. This relationship is expected to be expressed via
/// the [`declare_local_id`] macro.
#[allow(async_fn_in_trait)]
pub trait ModelIdExtension: ModelExtension + Model<IdType: LocalIdMarker> {
    /// Remote Id type.
    type RemoteId: ProtonIdMarker + ToSql + FromSql;

    /// Return the remote id for this model.
    fn remote_id(&self) -> Option<&Self::RemoteId>;

    /// Returns whether this item has been synced.
    fn is_synced(&self) -> bool {
        self.remote_id().is_some()
    }

    /// Remote id field name.
    #[must_use]
    fn remote_id_field_name() -> &'static str {
        "remote_id"
    }

    async fn find_by_remote_id(
        id: Self::RemoteId,
        tether: &Tether<Self::Database>,
    ) -> Result<Option<Self>, StashError> {
        tether
            .sync_query(move |conn| Self::find_by_remote_id_sync(&id, conn))
            .await
    }

    fn find_by_remote_id_sync(
        id: &Self::RemoteId,
        conn: &Connection,
    ) -> Result<Option<Self>, StashError> {
        Self::find_first_sync(
            format!("WHERE {} =?", Self::remote_id_field_name()),
            (id,),
            conn,
        )
    }

    fn find_by_remote_ids_sync(
        ids: &[Self::RemoteId],
        conn: &Connection,
    ) -> Result<Vec<Self>, StashError> {
        if ids.is_empty() {
            return Ok(vec![]);
        }
        let field_name = Self::remote_id_field_name();
        let placeholders = placeholders(ids);

        Self::find_sync(
            format!("WHERE {field_name} IN ({placeholders})"),
            params_from_iter(ids),
            conn,
        )
    }

    async fn find_by_remote_ids(
        ids: impl IntoIterator<Item = Self::RemoteId>,
        tether: &Tether<Self::Database>,
    ) -> Result<Vec<Self>, StashError> {
        let ids = ids.bridge_sql();
        if ids.is_empty() {
            return Ok(vec![]);
        }
        let field_name = Self::remote_id_field_name();
        let placeholders = placeholders(&ids);

        Self::find(
            format!("WHERE {field_name} IN ({placeholders})"),
            ids,
            tether,
        )
        .await
    }

    async fn delete_by_remote_id(
        remote_id: Self::RemoteId,
        bond: &WriteTx<'_>,
    ) -> Result<usize, StashError> {
        let table = Self::table_name();
        let query = format!(
            "DELETE FROM {table} WHERE {} = ?",
            Self::remote_id_field_name()
        );

        bond.execute(query, params![remote_id]).await
    }

    async fn remote_id_counterpart(
        remote_id: Self::RemoteId,
        tether: &Tether<Self::Database>,
    ) -> Result<Option<Self::IdType>, StashError> {
        tether
            .sync_query(move |c| Self::remote_id_counterpart_sync(&remote_id, c))
            .await
    }

    fn remote_id_counterpart_sync(
        remote_id: &Self::RemoteId,
        conn: &Connection,
    ) -> Result<Option<Self::IdType>, StashError> {
        conn.query_row_col(
            formatdoc! {
                "
                SELECT {} FROM {} WHERE {} = ?
                LIMIT 1
                ",
                Self::id_field_name(),
                Self::table_name(),
                Self::remote_id_field_name(),
            },
            (remote_id,),
        )
        .optional()
        .map_err(StashError::from)
    }

    fn remote_ids_counterpart_sync(
        remote_ids: &[Self::RemoteId],
        conn: &Connection,
    ) -> Result<Vec<Self::IdType>, StashError> {
        conn.query_rows_col(
            formatdoc! {
                "
                SELECT {} FROM {} WHERE {} IN ({})
                ",
                Self::id_field_name(),
                Self::table_name(),
                Self::remote_id_field_name(),
                placeholders(remote_ids),
            },
            params_from_iter(remote_ids),
        )
        .map_err(StashError::from)
    }

    #[must_use]
    async fn remote_ids_counterpart(
        remote_ids: Vec<Self::RemoteId>,
        tether: &Tether<Self::Database>,
    ) -> Result<Vec<Self::IdType>, StashError> {
        tether
            .query_values(
                formatdoc!(
                    "
                            SELECT
                                {}
                            FROM
                                {}
                            WHERE
                                {} IN ({})
                            ",
                    Self::id_field_name(),
                    Self::table_name(),
                    Self::remote_id_field_name(),
                    placeholders(&remote_ids),
                ),
                remote_ids.to_sql(),
            )
            .await
    }

    async fn local_id_counterpart(
        local_id: Self::IdType,
        tether: &Tether<Self::Database>,
    ) -> Result<Option<Self::RemoteId>, StashError> {
        match tether
            .query_value::<_, Option<Self::RemoteId>>(
                formatdoc!(
                    "
                            SELECT
                                {}
                            FROM
                                {}
                            WHERE
                                {} = ?
                            LIMIT 1
                            ",
                    Self::remote_id_field_name(),
                    Self::table_name(),
                    Self::id_field_name(),
                ),
                params![local_id],
            )
            .await
        {
            Ok(v) => Ok(v),
            Err(e) => {
                if matches!(
                    e,
                    StashError::ExecutionError(SqliteError::QueryReturnedNoRows)
                ) {
                    Ok(None)
                } else {
                    Err(e)
                }
            }
        }
    }

    #[must_use]
    async fn local_ids_counterpart(
        local_ids: Vec<Self::IdType>,
        tether: &Tether<Self::Database>,
    ) -> Result<Vec<Self::RemoteId>, StashError> {
        tether
            .query_values(
                formatdoc!(
                    "
                            SELECT
                                {}
                            FROM
                                {}
                            WHERE
                                {} IN ({})
                            AND
                                {} IS NOT NULL
                            ",
                    Self::remote_id_field_name(),
                    Self::table_name(),
                    Self::id_field_name(),
                    placeholders(&local_ids),
                    Self::remote_id_field_name(),
                ),
                local_ids.to_sql(),
            )
            .await
    }

    async fn find_remote_ids<Q>(
        query_logic: Q,
        params: Vec<Box<dyn ToSql + Send>>,
        tether: &Tether<Self::Database>,
    ) -> Result<Vec<Self::RemoteId>, StashError>
    where
        Q: Into<String> + Send,
    {
        tether
            .query_values::<_, Self::RemoteId>(
                formatdoc!(
                    "
                    SELECT
                        {}
                    FROM
                        {}
                    {}
                    ",
                    Self::remote_id_field_name(),
                    Self::table_name(),
                    query_logic.into(),
                ),
                params,
            )
            .await
    }
}

impl<T: Model> ModelExtension for T {}

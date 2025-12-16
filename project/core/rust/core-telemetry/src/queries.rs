// SQL query constants for telemetry storage operations.

pub(crate) const CREATE_EVENTS_TABLE: &str = "
    CREATE TABLE IF NOT EXISTS telemetry_events (
        id TEXT PRIMARY KEY,
        measurement_group TEXT NOT NULL,
        event TEXT NOT NULL,
        values_json BLOB NOT NULL,
        dimensions_json BLOB NOT NULL,
        created_at INTEGER NOT NULL DEFAULT (strftime('%s', 'now'))
    )";

pub(crate) const GET_EVENTS: &str = "
    SELECT id, measurement_group, event, values_json, dimensions_json
    FROM telemetry_events
    ORDER BY created_at ASC
    LIMIT ?1";

pub(crate) const INSERT_EVENT: &str = "
    INSERT OR REPLACE INTO telemetry_events
    (id, measurement_group, event, values_json, dimensions_json)
    VALUES (?1, ?2, ?3, ?4, ?5)";

pub(crate) const DELETE_EVENT: &str = "
    DELETE FROM telemetry_events WHERE id = ?";

#[cfg(test)]
mod tests {
    use super::*;

    /// This test ensures that SQL queries stay in sync with the TelemetryEvent struct.
    ///
    /// It uses the nameof! macro to extract field names, ensuring compile-time validation.
    /// If someone renames a field in TelemetryEvent without updating the SQL queries,
    /// this test will fail.
    #[test]
    fn test_sql_queries_match_telemetry_event_fields() {
        use crate::TelemetryEvent;
        use nameof::name_of;

        // Extract field names - these will fail to compile if fields are renamed
        let id_field = name_of!(id in TelemetryEvent);
        let measurement_group_field = name_of!(measurement_group in TelemetryEvent);
        let event_field = name_of!(event in TelemetryEvent);
        let values_field = name_of!(values in TelemetryEvent);
        let dimensions_field = name_of!(dimensions in TelemetryEvent);

        // Now verify that the SQL queries contain these exact field names
        // This will catch cases where the field was renamed but SQL wasn't updated
        // Note: DB columns use _json suffix for the HashMap fields

        // CREATE_EVENTS_TABLE checks
        assert!(
            CREATE_EVENTS_TABLE.contains(id_field),
            "CREATE_EVENTS_TABLE missing field: {}",
            id_field
        );
        assert!(
            CREATE_EVENTS_TABLE.contains(measurement_group_field),
            "CREATE_EVENTS_TABLE missing field: {}",
            measurement_group_field
        );
        assert!(
            CREATE_EVENTS_TABLE.contains(event_field),
            "CREATE_EVENTS_TABLE missing field: {}",
            event_field
        );
        assert!(
            CREATE_EVENTS_TABLE.contains(&format!("{values_field}_json")),
            "CREATE_EVENTS_TABLE missing field: {values_field}_json"
        );
        assert!(
            CREATE_EVENTS_TABLE.contains(&format!("{dimensions_field}_json")),
            "CREATE_EVENTS_TABLE missing field: {dimensions_field}_json"
        );

        // GET_EVENTS checks
        assert!(
            GET_EVENTS.contains(id_field),
            "GET_EVENTS missing field: {}",
            id_field
        );
        assert!(
            GET_EVENTS.contains(measurement_group_field),
            "GET_EVENTS missing field: {}",
            measurement_group_field
        );
        assert!(
            GET_EVENTS.contains(event_field),
            "GET_EVENTS missing field: {}",
            event_field
        );
        assert!(
            GET_EVENTS.contains(&format!("{values_field}_json")),
            "GET_EVENTS missing field: {values_field}_json"
        );
        assert!(
            GET_EVENTS.contains(&format!("{dimensions_field}_json")),
            "GET_EVENTS missing field: {dimensions_field}_json"
        );

        // INSERT_EVENT checks
        assert!(
            INSERT_EVENT.contains(id_field),
            "INSERT_EVENT missing field: {}",
            id_field
        );
        assert!(
            INSERT_EVENT.contains(measurement_group_field),
            "INSERT_EVENT missing field: {}",
            measurement_group_field
        );
        assert!(
            INSERT_EVENT.contains(event_field),
            "INSERT_EVENT missing field: {}",
            event_field
        );
        assert!(
            INSERT_EVENT.contains(&format!("{values_field}_json")),
            "INSERT_EVENT missing field: {values_field}_json"
        );
        assert!(
            INSERT_EVENT.contains(&format!("{dimensions_field}_json")),
            "INSERT_EVENT missing field: {dimensions_field}_json"
        );

        // DELETE_EVENT checks
        assert!(
            DELETE_EVENT.contains(id_field),
            "DELETE_EVENT missing field: {}",
            id_field
        );
    }
}

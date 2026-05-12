#!/usr/bin/env bash
# Inspect the persisted SQLite database from historic_load_test or historic_load_trial.
#
# Usage (from project/mail/rust):
#   ./scripts/inspect-historic-load-db.sh                     # auto-detect
#   ./scripts/inspect-historic-load-db.sh path/to/user.db     # explicit path

set -euo pipefail

find_user_db() {
    local search_dirs=(
        "historic_load_test_db/user"
        "historic_load_trial_db/user"
    )
    for dir in "${search_dirs[@]}"; do
        if [[ -d "$dir" ]]; then
            local db
            db=$(find "$dir" -maxdepth 1 -name '*.db' -print -quit 2> /dev/null)
            if [[ -n "$db" ]]; then
                echo "$db"
                return 0
            fi
        fi
    done
    return 1
}

if [[ $# -ge 1 ]]; then
    DB="$1"
else
    DB=$(find_user_db) || {
        echo "Error: no user DB found in historic_load_test_db/ or historic_load_trial_db/"
        echo "Run with --persist-db or pass an explicit path."
        exit 1
    }
fi

if [[ ! -f "$DB" ]]; then
    echo "Error: file not found: $DB"
    exit 1
fi

echo "=== Inspecting: $DB ==="
echo ""

section() { printf "\n--- %s ---\n" "$1"; }

section "Table list"
sqlite3 "$DB" ".tables"

section "Row counts"
sqlite3 "$DB" -header -column "
    SELECT 'messages'                    AS 'table', COUNT(*) AS rows FROM messages
    UNION ALL
    SELECT 'raw_message_body',           COUNT(*) FROM raw_message_body
    UNION ALL
    SELECT 'search_index_intents',       COUNT(*) FROM search_index_intents
    UNION ALL
    SELECT 'search_index_blobs',         COUNT(*) FROM search_index_blobs
    UNION ALL
    SELECT 'search_index_content_hashes',COUNT(*) FROM search_index_content_hashes;
"

section "Intent queue breakdown"
sqlite3 "$DB" -header -column "
    SELECT operation, COUNT(*) AS count, AVG(retry_count) AS avg_retries
    FROM search_index_intents
    GROUP BY operation;
"

section "Blob storage"
sqlite3 "$DB" -header -column "
    SELECT COUNT(*)                                AS blob_count,
           COALESCE(SUM(LENGTH(blob_data)),0)      AS total_bytes,
           COALESCE(SUM(LENGTH(blob_data))/1024/1024, 0) AS total_mb,
           COALESCE(AVG(LENGTH(blob_data)),0)      AS avg_blob_bytes
    FROM search_index_blobs;
"

section "Body coverage"
sqlite3 "$DB" -header -column "
    SELECT
        (SELECT COUNT(*) FROM messages)          AS total_messages,
        (SELECT COUNT(*) FROM raw_message_body)  AS with_body,
        (SELECT COUNT(*) FROM messages m
         LEFT JOIN raw_message_body mb ON m.local_id = mb.message_id
         WHERE mb.message_id IS NULL)            AS missing_body,
        (SELECT COUNT(*) FROM raw_message_body
         WHERE decryption_error IS NOT NULL)     AS decryption_errors,
        (SELECT COUNT(*) FROM raw_message_body
         WHERE LENGTH(body) = 0)                 AS empty_bodies;
"

section "Index coverage"
sqlite3 "$DB" -header -column "
    SELECT
        (SELECT COUNT(*) FROM search_index_content_hashes) AS indexed,
        (SELECT COUNT(*) FROM raw_message_body
         WHERE decryption_error IS NULL AND LENGTH(body) > 0) AS indexable,
        (SELECT COUNT(*) FROM search_index_intents
         WHERE operation = 'index')              AS pending_index;
"

section "Database file size"
ls -lh "$DB" | awk '{print $5, $NF}'

echo ""
echo "Done. For ad-hoc queries:"
echo "  sqlite3 \"$DB\""

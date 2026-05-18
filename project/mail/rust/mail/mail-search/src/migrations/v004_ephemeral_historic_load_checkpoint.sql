-- Resume anchor for ephemeral historic load (All Mail metadata pagination, single row).
CREATE TABLE IF NOT EXISTS ephemeral_historic_load_checkpoint (
  id INTEGER PRIMARY KEY CHECK (id = 1),
  anchor_time INTEGER NOT NULL,
  anchor_message_id TEXT NOT NULL,
  updated_at INTEGER NOT NULL
);

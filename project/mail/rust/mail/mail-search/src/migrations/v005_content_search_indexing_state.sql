-- Durable state for content search historic indexing (singleton row).
--
-- Status values: none, ongoing, interrupted, completed. A distinct 'failed'
-- status is intentionally not modelled; non-clean exits land in
-- 'interrupted' with last_error populated.
CREATE TABLE IF NOT EXISTS content_search_indexing_state (
  id INTEGER PRIMARY KEY CHECK (id = 1),
  enabled INTEGER NOT NULL DEFAULT 0,
  status TEXT NOT NULL DEFAULT 'none',
  messages_indexed_total INTEGER NOT NULL DEFAULT 0,
  messages_fetched_total INTEGER NOT NULL DEFAULT 0,
  messages_skipped_total INTEGER NOT NULL DEFAULT 0,
  batches_completed INTEGER NOT NULL DEFAULT 0,
  last_error TEXT,  -- stable error code; see ContentSearchIndexingLastErrorCode
  started_at INTEGER,
  updated_at INTEGER NOT NULL
);

-- Seed singleton row so reads always succeed even before first write.
INSERT
  OR IGNORE INTO content_search_indexing_state (id, updated_at)
VALUES
  (1, strftime('%s', 'now') * 1000);

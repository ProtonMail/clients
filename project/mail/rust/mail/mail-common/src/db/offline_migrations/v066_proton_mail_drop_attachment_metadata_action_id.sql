DROP TRIGGER IF EXISTS cleanup_draft_attachments_trigger;

CREATE TABLE draft_attachment_metadata_v2 (
  local_attachment_id INTEGER PRIMARY KEY,
  metadata_id INTEGER NOT NULL,
  timestamp INTEGER NOT NULL DEFAULT (now ()),
  state INTEGER NOT NULL,
  error TEXT DEFAULT NULL,
  display_order INTEGER NOT NULL DEFAULT 0,
  ownership INTEGER NOT NULL,
  deleted INTEGER NOT NULL DEFAULT 0,
  is_public_key INTEGER NOT NULL DEFAULT 0,
  CONSTRAINT draft_attachment_metadata_metadata_id FOREIGN KEY (metadata_id) REFERENCES draft_metadata (id) ON DELETE CASCADE
);

INSERT INTO
  draft_attachment_metadata_v2 (
    local_attachment_id,
    metadata_id,
    timestamp,
    state,
    error,
    display_order,
    ownership,
    deleted,
    is_public_key
  )
SELECT
  local_attachment_id,
  metadata_id,
  timestamp,
  state,
  error,
  display_order,
  ownership,
  deleted,
  is_public_key
FROM
  draft_attachment_metadata;

DROP TABLE draft_attachment_metadata;

ALTER TABLE
  draft_attachment_metadata_v2 RENAME TO draft_attachment_metadata;

-- Delete attachments that were not uploaded to the server on draft discard.
CREATE TRIGGER cleanup_draft_attachments_trigger
AFTER
  DELETE ON draft_attachment_metadata
BEGIN
DELETE FROM
  attachments
WHERE
  local_id = OLD.local_attachment_id
  AND OLD.state <> 1;

END;

CREATE TABLE draft_attachment_actions (
  local_attachment_id INTEGER NOT NULL,
  action_id INTEGER NOT NULL,
  action_type TEXT NOT NULL,
  PRIMARY KEY (local_attachment_id, action_id),
  CONSTRAINT draft_attachments_ations_attachment_id FOREIGN KEY (local_attachment_id) REFERENCES attachments (local_id) ON DELETE CASCADE,
  CONSTRAINT draft_attachments_actions_action_id FOREIGN KEY (action_id) REFERENCES action_queue (id) ON DELETE CASCADE
);

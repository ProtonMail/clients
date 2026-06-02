-- Fix race condition: unleash and legacy flags with the same name need
-- separate rows so concurrent refresh doesn't overwrite one source with the other.
ALTER TABLE
  user_feature_flags RENAME TO old_user_feature_flags;

CREATE TABLE user_feature_flags (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  name TEXT NOT NULL,
  enabled BOOLEAN NOT NULL DEFAULT TRUE,
  source INTEGER NOT NULL DEFAULT 0,  -- 0 Unleash, 1 Legacy
  writable BOOLEAN NOT NULL,
  overridden_to BOOLEAN,
  overridden_at INTEGER,  -- Remote update at, set when the flag was overridden
  modify_time INTEGER NOT NULL,
  variant_name TEXT,
  variant_enabled BOOLEAN,
  variant_payload_type INTEGER,
  variant_payload_value TEXT,
  UNIQUE (name, source),  -- That additionally creates an index so we dont need another one for `name`
  CHECK (
    (
      (
        source = 0
        AND writable = FALSE
        AND overridden_to IS NULL
        AND overridden_at IS NULL
      )
      OR (
        source = 1
        AND variant_name IS NULL
        AND variant_enabled IS NULL
        AND variant_payload_type IS NULL
        AND variant_payload_value IS NULL
      )
    )
    AND (
      (
        variant_name IS NULL
        AND variant_enabled IS NULL
      )
      OR (
        variant_name IS NOT NULL
        AND variant_enabled IS NOT NULL
      )
    )
    AND (
      (
        variant_payload_type IS NULL
        AND variant_payload_value IS NULL
      )
      OR (
        variant_payload_type IS NOT NULL
        AND variant_payload_value IS NOT NULL
      )
    )
  )
);

INSERT INTO
  user_feature_flags (
    name,
    enabled,
    source,
    writable,
    overridden_to,
    overridden_at,
    modify_time,
    variant_name,
    variant_enabled,
    variant_payload_type,
    variant_payload_value
  )
SELECT
  name,
  enabled,
  source,
  writable,
  overridden_to,
  overridden_at,
  modify_time,
  variant_name,
  variant_enabled,
  variant_payload_type,
  variant_payload_value
FROM
  old_user_feature_flags;

DROP TABLE old_user_feature_flags;

ALTER TABLE feature_flags RENAME TO old_feature_flags;

CREATE TABLE feature_flags (
    name TEXT NOT NULL PRIMARY KEY,
    enabled BOOLEAN NOT NULL DEFAULT TRUE,
    modify_time INTEGER NOT NULL,
    variant_name TEXT,
    variant_enabled BOOLEAN,
    variant_payload_type INTEGER,
    variant_payload_value TEXT,

    CHECK (
        (
            (variant_name IS NULL AND variant_enabled IS NULL)
            OR (variant_name IS NOT NULL AND variant_enabled IS NOT NULL)
        )
        AND (
            (variant_payload_type IS NULL AND variant_payload_value IS NULL)
            OR (variant_payload_type IS NOT NULL AND variant_payload_value IS NOT NULL)
        )
    )
);

INSERT INTO feature_flags
    (name, enabled, modify_time, variant_name, variant_enabled, variant_payload_type, variant_payload_value)
SELECT name, enabled, modify_time, NULL, NULL, NULL, NULL FROM old_feature_flags;

DROP TABLE old_feature_flags;

-- Creat standalone table for contact groups
CREATE TABLE contact_group (
  local_id INTEGER PRIMARY KEY AUTOINCREMENT,
  remote_id TEXT UNIQUE DEFAULT NULL,
  display INTEGER NOT NULL DEFAULT 0,
  display_order INTEGER NOT NULL,
  name TEXT NOT NULL,
  color TEXT NOT NULL,
  deleted INTEGER NOT NULL DEFAULT 0,
  sticky INTEGER NOT NULL DEFAULT 0
);

CREATE UNIQUE INDEX index_contact_group_rid ON contact_group (`remote_id`);

CREATE INDEX index_contact_group_order ON contact_group (`display_order`);

-- Map contact group ids to contact emails
CREATE TABLE contact_email_groups (
  local_contact_email_id INTEGER NOT NULL,
  local_contact_group_id INTEGER NOT NULL,
  PRIMARY KEY (local_contact_email_id, local_contact_group_id),
  CONSTRAINT constraint_contact_email_groups_email_id FOREIGN KEY (local_contact_email_id) REFERENCES contact_emails (local_id) ON DELETE CASCADE,
  CONSTRAINT constraint_contact_email_groups_cg_id FOREIGN KEY (local_contact_group_id) REFERENCES contact_group (local_id) ON DELETE CASCADE
);

CREATE INDEX index_contact_email_groups_gid ON contact_email_groups (local_contact_group_id);

-- This is safe to do since we never referenced local ids for contact groups before.
INSERT INTO
  contact_group (
    remote_id,
    display,
    display_order,
    name,
    color,
    deleted,
    sticky
  )
SELECT
  remote_id,
  display,
  display_order,
  name,
  color,
  deleted,
  sticky
FROM
  labels
WHERE
  label_type = 2;

DELETE FROM
  labels
WHERE
  label_type = 2;

-- Link the contact_group to contact_email
INSERT INTO
  contact_email_groups (local_contact_email_id, local_contact_group_id)
SELECT
  contact_emails.local_id,
  contact_group.local_id
FROM
  contact_emails,
  json_each(contact_emails.label_ids) j
  JOIN contact_group ON contact_group.remote_id = j.value;

-- No longer required
ALTER TABLE
  contact_emails
  DROP COLUMN label_ids;

-- Map contact group ids to contacts
CREATE TABLE contact_contact_groups (
  local_contact_id INTEGER NOT NULL,
  local_contact_group_id INTEGER NOT NULL,
  PRIMARY KEY (local_contact_id, local_contact_group_id),
  CONSTRAINT constraint_contact_contact_groups_email_id FOREIGN KEY (local_contact_id) REFERENCES contacts (local_id) ON DELETE CASCADE,
  CONSTRAINT constraint_contact_contactl_groups_cg_id FOREIGN KEY (local_contact_group_id) REFERENCES contact_group (local_id) ON DELETE CASCADE
);

CREATE INDEX index_contact_contact_groups_gid ON contact_contact_groups (local_contact_group_id);

-- Link the contact_contact_group to contact
INSERT INTO
  contact_contact_groups (local_contact_id, local_contact_group_id)
SELECT
  contacts.local_id,
  contact_group.local_id
FROM
  contacts,
  json_each(contacts.label_ids) j
  JOIN contact_group ON contact_group.remote_id = j.value;

-- No longer required
ALTER TABLE
  contacts
  DROP COLUMN label_ids;

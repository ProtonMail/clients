ALTER TABLE mail_conversation_scroll_data RENAME TO mail_conversation_scroll_data_old;

CREATE TABLE mail_conversation_scroll_data (
    id INTEGER NOT NULL DEFAULT 0,
    local_label_id INTEGER NOT NULL,
    unread INTEGER NOT NULL DEFAULT 0,
    remote_conversation_id TEXT NOT NULL,
    conversation_time INTEGER NOT NULL,
    snooze_time INTEGER DEFAULT 0,
    display_order INTEGER NOT NULL,
    order_dir INTEGER NOT NULL DEFAULT 0,
    order_field INTEGER NOT NULL DEFAULT 0,
    category TEXT NOT NULL DEFAULT '',
    PRIMARY KEY (local_label_id, unread, order_dir, category),
    CONSTRAINT local_label_id_mail_conversation_scroll_data
        FOREIGN KEY (local_label_id) REFERENCES labels (local_id) ON DELETE CASCADE
);

INSERT INTO mail_conversation_scroll_data
    SELECT id, local_label_id, unread, remote_conversation_id,
           conversation_time, snooze_time, display_order, order_dir, order_field, ''
    FROM mail_conversation_scroll_data_old;

DROP TABLE mail_conversation_scroll_data_old;

ALTER TABLE mail_message_scroll_data RENAME TO mail_message_scroll_data_old;

CREATE TABLE mail_message_scroll_data (
    id INTEGER NOT NULL DEFAULT 0,
    local_label_id INTEGER NOT NULL,
    unread INTEGER NOT NULL DEFAULT 0,
    remote_message_id TEXT NOT NULL,
    message_time INTEGER NOT NULL,
    snooze_time INTEGER DEFAULT 0,
    display_order INTEGER NOT NULL,
    order_dir INTEGER NOT NULL DEFAULT 0,
    order_field INTEGER NOT NULL DEFAULT 0,
    category TEXT NOT NULL DEFAULT '',
    PRIMARY KEY (local_label_id, unread, order_dir, category),
    CONSTRAINT local_label_id_mail_message_scroll_data
        FOREIGN KEY (local_label_id) REFERENCES labels (local_id) ON DELETE CASCADE
);

INSERT INTO mail_message_scroll_data
    SELECT id, local_label_id, unread, remote_message_id,
           message_time, snooze_time, display_order, order_dir, order_field, ''
    FROM mail_message_scroll_data_old;

DROP TABLE mail_message_scroll_data_old;

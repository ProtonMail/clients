INSERT OR IGNORE INTO labels (remote_id, label_type, name, color, display_order)
VALUES ('25', 4, 'Newsletter',     '#000000', 0),
       ('26', 4, 'Transactions',   '#000000', 1);

INSERT OR IGNORE INTO message_counters
SELECT local_id, 0, 0 FROM labels WHERE remote_id IN ('25', '26');

INSERT OR IGNORE INTO conversation_counters
SELECT local_id, 0, 0 FROM labels WHERE remote_id IN ('25', '26');

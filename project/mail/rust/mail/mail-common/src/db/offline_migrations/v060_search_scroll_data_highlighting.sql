CREATE TABLE IF NOT EXISTS mail_search_highlighting (
  local_message_id INTEGER PRIMARY KEY,
  highlighting_positions TEXT NOT NULL,
  CONSTRAINT local_message_id_mail_search_highlighting FOREIGN KEY (local_message_id) REFERENCES messages (local_id) ON DELETE CASCADE
);

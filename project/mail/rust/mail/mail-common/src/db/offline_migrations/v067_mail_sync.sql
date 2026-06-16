CREATE TABLE mail_sync_batch (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  begin_id TEXT NOT NULL,
  begin_time INTEGER NOT NULL,
  end_id TEXT NOT NULL,
  end_time INTEGER NOT NULL,
  sync_time INTEGER NOT NULL
);

CREATE INDEX mail_sync_end_time ON mail_sync_batch(end_time);

CREATE INDEX mail_sync_begin_time ON mail_sync_batch(begin_time);

CREATE TABLE mail_sync_settings(
  id INTEGER PRIMARY KEY,
  backward_sync_start INTEGER DEFAULT NULL,
  backward_sync_complete INTEGER DEFAULT NULL
);

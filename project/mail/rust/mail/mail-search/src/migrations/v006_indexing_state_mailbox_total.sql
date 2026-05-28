-- All Mail message count captured from the first metadata page of a historic
-- indexing pass. Used to compute estimated_fraction on progress snapshots.
ALTER TABLE
  content_search_indexing_state
ADD
  COLUMN mailbox_messages_total INTEGER;

-- Live devices have incorrect counts for the Category Labels since we only care about
-- Category Labels counts in Inbox and request is being modified in this commit
DELETE FROM
  initialized_components
WHERE
  key = 'label_counters'
  OR key = 'mail_user_context';

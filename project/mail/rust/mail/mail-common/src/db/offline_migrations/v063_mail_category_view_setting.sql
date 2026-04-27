ALTER TABLE mail_settings ADD COLUMN mail_category_view INTEGER NOT NULL DEFAULT 0;

-- Force re-fetch of mail_settings so the real mail_category_view value is pulled from the API.
-- mail_user_context must also be reset because initialize_context() has an early-exit guard
-- at MailUserContext::is_initialized() that skips all sub-component inits when already Succeeded.
DELETE FROM initialized_components WHERE key = 'mail_settings' OR key = 'mail_user_context';

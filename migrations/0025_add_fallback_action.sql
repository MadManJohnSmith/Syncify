-- Add fallback_action to folder_settings
ALTER TABLE folder_settings ADD COLUMN fallback_action TEXT DEFAULT 'try_next';

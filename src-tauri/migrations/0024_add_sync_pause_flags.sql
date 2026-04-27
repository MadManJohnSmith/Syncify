-- Add pause-on-metered and pause-on-low-battery flags to sync_settings
ALTER TABLE sync_settings ADD COLUMN pause_on_metered BOOLEAN NOT NULL DEFAULT 1;
ALTER TABLE sync_settings ADD COLUMN pause_on_low_battery BOOLEAN NOT NULL DEFAULT 1;

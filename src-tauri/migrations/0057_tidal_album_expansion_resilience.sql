-- Migration 0057: Tidal Album Expansion Resilience & Availability Tracking
-- Created: 2026-08-19

CREATE TABLE IF NOT EXISTS service_album_availability (
    service_id INTEGER NOT NULL REFERENCES services(id) ON DELETE CASCADE,
    service_album_id TEXT NOT NULL,
    availability_status TEXT NOT NULL DEFAULT 'available',
    http_status INTEGER,
    sub_status INTEGER,
    reason TEXT,
    last_checked TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    PRIMARY KEY (service_id, service_album_id)
);

CREATE INDEX IF NOT EXISTS idx_service_album_avail_status 
    ON service_album_availability(service_id, availability_status, last_checked);

-- Migration 0018: Metadata Enrichment
-- Adds extended metadata fields for download tagging
-- Includes audio features, credits, and enrichment status tracking

-- ==============================================
-- EXTENDED METADATA FIELDS
-- ==============================================

-- Genre information
ALTER TABLE tracks ADD COLUMN genre TEXT;
ALTER TABLE tracks ADD COLUMN subgenre TEXT;

-- Release information (more accurate from MusicBrainz)
ALTER TABLE tracks ADD COLUMN release_year INTEGER;
ALTER TABLE tracks ADD COLUMN record_label TEXT;

-- ==============================================
-- AUDIO FEATURES (from Spotify)
-- ==============================================

-- Tempo/BPM
ALTER TABLE tracks ADD COLUMN bpm REAL;

-- Musical key (e.g., "C", "D#m", "Fm")
ALTER TABLE tracks ADD COLUMN musical_key TEXT;

-- Audio feature scores (0.0 - 1.0 range)
ALTER TABLE tracks ADD COLUMN energy REAL;
ALTER TABLE tracks ADD COLUMN danceability REAL;
ALTER TABLE tracks ADD COLUMN valence REAL;           -- Mood: 0=sad, 1=happy
ALTER TABLE tracks ADD COLUMN acousticness REAL;
ALTER TABLE tracks ADD COLUMN instrumentalness REAL;

-- ==============================================
-- ENRICHMENT STATUS TRACKING
-- ==============================================

-- Track enrichment progress
ALTER TABLE tracks ADD COLUMN enrichment_status TEXT DEFAULT 'pending';
-- Values: 'pending', 'spotify_done', 'musicbrainz_done', 'complete', 'failed'

ALTER TABLE tracks ADD COLUMN enriched_at TEXT;
ALTER TABLE tracks ADD COLUMN enrichment_error TEXT;

-- ==============================================
-- TRACK CREDITS (Composers, Producers, Writers)
-- ==============================================

CREATE TABLE IF NOT EXISTS track_credits (
    track_id INTEGER NOT NULL REFERENCES tracks(id) ON DELETE CASCADE,
    artist_id INTEGER NOT NULL REFERENCES artists(id) ON DELETE CASCADE,
    role TEXT NOT NULL,  -- 'composer', 'lyricist', 'producer', 'writer', 'engineer', 'mixer'
    PRIMARY KEY (track_id, artist_id, role)
);

CREATE INDEX IF NOT EXISTS idx_track_credits_track ON track_credits(track_id);
CREATE INDEX IF NOT EXISTS idx_track_credits_role ON track_credits(role);

-- ==============================================
-- INDEXES FOR ENRICHMENT QUERIES
-- ==============================================

CREATE INDEX IF NOT EXISTS idx_tracks_enrichment_status ON tracks(enrichment_status);
CREATE INDEX IF NOT EXISTS idx_tracks_genre ON tracks(genre);
CREATE INDEX IF NOT EXISTS idx_tracks_bpm ON tracks(bpm);
CREATE INDEX IF NOT EXISTS idx_tracks_release_year ON tracks(release_year);

-- Migration 0046: Add enrichment source_type columns for field traceability
-- Allows distinguishing between manual user overrides ('manual') and automated enrichment ('enrichment').

ALTER TABLE tracks ADD COLUMN genre_source_type TEXT DEFAULT 'enrichment';
ALTER TABLE tracks ADD COLUMN style_source_type TEXT DEFAULT 'enrichment';
ALTER TABLE tracks ADD COLUMN mood_source_type TEXT DEFAULT 'enrichment';
ALTER TABLE tracks ADD COLUMN bpm_source_type TEXT DEFAULT 'enrichment';
ALTER TABLE tracks ADD COLUMN key_source_type TEXT DEFAULT 'enrichment';
ALTER TABLE tracks ADD COLUMN label_source_type TEXT DEFAULT 'enrichment';

ALTER TABLE albums ADD COLUMN genre_source_type TEXT DEFAULT 'enrichment';
ALTER TABLE albums ADD COLUMN style_source_type TEXT DEFAULT 'enrichment';
ALTER TABLE albums ADD COLUMN release_country_source_type TEXT DEFAULT 'enrichment';
ALTER TABLE albums ADD COLUMN label_source_type TEXT DEFAULT 'enrichment';

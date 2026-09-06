-- Migration 0070: Reclassify enrichment_status for tracks falsely marked as 'enriched'
-- TASK-140: Fix unconditional writing of enrichment_status = 'enriched'
-- 1. Reclassify tracks currently marked as 'enriched' but missing key acoustic fields (bpm, musical_key, acoustid_fingerprint) to 'partial'.
-- 2. Install durable recurrence-prevention triggers to ensure tracks without complete acoustic fields cannot be persisted or updated as 'enriched'.

-- 1. Reclassify falsely marked tracks
UPDATE tracks
SET enrichment_status = 'partial'
WHERE enrichment_status = 'enriched'
  AND (bpm IS NULL OR musical_key IS NULL OR acoustid_fingerprint IS NULL);

-- 2. Durable recurrence-prevention triggers
CREATE TRIGGER IF NOT EXISTS trg_tracks_enrichment_status_enforce_insert
AFTER INSERT ON tracks
FOR EACH ROW
WHEN NEW.enrichment_status = 'enriched'
  AND (NEW.bpm IS NULL OR NEW.musical_key IS NULL OR TRIM(NEW.musical_key) = '' OR NEW.acoustid_fingerprint IS NULL OR TRIM(NEW.acoustid_fingerprint) = '')
BEGIN
    UPDATE tracks
    SET enrichment_status = 'partial'
    WHERE id = NEW.id;
END;

CREATE TRIGGER IF NOT EXISTS trg_tracks_enrichment_status_enforce_update
AFTER UPDATE OF enrichment_status, bpm, musical_key, acoustid_fingerprint ON tracks
FOR EACH ROW
WHEN NEW.enrichment_status = 'enriched'
  AND (NEW.bpm IS NULL OR NEW.musical_key IS NULL OR TRIM(NEW.musical_key) = '' OR NEW.acoustid_fingerprint IS NULL OR TRIM(NEW.acoustid_fingerprint) = '')
BEGIN
    UPDATE tracks
    SET enrichment_status = 'partial'
    WHERE id = NEW.id;
END;

-- Migration 0072: Normalize audio_quality enum to canonical lowercase values ('lossless', 'hires', 'lossy')
-- TASK-145: Normalize casing and non-canonical strings in tracks.audio_quality
-- 1. Normalizes all existing records with non-canonical casing or legacy values.
-- 2. Installs durable recurrence-prevention triggers on tracks for INSERT and UPDATE.

-- ============================================================================
-- 1. Normalize Existing Records in tracks
-- ============================================================================

-- Map lossless variants ('LOSSLESS', 'flac', etc.) to canonical 'lossless'
UPDATE tracks
SET audio_quality = 'lossless'
WHERE LOWER(audio_quality) IN ('lossless', 'flac');

-- Map hi-res variants ('HIRES', 'hi-res', 'high_resolution', 'hi_res', etc.) to canonical 'hires'
UPDATE tracks
SET audio_quality = 'hires'
WHERE LOWER(audio_quality) IN ('hires', 'hi-res', 'high_resolution', 'hi_res');

-- Map lossy variants ('standard', 'HIGH', 'LOW', 'normal', 'mp3', 'aac', etc.) to canonical 'lossy'
UPDATE tracks
SET audio_quality = 'lossy'
WHERE LOWER(audio_quality) IN ('lossy', 'standard', 'high', 'low', 'normal', 'mp3', 'aac');

-- ============================================================================
-- 2. Durable Recurrence-Prevention Triggers
-- ============================================================================

CREATE TRIGGER IF NOT EXISTS trg_tracks_normalize_audio_quality_ins
AFTER INSERT ON tracks
FOR EACH ROW
WHEN NEW.audio_quality IS NOT NULL
  AND NEW.audio_quality NOT IN ('lossless', 'hires', 'lossy')
BEGIN
    UPDATE tracks
    SET audio_quality = CASE
        WHEN LOWER(NEW.audio_quality) IN ('hires', 'hi-res', 'high_resolution', 'hi_res', 'hi_res_lossless', 'hires_lossless', 'max', '24-192', '24-96') THEN 'hires'
        WHEN LOWER(NEW.audio_quality) IN ('lossless', 'flac', 'cd', '16-44', 'alac', 'wav') THEN 'lossless'
        ELSE 'lossy'
    END
    WHERE id = NEW.id;
END;

CREATE TRIGGER IF NOT EXISTS trg_tracks_normalize_audio_quality_upd
AFTER UPDATE OF audio_quality ON tracks
FOR EACH ROW
WHEN NEW.audio_quality IS NOT NULL
  AND NEW.audio_quality NOT IN ('lossless', 'hires', 'lossy')
BEGIN
    UPDATE tracks
    SET audio_quality = CASE
        WHEN LOWER(NEW.audio_quality) IN ('hires', 'hi-res', 'high_resolution', 'hi_res', 'hi_res_lossless', 'hires_lossless', 'max', '24-192', '24-96') THEN 'hires'
        WHEN LOWER(NEW.audio_quality) IN ('lossless', 'flac', 'cd', '16-44', 'alac', 'wav') THEN 'lossless'
        ELSE 'lossy'
    END
    WHERE id = NEW.id;
END;

-- F4.7 / M11: Resetear enrichment_status = 'pending' para pistas con bpm IS NULL
-- Permite que el worker de análisis DSP de audio procese BPM y tonalidad reales.

PRAGMA foreign_keys = ON;
BEGIN TRANSACTION;

UPDATE tracks
SET enrichment_status = 'pending'
WHERE bpm IS NULL;

COMMIT;

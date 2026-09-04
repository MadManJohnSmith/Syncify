-- scripts/recalculate_audio_quality.sql
-- Recalcula tracks.audio_quality basado en la mejor fuente física real en track_sources
-- F3.6: Reparar pistas mal etiquetadas asignando el nivel de calidad más alto disponible

PRAGMA foreign_keys = ON;

BEGIN TRANSACTION;

WITH ranked AS (
    SELECT 
        track_id,
        CASE
            WHEN UPPER(TRIM(format)) IN ('FLAC', 'ALAC', 'WAV', 'AIFF', 'APE', 'LOSSLESS')
                 AND ((bit_depth IS NOT NULL AND bit_depth >= 24) OR (sample_rate IS NOT NULL AND (sample_rate > 48000 OR (sample_rate > 48 AND sample_rate <= 384))))
                THEN 'hires'
            WHEN UPPER(TRIM(format)) IN ('FLAC', 'ALAC', 'WAV', 'AIFF', 'APE', 'LOSSLESS')
                THEN 'lossless'
            WHEN bitrate IS NOT NULL AND bitrate >= 256
                THEN 'high'
            WHEN bitrate IS NOT NULL AND bitrate >= 128
                THEN 'medium'
            ELSE 'low'
        END AS calculated_quality,
        ROW_NUMBER() OVER (
            PARTITION BY track_id 
            ORDER BY 
                available DESC, 
                CASE
                    WHEN UPPER(TRIM(format)) IN ('FLAC', 'ALAC', 'WAV', 'AIFF', 'APE', 'LOSSLESS')
                         AND ((bit_depth IS NOT NULL AND bit_depth >= 24) OR (sample_rate IS NOT NULL AND (sample_rate > 48000 OR (sample_rate > 48 AND sample_rate <= 384))))
                        THEN 5
                    WHEN UPPER(TRIM(format)) IN ('FLAC', 'ALAC', 'WAV', 'AIFF', 'APE', 'LOSSLESS')
                        THEN 4
                    WHEN bitrate IS NOT NULL AND bitrate >= 256
                        THEN 3
                    WHEN bitrate IS NOT NULL AND bitrate >= 128
                        THEN 2
                    ELSE 1
                END DESC,
                id ASC
        ) as rn
    FROM track_sources
)
UPDATE tracks
SET audio_quality = ranked.calculated_quality
FROM ranked
WHERE tracks.id = ranked.track_id
  AND ranked.rn = 1
  AND (tracks.audio_quality IS NULL OR tracks.audio_quality != ranked.calculated_quality);

COMMIT;

PRAGMA foreign_key_check;

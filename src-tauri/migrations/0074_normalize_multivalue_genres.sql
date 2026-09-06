-- Migration 0074: Normalize multi-value genre strings in tracks table
-- TASK-142: Normalize compound genre strings with ';' or '/' in tracks.genre to primary clean genre.
-- Discrete multi-genre Vorbis comments are emitted in physical FLAC tags (Symfonium standard),
-- while tracks.genre in SQLite stores the clean primary genre without ';' or '/' delimiters.

-- ============================================================================
-- 1. Batch Normalization of Existing Records
-- ============================================================================

-- Strip secondary genres delimited by semicolon (';')
UPDATE tracks
SET genre = TRIM(SUBSTR(genre, 1, INSTR(genre, ';') - 1))
WHERE genre LIKE '%;%';

-- Strip secondary genres delimited by forward slash ('/')
UPDATE tracks
SET genre = TRIM(SUBSTR(genre, 1, INSTR(genre, '/') - 1))
WHERE genre LIKE '%/%';

-- Clean any residual empty strings to NULL
UPDATE tracks
SET genre = NULL
WHERE genre IS NOT NULL AND TRIM(genre) = '';

-- ============================================================================
-- 2. Durable Recurrence-Prevention Triggers
-- ============================================================================

CREATE TRIGGER IF NOT EXISTS trg_tracks_normalize_genre_ins
AFTER INSERT ON tracks
FOR EACH ROW
WHEN NEW.genre IS NOT NULL AND (NEW.genre LIKE '%;%' OR NEW.genre LIKE '%/%')
BEGIN
    UPDATE tracks
    SET genre = NULLIF(CASE
        WHEN INSTR(NEW.genre, ';') > 0 AND INSTR(NEW.genre, '/') > 0 THEN
            CASE
                WHEN INSTR(NEW.genre, ';') < INSTR(NEW.genre, '/') THEN
                    TRIM(SUBSTR(NEW.genre, 1, INSTR(NEW.genre, ';') - 1))
                ELSE
                    TRIM(SUBSTR(NEW.genre, 1, INSTR(NEW.genre, '/') - 1))
            END
        WHEN INSTR(NEW.genre, ';') > 0 THEN
            TRIM(SUBSTR(NEW.genre, 1, INSTR(NEW.genre, ';') - 1))
        WHEN INSTR(NEW.genre, '/') > 0 THEN
            TRIM(SUBSTR(NEW.genre, 1, INSTR(NEW.genre, '/') - 1))
        ELSE
            TRIM(NEW.genre)
    END, '')
    WHERE id = NEW.id;
END;

CREATE TRIGGER IF NOT EXISTS trg_tracks_normalize_genre_upd
AFTER UPDATE OF genre ON tracks
FOR EACH ROW
WHEN NEW.genre IS NOT NULL AND (NEW.genre LIKE '%;%' OR NEW.genre LIKE '%/%')
BEGIN
    UPDATE tracks
    SET genre = NULLIF(CASE
        WHEN INSTR(NEW.genre, ';') > 0 AND INSTR(NEW.genre, '/') > 0 THEN
            CASE
                WHEN INSTR(NEW.genre, ';') < INSTR(NEW.genre, '/') THEN
                    TRIM(SUBSTR(NEW.genre, 1, INSTR(NEW.genre, ';') - 1))
                ELSE
                    TRIM(SUBSTR(NEW.genre, 1, INSTR(NEW.genre, '/') - 1))
            END
        WHEN INSTR(NEW.genre, ';') > 0 THEN
            TRIM(SUBSTR(NEW.genre, 1, INSTR(NEW.genre, ';') - 1))
        WHEN INSTR(NEW.genre, '/') > 0 THEN
            TRIM(SUBSTR(NEW.genre, 1, INSTR(NEW.genre, '/') - 1))
        ELSE
            TRIM(NEW.genre)
    END, '')
    WHERE id = NEW.id;
END;

-- Migration 0082: Purge Technical Role Artists and Harden
-- TASK-68: Depuración de Artistas Hipertrofiados con Roles Técnicos de Qobuz en la Tabla `artists`.
--          Unify contaminated artists ('Guitar - Juan Perez', 'Choir - Coro de Praga', 'Composer - Beethoven',
--          'Producer - Quincy Jones', 'Vocals - John Doe') into canonical clean artists or rename survivors.
--          Reassign track_credits (preserving/assigning technical role), track_artists, and album_artists.
--          Purge unlinked residual contaminated records and install recurrence prevention triggers.

-- ============================================================================
-- 1. Identify all contaminated artists matching technical role prefixes
-- ============================================================================
DROP TABLE IF EXISTS _contaminated_artists_0082;
CREATE TEMP TABLE _contaminated_artists_0082 AS
SELECT
    id,
    name,
    trim(substr(
        name,
        CASE 
            WHEN instr(name, ' - ') > 0 THEN instr(name, ' - ') + 3
            WHEN instr(name, ' – ') > 0 THEN instr(name, ' – ') + 3
            WHEN instr(name, ' — ') > 0 THEN instr(name, ' — ') + 3
            WHEN instr(name, ': ') > 0 THEN instr(name, ': ') + 2
            WHEN instr(name, ':') > 0 THEN instr(name, ':') + 1
            ELSE 1
        END
    )) AS clean_name,
    trim(substr(
        name,
        1,
        CASE 
            WHEN instr(name, ' - ') > 0 THEN instr(name, ' - ') - 1
            WHEN instr(name, ' – ') > 0 THEN instr(name, ' – ') - 1
            WHEN instr(name, ' — ') > 0 THEN instr(name, ' — ') - 1
            WHEN instr(name, ': ') > 0 THEN instr(name, ': ') - 1
            WHEN instr(name, ':') > 0 THEN instr(name, ':') - 1
            ELSE 0
        END
    )) AS extracted_role
FROM artists
WHERE
    name LIKE 'Guitar - %'
    OR name LIKE 'Electric Guitar - %'
    OR name LIKE 'Acoustic Guitar - %'
    OR name LIKE 'Classical Guitar - %'
    OR name LIKE 'Lead Guitar - %'
    OR name LIKE 'Rhythm Guitar - %'
    OR name LIKE 'Bass - %'
    OR name LIKE 'Bass Guitar - %'
    OR name LIKE 'Acoustic Bass - %'
    OR name LIKE 'Double Bass - %'
    OR name LIKE 'Contrabass - %'
    OR name LIKE 'Drums - %'
    OR name LIKE 'Drum - %'
    OR name LIKE 'Percussion - %'
    OR name LIKE 'Vocals - %'
    OR name LIKE 'Vocal - %'
    OR name LIKE 'Lead Vocals - %'
    OR name LIKE 'Backing Vocals - %'
    OR name LIKE 'Background Vocals - %'
    OR name LIKE 'Additional Vocals - %'
    OR name LIKE 'Guest Vocals - %'
    OR name LIKE 'Voice - %'
    OR name LIKE 'Voices - %'
    OR name LIKE 'Choir - %'
    OR name LIKE 'Chorus - %'
    OR name LIKE 'Piano - %'
    OR name LIKE 'Keyboards - %'
    OR name LIKE 'Keyboard - %'
    OR name LIKE 'Organ - %'
    OR name LIKE 'Synthesizer - %'
    OR name LIKE 'Synth - %'
    OR name LIKE 'Synths - %'
    OR name LIKE 'Violin - %'
    OR name LIKE 'Viola - %'
    OR name LIKE 'Cello - %'
    OR name LIKE 'Violoncello - %'
    OR name LIKE 'Strings - %'
    OR name LIKE 'Harp - %'
    OR name LIKE 'Fiddle - %'
    OR name LIKE 'Banjo - %'
    OR name LIKE 'Mandolin - %'
    OR name LIKE 'Ukulele - %'
    OR name LIKE 'Trumpet - %'
    OR name LIKE 'Trombone - %'
    OR name LIKE 'Tuba - %'
    OR name LIKE 'French Horn - %'
    OR name LIKE 'Horn - %'
    OR name LIKE 'Horns - %'
    OR name LIKE 'Brass - %'
    OR name LIKE 'Saxophone - %'
    OR name LIKE 'Sax - %'
    OR name LIKE 'Alto Saxophone - %'
    OR name LIKE 'Tenor Saxophone - %'
    OR name LIKE 'Baritone Saxophone - %'
    OR name LIKE 'Soprano Saxophone - %'
    OR name LIKE 'Flute - %'
    OR name LIKE 'Clarinet - %'
    OR name LIKE 'Oboe - %'
    OR name LIKE 'Bassoon - %'
    OR name LIKE 'Woodwinds - %'
    OR name LIKE 'Harmonica - %'
    OR name LIKE 'Producer - %'
    OR name LIKE 'Co-Producer - %'
    OR name LIKE 'Executive Producer - %'
    OR name LIKE 'Associate Producer - %'
    OR name LIKE 'Additional Producer - %'
    OR name LIKE 'Composer - %'
    OR name LIKE 'Songwriter - %'
    OR name LIKE 'Writer - %'
    OR name LIKE 'Lyricist - %'
    OR name LIKE 'Arranger - %'
    OR name LIKE 'Conductor - %'
    OR name LIKE 'Mixer - %'
    OR name LIKE 'Mixing - %'
    OR name LIKE 'Mixing Engineer - %'
    OR name LIKE 'Sound Engineer - %'
    OR name LIKE 'Audio Engineer - %'
    OR name LIKE 'Recording Engineer - %'
    OR name LIKE 'Engineer - %'
    OR name LIKE 'Mastering Engineer - %'
    OR name LIKE 'Mastering - %'
    OR name LIKE 'Remastering - %'
    OR name LIKE 'Remastering Engineer - %'
    OR name LIKE 'Editing Engineer - %'
    OR name LIKE 'Programmer - %'
    OR name LIKE 'Programming - %'
    OR name LIKE 'DJ - %'
    OR name LIKE 'Guitar – %'
    OR name LIKE 'Bass – %'
    OR name LIKE 'Drums – %'
    OR name LIKE 'Vocals – %'
    OR name LIKE 'Choir – %'
    OR name LIKE 'Piano – %'
    OR name LIKE 'Producer – %'
    OR name LIKE 'Composer – %'
    OR name LIKE 'Mixer – %'
    OR name LIKE 'Engineer – %'
    OR name LIKE 'Guitar — %'
    OR name LIKE 'Bass — %'
    OR name LIKE 'Drums — %'
    OR name LIKE 'Vocals — %'
    OR name LIKE 'Choir — %'
    OR name LIKE 'Piano — %'
    OR name LIKE 'Producer — %'
    OR name LIKE 'Composer — %'
    OR name LIKE 'Mixer — %'
    OR name LIKE 'Engineer — %'
    OR name LIKE 'Producer: %'
    OR name LIKE 'Composer: %'
    OR name LIKE 'Guitar: %'
    OR name LIKE 'Engineer: %';

-- ============================================================================
-- 1b. Purge unlinked residual contaminated artists before remapping
-- ============================================================================
DELETE FROM track_credits
WHERE artist_id IN (
    SELECT id FROM _contaminated_artists_0082
    WHERE NOT EXISTS (SELECT 1 FROM track_artists ta WHERE ta.artist_id = _contaminated_artists_0082.id)
      AND NOT EXISTS (SELECT 1 FROM album_artists aa WHERE aa.artist_id = _contaminated_artists_0082.id)
      AND NOT EXISTS (SELECT 1 FROM track_credits tc WHERE tc.artist_id = _contaminated_artists_0082.id)
);

DELETE FROM artists
WHERE id IN (
    SELECT id FROM _contaminated_artists_0082
    WHERE NOT EXISTS (SELECT 1 FROM track_artists ta WHERE ta.artist_id = _contaminated_artists_0082.id)
      AND NOT EXISTS (SELECT 1 FROM album_artists aa WHERE aa.artist_id = _contaminated_artists_0082.id)
      AND NOT EXISTS (SELECT 1 FROM track_credits tc WHERE tc.artist_id = _contaminated_artists_0082.id)
);

DELETE FROM _contaminated_artists_0082
WHERE id NOT IN (SELECT id FROM artists);

-- ============================================================================
-- 2. Build mapping to canonical target (existing canonical artist or ranked winner)
-- ============================================================================
DROP TABLE IF EXISTS _artist_remapping_0082;
CREATE TEMP TABLE _artist_remapping_0082 AS
WITH existing_canonical AS (
    SELECT id, LOWER(TRIM(name)) AS norm_name
    FROM artists
    WHERE id NOT IN (SELECT id FROM _contaminated_artists_0082)
),
ranked_contaminated AS (
    SELECT
        c.id,
        c.name,
        c.clean_name,
        c.extracted_role,
        ec.id AS existing_id,
        ROW_NUMBER() OVER (
            PARTITION BY LOWER(TRIM(c.clean_name))
            ORDER BY
                (SELECT COUNT(*) FROM track_artists ta WHERE ta.artist_id = c.id) DESC,
                (SELECT COUNT(*) FROM album_artists aa WHERE aa.artist_id = c.id) DESC,
                (SELECT COUNT(*) FROM track_credits tc WHERE tc.artist_id = c.id) DESC,
                c.id ASC
        ) as rn
    FROM _contaminated_artists_0082 c
    LEFT JOIN existing_canonical ec ON ec.norm_name = LOWER(TRIM(c.clean_name))
    WHERE c.clean_name != ''
)
SELECT
    rc.id AS source_id,
    COALESCE(
        rc.existing_id,
        (SELECT r2.id FROM ranked_contaminated r2 WHERE LOWER(TRIM(r2.clean_name)) = LOWER(TRIM(rc.clean_name)) AND r2.rn = 1)
    ) AS target_id,
    rc.clean_name,
    rc.extracted_role,
    CASE WHEN rc.existing_id IS NULL AND rc.rn = 1 THEN 1 ELSE 0 END AS is_winner_to_rename
FROM ranked_contaminated rc;

-- ============================================================================
-- 3. Consolidate metadata & service IDs onto target artists before deletion
-- ============================================================================
UPDATE artists
SET
    musicbrainz_id = COALESCE(artists.musicbrainz_id, (
        SELECT src.musicbrainz_id FROM artists src
        JOIN _artist_remapping_0082 m ON m.source_id = src.id
        WHERE m.target_id = artists.id AND m.source_id != m.target_id
          AND src.musicbrainz_id IS NOT NULL AND src.musicbrainz_id != ''
        LIMIT 1
    )),
    spotify_id = COALESCE(artists.spotify_id, (
        SELECT src.spotify_id FROM artists src
        JOIN _artist_remapping_0082 m ON m.source_id = src.id
        WHERE m.target_id = artists.id AND m.source_id != m.target_id
          AND src.spotify_id IS NOT NULL AND src.spotify_id != ''
        LIMIT 1
    )),
    tidal_id = COALESCE(artists.tidal_id, (
        SELECT src.tidal_id FROM artists src
        JOIN _artist_remapping_0082 m ON m.source_id = src.id
        WHERE m.target_id = artists.id AND m.source_id != m.target_id
          AND src.tidal_id IS NOT NULL AND src.tidal_id != ''
        LIMIT 1
    )),
    qobuz_id = COALESCE(artists.qobuz_id, (
        SELECT src.qobuz_id FROM artists src
        JOIN _artist_remapping_0082 m ON m.source_id = src.id
        WHERE m.target_id = artists.id AND m.source_id != m.target_id
          AND src.qobuz_id IS NOT NULL AND src.qobuz_id != ''
        LIMIT 1
    )),
    image_url = COALESCE(artists.image_url, (
        SELECT src.image_url FROM artists src
        JOIN _artist_remapping_0082 m ON m.source_id = src.id
        WHERE m.target_id = artists.id AND m.source_id != m.target_id
          AND src.image_url IS NOT NULL AND src.image_url != ''
        LIMIT 1
    )),
    favorite_at = COALESCE(artists.favorite_at, (
        SELECT src.favorite_at FROM artists src
        JOIN _artist_remapping_0082 m ON m.source_id = src.id
        WHERE m.target_id = artists.id AND m.source_id != m.target_id
          AND src.favorite_at IS NOT NULL
        LIMIT 1
    )),
    is_favorite = MAX(
        COALESCE(artists.is_favorite, 0),
        COALESCE((
            SELECT MAX(src.is_favorite) FROM artists src
            JOIN _artist_remapping_0082 m ON m.source_id = src.id
            WHERE m.target_id = artists.id AND m.source_id != m.target_id
        ), 0)
    )
WHERE id IN (SELECT target_id FROM _artist_remapping_0082 WHERE source_id != target_id);

-- Clear service identifiers on source losers before deletion to prevent unique constraints
UPDATE artists
SET musicbrainz_id = NULL, tidal_id = NULL, qobuz_id = NULL, spotify_id = NULL
WHERE id IN (SELECT source_id FROM _artist_remapping_0082 WHERE source_id != target_id);

-- ============================================================================
-- 4. Reassign track_credits with proper roles
-- ============================================================================
-- First, ensure technical role is set on track_credits for contaminated artists
UPDATE track_credits
SET role = (
    SELECT m.extracted_role FROM _artist_remapping_0082 m
    WHERE m.source_id = track_credits.artist_id
)
WHERE artist_id IN (SELECT source_id FROM _artist_remapping_0082)
  AND (role IS NULL OR role = 'performer' OR role = '')
  AND EXISTS (
      SELECT 1 FROM _artist_remapping_0082 m2
      WHERE m2.source_id = track_credits.artist_id
        AND m2.extracted_role IS NOT NULL
        AND m2.extracted_role != ''
  );

-- Delete duplicate track_credits before updating artist_id
DELETE FROM track_credits
WHERE artist_id IN (SELECT source_id FROM _artist_remapping_0082 WHERE source_id != target_id)
  AND EXISTS (
      SELECT 1 FROM track_credits tc2
      JOIN _artist_remapping_0082 m ON m.source_id = track_credits.artist_id
      WHERE tc2.track_id = track_credits.track_id
        AND tc2.artist_id = m.target_id
        AND tc2.role = track_credits.role
  );

UPDATE track_credits
SET artist_id = (SELECT target_id FROM _artist_remapping_0082 WHERE source_id = track_credits.artist_id)
WHERE artist_id IN (SELECT source_id FROM _artist_remapping_0082 WHERE source_id != target_id);

-- ============================================================================
-- 5. Reassign track_artists
-- ============================================================================
DELETE FROM track_artists
WHERE artist_id IN (SELECT source_id FROM _artist_remapping_0082 WHERE source_id != target_id)
  AND EXISTS (
      SELECT 1 FROM track_artists ta2
      JOIN _artist_remapping_0082 m ON m.source_id = track_artists.artist_id
      WHERE ta2.track_id = track_artists.track_id
        AND ta2.artist_id = m.target_id
        AND COALESCE(ta2.role, 'primary') = COALESCE(track_artists.role, 'primary')
  );

UPDATE track_artists
SET artist_id = (SELECT target_id FROM _artist_remapping_0082 WHERE source_id = track_artists.artist_id)
WHERE artist_id IN (SELECT source_id FROM _artist_remapping_0082 WHERE source_id != target_id);

-- ============================================================================
-- 6. Reassign album_artists
-- ============================================================================
DELETE FROM album_artists
WHERE artist_id IN (SELECT source_id FROM _artist_remapping_0082 WHERE source_id != target_id)
  AND EXISTS (
      SELECT 1 FROM album_artists aa2
      JOIN _artist_remapping_0082 m ON m.source_id = album_artists.artist_id
      WHERE aa2.album_id = album_artists.album_id
        AND aa2.artist_id = m.target_id
  );

UPDATE album_artists
SET artist_id = (SELECT target_id FROM _artist_remapping_0082 WHERE source_id = album_artists.artist_id)
WHERE artist_id IN (SELECT source_id FROM _artist_remapping_0082 WHERE source_id != target_id);

-- ============================================================================
-- 7. Delete merged source artists
-- ============================================================================
DELETE FROM artists
WHERE id IN (SELECT source_id FROM _artist_remapping_0082 WHERE source_id != target_id);

-- ============================================================================
-- 8. Rename winner artists to clean names
-- ============================================================================
UPDATE artists
SET name = (SELECT clean_name FROM _artist_remapping_0082 WHERE source_id = artists.id)
WHERE id IN (SELECT source_id FROM _artist_remapping_0082 WHERE is_winner_to_rename = 1);

-- ============================================================================
-- 9. Purge residual unlinked contaminated artists
-- ============================================================================
DELETE FROM track_credits
WHERE artist_id IN (
    SELECT id FROM artists
    WHERE (
        name LIKE 'Guitar - %'
        OR name LIKE 'Bass - %'
        OR name LIKE 'Drums - %'
        OR name LIKE 'Vocals - %'
        OR name LIKE 'Choir - %'
        OR name LIKE 'Piano - %'
        OR name LIKE 'Producer - %'
        OR name LIKE 'Composer - %'
        OR name LIKE 'Mixer - %'
        OR name LIKE 'Engineer - %'
    )
    AND NOT EXISTS (SELECT 1 FROM track_artists ta WHERE ta.artist_id = artists.id)
    AND NOT EXISTS (SELECT 1 FROM album_artists aa WHERE aa.artist_id = artists.id)
);

DELETE FROM artists
WHERE (
    name LIKE 'Guitar - %'
    OR name LIKE 'Bass - %'
    OR name LIKE 'Drums - %'
    OR name LIKE 'Vocals - %'
    OR name LIKE 'Choir - %'
    OR name LIKE 'Piano - %'
    OR name LIKE 'Producer - %'
    OR name LIKE 'Composer - %'
    OR name LIKE 'Mixer - %'
    OR name LIKE 'Engineer - %'
)
AND NOT EXISTS (SELECT 1 FROM track_artists ta WHERE ta.artist_id = artists.id)
AND NOT EXISTS (SELECT 1 FROM album_artists aa WHERE aa.artist_id = artists.id);

DROP TABLE IF EXISTS _artist_remapping_0082;
DROP TABLE IF EXISTS _contaminated_artists_0082;

-- ============================================================================
-- 10. Recurrence prevention triggers
-- ============================================================================
CREATE TRIGGER IF NOT EXISTS trg_artists_reject_technical_roles_ins
BEFORE INSERT ON artists
FOR EACH ROW
WHEN 
    NEW.name LIKE 'Guitar - %'
    OR NEW.name LIKE 'Electric Guitar - %'
    OR NEW.name LIKE 'Acoustic Guitar - %'
    OR NEW.name LIKE 'Classical Guitar - %'
    OR NEW.name LIKE 'Lead Guitar - %'
    OR NEW.name LIKE 'Rhythm Guitar - %'
    OR NEW.name LIKE 'Bass - %'
    OR NEW.name LIKE 'Bass Guitar - %'
    OR NEW.name LIKE 'Acoustic Bass - %'
    OR NEW.name LIKE 'Double Bass - %'
    OR NEW.name LIKE 'Contrabass - %'
    OR NEW.name LIKE 'Drums - %'
    OR NEW.name LIKE 'Drum - %'
    OR NEW.name LIKE 'Percussion - %'
    OR NEW.name LIKE 'Vocals - %'
    OR NEW.name LIKE 'Vocal - %'
    OR NEW.name LIKE 'Lead Vocals - %'
    OR NEW.name LIKE 'Backing Vocals - %'
    OR NEW.name LIKE 'Background Vocals - %'
    OR NEW.name LIKE 'Voice - %'
    OR NEW.name LIKE 'Voices - %'
    OR NEW.name LIKE 'Choir - %'
    OR NEW.name LIKE 'Chorus - %'
    OR NEW.name LIKE 'Piano - %'
    OR NEW.name LIKE 'Keyboards - %'
    OR NEW.name LIKE 'Keyboard - %'
    OR NEW.name LIKE 'Organ - %'
    OR NEW.name LIKE 'Synthesizer - %'
    OR NEW.name LIKE 'Synth - %'
    OR NEW.name LIKE 'Violin - %'
    OR NEW.name LIKE 'Viola - %'
    OR NEW.name LIKE 'Cello - %'
    OR NEW.name LIKE 'Violoncello - %'
    OR NEW.name LIKE 'Strings - %'
    OR NEW.name LIKE 'Harp - %'
    OR NEW.name LIKE 'Trumpet - %'
    OR NEW.name LIKE 'Trombone - %'
    OR NEW.name LIKE 'Tuba - %'
    OR NEW.name LIKE 'French Horn - %'
    OR NEW.name LIKE 'Horn - %'
    OR NEW.name LIKE 'Horns - %'
    OR NEW.name LIKE 'Brass - %'
    OR NEW.name LIKE 'Saxophone - %'
    OR NEW.name LIKE 'Sax - %'
    OR NEW.name LIKE 'Flute - %'
    OR NEW.name LIKE 'Clarinet - %'
    OR NEW.name LIKE 'Oboe - %'
    OR NEW.name LIKE 'Bassoon - %'
    OR NEW.name LIKE 'Woodwinds - %'
    OR NEW.name LIKE 'Harmonica - %'
    OR NEW.name LIKE 'Producer - %'
    OR NEW.name LIKE 'Co-Producer - %'
    OR NEW.name LIKE 'Executive Producer - %'
    OR NEW.name LIKE 'Composer - %'
    OR NEW.name LIKE 'Songwriter - %'
    OR NEW.name LIKE 'Writer - %'
    OR NEW.name LIKE 'Lyricist - %'
    OR NEW.name LIKE 'Arranger - %'
    OR NEW.name LIKE 'Conductor - %'
    OR NEW.name LIKE 'Mixer - %'
    OR NEW.name LIKE 'Mixing - %'
    OR NEW.name LIKE 'Mixing Engineer - %'
    OR NEW.name LIKE 'Sound Engineer - %'
    OR NEW.name LIKE 'Audio Engineer - %'
    OR NEW.name LIKE 'Recording Engineer - %'
    OR NEW.name LIKE 'Engineer - %'
    OR NEW.name LIKE 'Mastering Engineer - %'
    OR NEW.name LIKE 'Mastering - %'
    OR NEW.name LIKE 'Remastering - %'
    OR NEW.name LIKE 'Editing Engineer - %'
    OR NEW.name LIKE 'Programmer - %'
    OR NEW.name LIKE 'Programming - %'
    OR NEW.name LIKE 'DJ - %'
BEGIN
    SELECT RAISE(ABORT, 'Rejected artist name with technical role prefix');
END;

CREATE TRIGGER IF NOT EXISTS trg_artists_reject_technical_roles_upd
BEFORE UPDATE OF name ON artists
FOR EACH ROW
WHEN 
    NEW.name LIKE 'Guitar - %'
    OR NEW.name LIKE 'Electric Guitar - %'
    OR NEW.name LIKE 'Acoustic Guitar - %'
    OR NEW.name LIKE 'Classical Guitar - %'
    OR NEW.name LIKE 'Lead Guitar - %'
    OR NEW.name LIKE 'Rhythm Guitar - %'
    OR NEW.name LIKE 'Bass - %'
    OR NEW.name LIKE 'Bass Guitar - %'
    OR NEW.name LIKE 'Acoustic Bass - %'
    OR NEW.name LIKE 'Double Bass - %'
    OR NEW.name LIKE 'Contrabass - %'
    OR NEW.name LIKE 'Drums - %'
    OR NEW.name LIKE 'Drum - %'
    OR NEW.name LIKE 'Percussion - %'
    OR NEW.name LIKE 'Vocals - %'
    OR NEW.name LIKE 'Vocal - %'
    OR NEW.name LIKE 'Lead Vocals - %'
    OR NEW.name LIKE 'Backing Vocals - %'
    OR NEW.name LIKE 'Background Vocals - %'
    OR NEW.name LIKE 'Voice - %'
    OR NEW.name LIKE 'Voices - %'
    OR NEW.name LIKE 'Choir - %'
    OR NEW.name LIKE 'Chorus - %'
    OR NEW.name LIKE 'Piano - %'
    OR NEW.name LIKE 'Keyboards - %'
    OR NEW.name LIKE 'Keyboard - %'
    OR NEW.name LIKE 'Organ - %'
    OR NEW.name LIKE 'Synthesizer - %'
    OR NEW.name LIKE 'Synth - %'
    OR NEW.name LIKE 'Violin - %'
    OR NEW.name LIKE 'Viola - %'
    OR NEW.name LIKE 'Cello - %'
    OR NEW.name LIKE 'Violoncello - %'
    OR NEW.name LIKE 'Strings - %'
    OR NEW.name LIKE 'Harp - %'
    OR NEW.name LIKE 'Trumpet - %'
    OR NEW.name LIKE 'Trombone - %'
    OR NEW.name LIKE 'Tuba - %'
    OR NEW.name LIKE 'French Horn - %'
    OR NEW.name LIKE 'Horn - %'
    OR NEW.name LIKE 'Horns - %'
    OR NEW.name LIKE 'Brass - %'
    OR NEW.name LIKE 'Saxophone - %'
    OR NEW.name LIKE 'Sax - %'
    OR NEW.name LIKE 'Flute - %'
    OR NEW.name LIKE 'Clarinet - %'
    OR NEW.name LIKE 'Oboe - %'
    OR NEW.name LIKE 'Bassoon - %'
    OR NEW.name LIKE 'Woodwinds - %'
    OR NEW.name LIKE 'Harmonica - %'
    OR NEW.name LIKE 'Producer - %'
    OR NEW.name LIKE 'Co-Producer - %'
    OR NEW.name LIKE 'Executive Producer - %'
    OR NEW.name LIKE 'Composer - %'
    OR NEW.name LIKE 'Songwriter - %'
    OR NEW.name LIKE 'Writer - %'
    OR NEW.name LIKE 'Lyricist - %'
    OR NEW.name LIKE 'Arranger - %'
    OR NEW.name LIKE 'Conductor - %'
    OR NEW.name LIKE 'Mixer - %'
    OR NEW.name LIKE 'Mixing - %'
    OR NEW.name LIKE 'Mixing Engineer - %'
    OR NEW.name LIKE 'Sound Engineer - %'
    OR NEW.name LIKE 'Audio Engineer - %'
    OR NEW.name LIKE 'Recording Engineer - %'
    OR NEW.name LIKE 'Engineer - %'
    OR NEW.name LIKE 'Mastering Engineer - %'
    OR NEW.name LIKE 'Mastering - %'
    OR NEW.name LIKE 'Remastering - %'
    OR NEW.name LIKE 'Editing Engineer - %'
    OR NEW.name LIKE 'Programmer - %'
    OR NEW.name LIKE 'Programming - %'
    OR NEW.name LIKE 'DJ - %'
BEGIN
    SELECT RAISE(ABORT, 'Rejected artist name with technical role prefix');
END;

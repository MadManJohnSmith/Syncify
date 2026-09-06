-- Migration 0071: Clean residual mojibake and control characters in albums and tracks
-- TASK-144: Fix residual mojibake sequences (Â¿, àº, Êº) and embedded control characters (\r, \n, \t).
-- 1. Updates specific known corrupted records (track 4037, album 4918).
-- 2. Sanitizes embedded carriage returns, newlines, tabs, and empty parentheticals in albums and tracks.
-- 3. Cleans residual mojibake sequences across all album and track titles.
-- 4. Installs durable recurrence-prevention triggers to sanitize future inserts and updates.

-- ============================================================================
-- 1. Point Fixes for Known Corrupted Records
-- ============================================================================

UPDATE tracks
SET title = '¿Y Tú Qué Has Hecho?'
WHERE id = 4037;

UPDATE albums
SET title = 'Attack Decay Sustain Release'
WHERE id = 4918;

-- ============================================================================
-- 2. Embedded Control Characters Sanitization (Tracks & Albums)
-- ============================================================================

-- Clean carriage returns, newlines, and tabs
UPDATE tracks
SET title = REPLACE(REPLACE(REPLACE(title, char(13), ' '), char(10), ' '), char(9), ' ')
WHERE title LIKE '%' || char(13) || '%'
   OR title LIKE '%' || char(10) || '%'
   OR title LIKE '%' || char(9) || '%';

UPDATE albums
SET title = REPLACE(REPLACE(REPLACE(title, char(13), ' '), char(10), ' '), char(9), ' ')
WHERE title LIKE '%' || char(13) || '%'
   OR title LIKE '%' || char(10) || '%'
   OR title LIKE '%' || char(9) || '%';

-- Collapse multiple consecutive spaces
UPDATE tracks SET title = REPLACE(title, '        ', ' ') WHERE title LIKE '%        %';
UPDATE tracks SET title = REPLACE(title, '    ', ' ') WHERE title LIKE '%    %';
UPDATE tracks SET title = REPLACE(title, '  ', ' ') WHERE title LIKE '%  %';
UPDATE tracks SET title = REPLACE(title, '  ', ' ') WHERE title LIKE '%  %';

UPDATE albums SET title = REPLACE(title, '        ', ' ') WHERE title LIKE '%        %';
UPDATE albums SET title = REPLACE(title, '    ', ' ') WHERE title LIKE '%    %';
UPDATE albums SET title = REPLACE(title, '  ', ' ') WHERE title LIKE '%  %';
UPDATE albums SET title = REPLACE(title, '  ', ' ') WHERE title LIKE '%  %';

-- Clean empty parentheticals left by stripped controls
UPDATE tracks SET title = TRIM(REPLACE(title, '(   )', '')) WHERE title LIKE '%(   )%';
UPDATE tracks SET title = TRIM(REPLACE(title, '(  )', '')) WHERE title LIKE '%(  )%';
UPDATE tracks SET title = TRIM(REPLACE(title, '( )', '')) WHERE title LIKE '%( )%';
UPDATE tracks SET title = TRIM(REPLACE(title, '()', '')) WHERE title LIKE '%()%';
UPDATE tracks SET title = TRIM(title) WHERE title != TRIM(title);

UPDATE albums SET title = TRIM(REPLACE(title, '(   )', '')) WHERE title LIKE '%(   )%';
UPDATE albums SET title = TRIM(REPLACE(title, '(  )', '')) WHERE title LIKE '%(  )%';
UPDATE albums SET title = TRIM(REPLACE(title, '( )', '')) WHERE title LIKE '%( )%';
UPDATE albums SET title = TRIM(REPLACE(title, '()', '')) WHERE title LIKE '%()%';
UPDATE albums SET title = TRIM(title) WHERE title != TRIM(title);

-- ============================================================================
-- 3. Residual Mojibake Sequences Sanitization (Tracks & Albums)
-- ============================================================================

UPDATE tracks
SET title = REPLACE(
    REPLACE(
        REPLACE(
            REPLACE(
                REPLACE(title, 'Â¿', '¿'),
                'Â¡', '¡'
            ),
            'àº', 'ú'
        ),
        'Àº', 'Ú'
    ),
    'Êº', '”'
)
WHERE title LIKE '%Â¿%'
   OR title LIKE '%Â¡%'
   OR title LIKE '%àº%'
   OR title LIKE '%Àº%'
   OR title LIKE '%Êº%';

UPDATE albums
SET title = REPLACE(
    REPLACE(
        REPLACE(
            REPLACE(
                REPLACE(title, 'Â¿', '¿'),
                'Â¡', '¡'
            ),
            'àº', 'ú'
        ),
        'Àº', 'Ú'
    ),
    'Êº', '”'
)
WHERE title LIKE '%Â¿%'
   OR title LIKE '%Â¡%'
   OR title LIKE '%àº%'
   OR title LIKE '%Àº%'
   OR title LIKE '%Êº%';

-- ============================================================================
-- 4. Durable Recurrence-Prevention Triggers
-- ============================================================================

CREATE TRIGGER IF NOT EXISTS trg_albums_clean_mojibake_controls_ins
AFTER INSERT ON albums
FOR EACH ROW
WHEN NEW.title LIKE '%' || char(10) || '%'
  OR NEW.title LIKE '%' || char(13) || '%'
  OR NEW.title LIKE '%' || char(9) || '%'
  OR NEW.title LIKE '%Â¿%'
  OR NEW.title LIKE '%Â¡%'
  OR NEW.title LIKE '%àº%'
  OR NEW.title LIKE '%Àº%'
  OR NEW.title LIKE '%Êº%'
  OR NEW.title LIKE '%( )%'
  OR NEW.title LIKE '%(  )%'
  OR NEW.title LIKE '%()%'
BEGIN
    UPDATE albums
    SET title = TRIM(
        REPLACE(
            REPLACE(
                REPLACE(
                    REPLACE(
                        REPLACE(
                            REPLACE(
                                REPLACE(
                                    REPLACE(
                                        REPLACE(
                                            REPLACE(
                                                REPLACE(NEW.title, char(13), ' '),
                                                char(10), ' '
                                            ),
                                            char(9), ' '
                                        ),
                                        'Â¿', '¿'
                                    ),
                                    'Â¡', '¡'
                                ),
                                'àº', 'ú'
                            ),
                            'Àº', 'Ú'
                        ),
                        'Êº', '”'
                    ),
                    '(  )', ''
                ),
                '( )', ''
            ),
            '()', ''
        )
    )
    WHERE id = NEW.id;
END;

CREATE TRIGGER IF NOT EXISTS trg_albums_clean_mojibake_controls_upd
AFTER UPDATE OF title ON albums
FOR EACH ROW
WHEN NEW.title LIKE '%' || char(10) || '%'
  OR NEW.title LIKE '%' || char(13) || '%'
  OR NEW.title LIKE '%' || char(9) || '%'
  OR NEW.title LIKE '%Â¿%'
  OR NEW.title LIKE '%Â¡%'
  OR NEW.title LIKE '%àº%'
  OR NEW.title LIKE '%Àº%'
  OR NEW.title LIKE '%Êº%'
  OR NEW.title LIKE '%( )%'
  OR NEW.title LIKE '%(  )%'
  OR NEW.title LIKE '%()%'
BEGIN
    UPDATE albums
    SET title = TRIM(
        REPLACE(
            REPLACE(
                REPLACE(
                    REPLACE(
                        REPLACE(
                            REPLACE(
                                REPLACE(
                                    REPLACE(
                                        REPLACE(
                                            REPLACE(
                                                REPLACE(NEW.title, char(13), ' '),
                                                char(10), ' '
                                            ),
                                            char(9), ' '
                                        ),
                                        'Â¿', '¿'
                                    ),
                                    'Â¡', '¡'
                                ),
                                'àº', 'ú'
                            ),
                            'Àº', 'Ú'
                        ),
                        'Êº', '”'
                    ),
                    '(  )', ''
                ),
                '( )', ''
            ),
            '()', ''
        )
    )
    WHERE id = NEW.id;
END;

CREATE TRIGGER IF NOT EXISTS trg_tracks_clean_mojibake_controls_ins
AFTER INSERT ON tracks
FOR EACH ROW
WHEN NEW.title LIKE '%' || char(10) || '%'
  OR NEW.title LIKE '%' || char(13) || '%'
  OR NEW.title LIKE '%' || char(9) || '%'
  OR NEW.title LIKE '%Â¿%'
  OR NEW.title LIKE '%Â¡%'
  OR NEW.title LIKE '%àº%'
  OR NEW.title LIKE '%Àº%'
  OR NEW.title LIKE '%Êº%'
  OR NEW.title LIKE '%( )%'
  OR NEW.title LIKE '%(  )%'
  OR NEW.title LIKE '%()%'
BEGIN
    UPDATE tracks
    SET title = TRIM(
        REPLACE(
            REPLACE(
                REPLACE(
                    REPLACE(
                        REPLACE(
                            REPLACE(
                                REPLACE(
                                    REPLACE(
                                        REPLACE(
                                            REPLACE(
                                                REPLACE(NEW.title, char(13), ' '),
                                                char(10), ' '
                                            ),
                                            char(9), ' '
                                        ),
                                        'Â¿', '¿'
                                    ),
                                    'Â¡', '¡'
                                ),
                                'àº', 'ú'
                            ),
                            'Àº', 'Ú'
                        ),
                        'Êº', '”'
                    ),
                    '(  )', ''
                ),
                '( )', ''
            ),
            '()', ''
        )
    )
    WHERE id = NEW.id;
END;

CREATE TRIGGER IF NOT EXISTS trg_tracks_clean_mojibake_controls_upd
AFTER UPDATE OF title ON tracks
FOR EACH ROW
WHEN NEW.title LIKE '%' || char(10) || '%'
  OR NEW.title LIKE '%' || char(13) || '%'
  OR NEW.title LIKE '%' || char(9) || '%'
  OR NEW.title LIKE '%Â¿%'
  OR NEW.title LIKE '%Â¡%'
  OR NEW.title LIKE '%àº%'
  OR NEW.title LIKE '%Àº%'
  OR NEW.title LIKE '%Êº%'
  OR NEW.title LIKE '%( )%'
  OR NEW.title LIKE '%(  )%'
  OR NEW.title LIKE '%()%'
BEGIN
    UPDATE tracks
    SET title = TRIM(
        REPLACE(
            REPLACE(
                REPLACE(
                    REPLACE(
                        REPLACE(
                            REPLACE(
                                REPLACE(
                                    REPLACE(
                                        REPLACE(
                                            REPLACE(
                                                REPLACE(NEW.title, char(13), ' '),
                                                char(10), ' '
                                            ),
                                            char(9), ' '
                                        ),
                                        'Â¿', '¿'
                                    ),
                                    'Â¡', '¡'
                                ),
                                'àº', 'ú'
                            ),
                            'Àº', 'Ú'
                        ),
                        'Êº', '”'
                    ),
                    '(  )', ''
                ),
                '( )', ''
            ),
            '()', ''
        )
    )
    WHERE id = NEW.id;
END;

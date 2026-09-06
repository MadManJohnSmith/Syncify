#!/usr/bin/env python3
"""
Syncify Carriage Return Credits & Technical Artists Purge Script (TASK-133)
==========================================================================
1. Creates safety snapshot using VACUUM INTO in /tmp/ (syncify_backup_pre_repair_TASK-133_<timestamp>.db).
2. Identifies all artists containing raw carriage returns '\\r' (char 13), line feeds '\\n' (char 10),
   and technical credit prefixes ('Recording Engineer\\r - Tony Castle', 'Synthesizer\\r - Daft Punk').
3. Remaps contaminated artists to canonical existing artists or renames survivor records.
4. Reassigns track_credits, track_artists, and album_artists without violating composite uniqueness.
5. Consolidates external service IDs and favorites onto canonical artists.
6. Purges unlinked residual contaminated records.
7. Installs triggers preventing future recurrence of carriage returns or control characters.
8. Verifies database integrity and foreign key validity.

Usage:
    python3 scripts/purge_carriage_return_credits.py [--db-path PATH] [--backup-dir DIR] [--dry-run]
"""

import argparse
import os
import shutil
import sqlite3
import sys
import time

DEFAULT_DB_CANDIDATES = [
    os.path.expanduser("~/.local/share/com.syncify.app/syncify.db"),
    os.path.abspath("src-tauri/syncify.db"),
    os.path.abspath("syncify.db"),
    os.path.abspath("workspace/audit_archive/data/syncify.db"),
]


def find_default_db() -> str:
    for path in DEFAULT_DB_CANDIDATES:
        if os.path.exists(path):
            return path
    return DEFAULT_DB_CANDIDATES[0]


def parse_args():
    parser = argparse.ArgumentParser(
        description="Purge carriage returns and technical credit prefixes from artists in Syncify SQLite DB."
    )
    default_db = find_default_db()
    parser.add_argument(
        "--db-path",
        default=default_db,
        help=f"Path to syncify.db (default: {default_db})",
    )
    parser.add_argument(
        "--backup-dir",
        default="/tmp",
        help="Directory to save pre-repair backup snapshot (default: /tmp)",
    )
    parser.add_argument(
        "--dry-run",
        action="store_true",
        help="Simulate and report diagnostics without committing changes to disk",
    )
    return parser.parse_args()


def create_safety_backup(db_path: str, backup_dir: str) -> str:
    timestamp = int(time.time())
    backup_path = os.path.join(backup_dir, f"syncify_backup_pre_repair_TASK-133_{timestamp}.db")
    os.makedirs(backup_dir, exist_ok=True)
    print(f"[TASK-133] Creating safety snapshot at {backup_path}...")

    try:
        src_conn = sqlite3.connect(f"file:{os.path.abspath(db_path)}?mode=ro", uri=True)
        src_conn.execute(f"VACUUM INTO '{backup_path}'")
        src_conn.close()
        print(f"[TASK-133] Safety snapshot created via VACUUM INTO: {backup_path}")
        return backup_path
    except Exception as e:
        print(f"[TASK-133] VACUUM INTO fallback ({e}), attempting file copy...")

    shutil.copy2(db_path, backup_path)
    print(f"[TASK-133] Safety snapshot created via copy: {backup_path}")
    return backup_path


PURGE_AND_HARDEN_SQL = """
-- 1. Identify all contaminated artists containing \\r (char 13) or \\n (char 10)
DROP TABLE IF EXISTS _contaminated_artists_0080;
CREATE TEMP TABLE _contaminated_artists_0080 AS
SELECT
    id,
    name,
    trim(ltrim(
        substr(name, CASE WHEN instr(name, char(13)) > 0 THEN instr(name, char(13)) ELSE instr(name, char(10)) END + 1),
        char(10) || char(13) || ' ' || char(9) || '-' || ':' || '–' || '—'
    )) AS clean_name,
    trim(substr(name, 1, CASE WHEN instr(name, char(13)) > 0 THEN instr(name, char(13)) ELSE instr(name, char(10)) END - 1)) AS extracted_role
FROM artists
WHERE instr(name, char(13)) > 0 OR instr(name, char(10)) > 0;

-- 2. Build mapping to canonical target (existing canonical artist or ranked winner)
DROP TABLE IF EXISTS _artist_remapping_0080;
CREATE TEMP TABLE _artist_remapping_0080 AS
WITH existing_canonical AS (
    SELECT id, LOWER(TRIM(name)) AS norm_name
    FROM artists
    WHERE instr(name, char(13)) = 0 AND instr(name, char(10)) = 0
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
    FROM _contaminated_artists_0080 c
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

-- 3. Consolidate metadata & service IDs onto target artists before deletion
UPDATE artists
SET
    musicbrainz_id = COALESCE(artists.musicbrainz_id, (
        SELECT src.musicbrainz_id FROM artists src
        JOIN _artist_remapping_0080 m ON m.source_id = src.id
        WHERE m.target_id = artists.id AND m.source_id != m.target_id
          AND src.musicbrainz_id IS NOT NULL AND src.musicbrainz_id != ''
        LIMIT 1
    )),
    spotify_id = COALESCE(artists.spotify_id, (
        SELECT src.spotify_id FROM artists src
        JOIN _artist_remapping_0080 m ON m.source_id = src.id
        WHERE m.target_id = artists.id AND m.source_id != m.target_id
          AND src.spotify_id IS NOT NULL AND src.spotify_id != ''
        LIMIT 1
    )),
    tidal_id = COALESCE(artists.tidal_id, (
        SELECT src.tidal_id FROM artists src
        JOIN _artist_remapping_0080 m ON m.source_id = src.id
        WHERE m.target_id = artists.id AND m.source_id != m.target_id
          AND src.tidal_id IS NOT NULL AND src.tidal_id != ''
        LIMIT 1
    )),
    qobuz_id = COALESCE(artists.qobuz_id, (
        SELECT src.qobuz_id FROM artists src
        JOIN _artist_remapping_0080 m ON m.source_id = src.id
        WHERE m.target_id = artists.id AND m.source_id != m.target_id
          AND src.qobuz_id IS NOT NULL AND src.qobuz_id != ''
        LIMIT 1
    )),
    image_url = COALESCE(artists.image_url, (
        SELECT src.image_url FROM artists src
        JOIN _artist_remapping_0080 m ON m.source_id = src.id
        WHERE m.target_id = artists.id AND m.source_id != m.target_id
          AND src.image_url IS NOT NULL AND src.image_url != ''
        LIMIT 1
    )),
    favorite_at = COALESCE(artists.favorite_at, (
        SELECT src.favorite_at FROM artists src
        JOIN _artist_remapping_0080 m ON m.source_id = src.id
        WHERE m.target_id = artists.id AND m.source_id != m.target_id
          AND src.favorite_at IS NOT NULL
        LIMIT 1
    )),
    is_favorite = MAX(
        COALESCE(artists.is_favorite, 0),
        COALESCE((
            SELECT MAX(src.is_favorite) FROM artists src
            JOIN _artist_remapping_0080 m ON m.source_id = src.id
            WHERE m.target_id = artists.id AND m.source_id != m.target_id
        ), 0)
    )
WHERE id IN (SELECT target_id FROM _artist_remapping_0080 WHERE source_id != target_id);

-- Clear service identifiers on source losers before deletion to prevent unique constraints
UPDATE artists
SET musicbrainz_id = NULL, tidal_id = NULL, qobuz_id = NULL, spotify_id = NULL
WHERE id IN (SELECT source_id FROM _artist_remapping_0080 WHERE source_id != target_id);

-- 4. Reassign track_credits
DELETE FROM track_credits
WHERE artist_id IN (SELECT source_id FROM _artist_remapping_0080 WHERE source_id != target_id)
  AND EXISTS (
      SELECT 1 FROM track_credits tc2
      JOIN _artist_remapping_0080 m ON m.source_id = track_credits.artist_id
      WHERE tc2.track_id = track_credits.track_id
        AND tc2.artist_id = m.target_id
        AND tc2.role = track_credits.role
  );

UPDATE track_credits
SET artist_id = (SELECT target_id FROM _artist_remapping_0080 WHERE source_id = track_credits.artist_id)
WHERE artist_id IN (SELECT source_id FROM _artist_remapping_0080 WHERE source_id != target_id);

-- 5. Reassign track_artists
DELETE FROM track_artists
WHERE artist_id IN (SELECT source_id FROM _artist_remapping_0080 WHERE source_id != target_id)
  AND EXISTS (
      SELECT 1 FROM track_artists ta2
      JOIN _artist_remapping_0080 m ON m.source_id = track_artists.artist_id
      WHERE ta2.track_id = track_artists.track_id
        AND ta2.artist_id = m.target_id
        AND COALESCE(ta2.role, 'primary') = COALESCE(track_artists.role, 'primary')
  );

UPDATE track_artists
SET artist_id = (SELECT target_id FROM _artist_remapping_0080 WHERE source_id = track_artists.artist_id)
WHERE artist_id IN (SELECT source_id FROM _artist_remapping_0080 WHERE source_id != target_id);

-- 6. Reassign album_artists
DELETE FROM album_artists
WHERE artist_id IN (SELECT source_id FROM _artist_remapping_0080 WHERE source_id != target_id)
  AND EXISTS (
      SELECT 1 FROM album_artists aa2
      JOIN _artist_remapping_0080 m ON m.source_id = album_artists.artist_id
      WHERE aa2.album_id = album_artists.album_id
        AND aa2.artist_id = m.target_id
  );

UPDATE album_artists
SET artist_id = (SELECT target_id FROM _artist_remapping_0080 WHERE source_id = album_artists.artist_id)
WHERE artist_id IN (SELECT source_id FROM _artist_remapping_0080 WHERE source_id != target_id);

-- 7. Delete merged source artists
DELETE FROM artists
WHERE id IN (SELECT source_id FROM _artist_remapping_0080 WHERE source_id != target_id);

-- 8. Rename winner artists to their clean names
UPDATE artists
SET name = (SELECT clean_name FROM _artist_remapping_0080 WHERE source_id = artists.id)
WHERE id IN (SELECT source_id FROM _artist_remapping_0080 WHERE is_winner_to_rename = 1);

-- 9. Purge residual unlinked artists containing \\r or \\n
DELETE FROM track_credits
WHERE artist_id IN (
    SELECT id FROM artists
    WHERE (instr(name, char(13)) > 0 OR instr(name, char(10)) > 0)
      AND NOT EXISTS (SELECT 1 FROM track_artists ta WHERE ta.artist_id = artists.id)
      AND NOT EXISTS (SELECT 1 FROM album_artists aa WHERE aa.artist_id = artists.id)
);

DELETE FROM artists
WHERE (instr(name, char(13)) > 0 OR instr(name, char(10)) > 0)
  AND NOT EXISTS (SELECT 1 FROM track_artists ta WHERE ta.artist_id = artists.id)
  AND NOT EXISTS (SELECT 1 FROM album_artists aa WHERE aa.artist_id = artists.id);

DROP TABLE IF EXISTS _artist_remapping_0080;
DROP TABLE IF EXISTS _contaminated_artists_0080;

-- 10. Recurrence prevention triggers
CREATE TRIGGER IF NOT EXISTS trg_artists_reject_control_chars_ins
BEFORE INSERT ON artists
FOR EACH ROW
WHEN instr(NEW.name, char(13)) > 0 
  OR instr(NEW.name, char(10)) > 0
  OR instr(NEW.name, char(9)) > 0
BEGIN
    SELECT RAISE(ABORT, 'Rejected artist name containing carriage return or line breaks');
END;

CREATE TRIGGER IF NOT EXISTS trg_artists_reject_control_chars_upd
BEFORE UPDATE OF name ON artists
FOR EACH ROW
WHEN instr(NEW.name, char(13)) > 0 
  OR instr(NEW.name, char(10)) > 0
  OR instr(NEW.name, char(9)) > 0
BEGIN
    SELECT RAISE(ABORT, 'Rejected artist name containing carriage return or line breaks');
END;
"""


def run_purge(db_path: str, backup_dir: str = "/tmp", dry_run: bool = False) -> bool:
    if not os.path.exists(db_path):
        print(f"[TASK-133] Error: Database not found at {db_path}", file=sys.stderr)
        return False

    print(f"[TASK-133] Target database: {db_path}")

    # Check contaminated artists count
    conn = sqlite3.connect(f"file:{os.path.abspath(db_path)}?mode=ro", uri=True)
    cur = conn.cursor()
    cur.execute("SELECT COUNT(*) FROM artists WHERE instr(name, char(13)) > 0 OR instr(name, char(10)) > 0")
    initial_contaminated = cur.fetchone()[0]
    conn.close()

    print(f"[TASK-133] Contaminated artists detected before repair: {initial_contaminated}")

    if dry_run:
        print("[TASK-133] Dry-run mode enabled. No changes committed to disk.")
        return True

    # 1. Safety snapshot
    create_safety_backup(db_path, backup_dir)

    # 2. Run purge in transaction
    conn = sqlite3.connect(db_path, timeout=60.0)
    conn.execute("PRAGMA foreign_keys = ON;")
    cur = conn.cursor()

    try:
        cur.execute("BEGIN TRANSACTION;")
        cur.executescript(PURGE_AND_HARDEN_SQL)
        conn.commit()
        print("[TASK-133] Purge transaction executed and committed successfully.")
    except Exception as e:
        conn.rollback()
        conn.close()
        print(f"[TASK-133] Error during purge transaction: {e}", file=sys.stderr)
        return False

    # 3. Post-execution assertions and verification
    cur.execute("SELECT COUNT(*) FROM artists WHERE instr(name, char(13)) > 0 OR instr(name, char(10)) > 0")
    remaining_contaminated = cur.fetchone()[0]

    cur.execute("PRAGMA foreign_key_check;")
    fk_violations = cur.fetchall()

    cur.execute("PRAGMA integrity_check;")
    integrity = cur.fetchall()

    conn.close()

    print(f"[TASK-133] Remaining contaminated artists: {remaining_contaminated}")
    print(f"[TASK-133] Foreign key violations: {len(fk_violations)}")
    print(f"[TASK-133] Integrity check result: {integrity[0][0] if integrity else 'unknown'}")

    if remaining_contaminated != 0:
        print(f"[TASK-133] Failure: {remaining_contaminated} contaminated artists remain!", file=sys.stderr)
        return False

    if fk_violations:
        print(f"[TASK-133] Failure: Foreign key violations detected: {fk_violations}", file=sys.stderr)
        return False

    if not integrity or integrity[0][0] != "ok":
        print(f"[TASK-133] Failure: Database integrity check failed: {integrity}", file=sys.stderr)
        return False

    print("[TASK-133] Verification PASSED (0 contaminated artists, 0 FK violations, integrity OK).")
    return True


def main():
    args = parse_args()
    success = run_purge(
        db_path=args.db_path,
        backup_dir=args.backup_dir,
        dry_run=args.dry_run,
    )
    sys.exit(0 if success else 1)


if __name__ == "__main__":
    main()

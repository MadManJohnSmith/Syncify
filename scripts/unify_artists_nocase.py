#!/usr/bin/env python3
"""
Syncify Canonical Artists Unification & Purge Script (TASK-105)
==============================================================
Unifies case-insensitive (NOCASE) colliding artist records, merges service identifiers
and favorite states, resolves HTML entities (&amp;), purges placeholder/garbage artists,
and removes pure orphan artists while strictly preserving user favorites and provider identities.

Usage:
    python3 scripts/unify_artists_nocase.py [--db-path PATH] [--backup-dir DIR] [--dry-run]
"""

import argparse
import os
import shutil
import sqlite3
import sys
import time


def parse_args():
    parser = argparse.ArgumentParser(
        description="Unify case-insensitive artist collisions and purge garbage/orphans in Syncify SQLite DB."
    )
    default_db = os.path.expanduser("~/.local/share/com.syncify.app/syncify.db")
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
    backup_path = os.path.join(backup_dir, f"syncify_backup_pre_repair_TASK-105_{timestamp}.db")
    os.makedirs(backup_dir, exist_ok=True)
    print(f"[TASK-105] Creating safety snapshot at {backup_path}...")

    # Attempt VACUUM INTO
    try:
        src_conn = sqlite3.connect(f"file:{os.path.abspath(db_path)}?mode=ro", uri=True)
        src_conn.execute(f"VACUUM INTO '{backup_path}'")
        src_conn.close()
        print(f"[TASK-105] Safety snapshot created via VACUUM INTO: {backup_path}")
        return backup_path
    except Exception as e:
        print(f"[TASK-105] VACUUM INTO fallback ({e}), attempting file copy...")

    # Fallback to copy2
    shutil.copy2(db_path, backup_path)
    print(f"[TASK-105] Safety snapshot created via copy: {backup_path}")
    return backup_path


def run_unification(db_path: str, backup_dir: str = "/tmp", dry_run: bool = False) -> bool:
    if not os.path.exists(db_path):
        print(f"Error: Database not found at {db_path}", file=sys.stderr)
        return False

    print(f"[TASK-105] Target database: {db_path}")

    if not dry_run:
        create_safety_backup(db_path, backup_dir)

    uri_str = f"file:{os.path.abspath(db_path)}"
    if dry_run:
        uri_str += "?mode=ro&immutable=1"
        conn = sqlite3.connect(uri_str, uri=True)
    else:
        conn = sqlite3.connect(db_path)

    cur = conn.cursor()
    if not dry_run:
        cur.execute("PRAGMA foreign_keys = ON;")

    # 1. Pre-execution Diagnostics
    total_artists_pre = cur.execute("SELECT COUNT(*) FROM artists").fetchone()[0]

    case_collisions_pre = cur.execute(
        """
        SELECT COUNT(*) FROM (
            SELECT LOWER(TRIM(REPLACE(name, '&amp;', '&')))
            FROM artists
            GROUP BY LOWER(TRIM(REPLACE(name, '&amp;', '&')))
            HAVING COUNT(*) > 1
        )
        """
    ).fetchone()[0]

    garbage_artists_pre = cur.execute(
        """
        SELECT COUNT(*) FROM artists
        WHERE TRIM(name) = ''
           OR LOWER(TRIM(name)) IN ('unknown', 'unknown artist', '\\p', '\\\\p')
           OR (LOWER(TRIM(name)) = 'various' AND NOT EXISTS (SELECT 1 FROM album_artists aa WHERE aa.artist_id = artists.id))
        """
    ).fetchone()[0]

    pure_orphans_pre = cur.execute(
        """
        SELECT COUNT(*) FROM artists a
        WHERE NOT EXISTS (SELECT 1 FROM track_artists ta WHERE ta.artist_id = a.id)
          AND NOT EXISTS (SELECT 1 FROM album_artists aa WHERE aa.artist_id = a.id)
          AND COALESCE(a.is_favorite, 0) = 0
          AND (a.spotify_id IS NULL OR a.spotify_id = '')
          AND (a.tidal_id IS NULL OR a.tidal_id = '')
          AND (a.qobuz_id IS NULL OR a.qobuz_id = '')
          AND (a.musicbrainz_id IS NULL OR a.musicbrainz_id = '')
        """
    ).fetchone()[0]

    preserved_orphans_pre = cur.execute(
        """
        SELECT COUNT(*) FROM artists a
        WHERE NOT EXISTS (SELECT 1 FROM track_artists ta WHERE ta.artist_id = a.id)
          AND NOT EXISTS (SELECT 1 FROM album_artists aa WHERE aa.artist_id = a.id)
          AND (COALESCE(a.is_favorite, 0) = 1
               OR (a.spotify_id IS NOT NULL AND a.spotify_id != '')
               OR (a.tidal_id IS NOT NULL AND a.tidal_id != '')
               OR (a.qobuz_id IS NOT NULL AND a.qobuz_id != '')
               OR (a.musicbrainz_id IS NOT NULL AND a.musicbrainz_id != ''))
        """
    ).fetchone()[0]

    print("[TASK-105] Pre-unification diagnostics:")
    print(f"  - Total artists: {total_artists_pre}")
    print(f"  - Colliding artist groups (NOCASE / &amp;): {case_collisions_pre}")
    print(f"  - Garbage/placeholder artists: {garbage_artists_pre}")
    print(f"  - Pure unlinked orphan artists to purge: {pure_orphans_pre}")
    print(f"  - Preserved favorite/provider orphan artists: {preserved_orphans_pre}")

    if dry_run:
        print("[TASK-105] Dry-run enabled. No modifications applied.")
        conn.close()
        return True

    print("[TASK-105] Executing canonical unification and purge...")
    t0 = time.time()

    cur.executescript(
        """
        BEGIN IMMEDIATE;

        -- Step 1: Deduplicate colliding artists based on LOWER(TRIM(REPLACE(name, '&amp;', '&')))
        DROP TABLE IF EXISTS _artist_dedup_map_0079;
        CREATE TEMP TABLE _artist_dedup_map_0079 AS
        WITH ranked_artists AS (
            SELECT
                a.id,
                a.name,
                a.musicbrainz_id,
                a.spotify_id,
                a.tidal_id,
                a.qobuz_id,
                a.image_url,
                a.favorite_at,
                a.is_favorite,
                ROW_NUMBER() OVER (
                    PARTITION BY LOWER(TRIM(REPLACE(a.name, '&amp;', '&')))
                    ORDER BY
                        (SELECT COUNT(*) FROM track_artists ta WHERE ta.artist_id = a.id) DESC,
                        (SELECT COUNT(*) FROM album_artists aa WHERE aa.artist_id = a.id) DESC,
                        ((CASE WHEN a.spotify_id IS NOT NULL AND a.spotify_id != '' THEN 1 ELSE 0 END) +
                         (CASE WHEN a.tidal_id IS NOT NULL AND a.tidal_id != '' THEN 1 ELSE 0 END) +
                         (CASE WHEN a.qobuz_id IS NOT NULL AND a.qobuz_id != '' THEN 1 ELSE 0 END) +
                         (CASE WHEN a.musicbrainz_id IS NOT NULL AND a.musicbrainz_id != '' THEN 1 ELSE 0 END)) DESC,
                        COALESCE(a.is_favorite, 0) DESC,
                        (CASE WHEN a.name != LOWER(a.name) THEN 1 ELSE 0 END) DESC,
                        a.id ASC
                ) AS rn
            FROM artists a
        )
        SELECT
            loser.id AS loser_id,
            winner.id AS winner_id,
            loser.musicbrainz_id AS loser_musicbrainz_id,
            loser.spotify_id AS loser_spotify_id,
            loser.tidal_id AS loser_tidal_id,
            loser.qobuz_id AS loser_qobuz_id,
            loser.image_url AS loser_image_url,
            loser.favorite_at AS loser_favorite_at,
            loser.is_favorite AS loser_is_favorite
        FROM ranked_artists loser
        JOIN ranked_artists winner
            ON LOWER(TRIM(REPLACE(loser.name, '&amp;', '&'))) = LOWER(TRIM(REPLACE(winner.name, '&amp;', '&')))
           AND winner.rn = 1
        WHERE loser.rn > 1;

        UPDATE artists
        SET musicbrainz_id = NULL, tidal_id = NULL, qobuz_id = NULL, spotify_id = NULL
        WHERE id IN (SELECT loser_id FROM _artist_dedup_map_0079);

        UPDATE artists
        SET
            musicbrainz_id = COALESCE(artists.musicbrainz_id, (
                SELECT m.loser_musicbrainz_id FROM _artist_dedup_map_0079 m
                WHERE m.winner_id = artists.id AND m.loser_musicbrainz_id IS NOT NULL AND m.loser_musicbrainz_id != ''
                LIMIT 1
            )),
            spotify_id = COALESCE(artists.spotify_id, (
                SELECT m.loser_spotify_id FROM _artist_dedup_map_0079 m
                WHERE m.winner_id = artists.id AND m.loser_spotify_id IS NOT NULL AND m.loser_spotify_id != ''
                LIMIT 1
            )),
            tidal_id = COALESCE(artists.tidal_id, (
                SELECT m.loser_tidal_id FROM _artist_dedup_map_0079 m
                WHERE m.winner_id = artists.id AND m.loser_tidal_id IS NOT NULL AND m.loser_tidal_id != ''
                LIMIT 1
            )),
            qobuz_id = COALESCE(artists.qobuz_id, (
                SELECT m.loser_qobuz_id FROM _artist_dedup_map_0079 m
                WHERE m.winner_id = artists.id AND m.loser_qobuz_id IS NOT NULL AND m.loser_qobuz_id != ''
                LIMIT 1
            )),
            image_url = COALESCE(artists.image_url, (
                SELECT m.loser_image_url FROM _artist_dedup_map_0079 m
                WHERE m.winner_id = artists.id AND m.loser_image_url IS NOT NULL AND m.loser_image_url != ''
                LIMIT 1
            )),
            favorite_at = COALESCE(artists.favorite_at, (
                SELECT m.loser_favorite_at FROM _artist_dedup_map_0079 m
                WHERE m.winner_id = artists.id AND m.loser_favorite_at IS NOT NULL
                LIMIT 1
            )),
            is_favorite = MAX(
                COALESCE(artists.is_favorite, 0),
                COALESCE((
                    SELECT MAX(m.loser_is_favorite) FROM _artist_dedup_map_0079 m
                    WHERE m.winner_id = artists.id
                ), 0)
            ),
            name = TRIM(REPLACE(artists.name, '&amp;', '&'))
        WHERE id IN (SELECT winner_id FROM _artist_dedup_map_0079);

        DELETE FROM track_artists
        WHERE artist_id IN (SELECT loser_id FROM _artist_dedup_map_0079)
          AND EXISTS (
              SELECT 1 FROM track_artists ta2
              JOIN _artist_dedup_map_0079 m ON m.loser_id = track_artists.artist_id
              WHERE ta2.track_id = track_artists.track_id
                AND ta2.artist_id = m.winner_id
                AND COALESCE(ta2.role, 'primary') = COALESCE(track_artists.role, 'primary')
          );

        UPDATE track_artists
        SET artist_id = (SELECT winner_id FROM _artist_dedup_map_0079 WHERE loser_id = track_artists.artist_id)
        WHERE artist_id IN (SELECT loser_id FROM _artist_dedup_map_0079);

        DELETE FROM album_artists
        WHERE artist_id IN (SELECT loser_id FROM _artist_dedup_map_0079)
          AND EXISTS (
              SELECT 1 FROM album_artists aa2
              JOIN _artist_dedup_map_0079 m ON m.loser_id = album_artists.artist_id
              WHERE aa2.album_id = album_artists.album_id
                AND aa2.artist_id = m.winner_id
          );

        UPDATE album_artists
        SET artist_id = (SELECT winner_id FROM _artist_dedup_map_0079 WHERE loser_id = album_artists.artist_id)
        WHERE artist_id IN (SELECT loser_id FROM _artist_dedup_map_0079);

        DELETE FROM track_credits
        WHERE artist_id IN (SELECT loser_id FROM _artist_dedup_map_0079)
          AND EXISTS (
              SELECT 1 FROM track_credits tc2
              JOIN _artist_dedup_map_0079 m ON m.loser_id = track_credits.artist_id
              WHERE tc2.track_id = track_credits.track_id
                AND tc2.artist_id = m.winner_id
                AND tc2.role = track_credits.role
          );

        UPDATE track_credits
        SET artist_id = (SELECT winner_id FROM _artist_dedup_map_0079 WHERE loser_id = track_credits.artist_id)
        WHERE artist_id IN (SELECT loser_id FROM _artist_dedup_map_0079);

        DELETE FROM artists WHERE id IN (SELECT loser_id FROM _artist_dedup_map_0079);
        DROP TABLE IF EXISTS _artist_dedup_map_0079;

        UPDATE artists
        SET name = TRIM(REPLACE(name, '&amp;', '&'))
        WHERE name LIKE '%&amp;%';

        -- Step 2: Purge garbage artists
        DELETE FROM track_artists
        WHERE artist_id IN (
            SELECT id FROM artists
            WHERE TRIM(name) = ''
               OR LOWER(TRIM(name)) IN ('unknown', 'unknown artist', '\\p', '\\\\p')
               OR (LOWER(TRIM(name)) = 'various' AND NOT EXISTS (SELECT 1 FROM album_artists aa WHERE aa.artist_id = artists.id))
        );

        DELETE FROM album_artists
        WHERE artist_id IN (
            SELECT id FROM artists
            WHERE TRIM(name) = ''
               OR LOWER(TRIM(name)) IN ('unknown', 'unknown artist', '\\p', '\\\\p')
               OR (LOWER(TRIM(name)) = 'various' AND NOT EXISTS (SELECT 1 FROM album_artists aa WHERE aa.artist_id = artists.id))
        );

        DELETE FROM track_credits
        WHERE artist_id IN (
            SELECT id FROM artists
            WHERE TRIM(name) = ''
               OR LOWER(TRIM(name)) IN ('unknown', 'unknown artist', '\\p', '\\\\p')
               OR LOWER(TRIM(name)) = 'various'
        );

        DELETE FROM artists
        WHERE TRIM(name) = ''
           OR LOWER(TRIM(name)) IN ('unknown', 'unknown artist', '\\p', '\\\\p')
           OR (LOWER(TRIM(name)) = 'various' AND NOT EXISTS (SELECT 1 FROM album_artists aa WHERE aa.artist_id = artists.id));

        -- Step 3: Purge pure orphans
        DROP TABLE IF EXISTS _orphan_artists_to_purge;
        CREATE TEMP TABLE _orphan_artists_to_purge AS
        SELECT a.id FROM artists a
        WHERE NOT EXISTS (SELECT 1 FROM track_artists ta WHERE ta.artist_id = a.id)
          AND NOT EXISTS (SELECT 1 FROM album_artists aa WHERE aa.artist_id = a.id)
          AND COALESCE(a.is_favorite, 0) = 0
          AND (a.spotify_id IS NULL OR a.spotify_id = '')
          AND (a.tidal_id IS NULL OR a.tidal_id = '')
          AND (a.qobuz_id IS NULL OR a.qobuz_id = '')
          AND (a.musicbrainz_id IS NULL OR a.musicbrainz_id = '');

        DELETE FROM track_credits WHERE artist_id IN (SELECT id FROM _orphan_artists_to_purge);
        DELETE FROM artists WHERE id IN (SELECT id FROM _orphan_artists_to_purge);
        DROP TABLE IF EXISTS _orphan_artists_to_purge;

        -- Step 4: Enforce trim, unique index, and triggers
        UPDATE artists SET name = TRIM(name) WHERE name != TRIM(name);

        CREATE UNIQUE INDEX IF NOT EXISTS idx_artists_canonical_name_unique ON artists(LOWER(TRIM(name)));

        CREATE TRIGGER IF NOT EXISTS trg_artists_reject_garbage_ins
        BEFORE INSERT ON artists
        FOR EACH ROW
        WHEN TRIM(NEW.name) = ''
          OR LOWER(TRIM(NEW.name)) IN ('unknown', 'unknown artist', '\\p', '\\\\p')
        BEGIN
            SELECT RAISE(ABORT, 'Rejected garbage or empty artist name');
        END;

        CREATE TRIGGER IF NOT EXISTS trg_artists_reject_garbage_upd
        BEFORE UPDATE OF name ON artists
        FOR EACH ROW
        WHEN TRIM(NEW.name) = ''
          OR LOWER(TRIM(NEW.name)) IN ('unknown', 'unknown artist', '\\p', '\\\\p')
        BEGIN
            SELECT RAISE(ABORT, 'Rejected garbage or empty artist name');
        END;

        COMMIT;
        """
    )

    elapsed = time.time() - t0
    print(f"[TASK-105] Operations executed in {elapsed:.2f}s")

    # Verification Checks
    fk_errors = cur.execute("PRAGMA foreign_key_check").fetchall()
    if fk_errors:
        print(f"[TASK-105] ERROR: Foreign key check failed: {fk_errors}", file=sys.stderr)
        conn.close()
        return False

    integrity = cur.execute("PRAGMA integrity_check").fetchone()[0]
    if integrity != "ok":
        print(f"[TASK-105] ERROR: Integrity check failed: {integrity}", file=sys.stderr)
        conn.close()
        return False

    total_artists_post = cur.execute("SELECT COUNT(*) FROM artists").fetchone()[0]
    case_collisions_post = cur.execute(
        """
        SELECT COUNT(*) FROM (
            SELECT LOWER(TRIM(name))
            FROM artists
            GROUP BY LOWER(TRIM(name))
            HAVING COUNT(*) > 1
        )
        """
    ).fetchone()[0]

    garbage_post = cur.execute(
        """
        SELECT COUNT(*) FROM artists
        WHERE TRIM(name) = ''
           OR LOWER(TRIM(name)) IN ('unknown', 'unknown artist', '\\p', '\\\\p')
        """
    ).fetchone()[0]

    pure_orphans_post = cur.execute(
        """
        SELECT COUNT(*) FROM artists a
        WHERE NOT EXISTS (SELECT 1 FROM track_artists ta WHERE ta.artist_id = a.id)
          AND NOT EXISTS (SELECT 1 FROM album_artists aa WHERE aa.artist_id = a.id)
          AND COALESCE(a.is_favorite, 0) = 0
          AND (a.spotify_id IS NULL OR a.spotify_id = '')
          AND (a.tidal_id IS NULL OR a.tidal_id = '')
          AND (a.qobuz_id IS NULL OR a.qobuz_id = '')
          AND (a.musicbrainz_id IS NULL OR a.musicbrainz_id = '')
        """
    ).fetchone()[0]

    print("[TASK-105] Post-unification verification:")
    print(f"  - Total artists: {total_artists_post} (purged {total_artists_pre - total_artists_post})")
    print(f"  - Colliding artist groups (NOCASE): {case_collisions_post} (target: 0)")
    print(f"  - Garbage/placeholder artists: {garbage_post} (target: 0)")
    print(f"  - Pure unlinked orphan artists: {pure_orphans_post} (target: 0)")
    print(f"  - PRAGMA foreign_key_check: {len(fk_errors)} errors")
    print(f"  - PRAGMA integrity_check: {integrity}")

    conn.close()

    assert case_collisions_post == 0, "Case collisions must be exactly 0"
    assert garbage_post == 0, "Garbage artists must be exactly 0"
    assert pure_orphans_post == 0, "Pure orphan artists must be exactly 0"

    print("[TASK-105] Unification and purge completed successfully.")
    return True


def main():
    args = parse_args()
    success = run_unification(
        db_path=args.db_path,
        backup_dir=args.backup_dir,
        dry_run=args.dry_run,
    )
    sys.exit(0 if success else 1)


if __name__ == "__main__":
    main()

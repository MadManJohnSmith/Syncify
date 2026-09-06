#!/usr/bin/env python3
"""
Syncify Catalog Ghost Albums Hydration & Placeholder Purge Script (TASK-103)
============================================================================
1. Marks albums with 0 tracks in `tracks` as `is_stub = 1`.
2. Marks tracks with duration < 30s as `is_preview = 1`.
3. Purges ghost / junk placeholder tracks (duration_ms = 0 or title in 'Unavailable', 'Unknown%', 'Track%').
4. Reclassifies falsely 'enriched' ghost tracks to 'pending'.
5. Ensures recurrence prevention triggers are installed.
6. Performs integrity and foreign key checks with mandatory pre-repair snapshot in /tmp/.

Usage:
    python3 scripts/purge_catalog_placeholders.py [--db-path PATH] [--backup-dir DIR] [--dry-run]
"""

import argparse
import os
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


def column_exists(cur: sqlite3.Cursor, table: str, column: str) -> bool:
    cur.execute(f"PRAGMA table_info({table});")
    columns = [row[1] for row in cur.fetchall()]
    return column in columns


def purge_catalog_placeholders(
    db_path: str,
    backup_dir: str = "/tmp",
    dry_run: bool = False,
) -> bool:
    if not os.path.exists(db_path):
        print(f"[TASK-103] Error: Database not found at {db_path}", file=sys.stderr)
        return False

    print(f"[TASK-103] Target database: {db_path}")

    # 1. Safety snapshot if not dry-run
    if not dry_run:
        timestamp = int(time.time())
        backup_path = os.path.join(backup_dir, f"syncify_backup_pre_repair_TASK-103_{timestamp}.db")
        print(f"[TASK-103] Creating safety snapshot at {backup_path}...")
        try:
            src_conn = sqlite3.connect(f"file:{os.path.abspath(db_path)}?mode=ro", uri=True)
            src_conn.execute(f"VACUUM INTO '{backup_path}'")
            src_conn.close()
            print(f"[TASK-103] Safety snapshot created successfully via VACUUM INTO: {backup_path}")
        except Exception as e:
            print(f"[TASK-103] VACUUM INTO failed ({e}), falling back to file copy...")
            import shutil
            shutil.copy2(db_path, backup_path)
            print(f"[TASK-103] Safety snapshot created successfully via copy: {backup_path}")

    # 2. Connect
    if dry_run:
        conn = sqlite3.connect(f"file:{os.path.abspath(db_path)}?mode=ro&immutable=1", uri=True)
    else:
        conn = sqlite3.connect(db_path)
    cur = conn.cursor()
    if not dry_run:
        cur.execute("PRAGMA foreign_keys = ON;")

    # 3. Check pre-state schema
    has_is_stub = column_exists(cur, "albums", "is_stub")
    has_is_preview = column_exists(cur, "tracks", "is_preview")

    total_albums = cur.execute("SELECT COUNT(*) FROM albums;").fetchone()[0]
    albums_without_tracks = cur.execute(
        "SELECT COUNT(*) FROM albums WHERE id NOT IN (SELECT DISTINCT album_id FROM tracks WHERE album_id IS NOT NULL);"
    ).fetchone()[0]

    total_tracks = cur.execute("SELECT COUNT(*) FROM tracks;").fetchone()[0]
    preview_tracks = cur.execute(
        "SELECT COUNT(*) FROM tracks WHERE duration_ms > 0 AND duration_ms < 30000;"
    ).fetchone()[0]

    ghost_tracks = cur.execute(
        """
        SELECT COUNT(*) FROM tracks
        WHERE (duration_ms = 0 AND (
            title IS NULL 
            OR TRIM(title) = '' 
            OR LOWER(TRIM(title)) = 'unavailable'
            OR LOWER(TRIM(title)) = 'unknown'
            OR LOWER(TRIM(title)) LIKE 'unknown%'
            OR LOWER(TRIM(title)) LIKE 'track %'
            OR LOWER(TRIM(title)) = 'track'
        ))
        OR (id IN (9324, 12031, 12187) AND (duration_ms = 0 OR LOWER(TRIM(title)) = 'unavailable'));
        """
    ).fetchone()[0]

    print("[TASK-103] Pre-repair diagnostics:")
    print(f"  - Total albums: {total_albums}")
    print(f"  - Albums without tracks (stubs): {albums_without_tracks}")
    print(f"  - Albums column 'is_stub' present: {has_is_stub}")
    print(f"  - Total tracks: {total_tracks}")
    print(f"  - Tracks with preview duration (<30s): {preview_tracks}")
    print(f"  - Tracks column 'is_preview' present: {has_is_preview}")
    print(f"  - Ghost/placeholder tracks eligible for purge: {ghost_tracks}")

    if dry_run:
        print("[TASK-103] Dry-run enabled. No modifications applied.")
        conn.close()
        return True

    print("[TASK-103] Applying catalog placeholder purge, stub marking and triggers...")
    t0 = time.time()
    conn.execute("BEGIN IMMEDIATE;")

    # 4. Schema alterations if needed
    if not has_is_stub:
        conn.execute("ALTER TABLE albums ADD COLUMN is_stub INTEGER NOT NULL DEFAULT 0;")
        conn.execute("CREATE INDEX IF NOT EXISTS idx_albums_is_stub ON albums(is_stub);")
        print("  + Added column 'is_stub' to albums")

    if not has_is_preview:
        conn.execute("ALTER TABLE tracks ADD COLUMN is_preview INTEGER NOT NULL DEFAULT 0;")
        conn.execute("CREATE INDEX IF NOT EXISTS idx_tracks_is_preview ON tracks(is_preview);")
        print("  + Added column 'is_preview' to tracks")

    # 5. Purge ghost and placeholder tracks
    purge_subquery = """
        SELECT id FROM tracks
        WHERE (duration_ms = 0 AND (
            title IS NULL 
            OR TRIM(title) = '' 
            OR LOWER(TRIM(title)) = 'unavailable'
            OR LOWER(TRIM(title)) = 'unknown'
            OR LOWER(TRIM(title)) LIKE 'unknown%'
            OR LOWER(TRIM(title)) LIKE 'track %'
            OR LOWER(TRIM(title)) = 'track'
        ))
        OR (id IN (9324, 12031, 12187) AND (duration_ms = 0 OR LOWER(TRIM(title)) = 'unavailable'))
    """

    conn.execute(f"DELETE FROM track_artists WHERE track_id IN ({purge_subquery});")
    conn.execute(f"DELETE FROM track_sources WHERE track_id IN ({purge_subquery});")
    conn.execute(f"DELETE FROM playlist_tracks WHERE track_id IN ({purge_subquery});")
    conn.execute(f"DELETE FROM library_entries WHERE track_id IN ({purge_subquery});")
    conn.execute(f"DELETE FROM download_queue WHERE track_id IN ({purge_subquery});")
    conn.execute(f"UPDATE downloads SET track_id = NULL WHERE track_id IN ({purge_subquery});")
    cur_del = conn.execute(f"DELETE FROM tracks WHERE id IN ({purge_subquery});")
    purged_count = cur_del.rowcount
    print(f"  + Purged {purged_count} ghost/placeholder tracks")

    # 6. Mark preview tracks
    cur_prev = conn.execute(
        "UPDATE tracks SET is_preview = 1 WHERE duration_ms > 0 AND duration_ms < 30000;"
    )
    print(f"  + Marked {cur_prev.rowcount} tracks as is_preview = 1")

    # 7. Mark stubs for albums with 0 tracks
    cur_stub = conn.execute(
        "UPDATE albums SET is_stub = 1 WHERE id NOT IN (SELECT DISTINCT album_id FROM tracks WHERE album_id IS NOT NULL);"
    )
    conn.execute(
        "UPDATE albums SET is_stub = 0 WHERE id IN (SELECT DISTINCT album_id FROM tracks WHERE album_id IS NOT NULL);"
    )
    print(f"  + Marked {cur_stub.rowcount} albums as is_stub = 1")

    # 8. Reclassify falsely enriched residual tracks
    cur_enrich = conn.execute(
        "UPDATE tracks SET enrichment_status = 'pending' WHERE enrichment_status = 'enriched' AND (duration_ms = 0 OR is_preview = 1);"
    )
    print(f"  + Reclassified {cur_enrich.rowcount} falsely enriched tracks to 'pending'")

    # 9. Recurrence prevention triggers
    triggers = [
        """
        CREATE TRIGGER IF NOT EXISTS trg_tracks_clear_album_stub_ins
        AFTER INSERT ON tracks
        FOR EACH ROW
        WHEN NEW.album_id IS NOT NULL
        BEGIN
            UPDATE albums SET is_stub = 0 WHERE id = NEW.album_id AND is_stub = 1;
        END;
        """,
        """
        CREATE TRIGGER IF NOT EXISTS trg_tracks_set_album_stub_del
        AFTER DELETE ON tracks
        FOR EACH ROW
        WHEN OLD.album_id IS NOT NULL
        BEGIN
            UPDATE albums SET is_stub = 1 
            WHERE id = OLD.album_id 
              AND NOT EXISTS (SELECT 1 FROM tracks WHERE album_id = OLD.album_id);
        END;
        """,
        """
        CREATE TRIGGER IF NOT EXISTS trg_tracks_album_stub_upd
        AFTER UPDATE OF album_id ON tracks
        FOR EACH ROW
        BEGIN
            UPDATE albums SET is_stub = 0 WHERE id = NEW.album_id AND is_stub = 1;
            UPDATE albums SET is_stub = 1 
            WHERE id = OLD.album_id 
              AND OLD.album_id IS NOT NULL 
              AND NOT EXISTS (SELECT 1 FROM tracks WHERE album_id = OLD.album_id);
        END;
        """,
        """
        CREATE TRIGGER IF NOT EXISTS trg_tracks_is_preview_ins
        AFTER INSERT ON tracks
        FOR EACH ROW
        WHEN NEW.duration_ms > 0 AND NEW.duration_ms < 30000 AND (NEW.is_preview IS NULL OR NEW.is_preview = 0)
        BEGIN
            UPDATE tracks SET is_preview = 1 WHERE id = NEW.id;
        END;
        """,
        """
        CREATE TRIGGER IF NOT EXISTS trg_tracks_is_preview_upd
        AFTER UPDATE OF duration_ms ON tracks
        FOR EACH ROW
        WHEN NEW.duration_ms > 0 AND NEW.duration_ms < 30000 AND (NEW.is_preview IS NULL OR NEW.is_preview = 0)
        BEGIN
            UPDATE tracks SET is_preview = 1 WHERE id = NEW.id;
        END;
        """,
        """
        CREATE TRIGGER IF NOT EXISTS trg_tracks_clear_preview_upd
        AFTER UPDATE OF duration_ms ON tracks
        FOR EACH ROW
        WHEN (NEW.duration_ms >= 30000 OR NEW.duration_ms <= 0 OR NEW.duration_ms IS NULL) AND NEW.is_preview = 1
        BEGIN
            UPDATE tracks SET is_preview = 0 WHERE id = NEW.id;
        END;
        """,
    ]

    for trg in triggers:
        conn.execute(trg)
    print("  + Installed durable recurrence prevention triggers")

    # 10. Integrity assertions
    fk_violations = list(conn.execute("PRAGMA foreign_key_check;").fetchall())
    if fk_violations:
        conn.rollback()
        conn.close()
        print(f"[TASK-103] Transaction rolled back! Foreign key violations detected: {fk_violations}", file=sys.stderr)
        return False

    integrity = conn.execute("PRAGMA integrity_check;").fetchall()
    if integrity != [("ok",)]:
        conn.rollback()
        conn.close()
        print(f"[TASK-103] Transaction rolled back! Integrity check failed: {integrity}", file=sys.stderr)
        return False

    conn.commit()
    elapsed = time.time() - t0
    print(f"[TASK-103] Remediation committed successfully in {elapsed:.3f}s")

    # 11. Post-state summary
    stub_count_post = cur.execute("SELECT COUNT(*) FROM albums WHERE is_stub = 1;").fetchone()[0]
    preview_count_post = cur.execute("SELECT COUNT(*) FROM tracks WHERE is_preview = 1;").fetchone()[0]
    remaining_ghosts = cur.execute(purge_subquery.replace("SELECT id FROM tracks", "SELECT COUNT(*) FROM tracks")).fetchone()[0]

    print("[TASK-103] Post-repair validation:")
    print(f"  - Albums marked as stub: {stub_count_post}")
    print(f"  - Tracks marked as preview: {preview_count_post}")
    print(f"  - Ghost tracks remaining: {remaining_ghosts}")
    print("  - PRAGMA foreign_key_check: 0 violations")
    print("  - PRAGMA integrity_check: ok")

    conn.close()
    return True


def main():
    parser = argparse.ArgumentParser(description="Syncify TASK-103 Album Stubs & Placeholder Purge")
    parser.add_argument("--db-path", default=None, help="Path to SQLite database file")
    parser.add_argument("--backup-dir", default="/tmp", help="Directory to store safety snapshot")
    parser.add_argument("--dry-run", action="store_true", help="Diagnose without making changes")
    args = parser.parse_args()

    db_path = args.db_path or find_default_db()
    success = purge_catalog_placeholders(db_path, args.backup_dir, args.dry_run)
    sys.exit(0 if success else 1)


if __name__ == "__main__":
    main()

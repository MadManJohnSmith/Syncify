#!/usr/bin/env python3
"""
Syncify Playlist Position Recompaction & Track Count Reconciliation Script (TASK-79)
==================================================================================
Recompacts playlist positions to strictly 1-indexed, sequential, and gap-free (1, 2, 3... N).
Atomically reconciles `playlists.track_count` to match `SELECT COUNT(*) FROM playlist_tracks`.

Usage:
    python3 scripts/recompact_playlist_positions.py [--db-path PATH] [--backup-dir DIR] [--dry-run]
"""

import argparse
import os
import sqlite3
import sys
import time


def recompact_database(db_path: str, backup_dir: str = "/tmp", dry_run: bool = False) -> bool:
    if not os.path.exists(db_path):
        print(f"Error: Database not found at {db_path}", file=sys.stderr)
        return False

    print(f"[TASK-79] Target database: {db_path}")

    # Backup if not dry-run
    if not dry_run:
        timestamp = int(time.time())
        backup_path = os.path.join(backup_dir, f"syncify_backup_pre_repair_TASK-79_{timestamp}.db")
        print(f"[TASK-79] Creating safety snapshot at {backup_path}...")
        try:
            # Try VACUUM INTO via sqlite3
            src_conn = sqlite3.connect(f"file:{os.path.abspath(db_path)}?mode=ro", uri=True)
            src_conn.execute(f"VACUUM INTO '{backup_path}'")
            src_conn.close()
            print(f"[TASK-79] Safety snapshot created successfully via VACUUM INTO: {backup_path}")
        except Exception as e:
            print(f"[TASK-79] VACUUM INTO failed ({e}), falling back to file copy...")
            import shutil
            shutil.copy2(db_path, backup_path)
            print(f"[TASK-79] Safety snapshot created successfully via copy: {backup_path}")

    if dry_run:
        conn = sqlite3.connect(f"file:{os.path.abspath(db_path)}?mode=ro&immutable=1", uri=True)
    else:
        conn = sqlite3.connect(db_path)
    cur = conn.cursor()
    if not dry_run:
        cur.execute("PRAGMA foreign_keys = ON;")

    # Check pre-state
    mismatched_pre = cur.execute(
        "SELECT COUNT(*) FROM playlists p WHERE p.track_count != (SELECT COUNT(*) FROM playlist_tracks pt WHERE pt.playlist_id = p.id)"
    ).fetchone()[0]

    discontinuous_pre = cur.execute(
        "SELECT COUNT(*) FROM (SELECT playlist_id FROM playlist_tracks GROUP BY playlist_id HAVING MAX(position) - MIN(position) + 1 != COUNT(*))"
    ).fetchone()[0]

    print(f"[TASK-79] Pre-recompact diagnostics:")
    print(f"  - Playlists with mismatched track_count: {mismatched_pre}")
    print(f"  - Playlists with discontinuous positions: {discontinuous_pre}")

    if dry_run:
        print("[TASK-79] Dry-run enabled. No modifications applied.")
        conn.close()
        return True

    print("[TASK-79] Applying transactional position recompaction and track_count reconciliation...")
    t0 = time.time()
    cur.execute("BEGIN IMMEDIATE TRANSACTION;")
    try:
        # 1. Create temporary staging table with PRIMARY KEY on id for O(1) indexed lookups
        cur.execute("DROP TABLE IF EXISTS _playlist_tracks_recompact;")
        cur.execute("""
            CREATE TEMP TABLE _playlist_tracks_recompact (
                id INTEGER PRIMARY KEY,
                new_pos INTEGER NOT NULL
            );
        """)

        cur.execute("""
            INSERT INTO _playlist_tracks_recompact (id, new_pos)
            SELECT
                id,
                ROW_NUMBER() OVER (
                    PARTITION BY playlist_id
                    ORDER BY position ASC, added_at ASC, id ASC
                )
            FROM playlist_tracks;
        """)

        # 2. Stage existing positions to unique negative values to avoid UNIQUE(playlist_id, position) collisions
        cur.execute("UPDATE playlist_tracks SET position = -(id + 1);")

        # 3. Reassign positions from indexed staging table
        cur.execute("""
            UPDATE playlist_tracks
            SET position = (
                SELECT r.new_pos
                FROM _playlist_tracks_recompact r
                WHERE r.id = playlist_tracks.id
            );
        """)

        # 4. Clean up staging table
        cur.execute("DROP TABLE IF EXISTS _playlist_tracks_recompact;")

        # 5. Atomically update track_count on playlists table
        cur.execute("""
            UPDATE playlists
            SET track_count = (
                SELECT COUNT(*)
                FROM playlist_tracks
                WHERE playlist_tracks.playlist_id = playlists.id
            );
        """)

        conn.commit()
        elapsed = time.time() - t0
        print(f"[TASK-79] Transaction committed successfully in {elapsed:.2f}s.")
    except Exception as e:
        conn.rollback()
        conn.close()
        print(f"[TASK-79] ERROR during recompaction transaction: {e}", file=sys.stderr)
        return False

    # Post-checks
    mismatched_post = cur.execute(
        "SELECT COUNT(*) FROM playlists p WHERE p.track_count != (SELECT COUNT(*) FROM playlist_tracks pt WHERE pt.playlist_id = p.id)"
    ).fetchone()[0]

    discontinuous_post = cur.execute(
        "SELECT COUNT(*) FROM (SELECT playlist_id FROM playlist_tracks GROUP BY playlist_id HAVING MAX(position) - MIN(position) + 1 != COUNT(*))"
    ).fetchone()[0]

    zero_pos = cur.execute("SELECT COUNT(*) FROM playlist_tracks WHERE position <= 0").fetchone()[0]

    integrity = cur.execute("PRAGMA integrity_check").fetchone()[0]
    fk_errors = cur.execute("PRAGMA foreign_key_check").fetchall()

    conn.close()

    print(f"[TASK-79] Post-recompact assertions:")
    print(f"  - Playlists with mismatched track_count: {mismatched_post} (Expected: 0)")
    print(f"  - Playlists with discontinuous positions: {discontinuous_post} (Expected: 0)")
    print(f"  - Positions <= 0: {zero_pos} (Expected: 0)")
    print(f"  - PRAGMA integrity_check: {integrity} (Expected: ok)")
    print(f"  - PRAGMA foreign_key_check errors: {len(fk_errors)} (Expected: 0)")

    success = (
        mismatched_post == 0
        and discontinuous_post == 0
        and zero_pos == 0
        and integrity == "ok"
        and len(fk_errors) == 0
    )

    if success:
        print("[TASK-79] PASSED: Database fully sanitized and consistent.")
    else:
        print("[TASK-79] FAILED: Post-recompaction assertions not met.", file=sys.stderr)

    return success


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description="Recompact playlist track positions and reconcile track count.")
    default_db = os.path.expanduser("~/.local/share/com.syncify.app/syncify.db")
    parser.add_argument("--db-path", default=default_db, help=f"Path to SQLite DB (default: {default_db})")
    parser.add_argument("--backup-dir", default="/tmp", help="Directory for backup snapshot (default: /tmp)")
    parser.add_argument("--dry-run", action="store_true", help="Check diagnostics without modifying DB")

    args = parser.parse_args()
    success = recompact_database(args.db_path, args.backup_dir, args.dry_run)
    sys.exit(0 if success else 1)

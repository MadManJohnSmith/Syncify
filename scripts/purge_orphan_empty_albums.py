#!/usr/bin/env python3
"""
Syncify Maintenance Script: Purge Orphan Empty Albums (TASK-70)
==============================================================
Purges orphan empty albums (0 tracks in `tracks`) while strictly preserving
legitimate stub albums (`is_stub = 1`), and cleans up corresponding orphan
references in `album_artists`.

Features:
- Mandatory pre-repair snapshot via `VACUUM INTO` in `--backup-dir` (/tmp default).
- Support for `--dry-run`, `--db-path`, `--backup-dir`.
- Validates `PRAGMA foreign_key_check` (0 violations) and `PRAGMA integrity_check` (ok).

Usage:
    python3 scripts/purge_orphan_empty_albums.py [--db-path PATH] [--backup-dir DIR] [--dry-run]
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


def column_exists(cur: sqlite3.Cursor, table: str, column: str) -> bool:
    cur.execute(f"PRAGMA table_info({table});")
    columns = [row[1] for row in cur.fetchall()]
    return column in columns


def purge_orphan_empty_albums(
    db_path: str,
    backup_dir: str = "/tmp",
    dry_run: bool = False,
) -> bool:
    if not os.path.exists(db_path):
        print(f"[TASK-70] Error: Database file not found at {db_path}", file=sys.stderr)
        return False

    print(f"[TASK-70] Target database: {db_path}")

    # 1. Safety snapshot (if not dry-run)
    if not dry_run:
        os.makedirs(backup_dir, exist_ok=True)
        timestamp = int(time.time())
        backup_path = os.path.join(backup_dir, f"syncify_backup_pre_repair_TASK-70_{timestamp}.db")
        print(f"[TASK-70] Creating safety snapshot at {backup_path}...")
        try:
            src_conn = sqlite3.connect(f"file:{os.path.abspath(db_path)}?mode=ro", uri=True)
            src_conn.execute(f"VACUUM INTO '{backup_path}'")
            src_conn.close()
            print(f"[TASK-70] Safety snapshot created successfully via VACUUM INTO: {backup_path}")
        except Exception as e:
            print(f"[TASK-70] VACUUM INTO failed ({e}), falling back to file copy...")
            shutil.copy2(db_path, backup_path)
            print(f"[TASK-70] Safety snapshot created successfully via copy: {backup_path}")

    # 2. Open connection
    if dry_run:
        conn = sqlite3.connect(f"file:{os.path.abspath(db_path)}?mode=ro&immutable=1", uri=True)
    else:
        conn = sqlite3.connect(db_path)
    cur = conn.cursor()
    cur.execute("PRAGMA foreign_keys = ON;")

    has_is_stub = column_exists(cur, "albums", "is_stub")

    total_albums = cur.execute("SELECT COUNT(*) FROM albums;").fetchone()[0]
    total_tracks = cur.execute("SELECT COUNT(*) FROM tracks;").fetchone()[0]

    empty_albums_count = cur.execute(
        "SELECT COUNT(*) FROM albums WHERE id NOT IN (SELECT DISTINCT album_id FROM tracks WHERE album_id IS NOT NULL);"
    ).fetchone()[0]

    if has_is_stub:
        preserved_stubs_count = cur.execute(
            "SELECT COUNT(*) FROM albums WHERE id NOT IN (SELECT DISTINCT album_id FROM tracks WHERE album_id IS NOT NULL) AND is_stub = 1;"
        ).fetchone()[0]
        orphan_albums_to_purge = cur.execute(
            "SELECT COUNT(*) FROM albums WHERE id NOT IN (SELECT DISTINCT album_id FROM tracks WHERE album_id IS NOT NULL) AND (is_stub != 1 OR is_stub IS NULL);"
        ).fetchone()[0]
        orphan_album_artists_count = cur.execute(
            """
            SELECT COUNT(*) FROM album_artists
            WHERE album_id IN (
                SELECT id FROM albums
                WHERE id NOT IN (SELECT DISTINCT album_id FROM tracks WHERE album_id IS NOT NULL)
                  AND (is_stub != 1 OR is_stub IS NULL)
            )
            OR album_id NOT IN (SELECT id FROM albums);
            """
        ).fetchone()[0]
    else:
        preserved_stubs_count = 0
        orphan_albums_to_purge = empty_albums_count
        orphan_album_artists_count = cur.execute(
            """
            SELECT COUNT(*) FROM album_artists
            WHERE album_id IN (
                SELECT id FROM albums
                WHERE id NOT IN (SELECT DISTINCT album_id FROM tracks WHERE album_id IS NOT NULL)
            )
            OR album_id NOT IN (SELECT id FROM albums);
            """
        ).fetchone()[0]

    print("\n[TASK-70] ══════════════ DATABASE AUDIT BEFORE PURGE ══════════════")
    print(f"[TASK-70] Total albums in database:          {total_albums}")
    print(f"[TASK-70] Total tracks in database:          {total_tracks}")
    print(f"[TASK-70] Albums without tracks (0 tracks):   {empty_albums_count}")
    print(f"[TASK-70] Preserved stubs (is_stub = 1):      {preserved_stubs_count}")
    print(f"[TASK-70] Orphan empty albums to purge:       {orphan_albums_to_purge}")
    print(f"[TASK-70] Orphan album_artists to purge:     {orphan_album_artists_count}")
    print("[TASK-70] ═══════════════════════════════════════════════════════════\n")

    if dry_run:
        print("[TASK-70] DRY-RUN MODE: No changes committed to database.")
    else:
        print("[TASK-70] Applying transactional purge...")
        try:
            cur.execute("BEGIN IMMEDIATE;")

            # 1. Delete orphan album_artists
            if has_is_stub:
                cur.execute(
                    """
                    DELETE FROM album_artists
                    WHERE album_id IN (
                        SELECT id FROM albums
                        WHERE id NOT IN (SELECT DISTINCT album_id FROM tracks WHERE album_id IS NOT NULL)
                          AND (is_stub != 1 OR is_stub IS NULL)
                    )
                    OR album_id NOT IN (SELECT id FROM albums);
                    """
                )
            else:
                cur.execute(
                    """
                    DELETE FROM album_artists
                    WHERE album_id IN (
                        SELECT id FROM albums
                        WHERE id NOT IN (SELECT DISTINCT album_id FROM tracks WHERE album_id IS NOT NULL)
                    )
                    OR album_id NOT IN (SELECT id FROM albums);
                    """
                )
            deleted_album_artists = cur.rowcount

            # 2. Delete orphan empty albums
            if has_is_stub:
                cur.execute(
                    """
                    DELETE FROM albums
                    WHERE id NOT IN (SELECT DISTINCT album_id FROM tracks WHERE album_id IS NOT NULL)
                      AND (is_stub != 1 OR is_stub IS NULL);
                    """
                )
            else:
                cur.execute(
                    """
                    DELETE FROM albums
                    WHERE id NOT IN (SELECT DISTINCT album_id FROM tracks WHERE album_id IS NOT NULL);
                    """
                )
            deleted_albums = cur.rowcount

            conn.commit()
            print(f"[TASK-70] Successfully purged {deleted_albums} orphan albums and {deleted_album_artists} album_artists references.")
        except Exception as e:
            conn.rollback()
            print(f"[TASK-70] Error during purge transaction, rolled back: {e}", file=sys.stderr)
            conn.close()
            return False

    # 3. Validation gates
    print("[TASK-70] Running validation checks...")
    fk_violations = cur.execute("PRAGMA foreign_key_check;").fetchall()
    if fk_violations:
        print(f"[TASK-70] CRITICAL: foreign_key_check returned {len(fk_violations)} violations: {fk_violations}", file=sys.stderr)
        conn.close()
        return False
    print("[TASK-70] PRAGMA foreign_key_check: OK (0 violations)")

    integrity_result = cur.execute("PRAGMA integrity_check;").fetchone()[0]
    if integrity_result != "ok":
        print(f"[TASK-70] CRITICAL: integrity_check failed: {integrity_result}", file=sys.stderr)
        conn.close()
        return False
    print(f"[TASK-70] PRAGMA integrity_check: {integrity_result}")

    # Post-purge audit
    remaining_total_albums = cur.execute("SELECT COUNT(*) FROM albums;").fetchone()[0]
    remaining_empty_albums = cur.execute(
        "SELECT COUNT(*) FROM albums WHERE id NOT IN (SELECT DISTINCT album_id FROM tracks WHERE album_id IS NOT NULL);"
    ).fetchone()[0]
    if has_is_stub:
        remaining_stubs = cur.execute("SELECT COUNT(*) FROM albums WHERE is_stub = 1;").fetchone()[0]
    else:
        remaining_stubs = 0

    print("\n[TASK-70] ══════════════ POST-PURGE VERIFICATION ═══════════════")
    print(f"[TASK-70] Remaining total albums:            {remaining_total_albums}")
    print(f"[TASK-70] Remaining empty albums:            {remaining_empty_albums} (all must be stubs)")
    print(f"[TASK-70] Remaining stubs (is_stub = 1):     {remaining_stubs}")
    print("[TASK-70] ═══════════════════════════════════════════════════════════\n")

    if not dry_run and remaining_empty_albums != remaining_stubs:
        print(f"[TASK-70] WARNING: Empty albums ({remaining_empty_albums}) != Stubs ({remaining_stubs})", file=sys.stderr)
        conn.close()
        return False

    conn.close()
    print("[TASK-70] Purge operation completed successfully.")
    return True


def main():
    parser = argparse.ArgumentParser(
        description="Syncify TASK-70: Purge Orphan Empty Albums & Clean Album Artists"
    )
    parser.add_argument(
        "--db-path",
        default=find_default_db(),
        help="Path to syncify.db SQLite database",
    )
    parser.add_argument(
        "--backup-dir",
        default="/tmp",
        help="Directory to place pre-repair VACUUM INTO backup (default: /tmp)",
    )
    parser.add_argument(
        "--dry-run",
        action="store_true",
        help="Simulate purge without writing changes",
    )

    args = parser.parse_args()
    success = purge_orphan_empty_albums(
        db_path=args.db_path,
        backup_dir=args.backup_dir,
        dry_run=args.dry_run,
    )
    sys.exit(0 if success else 1)


if __name__ == "__main__":
    main()

#!/usr/bin/env python3
"""
Syncify Album Total Tracks Recalculation & Reconciliation Script (TASK-138)
===========================================================================
Portable maintenance script to reconcile and synchronize `albums.total_tracks`
with the actual count of tracks in SQLite (`COUNT(tracks.id)`), preserving
documented stub albums (`is_stub == 1`).

Features:
1. Creates a pre-repair safety snapshot using `VACUUM INTO` in /tmp/
   (syncify_backup_pre_repair_TASK-138_<timestamp>.db) with robust fallbacks.
2. Identifies all non-stub albums with divergent `total_tracks` (excess, deficit,
   NULL, or zero tracks without stub classification).
3. Atomically recalculates `albums.total_tracks = (SELECT COUNT(*) FROM tracks WHERE tracks.album_id = albums.id) WHERE is_stub != 1 OR is_stub IS NULL`.
4. Installs recurrence-prevention SQLite triggers on `tracks` (INSERT, DELETE, UPDATE of album_id)
   to guarantee ongoing synchronization at the database engine level.
5. Verifies relational integrity via `PRAGMA foreign_key_check = 0` and `PRAGMA integrity_check = ok`.
6. Supports `--dry-run`, `--db-path`, and `--backup-dir`.

Usage:
    python3 scripts/recalculate_album_total_tracks.py [--db-path PATH] [--backup-dir DIR] [--dry-run]
"""

import argparse
import os
import shutil
import sqlite3
import sys
import time

DEFAULT_DB_CANDIDATES = [
    os.path.expanduser("~/.local/share/com.syncify.app/syncify.db"),
    os.path.abspath("workspace/audit_archive/data/syncify.db"),
    os.path.abspath("src-tauri/syncify.db"),
    os.path.abspath("syncify.db"),
]


def find_default_db() -> str:
    for path in DEFAULT_DB_CANDIDATES:
        if os.path.exists(path):
            return path
    return DEFAULT_DB_CANDIDATES[0]


def parse_args():
    parser = argparse.ArgumentParser(
        description="Recalculate albums.total_tracks to match actual COUNT(tracks), preserving stubs (TASK-138)."
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
    parser.add_argument(
        "--no-triggers",
        action="store_true",
        help="Skip installation of recurrence prevention SQLite triggers",
    )
    return parser.parse_args()


def column_exists(cur: sqlite3.Cursor, table: str, column: str) -> bool:
    cur.execute(f"PRAGMA table_info({table});")
    columns = [row[1] for row in cur.fetchall()]
    return column in columns


def create_safety_backup(db_path: str, backup_dir: str) -> str:
    timestamp = int(time.time())
    backup_path = os.path.join(backup_dir, f"syncify_backup_pre_repair_TASK-138_{timestamp}.db")
    os.makedirs(backup_dir, exist_ok=True)
    print(f"[TASK-138] Creating safety snapshot at {backup_path}...")

    # Attempt 1: VACUUM INTO via read-only URI
    try:
        src_conn = sqlite3.connect(f"file:{os.path.abspath(db_path)}?mode=ro", uri=True)
        src_conn.execute(f"VACUUM INTO '{backup_path}'")
        src_conn.close()
        print(f"[TASK-138] Safety snapshot created via VACUUM INTO: {backup_path}")
        return backup_path
    except Exception as e:
        print(f"[TASK-138] VACUUM INTO fallback ({e}), attempting sqlite3 backup API...")

    # Attempt 2: sqlite3 backup API
    try:
        src_conn = sqlite3.connect(f"file:{os.path.abspath(db_path)}?mode=ro&immutable=1", uri=True)
        dst_conn = sqlite3.connect(backup_path)
        src_conn.backup(dst_conn)
        dst_conn.close()
        src_conn.close()
        print(f"[TASK-138] Safety snapshot created via backup API: {backup_path}")
        return backup_path
    except Exception as e2:
        print(f"[TASK-138] Backup API fallback ({e2}), attempting shutil.copy2...")

    # Attempt 3: direct copy
    shutil.copy2(db_path, backup_path)
    print(f"[TASK-138] Safety snapshot created via copy: {backup_path}")
    return backup_path


def install_recurrence_triggers(cur: sqlite3.Cursor) -> None:
    print("[TASK-138] Installing recurrence-prevention SQLite triggers...")
    triggers = [
        """
        CREATE TRIGGER IF NOT EXISTS trg_tracks_sync_album_total_tracks_ins
        AFTER INSERT ON tracks
        FOR EACH ROW
        WHEN NEW.album_id IS NOT NULL
        BEGIN
            UPDATE albums
            SET total_tracks = (SELECT COUNT(*) FROM tracks WHERE tracks.album_id = NEW.album_id)
            WHERE id = NEW.album_id AND (is_stub != 1 OR is_stub IS NULL);
        END;
        """,
        """
        CREATE TRIGGER IF NOT EXISTS trg_tracks_sync_album_total_tracks_del
        AFTER DELETE ON tracks
        FOR EACH ROW
        WHEN OLD.album_id IS NOT NULL
        BEGIN
            UPDATE albums
            SET total_tracks = (SELECT COUNT(*) FROM tracks WHERE tracks.album_id = OLD.album_id)
            WHERE id = OLD.album_id AND (is_stub != 1 OR is_stub IS NULL);
        END;
        """,
        """
        CREATE TRIGGER IF NOT EXISTS trg_tracks_sync_album_total_tracks_upd
        AFTER UPDATE OF album_id ON tracks
        FOR EACH ROW
        BEGIN
            UPDATE albums
            SET total_tracks = (SELECT COUNT(*) FROM tracks WHERE tracks.album_id = NEW.album_id)
            WHERE NEW.album_id IS NOT NULL AND id = NEW.album_id AND (is_stub != 1 OR is_stub IS NULL);

            UPDATE albums
            SET total_tracks = (SELECT COUNT(*) FROM tracks WHERE tracks.album_id = OLD.album_id)
            WHERE OLD.album_id IS NOT NULL AND id = OLD.album_id AND (is_stub != 1 OR is_stub IS NULL);
        END;
        """,
    ]
    for trg_sql in triggers:
        cur.execute(trg_sql)
    print("[TASK-138] Recurrence-prevention triggers installed successfully.")


def run_recalculation(
    db_path: str,
    backup_dir: str = "/tmp",
    dry_run: bool = False,
    install_triggers: bool = True,
) -> bool:
    if not os.path.exists(db_path):
        print(f"[TASK-138] Error: Database not found at {db_path}", file=sys.stderr)
        return False

    print(f"[TASK-138] Target database: {db_path}")

    # 1. Pre-repair safety snapshot
    if not dry_run:
        create_safety_backup(db_path, backup_dir)

    # 2. Connect
    if dry_run:
        conn = sqlite3.connect(f"file:{os.path.abspath(db_path)}?mode=ro&immutable=1", uri=True)
    else:
        conn = sqlite3.connect(db_path)
    cur = conn.cursor()

    if not dry_run:
        cur.execute("PRAGMA foreign_keys = ON;")

    # 3. Schema analysis
    has_is_stub = column_exists(cur, "albums", "is_stub")
    stub_filter = "(is_stub != 1 OR is_stub IS NULL)" if has_is_stub else "1=1"

    total_albums = cur.execute("SELECT COUNT(*) FROM albums;").fetchone()[0]
    total_tracks = cur.execute("SELECT COUNT(*) FROM tracks;").fetchone()[0]

    stub_albums_count = 0
    if has_is_stub:
        stub_albums_count = cur.execute("SELECT COUNT(*) FROM albums WHERE is_stub = 1;").fetchone()[0]

    # Pre-diagnostics
    div_query = f"""
    SELECT COUNT(*) FROM albums
    WHERE {stub_filter}
      AND (total_tracks IS NULL OR total_tracks != (SELECT COUNT(*) FROM tracks WHERE tracks.album_id = albums.id));
    """
    divergent_before = cur.execute(div_query).fetchone()[0]

    excess_query = f"""
    SELECT COUNT(*) FROM albums
    WHERE {stub_filter}
      AND total_tracks > (SELECT COUNT(*) FROM tracks WHERE tracks.album_id = albums.id);
    """
    excess_before = cur.execute(excess_query).fetchone()[0]

    deficit_query = f"""
    SELECT COUNT(*) FROM albums
    WHERE {stub_filter}
      AND total_tracks IS NOT NULL
      AND total_tracks < (SELECT COUNT(*) FROM tracks WHERE tracks.album_id = albums.id);
    """
    deficit_before = cur.execute(deficit_query).fetchone()[0]

    null_query = f"""
    SELECT COUNT(*) FROM albums
    WHERE {stub_filter}
      AND total_tracks IS NULL;
    """
    null_before = cur.execute(null_query).fetchone()[0]

    ghost_query = f"""
    SELECT COUNT(*) FROM albums
    WHERE {stub_filter}
      AND total_tracks > 0
      AND (SELECT COUNT(*) FROM tracks WHERE tracks.album_id = albums.id) = 0;
    """
    ghost_before = cur.execute(ghost_query).fetchone()[0]

    print("=" * 68)
    print(f"[TASK-138] DIAGNOSTICS BEFORE REPAIR:")
    print(f"  - Total albums:                     {total_albums}")
    print(f"  - Documented stub albums (stub=1):  {stub_albums_count}")
    print(f"  - Total tracks:                     {total_tracks}")
    print(f"  - Divergent albums to reconcile:    {divergent_before}")
    print(f"      * Excess (declared > actual):   {excess_before}")
    print(f"      * Deficit (declared < actual):  {deficit_before}")
    print(f"      * NULL total_tracks:            {null_before}")
    print(f"      * Ghost / 0-track non-stubs:    {ghost_before}")
    print("=" * 68)

    # Sample sample divergent albums for visibility
    sample_query = f"""
    SELECT id, title, total_tracks, (SELECT COUNT(*) FROM tracks WHERE tracks.album_id = albums.id) as actual
    FROM albums
    WHERE {stub_filter}
      AND (total_tracks IS NULL OR total_tracks != (SELECT COUNT(*) FROM tracks WHERE tracks.album_id = albums.id))
    LIMIT 5;
    """
    sample_rows = cur.execute(sample_query).fetchall()
    if sample_rows:
        print("[TASK-138] Sample divergent albums:")
        for row in sample_rows:
            print(f"    Album #{row[0]} '{row[1]}': total_tracks={row[2]} vs COUNT(tracks)={row[3]}")

    if dry_run:
        print(f"[TASK-138] [DRY RUN] Would recalculate total_tracks for {divergent_before} albums.")
        conn.close()
        return True

    # 4. Perform atomic update
    print(f"[TASK-138] Executing atomic total_tracks synchronization...")
    update_sql = f"""
    UPDATE albums
    SET total_tracks = (SELECT COUNT(*) FROM tracks WHERE tracks.album_id = albums.id)
    WHERE {stub_filter};
    """
    cur.execute(update_sql)
    updated_rows = cur.rowcount
    print(f"[TASK-138] Successfully updated {updated_rows} albums.")

    # 5. Install recurrence prevention triggers
    if install_triggers and has_is_stub:
        install_recurrence_triggers(cur)

    # 6. Integrity and foreign key assertions
    print("[TASK-138] Validating database integrity and foreign key constraints...")
    fk_violations = cur.execute("PRAGMA foreign_key_check;").fetchall()
    if fk_violations:
        print(f"[TASK-138] ERROR: PRAGMA foreign_key_check failed with {len(fk_violations)} violations!", file=sys.stderr)
        for viol in fk_violations[:10]:
            print(f"  FK violation: {viol}", file=sys.stderr)
        conn.rollback()
        conn.close()
        return False
    print("[TASK-138] PRAGMA foreign_key_check: OK (0 violations)")

    integrity_res = cur.execute("PRAGMA integrity_check;").fetchall()
    if not integrity_res or integrity_res[0][0] != "ok":
        print(f"[TASK-138] ERROR: PRAGMA integrity_check failed: {integrity_res}", file=sys.stderr)
        conn.rollback()
        conn.close()
        return False
    print("[TASK-138] PRAGMA integrity_check: OK (ok)")

    # 7. Post-repair assertions
    divergent_after = cur.execute(div_query).fetchone()[0]
    print("=" * 68)
    print(f"[TASK-138] POST-REPAIR VERIFICATION:")
    print(f"  - Divergent albums remaining:       {divergent_after} (EXPECTED: 0)")
    print("=" * 68)

    if divergent_after != 0:
        print(f"[TASK-138] ERROR: Divergent albums remain after repair: {divergent_after}", file=sys.stderr)
        conn.rollback()
        conn.close()
        return False

    # Check that stubs are preserved
    if has_is_stub and stub_albums_count > 0:
        stubs_retained = cur.execute("SELECT COUNT(*) FROM albums WHERE is_stub = 1;").fetchone()[0]
        assert stubs_retained == stub_albums_count, f"Stub count mismatch: {stubs_retained} vs {stub_albums_count}"
        print(f"[TASK-138] Stubs preserved: {stubs_retained}/{stub_albums_count} stubs intact.")

    conn.commit()
    conn.close()
    print("[TASK-138] Remediation transaction successfully committed.")
    return True


def main():
    args = parse_args()
    success = run_recalculation(
        db_path=args.db_path,
        backup_dir=args.backup_dir,
        dry_run=args.dry_run,
        install_triggers=not args.no_triggers,
    )
    sys.exit(0 if success else 1)


if __name__ == "__main__":
    main()

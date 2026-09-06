#!/usr/bin/env python3
"""
Syncify Playlist Sanitization & Deduplication Script (TASK-107)
==============================================================
Sanitizes playlists across the SQLite library database:
1. Purges duplicate track occurrences within each playlist, keeping the first occurrence (lowest position).
2. Recompacts playlist positions to strictly 1-indexed, sequential, and gap-free (1, 2, 3... N).
3. Synchronizes `playlists.track_count` with the exact count in `playlist_tracks`.
4. Disambiguates duplicate playlist names within the same account (account_id, LOWER(TRIM(name))).

Safety & Governance:
- Creates an atomic snapshot using `VACUUM INTO` in /tmp/syncify_backup_pre_repair_TASK-107_<timestamp>.db.
- Supports `--dry-run`, `--db-path`, and `--backup-dir`.
- Strict post-assertions: 0 remaining duplicate instances, 0 position gaps, 0 track_count mismatches,
  0 duplicate names per account, and 0 foreign key violations.
"""

import argparse
import os
import shutil
import sqlite3
import sys
import time
from typing import Dict, List, Tuple, Any, Optional


def find_default_db() -> str:
    home_db = os.path.expanduser("~/.local/share/com.syncify.app/syncify.db")
    if os.path.exists(home_db):
        return home_db
    local_db = "syncify.db"
    if os.path.exists(local_db):
        return local_db
    src_db = os.path.join("src-tauri", "syncify.db")
    if os.path.exists(src_db):
        return src_db
    return home_db


def create_safety_backup(db_path: str, backup_dir: str = "/tmp") -> str:
    os.makedirs(backup_dir, exist_ok=True)
    timestamp = int(time.time())
    backup_path = os.path.join(backup_dir, f"syncify_backup_pre_repair_TASK-107_{timestamp}.db")
    print(f"[TASK-107] Creating safety snapshot at {backup_path}...")
    try:
        src_conn = sqlite3.connect(f"file:{os.path.abspath(db_path)}?mode=ro", uri=True)
        src_conn.execute(f"VACUUM INTO '{backup_path}'")
        src_conn.close()
        print(f"[TASK-107] Safety snapshot created successfully via VACUUM INTO: {backup_path}")
    except Exception as e:
        print(f"[TASK-107] VACUUM INTO failed ({e}), falling back to file copy...")
        shutil.copy2(db_path, backup_path)
        print(f"[TASK-107] Safety snapshot created successfully via copy: {backup_path}")
    return backup_path


def run_diagnostics(cur: sqlite3.Cursor) -> Dict[str, Any]:
    dup_tracks = cur.execute(
        "SELECT COALESCE(SUM(cnt - 1), 0) FROM ("
        "  SELECT playlist_id, track_id, COUNT(*) as cnt "
        "  FROM playlist_tracks "
        "  GROUP BY playlist_id, track_id "
        "  HAVING cnt > 1"
        ")"
    ).fetchone()[0]

    gaps = cur.execute(
        "SELECT COUNT(*) FROM ("
        "  SELECT playlist_id FROM playlist_tracks "
        "  GROUP BY playlist_id "
        "  HAVING MAX(position) - MIN(position) + 1 != COUNT(*)"
        ")"
    ).fetchone()[0]

    mismatched_track_count = cur.execute(
        "SELECT COUNT(*) FROM playlists p "
        "WHERE p.track_count != (SELECT COUNT(*) FROM playlist_tracks pt WHERE pt.playlist_id = p.id)"
    ).fetchone()[0]

    dup_names = cur.execute(
        "SELECT COUNT(*) FROM ("
        "  SELECT account_id, LOWER(TRIM(name)) FROM playlists "
        "  GROUP BY account_id, LOWER(TRIM(name)) "
        "  HAVING COUNT(*) > 1"
        ")"
    ).fetchone()[0]

    return {
        "dup_tracks": dup_tracks,
        "gaps": gaps,
        "mismatched_track_count": mismatched_track_count,
        "dup_names": dup_names,
    }


def sanitize_playlists(
    db_path: str,
    backup_dir: str = "/tmp",
    dry_run: bool = False,
    sql_out_path: Optional[str] = None,
) -> bool:
    if not os.path.exists(db_path):
        print(f"Error: Database not found at {db_path}", file=sys.stderr)
        return False

    print(f"[TASK-107] Target database: {db_path}")
    print(f"[TASK-107] Mode: {'DRY RUN (simulation with rollback)' if dry_run else 'EXECUTION'}")

    if not dry_run:
        create_safety_backup(db_path, backup_dir)

    conn = sqlite3.connect(db_path)
    cur = conn.cursor()
    cur.execute("PRAGMA journal_mode = WAL;")
    cur.execute("PRAGMA foreign_keys = ON;")

    pre = run_diagnostics(cur)
    print(f"\n[TASK-107] Pre-sanitization diagnostics:")
    print(f"  - Duplicate track instances in playlists: {pre['dup_tracks']}")
    print(f"  - Playlists with discontinuous position gaps: {pre['gaps']}")
    print(f"  - Playlists with mismatched track_count: {pre['mismatched_track_count']}")
    print(f"  - Duplicate playlist name groups per account: {pre['dup_names']}")

    sql_statements: List[str] = []
    sql_statements.append("PRAGMA foreign_keys = ON;")
    sql_statements.append("BEGIN IMMEDIATE TRANSACTION;")

    cur.execute("BEGIN IMMEDIATE TRANSACTION;")
    t0 = time.time()

    try:
        # 1. Purge duplicate tracks within playlists, keeping the lowest position (first appearance)
        purge_sql = """
            DELETE FROM playlist_tracks
            WHERE id IN (
                SELECT id FROM (
                    SELECT id,
                           ROW_NUMBER() OVER (
                               PARTITION BY playlist_id, track_id
                               ORDER BY position ASC, added_at ASC, id ASC
                           ) as rn
                    FROM playlist_tracks
                ) WHERE rn > 1
            );
        """
        cur.execute(purge_sql)
        purged_tracks = cur.rowcount
        sql_statements.append(purge_sql.strip())

        # 2. Recompact positions sequentially 1..N
        cur.execute("DROP TABLE IF EXISTS _playlist_tracks_recompact;")
        cur.execute("""
            CREATE TEMP TABLE _playlist_tracks_recompact (
                id INTEGER PRIMARY KEY,
                new_pos INTEGER NOT NULL
            );
        """)
        recompact_insert_sql = """
            INSERT INTO _playlist_tracks_recompact (id, new_pos)
            SELECT
                id,
                ROW_NUMBER() OVER (
                    PARTITION BY playlist_id
                    ORDER BY position ASC, added_at ASC, id ASC
                )
            FROM playlist_tracks;
        """
        cur.execute(recompact_insert_sql)

        stage_sql = "UPDATE playlist_tracks SET position = -(id + 1);"
        cur.execute(stage_sql)

        apply_sql = """
            UPDATE playlist_tracks
            SET position = (
                SELECT r.new_pos
                FROM _playlist_tracks_recompact r
                WHERE r.id = playlist_tracks.id
            );
        """
        cur.execute(apply_sql)
        cur.execute("DROP TABLE IF EXISTS _playlist_tracks_recompact;")

        sql_statements.append(recompact_insert_sql.strip())
        sql_statements.append(stage_sql)
        sql_statements.append(apply_sql.strip())

        # 3. Synchronize track_count
        sync_sql = """
            UPDATE playlists
            SET track_count = (
                SELECT COUNT(*)
                FROM playlist_tracks
                WHERE playlist_tracks.playlist_id = playlists.id
            );
        """
        cur.execute(sync_sql)
        sql_statements.append(sync_sql.strip())

        # 4. Disambiguate duplicate playlist names under the same account
        dup_groups = cur.execute("""
            SELECT account_id, LOWER(TRIM(name))
            FROM playlists
            GROUP BY account_id, LOWER(TRIM(name))
            HAVING COUNT(*) > 1
            ORDER BY account_id, LOWER(TRIM(name))
        """).fetchall()

        renamed_count = 0
        for acc_id, norm_name in dup_groups:
            pls = cur.execute("""
                SELECT id, name
                FROM playlists
                WHERE account_id = ? AND LOWER(TRIM(name)) = ?
                ORDER BY id ASC
            """, (acc_id, norm_name)).fetchall()

            existing_names = set(
                r[0].lower()
                for r in cur.execute(
                    "SELECT TRIM(name) FROM playlists WHERE account_id = ?", (acc_id,)
                ).fetchall()
            )

            # pls[0] keeps original name; subsequent playlists get disambiguated with (2), (3)...
            for idx, (pid, orig_name) in enumerate(pls[1:], start=2):
                cand_idx = idx
                cand_name = f"{orig_name.strip()} ({cand_idx})"
                while cand_name.strip().lower() in existing_names:
                    cand_idx += 1
                    cand_name = f"{orig_name.strip()} ({cand_idx})"

                existing_names.add(cand_name.strip().lower())
                cur.execute(
                    "UPDATE playlists SET name = ?, updated_at = CURRENT_TIMESTAMP WHERE id = ?",
                    (cand_name, pid),
                )
                escaped_name = cand_name.replace("'", "''")
                sql_statements.append(
                    f"UPDATE playlists SET name = '{escaped_name}', updated_at = CURRENT_TIMESTAMP WHERE id = {pid};"
                )
                renamed_count += 1

        post = run_diagnostics(cur)
        integrity = cur.execute("PRAGMA integrity_check;").fetchone()[0]
        fk_errors = cur.execute("PRAGMA foreign_key_check;").fetchall()

        if dry_run:
            cur.execute("ROLLBACK;")
            sql_statements.append("ROLLBACK;")
            print("\n[TASK-107] DRY RUN: Transaction rolled back. Database was not modified.")
        else:
            conn.commit()
            sql_statements.append("COMMIT;")
            elapsed = time.time() - t0
            print(f"\n[TASK-107] Transaction committed successfully in {elapsed:.2f}s.")

    except Exception as e:
        conn.rollback()
        conn.close()
        print(f"[TASK-107] ERROR during sanitization transaction: {e}", file=sys.stderr)
        return False

    conn.close()

    if sql_out_path:
        os.makedirs(os.path.dirname(os.path.abspath(sql_out_path)), exist_ok=True)
        with open(sql_out_path, "w", encoding="utf-8") as f:
            f.write("\n".join(sql_statements) + "\n")
        print(f"[TASK-107] Generated SQL audit script written to: {sql_out_path}")

    print("\n" + "=" * 65)
    print("PLAYLIST SANITIZATION & DEDUPLICATION REPORT (TASK-107)")
    print("=" * 65)
    print(f"Duplicate tracks purged:           {purged_tracks} (Initial: {pre['dup_tracks']} -> Remaining: {post['dup_tracks']})")
    print(f"Playlists recompacted:             {pre['gaps']} (Remaining gaps: {post['gaps']})")
    print(f"Track counts synchronized:         {pre['mismatched_track_count']} (Remaining mismatches: {post['mismatched_track_count']})")
    print(f"Playlist names disambiguated:      {renamed_count} (Remaining duplicate groups: {post['dup_names']})")
    print(f"PRAGMA integrity_check:            {integrity} (Expected: ok)")
    print(f"PRAGMA foreign_key_check:          {len(fk_errors)} violations (Expected: 0)")
    print("=" * 65)

    success = (
        post["dup_tracks"] == 0
        and post["gaps"] == 0
        and post["mismatched_track_count"] == 0
        and post["dup_names"] == 0
        and integrity == "ok"
        and len(fk_errors) == 0
    )

    if success:
        print("[TASK-107] PASSED: All acceptance criteria met (0 duplicates, 0 gaps, 0 mismatches, 0 FK errors).")
    else:
        print("[TASK-107] FAILED: Post-sanitization assertions not met.", file=sys.stderr)

    return success


def main():
    default_db = find_default_db()
    parser = argparse.ArgumentParser(
        description="Syncify Playlist Sanitization & Deduplication Tool (TASK-107)"
    )
    parser.add_argument(
        "--db-path",
        default=default_db,
        help=f"Path to SQLite DB (default: {default_db})",
    )
    parser.add_argument(
        "--db",
        dest="db_path_alias",
        default=None,
        help="Alias for --db-path",
    )
    parser.add_argument(
        "--backup-dir",
        default="/tmp",
        help="Directory for safety backup snapshot (default: /tmp)",
    )
    parser.add_argument(
        "--dry-run",
        action="store_true",
        help="Simulate execution in transaction with rollback without modifying database",
    )
    parser.add_argument(
        "--sql-out",
        default=None,
        help="Optional path to output generated SQL script",
    )

    args = parser.parse_args()
    db_path = args.db_path_alias if args.db_path_alias else args.db_path

    success = sanitize_playlists(
        db_path=db_path,
        backup_dir=args.backup_dir,
        dry_run=args.dry_run,
        sql_out_path=args.sql_out,
    )
    sys.exit(0 if success else 1)


if __name__ == "__main__":
    main()

#!/usr/bin/env python3
"""
Syncify Compilation Artists Unification & Maintenance Script (TASK-137)
======================================================================
Unifies compilation artist variants ('Various Interprets', 'Unknown',
'Unknown Artist', 'VA', 'V.A.', 'Various') into canonical 'Various Artists'.
Reassigns album_artists, track_artists, and track_credits while strictly
preventing composite primary key collisions. Ensures 'is_compilation' column
exists on albums and sets is_compilation = 1 for compilation albums.
Purges residual unlinked obsolete artist records, verifies database
referential integrity (PRAGMA foreign_key_check), and installs recurrence
prevention triggers.

Usage:
    python3 scripts/unify_compilation_artists.py [--db-path PATH] [--backup-dir DIR] [--dry-run]
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

TARGET_VARIANTS = [
    "various interprets",
    "various interpret",
    "unknown artist",
    "unknown",
    "v.a.",
    "va",
    "v/a",
    "various",
]


def find_default_db() -> str:
    for path in DEFAULT_DB_CANDIDATES:
        if os.path.exists(path):
            return path
    return DEFAULT_DB_CANDIDATES[0]


def parse_args():
    parser = argparse.ArgumentParser(
        description="Unify compilation artist variants into canonical 'Various Artists' in Syncify SQLite DB."
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
    backup_path = os.path.join(backup_dir, f"syncify_backup_pre_repair_TASK-137_{timestamp}.db")
    os.makedirs(backup_dir, exist_ok=True)
    print(f"[TASK-137] Creating safety snapshot at {backup_path}...")

    try:
        src_conn = sqlite3.connect(f"file:{os.path.abspath(db_path)}?mode=ro", uri=True)
        src_conn.execute(f"VACUUM INTO '{backup_path}'")
        src_conn.close()
        print(f"[TASK-137] Safety snapshot created via VACUUM INTO: {backup_path}")
        return backup_path
    except Exception as e:
        print(f"[TASK-137] VACUUM INTO fallback ({e}), attempting file copy...")

    shutil.copy2(db_path, backup_path)
    print(f"[TASK-137] Safety snapshot created via copy: {backup_path}")
    return backup_path


def run_unification(db_path: str, backup_dir: str = "/tmp", dry_run: bool = False) -> bool:
    if not os.path.exists(db_path):
        print(f"Error: Database not found at {db_path}", file=sys.stderr)
        return False

    print(f"[TASK-137] Target database: {db_path}")

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
    cur.execute("SELECT id, name FROM artists WHERE LOWER(TRIM(name)) = 'various artists'")
    va_row = cur.fetchone()
    canonical_va_id = va_row[0] if va_row else None

    placeholders = ",".join("?" for _ in TARGET_VARIANTS)
    variant_artists = cur.execute(
        f"""
        SELECT ar.id, ar.name,
               (SELECT COUNT(*) FROM album_artists aa WHERE aa.artist_id = ar.id) AS alb_count,
               (SELECT COUNT(*) FROM track_artists ta WHERE ta.artist_id = ar.id) AS trk_count,
               (SELECT COUNT(*) FROM track_credits tc WHERE tc.artist_id = ar.id) AS cred_count
        FROM artists ar
        WHERE LOWER(TRIM(ar.name)) IN ({placeholders})
          AND (ar.id != ? OR ? IS NULL)
        ORDER BY alb_count DESC, trk_count DESC
        """,
        TARGET_VARIANTS + [canonical_va_id, canonical_va_id],
    ).fetchall()

    total_variant_albums = sum(row[2] for row in variant_artists)
    total_variant_tracks = sum(row[3] for row in variant_artists)
    total_variant_credits = sum(row[4] for row in variant_artists)

    print("\n--- Pre-Execution Diagnostics ---")
    print(f"Canonical 'Various Artists' ID: {canonical_va_id or 'NOT FOUND (Will be created)'}")
    print(f"Target compilation variants found: {len(variant_artists)}")
    for var_id, var_name, alb_cnt, trk_cnt, cred_cnt in variant_artists:
        print(f"  - [{var_id}] '{var_name}': {alb_cnt} albums, {trk_cnt} tracks, {cred_cnt} credits")
    print(f"Total albums under variants: {total_variant_albums}")
    print(f"Total tracks under variants: {total_variant_tracks}")
    print(f"Total credits under variants: {total_variant_credits}")

    if dry_run:
        print("\n[TASK-137] [DRY RUN] Complete. No modifications made to database.")
        conn.close()
        return True

    print("\n--- Executing Unification ---")
    try:
        # Step A: Ensure is_compilation column on albums
        album_cols = [col[1] for col in cur.execute("PRAGMA table_info(albums)").fetchall()]
        if "is_compilation" not in album_cols:
            print("[TASK-137] Adding is_compilation column to albums...")
            cur.execute("ALTER TABLE albums ADD COLUMN is_compilation INTEGER NOT NULL DEFAULT 0")
            cur.execute("CREATE INDEX IF NOT EXISTS idx_albums_is_compilation ON albums(is_compilation)")

        # Step B: Collect all source artist IDs
        source_rows = cur.execute(
            f"""
            SELECT id FROM artists
            WHERE LOWER(TRIM(name)) IN ({placeholders})
            """,
            TARGET_VARIANTS,
        ).fetchall()
        source_ids = [r[0] for r in source_rows]

        canonical_id = None
        if source_ids or canonical_va_id:
            cur.execute("INSERT OR IGNORE INTO artists (name) VALUES ('Various Artists')")
            cur.execute("SELECT id FROM artists WHERE LOWER(TRIM(name)) = 'various artists' ORDER BY id ASC LIMIT 1")
            canonical_id = cur.fetchone()[0]
            print(f"[TASK-137] Canonical 'Various Artists' ID: {canonical_id}")
            source_ids = [sid for sid in source_ids if sid != canonical_id]

        if source_ids:
            src_placeholders = ",".join("?" for _ in source_ids)

            # Step D: Consolidate external IDs and metadata onto canonical winner
            cur.execute(
                f"""
                UPDATE artists
                SET
                    musicbrainz_id = COALESCE(artists.musicbrainz_id, (
                        SELECT src.musicbrainz_id FROM artists src
                        WHERE src.id IN ({src_placeholders})
                          AND src.musicbrainz_id IS NOT NULL AND src.musicbrainz_id != ''
                        LIMIT 1
                    )),
                    spotify_id = COALESCE(artists.spotify_id, (
                        SELECT src.spotify_id FROM artists src
                        WHERE src.id IN ({src_placeholders})
                          AND src.spotify_id IS NOT NULL AND src.spotify_id != ''
                        LIMIT 1
                    )),
                    tidal_id = COALESCE(artists.tidal_id, (
                        SELECT src.tidal_id FROM artists src
                        WHERE src.id IN ({src_placeholders})
                          AND src.tidal_id IS NOT NULL AND src.tidal_id != ''
                        LIMIT 1
                    )),
                    qobuz_id = COALESCE(artists.qobuz_id, (
                        SELECT src.qobuz_id FROM artists src
                        WHERE src.id IN ({src_placeholders})
                          AND src.qobuz_id IS NOT NULL AND src.qobuz_id != ''
                        LIMIT 1
                    ))
                WHERE id = ?
                """,
                source_ids + source_ids + source_ids + source_ids + [canonical_id],
            )

            # Step E: Clear external service IDs on source records before deletion
            cur.execute(
                f"UPDATE artists SET musicbrainz_id = NULL, spotify_id = NULL, tidal_id = NULL, qobuz_id = NULL WHERE id IN ({src_placeholders})",
                source_ids,
            )

            # Step F: Check for albums.artist_id / tracks.artist_id if present
            if "artist_id" in album_cols:
                cur.execute(
                    f"UPDATE albums SET artist_id = ? WHERE artist_id IN ({src_placeholders})",
                    [canonical_id] + source_ids,
                )

            track_cols = [col[1] for col in cur.execute("PRAGMA table_info(tracks)").fetchall()]
            if "artist_id" in track_cols:
                cur.execute(
                    f"UPDATE tracks SET artist_id = ? WHERE artist_id IN ({src_placeholders})",
                    [canonical_id] + source_ids,
                )

            # Step G: Reassign album_artists (deleting collisions first)
            cur.execute(
                f"""
                DELETE FROM album_artists
                WHERE artist_id IN ({src_placeholders})
                  AND EXISTS (
                      SELECT 1 FROM album_artists aa2
                      WHERE aa2.album_id = album_artists.album_id
                        AND aa2.artist_id = ?
                  )
                """,
                source_ids + [canonical_id],
            )
            cur.execute(
                f"UPDATE album_artists SET artist_id = ? WHERE artist_id IN ({src_placeholders})",
                [canonical_id] + source_ids,
            )

            # Step H: Reassign track_artists (deleting collisions first)
            cur.execute(
                f"""
                DELETE FROM track_artists
                WHERE artist_id IN ({src_placeholders})
                  AND EXISTS (
                      SELECT 1 FROM track_artists ta2
                      WHERE ta2.track_id = track_artists.track_id
                        AND ta2.artist_id = ?
                        AND COALESCE(ta2.role, 'primary') = COALESCE(track_artists.role, 'primary')
                  )
                """,
                source_ids + [canonical_id],
            )
            cur.execute(
                f"UPDATE track_artists SET artist_id = ? WHERE artist_id IN ({src_placeholders})",
                [canonical_id] + source_ids,
            )

            # Step I: Reassign track_credits (deleting collisions first)
            cur.execute(
                f"""
                DELETE FROM track_credits
                WHERE artist_id IN ({src_placeholders})
                  AND EXISTS (
                      SELECT 1 FROM track_credits tc2
                      WHERE tc2.track_id = track_credits.track_id
                        AND tc2.artist_id = ?
                        AND tc2.role = track_credits.role
                  )
                """,
                source_ids + [canonical_id],
            )
            cur.execute(
                f"UPDATE track_credits SET artist_id = ? WHERE artist_id IN ({src_placeholders})",
                [canonical_id] + source_ids,
            )

            # Step J: Purge residual unlinked obsolete artists
            cur.execute(
                f"""
                DELETE FROM artists
                WHERE id IN ({src_placeholders})
                  AND NOT EXISTS (SELECT 1 FROM album_artists aa WHERE aa.artist_id = artists.id)
                  AND NOT EXISTS (SELECT 1 FROM track_artists ta WHERE ta.artist_id = artists.id)
                  AND NOT EXISTS (SELECT 1 FROM track_credits tc WHERE tc.artist_id = artists.id)
                """,
                source_ids,
            )

        # Step K: Mark compilation flag on albums associated with canonical Various Artists
        if canonical_id:
            cur.execute(
                """
                UPDATE albums
                SET is_compilation = 1
                WHERE id IN (
                    SELECT aa.album_id FROM album_artists aa
                    WHERE aa.artist_id = ?
                )
                """,
                (canonical_id,),
            )

        # Step L: Install recurrence prevention triggers
        cur.execute(
            """
            CREATE TRIGGER IF NOT EXISTS trg_artists_reject_va_variants_ins
            BEFORE INSERT ON artists
            FOR EACH ROW
            WHEN LOWER(TRIM(NEW.name)) IN ('various interprets', 'various interpret', 'v.a.', 'va', 'v/a')
            BEGIN
                SELECT RAISE(ABORT, 'Rejected compilation artist variant: use canonical Various Artists');
            END;
            """
        )
        cur.execute(
            """
            CREATE TRIGGER IF NOT EXISTS trg_artists_reject_va_variants_upd
            BEFORE UPDATE OF name ON artists
            FOR EACH ROW
            WHEN LOWER(TRIM(NEW.name)) IN ('various interprets', 'various interpret', 'v.a.', 'va', 'v/a')
            BEGIN
                SELECT RAISE(ABORT, 'Rejected compilation artist variant: use canonical Various Artists');
            END;
            """
        )
        cur.execute(
            """
            CREATE TRIGGER IF NOT EXISTS trg_album_artists_set_compilation_ins
            AFTER INSERT ON album_artists
            FOR EACH ROW
            WHEN NEW.artist_id = (SELECT id FROM artists WHERE LOWER(TRIM(name)) = 'various artists' LIMIT 1)
            BEGIN
                UPDATE albums SET is_compilation = 1 WHERE id = NEW.album_id;
            END;
            """
        )

        # Step M: Integrity & Foreign Key Verification
        fk_violations = cur.execute("PRAGMA foreign_key_check;").fetchall()
        if fk_violations:
            print(f"[TASK-137] Foreign key check failed: {fk_violations}", file=sys.stderr)
            conn.rollback()
            conn.close()
            return False

        integrity_issues = cur.execute("PRAGMA integrity_check;").fetchall()
        if integrity_issues != [("ok",)]:
            print(f"[TASK-137] Integrity check failed: {integrity_issues}", file=sys.stderr)
            conn.rollback()
            conn.close()
            return False

        conn.commit()

        # Step N: Post-execution Diagnostics
        post_variant_artists = cur.execute(
            f"""
            SELECT COUNT(*) FROM album_artists aa
            JOIN artists ar ON ar.id = aa.artist_id
            WHERE LOWER(TRIM(ar.name)) IN ({placeholders})
              AND (ar.id != ? OR ? IS NULL)
            """,
            TARGET_VARIANTS + [canonical_id, canonical_id],
        ).fetchone()[0]

        va_albums_post = (
            cur.execute(
                "SELECT COUNT(*) FROM album_artists WHERE artist_id = ?",
                (canonical_id,),
            ).fetchone()[0]
            if canonical_id
            else 0
        )

        comp_albums_post = cur.execute(
            "SELECT COUNT(*) FROM albums WHERE is_compilation = 1"
        ).fetchone()[0]

        print("\n--- Post-Execution Diagnostics ---")
        print(f"Remaining albums under compilation variants: {post_variant_artists}")
        print(f"Total albums now linked to canonical 'Various Artists': {va_albums_post}")
        print(f"Total albums marked is_compilation = 1: {comp_albums_post}")
        print(f"PRAGMA foreign_key_check: 0 violations")
        print(f"PRAGMA integrity_check: ok")
        print("[TASK-137] Compilation artist unification successfully completed!")

        conn.close()
        return True

    except Exception as e:
        print(f"[TASK-137] Execution failed with error: {e}", file=sys.stderr)
        conn.rollback()
        conn.close()
        return False


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

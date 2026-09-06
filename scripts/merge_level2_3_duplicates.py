#!/usr/bin/env python3
"""
Syncify Level 2/3 Duplicate Merge & Intra-Album Renumbering Script (TASK-104)
===========================================================================
Portable maintenance script to merge Level 2 (intra-album) and Level 3 (fuzzy)
duplicate tracks with ISRC reconciliation, strict explicit flag discrimination,
safe track_sources transfer without loss, and sequential album track renumbering.

Usage:
    python3 scripts/merge_level2_3_duplicates.py [--db-path PATH] [--backup-dir DIR] [--dry-run]
"""

import argparse
import os
import shutil
import sqlite3
import sys
import time
from collections import defaultdict


def parse_args():
    parser = argparse.ArgumentParser(
        description="Merge Level 2/3 duplicate tracks with ISRC reconciliation and renumber album tracks."
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
    backup_path = os.path.join(backup_dir, f"syncify_backup_pre_repair_TASK-104_{timestamp}.db")
    os.makedirs(backup_dir, exist_ok=True)
    print(f"[TASK-104] Creating safety snapshot at {backup_path}...")

    # Attempt VACUUM INTO via read-only URI
    try:
        src_conn = sqlite3.connect(f"file:{os.path.abspath(db_path)}?mode=ro", uri=True)
        src_conn.execute(f"VACUUM INTO '{backup_path}'")
        src_conn.close()
        print(f"[TASK-104] Safety snapshot created via VACUUM INTO: {backup_path}")
        return backup_path
    except Exception as e:
        print(f"[TASK-104] VACUUM INTO fallback ({e}), attempting sqlite3 backup API...")

    try:
        src_conn = sqlite3.connect(f"file:{os.path.abspath(db_path)}?mode=ro&immutable=1", uri=True)
        dst_conn = sqlite3.connect(backup_path)
        src_conn.backup(dst_conn)
        dst_conn.close()
        src_conn.close()
        print(f"[TASK-104] Safety snapshot created via backup API: {backup_path}")
        return backup_path
    except Exception as e2:
        print(f"[TASK-104] Backup API fallback ({e2}), attempting copy2...")

    shutil.copy2(db_path, backup_path)
    print(f"[TASK-104] Safety snapshot created via copy: {backup_path}")
    return backup_path


def clean_title(title: str) -> str:
    if not title:
        return ""
    t = title.lower()
    for suffix in [
        " (remaster", " (deluxe", " - remaster", " - live", " (live",
        " [remaster", " [deluxe", " [live"
    ]:
        pos = t.find(suffix)
        if pos != -1:
            t = t[:pos]
    return t.strip()


def run_merge(db_path: str, backup_dir: str = "/tmp", dry_run: bool = False) -> bool:
    if not os.path.exists(db_path):
        print(f"Error: Database not found at {db_path}", file=sys.stderr)
        return False

    print(f"[TASK-104] Target database: {db_path}")

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
    total_tracks_pre = cur.execute("SELECT COUNT(*) FROM tracks").fetchone()[0]
    total_sources_pre = cur.execute("SELECT COUNT(*) FROM track_sources").fetchone()[0]

    # Level 2: Intra-album pairs
    intra_album_pairs_query = """
    SELECT COUNT(*) FROM (
        SELECT a.id, b.id
        FROM tracks a
        JOIN tracks b ON a.album_id = b.album_id AND a.id < b.id
        WHERE a.album_id IS NOT NULL
          AND a.duration_ms > 10000 AND b.duration_ms > 10000
          AND TRIM(COALESCE(a.title, '')) != ''
          AND (
              LOWER(TRIM(a.title)) = LOWER(TRIM(b.title))
              OR (a.disc_number = b.disc_number AND a.track_number = b.track_number AND a.track_number > 0)
          )
          AND ABS(a.duration_ms - b.duration_ms) <= 2000
    )
    """
    intra_pairs_count = cur.execute(intra_album_pairs_query).fetchone()[0]

    # Collisions on (album_id, disc_number, track_number)
    collision_query = """
    SELECT COUNT(*) FROM (
        SELECT album_id, disc_number, track_number
        FROM tracks
        WHERE album_id IS NOT NULL AND track_number IS NOT NULL AND track_number > 0
        GROUP BY album_id, disc_number, track_number
        HAVING COUNT(*) > 1
    )
    """
    numbering_collisions_pre = cur.execute(collision_query).fetchone()[0]

    # Level 3: Fuzzy pairs (same primary artist, title, duration ± 2000ms)
    fuzzy_pairs_query = """
    SELECT COUNT(*) FROM (
        SELECT a.id, b.id
        FROM track_artists ta1
        JOIN track_artists ta2 ON ta1.artist_id = ta2.artist_id AND ta1.track_id < ta2.track_id
        JOIN tracks a ON a.id = ta1.track_id
        JOIN tracks b ON b.id = ta2.track_id
        WHERE ta1.role = 'primary' AND ta2.role = 'primary'
          AND a.duration_ms > 10000 AND b.duration_ms > 10000
          AND TRIM(COALESCE(a.title, '')) != ''
          AND LOWER(TRIM(a.title)) = LOWER(TRIM(b.title))
          AND ABS(a.duration_ms - b.duration_ms) <= 2000
    )
    """
    fuzzy_pairs_count = cur.execute(fuzzy_pairs_query).fetchone()[0]

    # ISRC divergent or absent
    isrc_divergent_query = """
    SELECT COUNT(*) FROM (
        SELECT a.id, b.id
        FROM track_artists ta1
        JOIN track_artists ta2 ON ta1.artist_id = ta2.artist_id AND ta1.track_id < ta2.track_id
        JOIN tracks a ON a.id = ta1.track_id
        JOIN tracks b ON b.id = ta2.track_id
        WHERE ta1.role = 'primary' AND ta2.role = 'primary'
          AND a.duration_ms > 10000 AND b.duration_ms > 10000
          AND TRIM(COALESCE(a.title, '')) != ''
          AND LOWER(TRIM(a.title)) = LOWER(TRIM(b.title))
          AND ABS(a.duration_ms - b.duration_ms) <= 2000
          AND (
              a.isrc IS NULL OR TRIM(a.isrc) = ''
              OR b.isrc IS NULL OR TRIM(b.isrc) = ''
              OR a.isrc != b.isrc
          )
    )
    """
    isrc_divergent_count = cur.execute(isrc_divergent_query).fetchone()[0]

    # Explicit contradictory pairs (MUST NOT BE MERGED)
    explicit_contradictory_query = """
    SELECT COUNT(*) FROM (
        SELECT a.id, b.id
        FROM track_artists ta1
        JOIN track_artists ta2 ON ta1.artist_id = ta2.artist_id AND ta1.track_id < ta2.track_id
        JOIN tracks a ON a.id = ta1.track_id
        JOIN tracks b ON b.id = ta2.track_id
        WHERE ta1.role = 'primary' AND ta2.role = 'primary'
          AND a.duration_ms > 10000 AND b.duration_ms > 10000
          AND TRIM(COALESCE(a.title, '')) != ''
          AND LOWER(TRIM(a.title)) = LOWER(TRIM(b.title))
          AND ABS(a.duration_ms - b.duration_ms) <= 2000
          AND COALESCE(a.explicit, 0) != COALESCE(b.explicit, 0)
    )
    """
    explicit_contradictory_count = cur.execute(explicit_contradictory_query).fetchone()[0]

    print("[TASK-104] Pre-merge diagnostics:")
    print(f"  - Total tracks: {total_tracks_pre}")
    print(f"  - Total track sources: {total_sources_pre}")
    print(f"  - Level 2 Intra-album duplicate candidate pairs: {intra_pairs_count}")
    print(f"  - Album track numbering collisions (album_id, disc, track): {numbering_collisions_pre}")
    print(f"  - Level 3 Fuzzy duplicate candidate pairs: {fuzzy_pairs_count}")
    print(f"  - Pairs with divergent or absent ISRC: {isrc_divergent_count}")
    print(f"  - Pairs with contradictory explicit flags (protected): {explicit_contradictory_count}")

    if dry_run:
        print("[TASK-104] Dry-run enabled. No modifications applied.")
        conn.close()
        return True

    print("[TASK-104] Executing duplicate merge and album renumbering...")
    t0 = time.time()

    cur.execute("BEGIN IMMEDIATE;")

    # Union-Find data structures
    parent = {}
    rank = {}
    component_isrc = {}

    def find_root(node):
        p = parent.get(node, node)
        if p == node:
            return node
        r = find_root(p)
        parent[node] = r
        return r

    def union(a, b):
        ra = find_root(a)
        rb = find_root(b)
        if ra == rb:
            return
        r_a = rank.get(ra, 0)
        r_b = rank.get(rb, 0)
        if r_a < r_b:
            parent[ra] = rb
        elif r_a > r_b:
            parent[rb] = ra
        else:
            parent[rb] = ra
            rank[ra] = r_a + 1

    # Fetch Level 2 intra-album pairs (ISRC reconciled within the same album)
    cur.execute(
        """
        SELECT a.id, b.id, a.isrc, b.isrc
        FROM tracks a
        JOIN tracks b ON a.album_id = b.album_id AND a.id < b.id
        WHERE a.album_id IS NOT NULL
          AND a.duration_ms > 10000 AND b.duration_ms > 10000
          AND TRIM(COALESCE(a.title, '')) != ''
          AND TRIM(COALESCE(b.title, '')) != ''
          AND COALESCE(a.explicit, 0) = COALESCE(b.explicit, 0)
          AND (
              LOWER(TRIM(a.title)) = LOWER(TRIM(b.title))
              OR (a.disc_number = b.disc_number AND a.track_number = b.track_number AND a.track_number > 0)
          )
          AND ABS(a.duration_ms - b.duration_ms) <= 2000
        """
    )
    for id_a, id_b, isrc_a, isrc_b in cur.fetchall():
        parent.setdefault(id_a, id_a)
        parent.setdefault(id_b, id_b)
        clean_a = isrc_a.strip() if isrc_a and isrc_a.strip() else None
        clean_b = isrc_b.strip() if isrc_b and isrc_b.strip() else None

        ra = find_root(id_a)
        rb = find_root(id_b)
        if ra == rb:
            continue

        isrc_for_a = component_isrc.get(ra) or clean_a
        isrc_for_b = component_isrc.get(rb) or clean_b
        merged_isrc = isrc_for_a or isrc_for_b

        union(id_a, id_b)
        new_root = find_root(id_a)
        if merged_isrc:
            component_isrc[new_root] = merged_isrc

    # Fetch Level 3 fuzzy pairs (preserve distinct masters across albums)
    cur.execute(
        """
        SELECT a.id, b.id, a.isrc, b.isrc
        FROM track_artists ta1
        JOIN track_artists ta2 ON ta1.artist_id = ta2.artist_id AND ta1.track_id < ta2.track_id
        JOIN tracks a ON a.id = ta1.track_id
        JOIN tracks b ON b.id = ta2.track_id
        WHERE (LOWER(COALESCE(ta1.role, 'primary')) IN ('primary', 'main') OR (SELECT COUNT(*) FROM track_artists WHERE track_id = a.id) = 1)
          AND (LOWER(COALESCE(ta2.role, 'primary')) IN ('primary', 'main') OR (SELECT COUNT(*) FROM track_artists WHERE track_id = b.id) = 1)
          AND a.duration_ms > 10000 AND b.duration_ms > 10000
          AND TRIM(COALESCE(a.title, '')) != ''
          AND TRIM(COALESCE(b.title, '')) != ''
          AND COALESCE(a.explicit, 0) = COALESCE(b.explicit, 0)
          AND LOWER(TRIM(a.title)) = LOWER(TRIM(b.title))
          AND ABS(a.duration_ms - b.duration_ms) <= 2000
        """
    )
    for id_a, id_b, isrc_a, isrc_b in cur.fetchall():
        parent.setdefault(id_a, id_a)
        parent.setdefault(id_b, id_b)
        clean_a = isrc_a.strip() if isrc_a and isrc_a.strip() else None
        clean_b = isrc_b.strip() if isrc_b and isrc_b.strip() else None

        ra = find_root(id_a)
        rb = find_root(id_b)
        if ra == rb:
            continue

        isrc_for_a = component_isrc.get(ra) or clean_a
        isrc_for_b = component_isrc.get(rb) or clean_b

        # Across albums, distinct valid ISRCs represent distinct releases/masters
        if isrc_for_a and isrc_for_b and isrc_for_a != isrc_for_b:
            continue

        merged_isrc = isrc_for_a or isrc_for_b
        union(id_a, id_b)
        new_root = find_root(id_a)
        if merged_isrc:
            component_isrc[new_root] = merged_isrc

    # Group tracks by connected component
    groups = defaultdict(list)
    for node in list(parent.keys()):
        root = find_root(node)
        groups[root].append(node)

    tracks_removed_count = 0
    groups_resolved_count = 0
    affected_albums = set()

    # Table existence check for operation_journal
    has_op_journal = cur.execute(
        "SELECT COUNT(*) > 0 FROM sqlite_master WHERE type = 'table' AND name = 'operation_journal'"
    ).fetchone()[0]

    for root_id, track_ids in groups.items():
        if len(track_ids) <= 1:
            continue

        # Fetch detailed metrics for each track to determine the winner
        track_infos = []
        for tid in track_ids:
            cur.execute(
                """
                SELECT
                    t.id,
                    (t.isrc IS NOT NULL AND TRIM(t.isrc) != '') as has_isrc,
                    COALESCE(
                        MAX(ts.quality_score),
                        CASE
                            WHEN MAX(d.bit_depth) >= 24 THEN 1200
                            WHEN MAX(d.bit_depth) >= 16 THEN 1000
                            WHEN MAX(d.file_path) IS NOT NULL THEN 500
                            ELSE NULL
                        END
                    ) as quality_score,
                    MAX(COALESCE(d.bit_depth, ts.bit_depth, 0)) as bit_depth,
                    MAX(COALESCE(d.sample_rate, ts.sample_rate, 0)) as sample_rate,
                    t.duration_ms,
                    MAX(COALESCE(ts.bitrate, 0)) as bitrate,
                    MAX(d.file_size_bytes) as file_size_bytes,
                    MAX(d.file_path) as file_path,
                    (
                        CASE WHEN t.title IS NOT NULL AND t.title != '' THEN 10 ELSE 0 END +
                        CASE WHEN EXISTS(SELECT 1 FROM track_artists WHERE track_id = t.id) THEN 10 ELSE 0 END +
                        CASE WHEN t.album_id IS NOT NULL THEN 10 ELSE 0 END +
                        CASE WHEN t.isrc IS NOT NULL AND t.isrc != '' THEN 20 ELSE 0 END +
                        CASE WHEN t.musicbrainz_id IS NOT NULL AND t.musicbrainz_id NOT IN ('NOT_FOUND', 'MISMATCH') THEN 20 ELSE 0 END +
                        CASE WHEN t.release_year IS NOT NULL AND t.release_year > 0 THEN 10 ELSE 0 END +
                        CASE WHEN t.genre IS NOT NULL AND t.genre != '' THEN 10 ELSE 0 END
                    ) as metadata_score,
                    (SELECT COUNT(*) FROM track_sources WHERE track_id = t.id) as source_count,
                    t.album_id
                FROM tracks t
                LEFT JOIN track_sources ts ON t.id = ts.track_id
                LEFT JOIN downloads d ON t.id = d.track_id
                WHERE t.id = ?
                GROUP BY t.id
                """,
                (tid,)
            )
            row = cur.fetchone()
            if row:
                track_infos.append({
                    "id": row[0],
                    "has_isrc": bool(row[1]),
                    "quality_score": row[2] or 0,
                    "bit_depth": row[3] or 0,
                    "sample_rate": row[4] or 0,
                    "duration_ms": row[5] or 0,
                    "bitrate": row[6] or 0,
                    "file_size_bytes": row[7] or 0,
                    "has_file": row[8] is not None,
                    "metadata_score": row[9],
                    "source_count": row[10],
                    "album_id": row[11],
                })

        if len(track_infos) <= 1:
            continue

        # Sort by survivor hierarchy:
        # has_isrc > has_file > quality_score > 24-bit > sample_rate > duration_ms > bitrate > metadata_score > source_count > id
        track_infos.sort(key=lambda x: (
            x["has_isrc"],
            x["has_file"],
            x["quality_score"],
            x["bit_depth"],
            x["sample_rate"],
            x["duration_ms"],
            x["bitrate"],
            x["metadata_score"],
            x["source_count"],
            x["file_size_bytes"],
            x["id"]
        ))

        winner = track_infos[-1]
        winner_id = winner["id"]
        if winner["album_id"]:
            affected_albums.add(winner["album_id"])

        for info in track_infos[:-1]:
            loser_id = info["id"]
            if info["album_id"]:
                affected_albums.add(info["album_id"])

            # Fetch loser metadata for backfill
            cur.execute(
                """
                SELECT
                    album_id, duration_ms, track_number, disc_number,
                    isrc, musicbrainz_id, genre, subgenre,
                    release_year, record_label, bpm, musical_key,
                    spotify_id, qobuz_id
                FROM tracks WHERE id = ?
                """,
                (loser_id,)
            )
            loser_row = cur.fetchone()
            if loser_row:
                # Clear unique constrained columns on loser before backfilling onto winner
                cur.execute(
                    "UPDATE tracks SET isrc = NULL, spotify_id = NULL, qobuz_id = NULL, musicbrainz_id = NULL WHERE id = ?",
                    (loser_id,)
                )

                # Backfill metadata on winner from loser
                cur.execute(
                    """
                    UPDATE tracks
                    SET
                        album_id = COALESCE(tracks.album_id, ?),
                        duration_ms = COALESCE(tracks.duration_ms, ?),
                        track_number = COALESCE(tracks.track_number, ?),
                        disc_number = COALESCE(tracks.disc_number, ?),
                        isrc = COALESCE(tracks.isrc, ?),
                        musicbrainz_id = COALESCE(tracks.musicbrainz_id, ?),
                        genre = COALESCE(tracks.genre, ?),
                        subgenre = COALESCE(tracks.subgenre, ?),
                        release_year = COALESCE(tracks.release_year, ?),
                        record_label = COALESCE(tracks.record_label, ?),
                        bpm = COALESCE(tracks.bpm, ?),
                        musical_key = COALESCE(tracks.musical_key, ?),
                        spotify_id = COALESCE(tracks.spotify_id, ?),
                        qobuz_id = COALESCE(tracks.qobuz_id, ?)
                    WHERE tracks.id = ?
                    """,
                    (*loser_row, winner_id)
                )

            # Preserve favorite status
            cur.execute(
                """
                UPDATE tracks SET is_favorite = 1
                WHERE id = ? AND EXISTS (SELECT 1 FROM tracks WHERE id = ? AND is_favorite = 1)
                """,
                (winner_id, loser_id)
            )

            # 1. Transfer track_sources respecting UNIQUE(track_id, service_id)
            cur.execute(
                """
                SELECT id, service_id, service_track_id, format, bit_depth, sample_rate, bitrate, quality_score
                FROM track_sources
                WHERE track_id = ?
                """,
                (loser_id,)
            )
            loser_sources = cur.fetchall()

            for ls_id, s_id, st_id, fmt, bd, sr, br, qs in loser_sources:
                cur.execute(
                    "SELECT id, quality_score FROM track_sources WHERE track_id = ? AND service_id = ?",
                    (winner_id, s_id)
                )
                winner_src = cur.fetchone()
                if not winner_src:
                    cur.execute("UPDATE track_sources SET track_id = ? WHERE id = ?", (winner_id, ls_id))
                else:
                    ws_id, ws_qs = winner_src
                    if (qs or 0) > (ws_qs or 0):
                        cur.execute(
                            """
                            UPDATE track_sources
                            SET service_track_id = ?, format = COALESCE(?, format),
                                bit_depth = COALESCE(?, bit_depth), sample_rate = COALESCE(?, sample_rate),
                                bitrate = COALESCE(?, bitrate), quality_score = ?
                            WHERE id = ?
                            """,
                            (st_id, fmt, bd, sr, br, qs, ws_id)
                        )
                    cur.execute("DELETE FROM track_sources WHERE id = ?", (ls_id,))

            # 2. Transfer playlist_tracks
            cur.execute("UPDATE playlist_tracks SET track_id = ? WHERE track_id = ?", (winner_id, loser_id))

            # 3. Transfer downloads
            cur.execute("SELECT COUNT(*) FROM downloads WHERE track_id = ?", (winner_id,))
            if cur.fetchone()[0] == 0:
                cur.execute("UPDATE downloads SET track_id = ? WHERE track_id = ?", (winner_id, loser_id))
            else:
                cur.execute("DELETE FROM downloads WHERE track_id = ?", (loser_id,))

            # 4. Transfer lyrics
            cur.execute("UPDATE OR IGNORE lyrics SET track_id = ? WHERE track_id = ?", (winner_id, loser_id))
            cur.execute("DELETE FROM lyrics WHERE track_id = ?", (loser_id,))

            # 5. Transfer library_entries
            cur.execute("UPDATE OR IGNORE library_entries SET track_id = ? WHERE track_id = ?", (winner_id, loser_id))
            cur.execute("DELETE FROM library_entries WHERE track_id = ?", (loser_id,))

            # 6. Transfer track_credits
            cur.execute("UPDATE OR IGNORE track_credits SET track_id = ? WHERE track_id = ?", (winner_id, loser_id))
            cur.execute("DELETE FROM track_credits WHERE track_id = ?", (loser_id,))

            # 7. Transfer track_artists
            cur.execute("UPDATE OR IGNORE track_artists SET track_id = ? WHERE track_id = ?", (winner_id, loser_id))
            cur.execute("DELETE FROM track_artists WHERE track_id = ?", (loser_id,))

            # 8. Transfer download_queue
            cur.execute("UPDATE OR IGNORE download_queue SET track_id = ? WHERE track_id = ?", (winner_id, loser_id))
            cur.execute("DELETE FROM download_queue WHERE track_id = ?", (loser_id,))

            # 9. Transfer enrichment_progress
            cur.execute("UPDATE OR IGNORE enrichment_progress SET track_id = ? WHERE track_id = ?", (winner_id, loser_id))
            cur.execute("DELETE FROM enrichment_progress WHERE track_id = ?", (loser_id,))

            # 10. Transfer operation_journal
            if has_op_journal:
                cur.execute("UPDATE OR IGNORE operation_journal SET track_id = ? WHERE track_id = ?", (winner_id, loser_id))

            # Delete loser track
            cur.execute("DELETE FROM tracks WHERE id = ?", (loser_id,))
            tracks_removed_count += 1

        groups_resolved_count += 1

    # Recompact track numbering in affected albums
    for album_id in affected_albums:
        cur.execute(
            "SELECT DISTINCT COALESCE(disc_number, 1) FROM tracks WHERE album_id = ? ORDER BY 1",
            (album_id,)
        )
        discs = [r[0] for r in cur.fetchall()]

        for disc in discs:
            cur.execute(
                """
                SELECT id FROM tracks
                WHERE album_id = ? AND COALESCE(disc_number, 1) = ?
                ORDER BY track_number ASC, id ASC
                """,
                (album_id, disc)
            )
            track_ids = [r[0] for r in cur.fetchall()]
            for idx, tid in enumerate(track_ids, start=1):
                cur.execute(
                    "UPDATE tracks SET track_number = ? WHERE id = ? AND track_number != ?",
                    (idx, tid, idx)
                )

    conn.commit()
    elapsed = time.time() - t0
    print(f"[TASK-104] Merge completed in {elapsed:.2f}s:")
    print(f"  - Duplicate groups resolved: {groups_resolved_count}")
    print(f"  - Redundant tracks removed: {tracks_removed_count}")
    print(f"  - Albums recompacted: {len(affected_albums)}")

    # Verification Checks
    fk_errors = cur.execute("PRAGMA foreign_key_check").fetchall()
    if fk_errors:
        print(f"[TASK-104] ERROR: Foreign key check failed: {fk_errors}", file=sys.stderr)
        conn.close()
        return False

    integrity = cur.execute("PRAGMA integrity_check").fetchone()[0]
    if integrity != "ok":
        print(f"[TASK-104] ERROR: Integrity check failed: {integrity}", file=sys.stderr)
        conn.close()
        return False

    total_tracks_post = cur.execute("SELECT COUNT(*) FROM tracks").fetchone()[0]
    total_sources_post = cur.execute("SELECT COUNT(*) FROM track_sources").fetchone()[0]
    numbering_collisions_post = cur.execute(collision_query).fetchone()[0]

    print("[TASK-104] Post-merge verification:")
    print(f"  - Total tracks: {total_tracks_post} (purged {total_tracks_pre - total_tracks_post})")
    print(f"  - Total track sources: {total_sources_post}")
    print(f"  - Numbering collisions: {numbering_collisions_post} (pre: {numbering_collisions_pre})")
    print(f"  - PRAGMA foreign_key_check: {len(fk_errors)} errors")
    print(f"  - PRAGMA integrity_check: {integrity}")

    conn.close()

    assert len(fk_errors) == 0, f"Foreign key errors found: {len(fk_errors)}"
    assert integrity == "ok", f"Integrity check failed: {integrity}"
    assert numbering_collisions_post <= numbering_collisions_pre, "Numbering collisions must decrease or stay 0"

    print("[TASK-104] Merge and renumbering verified successfully.")
    return True


def main():
    args = parse_args()
    success = run_merge(
        db_path=args.db_path,
        backup_dir=args.backup_dir,
        dry_run=args.dry_run,
    )
    sys.exit(0 if success else 1)


if __name__ == "__main__":
    main()

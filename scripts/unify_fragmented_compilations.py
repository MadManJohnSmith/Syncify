#!/usr/bin/env python3
"""
Syncify Fragmented Compilations Unification & Maintenance Script (TASK-136)
===========================================================================
Identifies and unifies fragmented compilation albums that share a normalized
title but were split across multiple single-track (or partial) album stubs
with disparate album_artists.

Remediation:
1. Creates a safety snapshot (VACUUM INTO) prior to any mutation.
2. Identifies candidate groups sharing the same normalized title.
3. Distinguishes multi-artist fragmented compilations from legitimate
   homonymous mono-artist releases (e.g. Queen 'Greatest Hits' vs The Cure 'Greatest Hits').
4. Selects canonical winning album (is_compilation, highest track count, oldest ID).
5. Repoints all tracks to the winner album, assigns album_artist to canonical 'Various Artists' (id 30698),
   marks albums.is_compilation = 1, recompacts track_number sequentially per disc, and purges empty loser albums.
6. Enforces PRAGMA foreign_key_check = 0 and PRAGMA integrity_check = ok.

Usage:
    python3 scripts/unify_fragmented_compilations.py [--db-path PATH] [--backup-dir DIR] [--dry-run]
"""

import argparse
import os
import re
import shutil
import sqlite3
import sys
import time
from typing import Dict, List, Optional, Set, Tuple

DEFAULT_DB_CANDIDATES = [
    os.path.expanduser("~/.local/share/com.syncify.app/syncify.db"),
    os.path.abspath("src-tauri/syncify.db"),
    os.path.abspath("syncify.db"),
    os.path.abspath("workspace/audit_archive/data/syncify.db"),
]

VA_VARIANTS = {
    "various artists",
    "various artist",
    "various interprets",
    "various interpret",
    "unknown artist",
    "unknown",
    "v.a.",
    "va",
    "v/a",
    "various",
    "verschiedene interpreten",
    "divers interprètes",
    "divers interpretes",
}

COMPILATION_KEYWORDS = {
    "soundtrack",
    "ost",
    "compilation",
    "various",
    "top hits",
    "greatest hits of",
    "best of 19",
    "best of 20",
    "vol.",
    "volume",
    "najlepszych",
    "hits",
    "summer",
    "party",
    "collection",
    "anthology",
    "tribute",
}


def find_default_db() -> str:
    for path in DEFAULT_DB_CANDIDATES:
        if os.path.exists(path):
            return path
    return DEFAULT_DB_CANDIDATES[0]


def parse_args():
    parser = argparse.ArgumentParser(
        description="Unify fragmented compilation albums under 'Various Artists' in Syncify SQLite DB."
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
    backup_path = os.path.join(backup_dir, f"syncify_backup_pre_repair_TASK-136_{timestamp}.db")
    os.makedirs(backup_dir, exist_ok=True)
    print(f"[TASK-136] Creating safety snapshot at {backup_path}...")

    try:
        src_conn = sqlite3.connect(f"file:{os.path.abspath(db_path)}?mode=ro", uri=True)
        src_conn.execute(f"VACUUM INTO '{backup_path}'")
        src_conn.close()
        print(f"[TASK-136] Safety snapshot created via VACUUM INTO: {backup_path}")
        return backup_path
    except Exception as e:
        print(f"[TASK-136] VACUUM INTO fallback ({e}), attempting file copy...")

    shutil.copy2(db_path, backup_path)
    print(f"[TASK-136] Safety snapshot created via copy: {backup_path}")
    return backup_path


def normalize_title(title: Optional[str]) -> str:
    if not title:
        return ""
    # Standardize whitespace and lowercase
    cleaned = re.sub(r"\s+", " ", title.strip().lower())
    return cleaned


def is_various_artists_name(name: Optional[str]) -> bool:
    if not name:
        return False
    return name.strip().lower() in VA_VARIANTS


def get_canonical_va_id(cur: sqlite3.Cursor, create_if_missing: bool = True) -> Optional[int]:
    cur.execute("SELECT id FROM artists WHERE LOWER(TRIM(name)) = 'various artists' ORDER BY id ASC LIMIT 1")
    row = cur.fetchone()
    if row:
        return row[0]

    if not create_if_missing:
        return None

    # Try inserting with canonical 30698 if available, or fallback
    try:
        cur.execute("INSERT OR IGNORE INTO artists (id, name) VALUES (30698, 'Various Artists')")
    except Exception:
        pass

    cur.execute("SELECT id FROM artists WHERE LOWER(TRIM(name)) = 'various artists' ORDER BY id ASC LIMIT 1")
    row = cur.fetchone()
    if row:
        return row[0]

    cur.execute("INSERT INTO artists (name) VALUES ('Various Artists')")
    return cur.lastrowid


def is_fragmented_compilation_group(
    norm_title: str,
    album_rows: List[Dict],
    album_artists_map: Dict[int, List[Tuple[int, str]]],
    track_artists_map: Dict[int, List[Tuple[int, str]]],
    tracks_per_album: Dict[int, List[Dict]],
) -> bool:
    """
    Decides if a set of albums sharing the same normalized title is a fragmented compilation
    or legitimate distinct mono-artist albums (e.g. Queen 'Greatest Hits' vs The Cure 'Greatest Hits').
    """
    if len(album_rows) < 2:
        return False

    # 1. If any album is explicitly marked as compilation
    if any(a.get("is_compilation", 0) == 1 for a in album_rows):
        return True

    # 2. If any album has Various Artists (or variant) as album artist
    for aid in album_artists_map:
        for _arid, arname in album_artists_map[aid]:
            if is_various_artists_name(arname):
                return True

    # 3. Collect distinct primary artist names across all tracks in the group
    all_track_artists: Set[str] = set()
    all_album_artists: Set[str] = set()

    for aid, alist in album_artists_map.items():
        for _arid, arname in alist:
            if arname and arname.strip():
                all_album_artists.add(arname.strip().lower())

    for aid, tlist in track_artists_map.items():
        for _arid, arname in tlist:
            if arname and arname.strip():
                all_track_artists.add(arname.strip().lower())

    # If all tracks and albums across the entire group share the same single artist, it's not a multi-artist compilation
    # (could be duplicate mono-artist album stubs, but let's check artist diversity)
    combined_artists = all_track_artists.union(all_album_artists)
    if len(combined_artists) <= 1:
        # Mono-artist with exact same artist: duplicates handled by standard dedup, not VA compilation
        return False

    # 4. Check if any single album already contains multiple distinct track artists
    for aid, tlist in track_artists_map.items():
        distinct_in_album = {arname.strip().lower() for _arid, arname in tlist if arname and arname.strip()}
        if len(distinct_in_album) > 1:
            return True

    # 5. Distinguish legitimate homonymous multi-track albums:
    # If all albums in the group are coherent mono-artist multi-track releases (e.g. > 3 tracks each,
    # where all tracks belong to that album's respective artist), they are distinct albums!
    all_coherent_mono = True
    for a in album_rows:
        aid = a["id"]
        t_count = len(tracks_per_album.get(aid, []))
        # An album fragment with 1 or 2 tracks is typical of fragmented compilations
        if t_count <= 2:
            all_coherent_mono = False
            break

        # Check if this album's tracks have divergent artists
        album_t_artists = {arname.strip().lower() for _arid, arname in track_artists_map.get(aid, [])}
        if len(album_t_artists) != 1:
            all_coherent_mono = False
            break

    if all_coherent_mono and len(album_rows) >= 2:
        # All albums are multi-track mono-artist albums by different artists (Queen vs The Cure)
        return False

    # 6. Check compilation keywords in normalized title
    has_comp_keyword = any(kw in norm_title for kw in COMPILATION_KEYWORDS)
    if has_comp_keyword:
        return True

    # 7. If multiple single-track fragments exist by different artists, this is the classic
    # 1.280 fragmented compilations symptom (e.g. 'Late Night Tales', '50 najlepszych polskich piosenek')
    fragment_count = sum(1 for a in album_rows if len(tracks_per_album.get(a["id"], [])) <= 2)
    if fragment_count >= 2:
        return True

    return False


def run_unification(db_path: str, backup_dir: str = "/tmp", dry_run: bool = False) -> bool:
    if not os.path.exists(db_path):
        print(f"Error: Database not found at {db_path}", file=sys.stderr)
        return False

    print(f"[TASK-136] Target database: {db_path}")

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

    # Check is_compilation column on albums
    album_cols = [col[1] for col in cur.execute("PRAGMA table_info(albums)").fetchall()]
    has_is_compilation = "is_compilation" in album_cols
    if not has_is_compilation:
        if not dry_run:
            print("[TASK-136] Adding is_compilation column to albums...")
            cur.execute("ALTER TABLE albums ADD COLUMN is_compilation INTEGER NOT NULL DEFAULT 0")
            cur.execute("CREATE INDEX IF NOT EXISTS idx_albums_is_compilation ON albums(is_compilation)")
            has_is_compilation = True
        else:
            print("[TASK-136] [DRY RUN] Note: is_compilation column would be added.")

    canonical_va_id = get_canonical_va_id(cur, create_if_missing=(not dry_run))
    print(f"[TASK-136] Canonical 'Various Artists' ID: {canonical_va_id or 'NOT FOUND (will create on write)'}")

    # Step 1: Discover candidate album groups with same normalized title
    cur.execute("""
        SELECT LOWER(TRIM(title)) AS norm_title, COUNT(*) AS cnt
        FROM albums
        WHERE title IS NOT NULL AND TRIM(title) != ''
        GROUP BY LOWER(TRIM(title))
        HAVING COUNT(*) > 1
    """)
    candidate_title_rows = cur.fetchall()
    print(f"[TASK-136] Found {len(candidate_title_rows)} title groups with multiple album rows.")

    compilation_groups_to_unify = []

    for norm_title, cnt in candidate_title_rows:
        if has_is_compilation:
            select_sql = """
                SELECT id, title, release_date, total_tracks, cover_art_url, is_compilation
                FROM albums
                WHERE LOWER(TRIM(title)) = ?
                ORDER BY is_compilation DESC, COALESCE(total_tracks, 0) DESC, id ASC
            """
        else:
            select_sql = """
                SELECT id, title, release_date, total_tracks, cover_art_url, 0 AS is_compilation
                FROM albums
                WHERE LOWER(TRIM(title)) = ?
                ORDER BY COALESCE(total_tracks, 0) DESC, id ASC
            """
        cur.execute(select_sql, (norm_title,))
        album_cols_list = ["id", "title", "release_date", "total_tracks", "cover_art_url", "is_compilation"]
        raw_albums = cur.fetchall()
        albums = [dict(zip(album_cols_list, r)) for r in raw_albums]

        album_artists_map: Dict[int, List[Tuple[int, str]]] = {}
        track_artists_map: Dict[int, List[Tuple[int, str]]] = {}
        tracks_per_album: Dict[int, List[Dict]] = {}

        for a in albums:
            aid = a["id"]
            # Album artists
            cur.execute("""
                SELECT ar.id, ar.name
                FROM artists ar
                JOIN album_artists aa ON aa.artist_id = ar.id
                WHERE aa.album_id = ?
            """, (aid,))
            album_artists_map[aid] = cur.fetchall()

            # Tracks
            cur.execute("""
                SELECT id, title, track_number, disc_number
                FROM tracks
                WHERE album_id = ?
                ORDER BY COALESCE(disc_number, 1) ASC, CASE WHEN track_number IS NOT NULL AND track_number > 0 THEN track_number ELSE 999999 END ASC, id ASC
            """, (aid,))
            tracks = [dict(zip(["id", "title", "track_number", "disc_number"], tr)) for tr in cur.fetchall()]
            tracks_per_album[aid] = tracks

            # Track artists
            cur.execute("""
                SELECT DISTINCT ar.id, ar.name
                FROM artists ar
                JOIN track_artists ta ON ta.artist_id = ar.id
                JOIN tracks t ON t.id = ta.track_id
                WHERE t.album_id = ?
            """, (aid,))
            track_artists_map[aid] = cur.fetchall()

        if is_fragmented_compilation_group(
            norm_title, albums, album_artists_map, track_artists_map, tracks_per_album
        ):
            # Select canonical winner:
            # 1. is_compilation == 1
            # 2. most tracks currently assigned
            # 3. highest declared total_tracks
            # 4. lowest id
            sorted_albums = sorted(
                albums,
                key=lambda x: (
                    x.get("is_compilation", 0),
                    len(tracks_per_album.get(x["id"], [])),
                    x.get("total_tracks") or 0,
                    -x["id"],
                ),
                reverse=True,
            )
            winner = sorted_albums[0]
            losers = sorted_albums[1:]

            compilation_groups_to_unify.append({
                "norm_title": norm_title,
                "winner": winner,
                "losers": losers,
                "tracks_per_album": tracks_per_album,
                "album_artists_map": album_artists_map,
            })

    print(f"\n[TASK-136] Identified {len(compilation_groups_to_unify)} fragmented compilation groups to unify.")
    total_loser_albums = sum(len(g["losers"]) for g in compilation_groups_to_unify)
    total_tracks_to_repoint = sum(
        sum(len(g["tracks_per_album"].get(l["id"], [])) for l in g["losers"])
        for g in compilation_groups_to_unify
    )
    print(f"[TASK-136] Total loser album rows to purge: {total_loser_albums}")
    print(f"[TASK-136] Total tracks to repoint to canonical winners: {total_tracks_to_repoint}")

    # Preview sample groups
    sample_preview = compilation_groups_to_unify[:5]
    for idx, g in enumerate(sample_preview, 1):
        w = g["winner"]
        l_ids = [l["id"] for l in g["losers"]]
        print(f"  Sample #{idx}: '{g['norm_title']}' -> Winner ID: {w['id']} ({len(g['tracks_per_album'].get(w['id'], []))} trks), Loser IDs: {l_ids}")

    if dry_run:
        print("\n[TASK-136] [DRY RUN] Complete. No modifications made to database.")
        conn.close()
        return True

    print("\n--- Executing Unification Transaction ---")
    canonical_va_id = get_canonical_va_id(cur, create_if_missing=True)

    try:
        cur.execute("BEGIN IMMEDIATE")

        repointed_track_count = 0
        purged_album_count = 0

        for g in compilation_groups_to_unify:
            winner = g["winner"]
            losers = g["losers"]
            winner_id = winner["id"]
            loser_ids = [l["id"] for l in losers]

            # A. Mark winner as compilation
            if has_is_compilation:
                cur.execute("UPDATE albums SET is_compilation = 1 WHERE id = ?", (winner_id,))

            # B. Associate winner album with canonical Various Artists as primary artist
            cur.execute(
                "UPDATE album_artists SET is_primary = 0 WHERE album_id = ? AND artist_id != ?",
                (winner_id, canonical_va_id),
            )
            cur.execute(
                "INSERT OR REPLACE INTO album_artists (album_id, artist_id, is_primary) VALUES (?, ?, 1)",
                (winner_id, canonical_va_id),
            )

            # C. Repoint tracks from loser albums to winner album
            loser_placeholders = ",".join("?" for _ in loser_ids)
            cur.execute(
                f"UPDATE tracks SET album_id = ? WHERE album_id IN ({loser_placeholders})",
                [winner_id] + loser_ids,
            )
            repointed_track_count += cur.rowcount

            # D. Recompact track numbers sequentially per disc
            cur.execute("""
                SELECT id, track_number, disc_number
                FROM tracks
                WHERE album_id = ?
                ORDER BY COALESCE(disc_number, 1) ASC,
                         CASE WHEN track_number IS NOT NULL AND track_number > 0 THEN track_number ELSE 999999 END ASC,
                         id ASC
            """, (winner_id,))
            all_winner_tracks = cur.fetchall()

            # Group tracks by disc
            tracks_by_disc: Dict[int, List[int]] = {}
            for tid, _tnum, dnum in all_winner_tracks:
                effective_disc = dnum if dnum and dnum > 0 else 1
                tracks_by_disc.setdefault(effective_disc, []).append(tid)

            for disc_num, track_ids in tracks_by_disc.items():
                for seq_idx, tid in enumerate(track_ids, 1):
                    cur.execute(
                        "UPDATE tracks SET track_number = ?, disc_number = ? WHERE id = ?",
                        (seq_idx, disc_num, tid),
                    )

            # E. Update winner's total_tracks
            new_total = len(all_winner_tracks)
            cur.execute(
                "UPDATE albums SET total_tracks = MAX(COALESCE(total_tracks, 0), ?) WHERE id = ?",
                (new_total, winner_id),
            )

            # F. Purge loser album_artists and loser albums
            cur.execute(
                f"DELETE FROM album_artists WHERE album_id IN ({loser_placeholders})",
                loser_ids,
            )
            cur.execute(
                f"""
                DELETE FROM albums
                WHERE id IN ({loser_placeholders})
                  AND id NOT IN (SELECT DISTINCT album_id FROM tracks WHERE album_id IS NOT NULL)
                """,
                loser_ids,
            )
            purged_album_count += cur.rowcount

        print(f"[TASK-136] Tracks repointed: {repointed_track_count}")
        print(f"[TASK-136] Loser albums purged: {purged_album_count}")

        # Post-execution validation
        print("\n--- Validating Database Referential Integrity ---")
        fk_violations = cur.execute("PRAGMA foreign_key_check").fetchall()
        if fk_violations:
            print(f"[TASK-136] ERROR: Foreign key check failed with {len(fk_violations)} violations!", file=sys.stderr)
            for v in fk_violations[:10]:
                print(f"  Violation: {v}", file=sys.stderr)
            conn.rollback()
            conn.close()
            return False
        print("[TASK-136] PRAGMA foreign_key_check: PASSED (0 violations)")

        integrity_row = cur.execute("PRAGMA integrity_check").fetchone()
        if not integrity_row or integrity_row[0] != "ok":
            print(f"[TASK-136] ERROR: Integrity check failed: {integrity_row}", file=sys.stderr)
            conn.rollback()
            conn.close()
            return False
        print("[TASK-136] PRAGMA integrity_check: PASSED (ok)")

        conn.commit()
        print("[TASK-136] Transaction committed successfully.")
        conn.close()
        return True

    except Exception as e:
        print(f"[TASK-136] ERROR during execution: {e}", file=sys.stderr)
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

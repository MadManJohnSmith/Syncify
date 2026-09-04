#!/usr/bin/env python3
"""
scripts/backfill_featured_artists.py
====================================
Backfill script for Task F4.3 (ref: M3):
Detect featured artists in track titles using regex and populate `track_artists`
with `role = 'featured'`.

Requirements:
- Parse patterns:
    * `(feat. Artist)` or `[feat. Artist]`
    * `(ft. Artist)` or `[ft. Artist]`
    * `(featuring Artist)` or `[featuring Artist]`
    * `feat. Artist` at the end or before dash
    * Multiple artists separated by ',', '&', or 'and'
- Avoid false positives like "BIRDS OF A FEATHER", "Light as a Feather",
  or "as featured in ...".
- Search or create artist in `artists`.
- Insert into `track_artists (track_id, artist_id, role)` with `role = 'featured'`.
- Enforce `PRAGMA foreign_keys = ON; BEGIN TRANSACTION; ... COMMIT;`.
"""

import argparse
import os
import re
import sqlite3
import sys
from typing import List, Tuple, Dict, Any


FEAT_KEYWORD_PATTERN = r'(?:\bfeaturing\b|\bfeat\b\.?|\bft\b\.?)'

BRACKET_REGEX = re.compile(
    r'[\(\[\{](?:[^\)\]\}]*?\b)?' + FEAT_KEYWORD_PATTERN + r'\s*([^\)\]\}]+)[\)\]\}]',
    re.IGNORECASE,
)

BARE_REGEX = re.compile(
    r'(?:^|[\s_])' + FEAT_KEYWORD_PATTERN + r'\s*([^\-]+?)(?:\s+-\s+.*|$)',
    re.IGNORECASE,
)

AS_FEATURED_REGEX = re.compile(r'\bas\s+featured\s+in\b', re.IGNORECASE)
SPLIT_REGEX = re.compile(r'\s*(?:,\s*(?:and\s+)?|\s+and\s+|\s*&\s*)\s*', re.IGNORECASE)


def extract_featured_artists(title: str) -> List[str]:
    """Extract featured collaborating artists from a track title."""
    if not title:
        return []

    trimmed = title.strip()
    if not trimmed:
        return []

    # Exclude soundtrack notes like "as featured in..."
    if AS_FEATURED_REGEX.search(trimmed):
        return []

    raw = None
    m = BRACKET_REGEX.search(trimmed)
    if m:
        raw = m.group(1)
    else:
        m = BARE_REGEX.search(trimmed)
        if m:
            raw = m.group(1)

    if not raw:
        return []

    raw_text = raw.strip()
    if not raw_text:
        return []

    # Protect known multi-word artist names containing internal commas
    protected = re.sub(r'Tyler,\s*The\s*Creator', 'Tyler__COMMA_SPACE__The Creator', raw_text, flags=re.IGNORECASE)

    tokens = SPLIT_REGEX.split(protected)
    results: List[str] = []

    for token in tokens:
        restored = token.replace('__COMMA_SPACE__', ', ').strip()
        # Clean leading "with " or "+ "
        cleaned = re.sub(r'^(?:with|\+)\s+', '', restored, flags=re.IGNORECASE).strip()
        # Clean quotes
        cleaned = cleaned.strip('\'"“”')
        cleaned = cleaned.strip()

        if cleaned and not any(existing.lower() == cleaned.lower() for existing in results):
            results.append(cleaned)

    return results


def run_backfill(db_path: str) -> Dict[str, Any]:
    print(f"[*] Opening database at: {db_path}")
    if not os.path.exists(db_path):
        raise FileNotFoundError(f"Database file not found: {db_path}")

    conn = sqlite3.connect(db_path)
    cur = conn.cursor()

    # Verify foreign keys
    cur.execute("PRAGMA foreign_keys = ON;")

    # Check pre-existing featured count
    pre_featured_count = cur.execute(
        "SELECT count(*) FROM track_artists WHERE role = 'featured'"
    ).fetchone()[0]
    print(f"[*] Pre-existing 'featured' entries in track_artists: {pre_featured_count}")

    # Query candidate tracks with feat/ft/featuring in title
    tracks = cur.execute(
        "SELECT id, title FROM tracks WHERE lower(title) LIKE '%feat%' OR lower(title) LIKE '%ft.%' OR lower(title) LIKE '%featuring%'"
    ).fetchall()
    print(f"[*] Candidate tracks with keyword in title: {len(tracks)}")

    cur.execute("BEGIN TRANSACTION;")

    tracks_with_featured = 0
    artists_found_existing = 0
    artists_created_new = 0
    links_inserted = 0
    links_skipped_already_linked = 0
    sample_links: List[Tuple[int, str, str]] = []

    for track_id, title in tracks:
        featured_artists = extract_featured_artists(title)
        if not featured_artists:
            continue

        tracks_with_featured += 1

        for artist_name in featured_artists:
            # 1. Look up existing artist (case-insensitive)
            art_row = cur.execute(
                "SELECT id, name FROM artists WHERE name = ? COLLATE NOCASE LIMIT 1",
                (artist_name,),
            ).fetchone()

            if art_row:
                artist_id = art_row[0]
                artists_found_existing += 1
            else:
                # 2. Insert new artist
                cur.execute("INSERT INTO artists (name) VALUES (?)", (artist_name,))
                artist_id = cur.lastrowid
                artists_created_new += 1

            # 3. Check if track_artist already linked (primary or featured)
            already_linked = cur.execute(
                "SELECT 1 FROM track_artists WHERE track_id = ? AND artist_id = ?",
                (track_id, artist_id),
            ).fetchone()

            if already_linked:
                links_skipped_already_linked += 1
                continue

            # 4. Insert into track_artists with role = 'featured'
            cur.execute(
                "INSERT INTO track_artists (track_id, artist_id, role) VALUES (?, ?, 'featured')",
                (track_id, artist_id),
            )
            links_inserted += 1

            if len(sample_links) < 15:
                sample_links.append((track_id, title, artist_name))

    conn.commit()

    # Validate FKs
    fk_violations = cur.execute("PRAGMA foreign_key_check;").fetchall()

    post_featured_count = cur.execute(
        "SELECT count(*) FROM track_artists WHERE role = 'featured'"
    ).fetchone()[0]

    print("\n" + "=" * 60)
    print("BACKFILL SUMMARY (F4.3 - Featured Artists Detection)")
    print("=" * 60)
    print(f"Candidate tracks inspected:        {len(tracks)}")
    print(f"Tracks with collaborations:        {tracks_with_featured}")
    print(f"Existing artists matched:          {artists_found_existing}")
    print(f"New artists inserted:              {artists_created_new}")
    print(f"Links skipped (already primary):   {links_skipped_already_linked}")
    print(f"Featured links inserted:           {links_inserted}")
    print(f"Total 'featured' rows post-repair: {post_featured_count}")
    print(f"Foreign key violations:            {len(fk_violations)}")
    print("-" * 60)
    print("Sample links created:")
    for tid, t_title, a_name in sample_links:
        print(f"  Track [{tid}] '{t_title}' -> Featured: '{a_name}'")
    print("=" * 60)

    conn.close()

    return {
        "db_path": db_path,
        "candidate_tracks": len(tracks),
        "tracks_with_featured": tracks_with_featured,
        "artists_found_existing": artists_found_existing,
        "artists_created_new": artists_created_new,
        "links_skipped_already_linked": links_skipped_already_linked,
        "links_inserted": links_inserted,
        "post_featured_count": post_featured_count,
        "fk_violations": len(fk_violations),
    }


def main():
    parser = argparse.ArgumentParser(description="Backfill featured artists into track_artists (F4.3)")
    parser.add_argument(
        "--db",
        default="syncify_backup_pre_repair.db",
        help="Path to database (default: syncify_backup_pre_repair.db)",
    )
    args = parser.parse_args()

    run_backfill(args.db)


if __name__ == "__main__":
    main()

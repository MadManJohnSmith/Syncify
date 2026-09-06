#!/usr/bin/env python3
"""
Syncify Featured Artists Backfill & Appearances Attribution Script (TASK-106)
=============================================================================
1. Creates safety snapshot using VACUUM INTO in /tmp/ (syncify_backup_pre_repair_TASK-106_<timestamp>.db).
2. Scans all tracks with featuring indicators: '(feat.', '(ft.', '(featuring', '[feat.', 'feat.', etc.
3. Excludes false positives ('BIRDS OF A FEATHER', 'as featured in', 'Feather', etc.).
4. Extracts clean title and list of guest artists.
5. Updates track title to clean title.
6. Ensures guest artists exist in `artists` table.
7. Links guest artists to `track_artists` with role = 'featured'.
8. Verifies database integrity and foreign key validity.

Usage:
    python3 scripts/backfill_featured_artists.py [--db-path PATH] [--backup-dir DIR] [--dry-run]
"""

import argparse
import os
import re
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

FEAT_KEYWORD_PATTERN = r"(?:\bfeaturing\b|\bfeat\b\.?|\bft\b\.?)"
BRACKET_FEAT_REGEX = re.compile(
    r"[\(\[\{](?:[^\)\]\}]*?\b)?" + FEAT_KEYWORD_PATTERN + r"\s*([^\)\]\}]+)[\)\]\}]",
    re.IGNORECASE,
)
BARE_FEAT_REGEX = re.compile(
    r"(?:^|[\s_])" + FEAT_KEYWORD_PATTERN + r"\s*([^\-]+?)(?:\s+-\s+.*|$)",
    re.IGNORECASE,
)
AS_FEATURED_REGEX = re.compile(r"(?i)\bas\s+featured\s+in\b")
SPLIT_SEPARATORS_REGEX = re.compile(r"(?i)\s*(?:,\s*(?:and\s+)?|\s+and\s+|\s*&\s*)\s*")


def find_default_db() -> str:
    for path in DEFAULT_DB_CANDIDATES:
        if os.path.exists(path):
            return path
    return DEFAULT_DB_CANDIDATES[0]


def parse_args():
    parser = argparse.ArgumentParser(
        description="Backfill featured artists and attribute appearances in Syncify SQLite DB (TASK-106)."
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
    backup_path = os.path.join(backup_dir, f"syncify_backup_pre_repair_TASK-106_{timestamp}.db")
    os.makedirs(backup_dir, exist_ok=True)
    print(f"[TASK-106] Creating safety snapshot at {backup_path}...")

    try:
        src_conn = sqlite3.connect(f"file:{os.path.abspath(db_path)}?mode=ro", uri=True)
        src_conn.execute(f"VACUUM INTO '{backup_path}'")
        src_conn.close()
        print(f"[TASK-106] Safety snapshot created via VACUUM INTO: {backup_path}")
        return backup_path
    except Exception as e:
        print(f"[TASK-106] VACUUM INTO fallback ({e}), attempting file copy...")

    shutil.copy2(db_path, backup_path)
    print(f"[TASK-106] Safety snapshot created via copy: {backup_path}")
    return backup_path


def clean_title_and_extract_featured(title: str):
    trimmed = title.strip()
    if not trimmed or AS_FEATURED_REGEX.search(trimmed):
        return trimmed, []

    # Check false positives
    lower = trimmed.lower()
    if lower in ("birds of a feather", "feather", "bloodfeather", "funny feathers", "light as a feather"):
        return trimmed, []

    m_bracket = BRACKET_FEAT_REGEX.search(trimmed)
    m_bare = BARE_FEAT_REGEX.search(trimmed)

    raw_capture = None
    cleaned = trimmed

    if m_bracket:
        raw_capture = m_bracket.group(1).strip()
        start, end = m_bracket.span(0)
        actual_start = start - 1 if start > 0 and trimmed[start - 1] == " " else start
        cleaned = (trimmed[:actual_start] + trimmed[end:]).strip()
    elif m_bare:
        raw_capture = m_bare.group(1).strip()
        start = m_bare.start(0)
        end = m_bare.end(1)
        cleaned = (trimmed[:start] + trimmed[end:]).strip()

    if not raw_capture:
        return trimmed, []

    # Clean up empty brackets and extra spaces
    cleaned = re.sub(r"\(\s*\)|\[\s*\]|\{\s*\}", "", cleaned)
    cleaned = " ".join(cleaned.split()).strip().rstrip(" -,").strip()
    if not cleaned:
        cleaned = trimmed

    # Split artists
    protected = (
        raw_capture
        .replace("Tyler, The Creator", "Tyler__COMMA__The Creator")
        .replace("Tyler, the Creator", "Tyler__COMMA__The Creator")
    )

    artists = []
    for token in SPLIT_SEPARATORS_REGEX.split(protected):
        restored = token.replace("__COMMA__", ", ").strip()
        if restored.lower().startswith("with "):
            restored = restored[5:].strip()
        elif restored.startswith("+"):
            restored = restored[1:].strip()
        restored = restored.strip("'\"“”`").strip()
        if restored and restored.lower() not in [a.lower() for a in artists]:
            artists.append(restored)

    return cleaned, artists


def run_backfill(db_path: str, backup_dir: str, dry_run: bool):
    if not os.path.exists(db_path):
        print(f"[TASK-106] Error: Database file not found at {db_path}", file=sys.stderr)
        return False

    print("=" * 70)
    print("SYNCIFY FEATURED ARTISTS & APPEARANCES BACKFILL (TASK-106)")
    print(f"Target Database: {db_path}")
    print(f"Dry Run: {dry_run}")
    print("=" * 70)

    if not dry_run:
        create_safety_backup(db_path, backup_dir)

    conn = sqlite3.connect(db_path)
    conn.execute("PRAGMA foreign_keys = ON")
    conn.execute("PRAGMA busy_timeout = 30000")
    cur = conn.cursor()

    # Query candidate tracks
    cur.execute(
        r"""
        SELECT id, title
        FROM tracks
        WHERE title LIKE '%feat%'
           OR title LIKE '%ft.%'
           OR title LIKE '%featuring%'
        ORDER BY id ASC
        """
    )
    rows = cur.fetchall()
    print(f"[TASK-106] Scanned {len(rows)} candidate tracks with featuring keywords.")

    tracks_cleaned = 0
    artists_created = 0
    featured_links_created = 0

    for track_id, title in rows:
        clean_title, feat_artists = clean_title_and_extract_featured(title)
        if not feat_artists:
            continue

        tracks_cleaned += 1

        if not dry_run:
            if clean_title != title:
                cur.execute("UPDATE tracks SET title = ? WHERE id = ?", (clean_title, track_id))

            for feat_name in feat_artists:
                feat_name_clean = feat_name.strip()
                if not feat_name_clean:
                    continue

                # Find or create artist
                cur.execute("SELECT id FROM artists WHERE LOWER(TRIM(name)) = LOWER(?) LIMIT 1", (feat_name_clean,))
                art_row = cur.fetchone()
                if art_row:
                    art_id = art_row[0]
                else:
                    cur.execute("INSERT INTO artists (name) VALUES (?)", (feat_name_clean,))
                    art_id = cur.lastrowid
                    artists_created += 1

                # Link into track_artists
                cur.execute(
                    "INSERT OR IGNORE INTO track_artists (track_id, artist_id, role) VALUES (?, ?, 'featured')",
                    (track_id, art_id),
                )
                if cur.rowcount > 0:
                    featured_links_created += 1
        else:
            featured_links_created += len(feat_artists)

    if not dry_run:
        conn.commit()
        print("[TASK-106] Transaction committed successfully.")

        # Integrity & FK checks
        cur.execute("PRAGMA foreign_key_check")
        fk_violations = cur.fetchall()
        if fk_violations:
            print(f"[TASK-106] WARNING: Foreign key violations detected: {fk_violations}", file=sys.stderr)
        else:
            print("[TASK-106] Foreign key check: 0 violations (OK).")

    conn.close()

    print("-" * 70)
    print("BACKFILL SUMMARY:")
    print(f"  Tracks parsed and cleaned:     {tracks_cleaned}")
    print(f"  New artists created:           {artists_created}")
    print(f"  Featured track_artist links:   {featured_links_created}")
    print("=" * 70)
    return True


if __name__ == "__main__":
    args = parse_args()
    success = run_backfill(args.db_path, args.backup_dir, args.dry_run)
    sys.exit(0 if success else 1)

#!/usr/bin/env python3
r"""
Canonical Disk Layout Normalization Engine - TASK-110

Resolves:
1. Normalizes album folders to canonical prefix `[{Year}] {Album}` (fixing non-prefixed folders).
2. Re-integrates Various Artists (VA) tracks to `Various Artists/[{Year}] {Album}/...`.
3. Sanitizes forbidden path characters (`:`, `"`, `/`, `\`, `|`, `?`, `*`) and collapses multiple consecutive spaces.
4. Atomically migrates physical files and updates `downloads.file_path` in SQLite.
5. Ensures `folder_settings.folder_template` defaults to `{AlbumArtist}/[{Year}] {Album}`.
6. Preserves the Symfonium invariant: CoverFront (0x03) = image/webp animated is never altered.

Safety:
- Captures safety snapshot via VACUUM INTO in /tmp/syncify_backup_pre_repair_TASK-110_<timestamp>.db.
- Supports --dry-run for complete non-destructive simulations.
"""

import argparse
import json
import os
import re
import shutil
import sqlite3
import sys
import time
from pathlib import Path
from typing import Any, Dict, List, Optional, Tuple

FORBIDDEN_CHARS_PATTERN = re.compile(r'[<>:"/\\|?*\x00-\x1f]')
MULTIPLE_SPACES_PATTERN = re.compile(r' {2,}')
WINDOWS_RESERVED = {
    "CON", "PRN", "AUX", "NUL",
    "COM1", "COM2", "COM3", "COM4", "COM5", "COM6", "COM7", "COM8", "COM9",
    "LPT1", "LPT2", "LPT3", "LPT4", "LPT5", "LPT6", "LPT7", "LPT8", "LPT9",
}

VA_INDICATORS = {"various artists", "various", "va", "v.a.", "v/a", "various artist"}


def is_various_artists(artist_name: Optional[str]) -> bool:
    if not artist_name:
        return False
    return artist_name.strip().lower() in VA_INDICATORS


def sanitize_filename(name: Optional[str]) -> str:
    """Strict Windows/Linux/macOS filename sanitization matching Rust layout engine."""
    if not name:
        return "Unknown"
    
    # 1. Replace forbidden characters and control characters with '_'
    s = FORBIDDEN_CHARS_PATTERN.sub('_', name)
    
    # 2. Collapse consecutive spaces to a single space
    s = MULTIPLE_SPACES_PATTERN.sub(' ', s)
    
    # 3. Trim whitespace and dots from ends
    trimmed = s.strip(' .')
    if not trimmed:
        return "Unknown"
    
    # 4. Protect Windows reserved device names
    if trimmed.upper() in WINDOWS_RESERVED:
        return f"{trimmed}_"
    
    return trimmed


def canonical_album_folder(album_title: str, year: Optional[int]) -> str:
    """Generates canonical album directory name: `[{Year}] {Album}` or `{Album}`."""
    safe_album = sanitize_filename(album_title)
    if year and 1900 <= year <= 2100:
        return f"[{year}] {safe_album}"
    return safe_album


def create_safety_backup(db_path: str, backup_dir: str) -> str:
    """Creates a safety database snapshot before any write operations."""
    timestamp = int(time.time())
    backup_path = os.path.join(backup_dir, f"syncify_backup_pre_repair_TASK-110_{timestamp}.db")
    os.makedirs(backup_dir, exist_ok=True)
    print(f"[TASK-110] Creating safety snapshot at {backup_path}...")

    abs_path = os.path.abspath(os.path.expanduser(db_path))

    # 1. Attempt VACUUM INTO
    try:
        src_conn = sqlite3.connect(abs_path)
        src_conn.execute(f"VACUUM INTO '{backup_path}'")
        src_conn.close()
        print(f"[TASK-110] Safety snapshot created successfully via VACUUM INTO: {backup_path}")
        return backup_path
    except Exception as e:
        print(f"[TASK-110] VACUUM INTO fallback ({e}), attempting backup API with immutable=1...")

    # 2. Attempt backup API with immutable=1
    try:
        src_conn = sqlite3.connect(f"file:{abs_path}?immutable=1", uri=True)
        dst_conn = sqlite3.connect(backup_path)
        src_conn.backup(dst_conn)
        dst_conn.close()
        src_conn.close()
        print(f"[TASK-110] Safety snapshot created via backup API: {backup_path}")
        return backup_path
    except Exception as e2:
        print(f"[TASK-110] Backup API fallback ({e2}), attempting copy...")

    # 3. File copy fallback
    shutil.copy2(abs_path, backup_path)
    print(f"[TASK-110] Safety snapshot created via file copy: {backup_path}")
    return backup_path


def compute_canonical_path(
    base_dir: Path,
    album_artist: str,
    track_artist: str,
    album_title: str,
    track_title: str,
    year: Optional[int],
    disc_number: int,
    total_discs: int,
    track_number: int,
    file_format: str,
) -> Path:
    """Computes the deterministic canonical path matching Rust `LibraryLayout::canonical_track_path`."""
    is_va = is_various_artists(album_artist)
    safe_artist = "Various Artists" if is_va else sanitize_filename(album_artist)
    album_dir_name = canonical_album_folder(album_title, year)
    
    target_dir = base_dir / safe_artist / album_dir_name
    if total_discs > 1:
        target_dir = target_dir / f"Disc {disc_number}"
        
    safe_title = sanitize_filename(track_title)
    ext = file_format.lower().lstrip('.')
    if not ext:
        ext = "flac"
        
    if is_va:
        safe_track_artist = sanitize_filename(track_artist)
        if not safe_track_artist or is_various_artists(safe_track_artist):
            filename = f"{track_number:02d} - {safe_title}.{ext}"
        else:
            filename = f"{track_number:02d} - {safe_track_artist} - {safe_title}.{ext}"
    else:
        filename = f"{track_number:02d} - {safe_title}.{ext}"
        
    return target_dir / filename


def normalize_disk_layout(
    db_path: str,
    music_dir: Optional[str] = None,
    dry_run: bool = True,
    backup_dir: str = "/tmp",
    verbose: bool = False,
) -> Dict[str, Any]:
    """Scans and migrates library disk layout and updates SQLite downloads ledger."""
    db_real_path = os.path.expanduser(db_path)
    if not os.path.exists(db_real_path):
        raise FileNotFoundError(f"SQLite database not found at {db_real_path}")

    abs_db_path = os.path.abspath(db_real_path)
    if dry_run:
        try:
            conn = sqlite3.connect(f"file:{abs_db_path}?immutable=1", uri=True)
            conn.row_factory = sqlite3.Row
            cursor = conn.cursor()
            cursor.execute("SELECT 1")
        except Exception:
            conn = sqlite3.connect(abs_db_path)
            conn.row_factory = sqlite3.Row
            cursor = conn.cursor()
    else:
        conn = sqlite3.connect(abs_db_path)
        conn.row_factory = sqlite3.Row
        cursor = conn.cursor()

    # Determine base music directory from settings or parameter
    cursor.execute("SELECT base_folder, folder_template FROM folder_settings WHERE id = 1")
    f_settings = cursor.fetchone()
    
    if music_dir:
        base_music_dir = Path(os.path.expanduser(music_dir)).resolve()
    elif f_settings and f_settings["base_folder"] and f_settings["base_folder"].strip():
        base_music_dir = Path(os.path.expanduser(f_settings["base_folder"].strip())).resolve()
    else:
        default_dir = os.path.expanduser("~/Music/Syncify")
        base_music_dir = Path(default_dir).resolve()

    print(f"[TASK-110] Music library base path: {base_music_dir}")
    print(f"[TASK-110] Operating mode: {'DRY-RUN (Simulated)' if dry_run else 'APPLY (Live Changes)'}")

    # Create safety backup if not dry-run
    backup_path = None
    if not dry_run:
        backup_path = create_safety_backup(db_real_path, backup_dir)

    # Fetch all download records with track, album, and artist metadata
    query = """
    SELECT 
        d.id as download_id,
        d.track_id,
        d.file_path,
        COALESCE(d.file_format, 'flac') as file_format,
        t.title as track_title,
        COALESCE(t.track_number, 1) as track_number,
        COALESCE(t.disc_number, 1) as disc_number,
        COALESCE(alb.title, 'Unknown Album') as album_title,
        alb.release_date,
        COALESCE(
            (SELECT art.name FROM track_artists ta JOIN artists art ON art.id = ta.artist_id WHERE ta.track_id = t.id ORDER BY CASE ta.role WHEN 'primary' THEN 1 WHEN 'main' THEN 2 ELSE 3 END, ta.artist_id ASC LIMIT 1),
            'Unknown Artist'
        ) as track_artist,
        COALESCE(
            (SELECT art.name FROM album_artists aa JOIN artists art ON art.id = aa.artist_id WHERE aa.album_id = t.album_id ORDER BY aa.is_primary DESC, aa.artist_id ASC LIMIT 1),
            (SELECT art.name FROM track_artists ta JOIN artists art ON art.id = ta.artist_id WHERE ta.track_id = t.id ORDER BY CASE ta.role WHEN 'primary' THEN 1 WHEN 'main' THEN 2 ELSE 3 END, ta.artist_id ASC LIMIT 1),
            'Unknown Artist'
        ) as album_artist,
        (SELECT COUNT(DISTINCT t2.disc_number) FROM tracks t2 WHERE t2.album_id = t.album_id) as total_discs
    FROM downloads d
    JOIN tracks t ON t.id = d.track_id
    LEFT JOIN albums alb ON alb.id = t.album_id
    WHERE d.file_path IS NOT NULL AND d.file_path != ''
    ORDER BY d.id ASC
    """
    cursor.execute(query)
    rows = cursor.fetchall()

    scanned_count = len(rows)
    migrated_count = 0
    va_reintegrated_count = 0
    year_prefix_added_count = 0
    sanitized_names_count = 0
    physical_moves_count = 0
    sidecars_moved_count = 0
    errors: List[str] = []
    actions_planned: List[Dict[str, Any]] = []

    for row in rows:
        dl_id = row["download_id"]
        current_path_str = row["file_path"]
        current_path = Path(current_path_str)

        # Parse release year
        rel_date = row["release_date"]
        year = None
        if rel_date:
            m = re.match(r"^(\d{4})", str(rel_date).strip())
            if m:
                year = int(m.group(1))

        album_artist = row["album_artist"]
        track_artist = row["track_artist"]
        album_title = row["album_title"]
        track_title = row["track_title"]
        disc_number = max(1, int(row["disc_number"] or 1))
        total_discs = max(1, int(row["total_discs"] or 1))
        try:
            track_num = int(str(row["track_number"]).split('/')[0])
        except (ValueError, AttributeError):
            track_num = 1
        file_format = row["file_format"]

        canonical_path = compute_canonical_path(
            base_dir=base_music_dir,
            album_artist=album_artist,
            track_artist=track_artist,
            album_title=album_title,
            track_title=track_title,
            year=year,
            disc_number=disc_number,
            total_discs=total_discs,
            track_number=track_num,
            file_format=file_format,
        )

        canonical_path_str = str(canonical_path)

        # Check if migration is necessary
        if current_path_str == canonical_path_str:
            continue

        is_va = is_various_artists(album_artist)
        had_year_prefix = bool(re.search(r'\[\d{4}\]', current_path_str))
        will_have_year_prefix = bool(re.search(r'\[\d{4}\]', canonical_path_str))

        if is_va and "Various Artists" not in current_path_str:
            va_reintegrated_count += 1
        if not had_year_prefix and will_have_year_prefix:
            year_prefix_added_count += 1
        if (":" in current_path_str or "  " in current_path_str) and ":" not in canonical_path_str and "  " not in canonical_path_str:
            sanitized_names_count += 1

        migrated_count += 1
        action_item = {
            "download_id": dl_id,
            "track_id": row["track_id"],
            "old_path": current_path_str,
            "new_path": canonical_path_str,
            "is_va": is_va,
            "year": year,
        }
        actions_planned.append(action_item)

        if verbose:
            print(f"[RENAME] DL-{dl_id}: '{current_path_str}' -> '{canonical_path_str}'")

        if not dry_run:
            try:
                # 1. Move physical audio file if it exists
                if current_path.exists():
                    canonical_path.parent.mkdir(parents=True, exist_ok=True)
                    shutil.move(str(current_path), str(canonical_path))
                    physical_moves_count += 1

                    # 2. Check and migrate matching sidecars (e.g. lyrics .lrc)
                    current_lrc = current_path.with_suffix(".lrc")
                    canonical_lrc = canonical_path.with_suffix(".lrc")
                    if current_lrc.exists():
                        shutil.move(str(current_lrc), str(canonical_lrc))
                        sidecars_moved_count += 1

                    # Clean empty parent directory if old folder is left deserted
                    old_parent = current_path.parent
                    try:
                        if old_parent.exists() and not any(old_parent.iterdir()):
                            old_parent.rmdir()
                    except Exception:
                        pass
                elif canonical_path.exists():
                    # Audio already at target, just needs database pointer fix
                    pass

                # 3. Update SQLite downloads ledger
                cursor.execute(
                    "UPDATE downloads SET file_path = ?, updated_at = CURRENT_TIMESTAMP WHERE id = ?",
                    (canonical_path_str, dl_id)
                )

            except Exception as e:
                err_msg = f"Failed to migrate DL-{dl_id} ('{current_path_str}'): {e}"
                errors.append(err_msg)
                print(f"[ERROR] {err_msg}", file=sys.stderr)

    # If applying live, ensure folder_settings has the canonical default template
    if not dry_run:
        try:
            cursor.execute(
                """
                UPDATE folder_settings
                SET folder_template = '{AlbumArtist}/[{Year}] {Album}',
                    updated_at = datetime('now')
                WHERE id = 1 AND (folder_template = '{AlbumArtist}/{Album}' OR folder_template IS NULL OR folder_template = '')
                """
            )
            conn.commit()
            print("[TASK-110] Transaction committed successfully in SQLite.")
        except Exception as e:
            conn.rollback()
            err_msg = f"Failed to commit database updates: {e}"
            errors.append(err_msg)
            print(f"[FATAL] {err_msg}", file=sys.stderr)
    else:
        conn.rollback()

    conn.close()

    result = {
        "success": len(errors) == 0,
        "dry_run": dry_run,
        "backup_snapshot": backup_path,
        "scanned_downloads": scanned_count,
        "planned_migrations": migrated_count,
        "year_prefix_folders_fixed": year_prefix_added_count,
        "va_orphans_reintegrated": va_reintegrated_count,
        "sanitized_paths_count": sanitized_names_count,
        "physical_audio_files_moved": physical_moves_count,
        "sidecars_moved": sidecars_moved_count,
        "errors": errors,
    }
    return result


def main():
    parser = argparse.ArgumentParser(
        description="Canonical Disk Layout Normalization Engine for Syncify (TASK-110)."
    )
    parser.add_argument(
        "--db-path", "--db", "-d",
        default="~/.local/share/com.syncify.app/syncify.db",
        help="Path to Syncify SQLite database (default: ~/.local/share/com.syncify.app/syncify.db)",
    )
    parser.add_argument(
        "--music-dir", "-m",
        default=None,
        help="Music library root directory (default: read from folder_settings or ~/Music/Syncify)",
    )
    parser.add_argument(
        "--dry-run",
        action="store_true",
        default=False,
        help="Simulate scan and report planned actions without modifying disk or database",
    )
    parser.add_argument(
        "--apply",
        action="store_true",
        default=False,
        help="Explicitly apply live migration changes to disk and database (takes safety snapshot)",
    )
    parser.add_argument(
        "--backup-dir",
        default="/tmp",
        help="Directory to store safety snapshot (default: /tmp)",
    )
    parser.add_argument(
        "--verbose", "-v",
        action="store_true",
        help="Print verbose logs for each planned/performed path migration",
    )
    parser.add_argument(
        "--json",
        action="store_true",
        help="Output results strictly formatted as JSON",
    )

    args = parser.parse_args()

    # Safety default: unless --apply is explicitly specified, default to dry-run
    dry_run = True if not args.apply else args.dry_run

    try:
        res = normalize_disk_layout(
            db_path=args.db_path,
            music_dir=args.music_dir,
            dry_run=dry_run,
            backup_dir=args.backup_dir,
            verbose=args.verbose,
        )

        if args.json:
            print(json.dumps(res, indent=2))
        else:
            print("=" * 70)
            print(f"CANONICAL DISK LAYOUT NORMALIZATION SUMMARY {'(DRY-RUN)' if res['dry_run'] else '(APPLIED)'}")
            print("=" * 70)
            print(f"Scanned Download Records      : {res['scanned_downloads']}")
            print(f"Planned / Migrated Paths      : {res['planned_migrations']}")
            print(f"  - Album Folders with [Year] : {res['year_prefix_folders_fixed']}")
            print(f"  - Various Artists Reintegrated: {res['va_orphans_reintegrated']}")
            print(f"  - Names Sanitized / Collapsed: {res['sanitized_paths_count']}")
            print(f"Physical Files Moved          : {res['physical_audio_files_moved']}")
            print(f"Sidecars Moved (.lrc)         : {res['sidecars_moved']}")
            if res.get("backup_snapshot"):
                print(f"Safety Backup Created         : {res['backup_snapshot']}")
            print(f"Errors Encountered            : {len(res['errors'])}")
            print("=" * 70)

        sys.exit(0 if res["success"] else 1)
    except Exception as e:
        if args.json:
            print(json.dumps({"success": False, "error": str(e)}))
        else:
            print(f"[FATAL] Disk layout normalization failed: {e}", file=sys.stderr)
        sys.exit(1)


if __name__ == "__main__":
    main()

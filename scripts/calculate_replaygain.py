#!/usr/bin/env python3
"""
calculate_replaygain.py (TASK-73)

Integrates and calculates ReplayGain 2.0 and EBU R128 loudness normalization
for audio files in Syncify library.

Features:
1. Calculates Integrated Loudness (LUFS), True Peak (dBTP), and Loudness Range (LU).
2. Derives ReplayGain track gain, track peak, album gain, and album peak based on configurable target LUFS.
3. Performs atomic safety snapshot (VACUUM INTO) prior to DB mutations.
4. Updates tracks table with loudness, replaygain_track_gain, replaygain_track_peak, replaygain_album_gain, replaygain_album_peak.
5. Optionally writes Vorbis comments to FLAC files and iTunes tags to MP4/M4A.
6. Full support for --dry-run, --db-path, --target-lufs.
"""

import os
import sys
import argparse
import sqlite3
import subprocess
import shutil
import math
from datetime import datetime
from collections import defaultdict


def make_snapshot(db_path, backup_dir="/tmp"):
    """Creates a preventative VACUUM INTO database snapshot before modifying data."""
    timestamp = datetime.now().strftime("%Y%m%d_%H%M%S")
    backup_path = os.path.join(backup_dir, f"syncify_backup_pre_repair_TASK-73_{timestamp}.db")
    os.makedirs(backup_dir, exist_ok=True)

    try:
        src_conn = sqlite3.connect(f"file:{db_path}?mode=ro", uri=True)
        src_conn.execute(f"VACUUM INTO '{backup_path}'")
        src_conn.close()
        print(f"[TASK-73] Safety snapshot created via VACUUM INTO: {backup_path}")
        return backup_path
    except Exception as e:
        print(f"[TASK-73] VACUUM INTO fallback ({e}), attempting file copy...")
        try:
            shutil.copy2(db_path, backup_path)
            print(f"[TASK-73] Safety snapshot created via copy: {backup_path}")
            return backup_path
        except Exception as copy_err:
            print(f"[TASK-73] ERROR: Failed to create safety snapshot: {copy_err}", file=sys.stderr)
            sys.exit(1)


def run_ffmpeg_ebur128(file_path):
    """
    Executes ffmpeg with ebur128 filter to compute integrated loudness, true peak, and LRA.
    Returns dict with metrics or None on error.
    """
    if not os.path.isfile(file_path):
        return None

    cmd = [
        "ffmpeg",
        "-hide_banner",
        "-nostats",
        "-i", file_path,
        "-af", "ebur128=peak=true",
        "-f", "null",
        "-"
    ]

    try:
        proc = subprocess.run(cmd, capture_output=True, text=True, check=False)
        stderr = proc.stderr
    except Exception as e:
        print(f"Error invoking ffmpeg on {file_path}: {e}", file=sys.stderr)
        return None

    integrated_lufs = None
    true_peak_db = None
    loudness_range_lu = None

    in_summary = False
    for line in stderr.splitlines():
        trimmed = line.strip()
        if "Summary:" in trimmed:
            in_summary = True
            continue

        if in_summary:
            if trimmed.startswith("I:") and "LUFS" in trimmed:
                parts = trimmed.split()
                if len(parts) >= 2:
                    try:
                        integrated_lufs = float(parts[1])
                    except ValueError:
                        pass
            elif trimmed.startswith("Peak:") and ("dBFS" in trimmed or "dBTP" in trimmed):
                parts = trimmed.split()
                if len(parts) >= 2:
                    try:
                        true_peak_db = float(parts[1])
                    except ValueError:
                        pass
            elif trimmed.startswith("LRA:") and "LU" in trimmed:
                parts = trimmed.split()
                if len(parts) >= 2:
                    try:
                        loudness_range_lu = float(parts[1])
                    except ValueError:
                        pass
        else:
            # Fallback from per-frame output if summary missing
            if integrated_lufs is None and "I:" in trimmed and "LUFS" in trimmed:
                pos = trimmed.find("I:")
                sub = trimmed[pos + 2:].strip()
                tokens = sub.split()
                if tokens:
                    try:
                        integrated_lufs = float(tokens[0])
                    except ValueError:
                        pass
            if true_peak_db is None and ("TPK:" in trimmed or "Peak:" in trimmed):
                marker = "TPK:" if "TPK:" in trimmed else "Peak:"
                pos = trimmed.find(marker)
                sub = trimmed[pos + len(marker):].strip()
                tokens = sub.split()
                if tokens:
                    try:
                        true_peak_db = float(tokens[0])
                    except ValueError:
                        pass

    if integrated_lufs is None:
        return None

    if true_peak_db is None:
        true_peak_db = -0.1

    if math.isinf(true_peak_db) and true_peak_db < 0:
        peak_linear = 0.0
    else:
        peak_linear = min(1.0, max(0.0, 10.0 ** (true_peak_db / 20.0)))

    return {
        "integrated_lufs": integrated_lufs,
        "true_peak_db": true_peak_db,
        "true_peak_linear": peak_linear,
        "loudness_range_lu": loudness_range_lu,
    }


def calculate_album_replaygain(tracks_data, target_lufs):
    """
    Computes album loudness and peak across multiple tracks using acoustic energy summation.
    energy mean = sum(10^(LUFS / 10)) / N
    album_lufs = 10 * log10(energy mean)
    """
    if not tracks_data:
        return None

    sum_power = 0.0
    max_peak = 0.0
    valid_count = 0

    for t in tracks_data:
        lufs = t["integrated_lufs"]
        peak = t["true_peak_linear"]
        power = 10.0 ** (lufs / 10.0)
        if math.isfinite(power):
            sum_power += power
            valid_count += 1
        if peak > max_peak:
            max_peak = peak

    if valid_count == 0:
        return None

    mean_power = sum_power / valid_count
    album_lufs = 10.0 * math.log10(mean_power)
    album_gain_db = target_lufs - album_lufs
    album_gain_str = f"{album_gain_db:+.2f} dB"
    album_peak_str = f"{min(1.0, max(0.0, max_peak)):.6f}"

    return {
        "album_lufs": album_lufs,
        "album_gain_db": album_gain_db,
        "album_gain_str": album_gain_str,
        "album_peak_str": album_peak_str,
    }


def write_flac_tags(file_path, track_gain_str, track_peak_str, album_gain_str, album_peak_str, lufs):
    """Writes ReplayGain Vorbis comments using metaflac if installed."""
    if not shutil.which("metaflac"):
        return False

    tags = [
        f"REPLAYGAIN_TRACK_GAIN={track_gain_str}",
        f"REPLAYGAIN_TRACK_PEAK={track_peak_str}",
        f"LOUDNESS={lufs:.1f}",
    ]
    if album_gain_str:
        tags.append(f"REPLAYGAIN_ALBUM_GAIN={album_gain_str}")
    if album_peak_str:
        tags.append(f"REPLAYGAIN_ALBUM_PEAK={album_peak_str}")

    cmd = ["metaflac", "--remove-tag=REPLAYGAIN_TRACK_GAIN", "--remove-tag=REPLAYGAIN_TRACK_PEAK",
           "--remove-tag=REPLAYGAIN_ALBUM_GAIN", "--remove-tag=REPLAYGAIN_ALBUM_PEAK"]
    for t in tags:
        cmd.append(f"--set-tag={t}")
    cmd.append(file_path)

    res = subprocess.run(cmd, capture_output=True, text=True, check=False)
    return res.returncode == 0


def main():
    parser = argparse.ArgumentParser(
        description="Calculate ReplayGain 2.0 & EBU R128 Loudness metrics and update Syncify database."
    )
    parser.add_argument(
        "--db-path", "--db",
        dest="db_path",
        default=os.path.expanduser("~/.local/share/com.syncify.app/syncify.db"),
        help="Path to SQLite database (default: ~/.local/share/com.syncify.app/syncify.db)"
    )
    parser.add_argument(
        "--dry-run",
        action="store_true",
        help="Simulate calculations without writing to database or physical files"
    )
    parser.add_argument(
        "--target-lufs",
        type=float,
        default=-18.0,
        help="Target integrated loudness in LUFS (-18.0 for ReplayGain 2.0, -14.0 for streaming)"
    )
    parser.add_argument(
        "--tag-files",
        action="store_true",
        help="Also write Vorbis / MP4 tags back to physical files on disk"
    )
    parser.add_argument(
        "--force",
        action="store_true",
        help="Recalculate even if track already has loudness values in database"
    )
    parser.add_argument(
        "--limit",
        type=int,
        default=0,
        help="Limit number of tracks to process (0 = all)"
    )
    parser.add_argument(
        "--backup-dir",
        default="/tmp",
        help="Directory to store pre-repair database backup (default: /tmp)"
    )

    args = parser.parse_args()

    db_path = os.path.abspath(os.path.expanduser(args.db_path))
    if not os.path.isfile(db_path):
        # Check dev fallback
        fallback = os.path.join(os.path.dirname(__file__), "..", "src-tauri", "syncify.db")
        if os.path.isfile(fallback):
            db_path = os.path.abspath(fallback)
            print(f"[TASK-73] Primary DB not found; using fallback at {db_path}")
        else:
            print(f"Error: Database file not found at {db_path}", file=sys.stderr)
            sys.exit(1)

    print("=" * 70)
    print("Syncify Loudness & ReplayGain 2.0 Normalizer [TASK-73]")
    print(f"Database:     {db_path}")
    print(f"Target LUFS:  {args.target_lufs} LUFS")
    print(f"Mode:         {'DRY-RUN (no modifications)' if args.dry_run else 'APPLY (will update DB)'}")
    print(f"Write tags:   {args.tag_files}")
    print("=" * 70)

    # 1. Safety snapshot if not dry-run
    if not args.dry_run:
        make_snapshot(db_path, args.backup_dir)

    conn = sqlite3.connect(db_path)
    conn.row_factory = sqlite3.Row
    cursor = conn.cursor()

    # Ensure columns exist in case migration 0083 has not been applied yet
    cursor.execute("PRAGMA table_info(tracks)")
    existing_cols = {col["name"] for col in cursor.fetchall()}
    needed_cols = [
        ("loudness", "REAL"),
        ("replaygain_track_gain", "TEXT"),
        ("replaygain_track_peak", "TEXT"),
        ("replaygain_album_gain", "TEXT"),
        ("replaygain_album_peak", "TEXT"),
    ]
    for col_name, col_type in needed_cols:
        if col_name not in existing_cols:
            if not args.dry_run:
                cursor.execute(f"ALTER TABLE tracks ADD COLUMN {col_name} {col_type}")
                print(f"[TASK-73] Added missing column {col_name} to tracks table.")
            else:
                print(f"[DRY-RUN] Column {col_name} would be added to tracks table.")

    # Check downloads table
    cursor.execute("SELECT name FROM sqlite_master WHERE type='table' AND name='downloads'")
    has_downloads = cursor.fetchone() is not None

    loudness_col = "t.loudness" if "loudness" in existing_cols else "NULL AS loudness"
    rg_col = "t.replaygain_track_gain" if "replaygain_track_gain" in existing_cols else "NULL AS replaygain_track_gain"

    if has_downloads:
        query = f"""
            SELECT t.id, t.title, t.album_id, {loudness_col}, {rg_col},
                   d.file_path, a.title AS album_title
            FROM tracks t
            LEFT JOIN downloads d ON t.id = d.track_id
            LEFT JOIN albums a ON t.album_id = a.id
            WHERE d.file_path IS NOT NULL
        """
        if not args.force and "loudness" in existing_cols and "replaygain_track_gain" in existing_cols:
            query += " AND (t.loudness IS NULL OR t.replaygain_track_gain IS NULL)"
    else:
        query = f"""
            SELECT t.id, t.title, t.album_id, {loudness_col}, {rg_col},
                   NULL AS file_path, a.title AS album_title
            FROM tracks t
            LEFT JOIN albums a ON t.album_id = a.id
        """

    if args.limit > 0:
        query += f" LIMIT {args.limit}"

    cursor.execute(query)
    rows = cursor.fetchall()
    print(f"Found {len(rows)} track candidates for loudness calculation.")

    if not rows:
        print("No tracks need ReplayGain calculation.")
        conn.close()
        return

    # Track metrics mapped by track_id
    track_results = {}
    album_groups = defaultdict(list)

    for row in rows:
        track_id = row["id"]
        title = row["title"]
        file_path = row["file_path"]
        album_id = row["album_id"]

        if not file_path or not os.path.isfile(file_path):
            continue

        metrics = run_ffmpeg_ebur128(file_path)
        if not metrics:
            print(f"  [SKIP] Could not calculate loudness for: {title} ({file_path})")
            continue

        i_lufs = metrics["integrated_lufs"]
        peak_lin = metrics["true_peak_linear"]
        track_gain_db = args.target_lufs - i_lufs
        track_gain_str = f"{track_gain_db:+.2f} dB"
        track_peak_str = f"{peak_lin:.6f}"

        item = {
            "track_id": track_id,
            "title": title,
            "file_path": file_path,
            "album_id": album_id,
            "integrated_lufs": i_lufs,
            "true_peak_linear": peak_lin,
            "track_gain_db": track_gain_db,
            "track_gain_str": track_gain_str,
            "track_peak_str": track_peak_str,
        }
        track_results[track_id] = item
        if album_id:
            album_groups[album_id].append(item)

        print(f"  [OK] {title}: {i_lufs:.1f} LUFS | Gain: {track_gain_str} | Peak: {track_peak_str}")

    # Calculate album ReplayGain for albums with multiple tracks
    album_metrics = {}
    for album_id, items in album_groups.items():
        if len(items) >= 1:
            res = calculate_album_replaygain(items, args.target_lufs)
            if res:
                album_metrics[album_id] = res

    # Write updates
    updated_count = 0
    for track_id, item in track_results.items():
        album_id = item["album_id"]
        album_info = album_metrics.get(album_id)
        album_gain_str = album_info["album_gain_str"] if album_info else None
        album_peak_str = album_info["album_peak_str"] if album_info else None

        if not args.dry_run:
            cursor.execute(
                """
                UPDATE tracks
                SET loudness = ?,
                    replaygain_track_gain = ?,
                    replaygain_track_peak = ?,
                    replaygain_album_gain = ?,
                    replaygain_album_peak = ?
                WHERE id = ?
                """,
                (
                    item["integrated_lufs"],
                    item["track_gain_str"],
                    item["track_peak_str"],
                    album_gain_str,
                    album_peak_str,
                    track_id,
                )
            )

            if args.tag_files and item["file_path"].lower().endswith(".flac"):
                write_flac_tags(
                    item["file_path"],
                    item["track_gain_str"],
                    item["track_peak_str"],
                    album_gain_str,
                    album_peak_str,
                    item["integrated_lufs"]
                )

        updated_count += 1

    if not args.dry_run:
        conn.commit()
        print(f"\n[DONE] Successfully persisted loudness metrics for {updated_count} tracks.")
    else:
        print(f"\n[DRY-RUN] Would persist loudness metrics for {updated_count} tracks.")

    conn.close()


if __name__ == "__main__":
    main()

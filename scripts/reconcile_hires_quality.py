#!/usr/bin/env python3
"""
Portable Maintenance Script: Reconcile False Hi-Res Audio Quality (TASK-109)

Reconciles tracks labeled as 'hires' in `tracks.audio_quality` whose downloaded
files on disk or in the `downloads` ledger are strictly 16-bit / 44.1kHz (or <=16-bit, <=48kHz).
Recategorizes them to 'lossless' (or 'lossy' if compressed), updates quality decisions
to 'CompletedWithQualityShortfall', and takes a preventive VACUUM snapshot before modifications.
"""

import argparse
import datetime
import os
import sqlite3
import subprocess
import sys


def inspect_flac_streaminfo(file_path):
    """
    Read STREAMINFO block from FLAC file:
    Tries metaflac first, then falls back to direct binary header inspection.
    """
    res = subprocess.run(
        ["metaflac", "--show-sample-rate", "--show-bps", "--show-channels", file_path],
        capture_output=True,
        text=True,
        check=False,
    )
    if res.returncode == 0:
        lines = [line.strip() for line in res.stdout.strip().splitlines() if line.strip()]
        if len(lines) >= 2:
            try:
                sample_rate = int(lines[0])
                bit_depth = int(lines[1])
                channels = int(lines[2]) if len(lines) >= 3 else 2
                return {"sample_rate": sample_rate, "bit_depth": bit_depth, "channels": channels}
            except ValueError:
                pass

    # Direct 42-byte binary STREAMINFO fallback
    try:
        with open(file_path, "rb") as f:
            header = f.read(42)
            if len(header) >= 42 and header.startswith(b"fLaC"):
                si = header[8:42]
                sample_first = (si[10] << 8) | si[11]
                sample_channel_bps = si[12]
                sample_rate = (sample_first << 4) | (sample_channel_bps >> 4)
                channels = ((sample_channel_bps >> 1) & 0x07) + 1
                bps_hi = (sample_channel_bps & 0x01) << 4
                next_byte = si[13]
                bps_lo = (next_byte >> 4) & 0x0F
                bit_depth = (bps_hi | bps_lo) + 1
                return {"sample_rate": sample_rate, "bit_depth": bit_depth, "channels": channels}
    except Exception:
        pass

    return None


def classify_tier(bit_depth, sample_rate, fmt):
    """
    Classifies audio metrics into canonical tier:
    - 'lossy' if lossy format
    - 'hires' if bit_depth > 16 or sample_rate > 48000
    - 'lossless' otherwise
    """
    norm_fmt = (fmt or "").strip().upper()
    if norm_fmt in ("MP3", "AAC", "M4A", "OGG", "OPUS", "VORBIS", "WMA", "LOSSY"):
        return "lossy"
    bd = bit_depth or 16
    sr = sample_rate or 44100
    if bd > 16 or sr > 48000 or (48 < sr <= 384):
        return "hires"
    return "lossless"


def find_default_db():
    candidates = [
        os.path.expanduser("~/.local/share/com.syncify.app/syncify.db"),
        os.path.abspath("syncify.db"),
        os.path.abspath("src-tauri/syncify.db"),
    ]
    for c in candidates:
        if os.path.isfile(c):
            return c
    return candidates[0]


def main():
    parser = argparse.ArgumentParser(
        description="TASK-109: Reconcile False Hi-Res Audio Quality in syncify.db"
    )
    parser.add_argument(
        "--db-path",
        default=None,
        help="Path to SQLite database file (default: ~/.local/share/com.syncify.app/syncify.db or ./syncify.db)",
    )
    parser.add_argument(
        "--dry-run",
        action="store_true",
        help="Simulate reconciliation and output affected records without writing changes",
    )
    parser.add_argument(
        "--skip-backup",
        action="store_true",
        help="Skip VACUUM INTO snapshot (only recommended for unit testing)",
    )
    parser.add_argument(
        "--scan-physical-files",
        action="store_true",
        help="Inspect physical audio files on disk if present to extract verified STREAMINFO",
    )

    args = parser.parse_args()

    db_path = args.db_path or find_default_db()
    if not os.path.isfile(db_path):
        print(f"[ERROR] Database file not found at: {db_path}", file=sys.stderr)
        sys.exit(1)

    print(f"[TASK-109] Connecting to database: {db_path}")
    if args.dry_run:
        try:
            conn = sqlite3.connect(f"file:{os.path.abspath(db_path)}?mode=ro", uri=True)
            conn.row_factory = sqlite3.Row
            # Test execute
            conn.cursor().execute("SELECT 1")
        except Exception:
            conn = sqlite3.connect(f"file:{os.path.abspath(db_path)}?immutable=1", uri=True)
    else:
        conn = sqlite3.connect(db_path)
    conn.row_factory = sqlite3.Row
    cursor = conn.cursor()

    # Step 1: Preventative Snapshot
    if not args.dry_run and not args.skip_backup:
        timestamp = datetime.datetime.now().strftime("%Y%m%d_%H%M%S")
        backup_path = f"/tmp/syncify_backup_pre_repair_TASK-109_{timestamp}.db"
        print(f"[TASK-109] Creating preventive snapshot via VACUUM INTO: {backup_path}")
        try:
            # Ensure target directory exists
            os.makedirs(os.path.dirname(backup_path), exist_ok=True)
            cursor.execute(f"VACUUM INTO '{backup_path}'")
            print(f"[TASK-109] ✓ Preventive snapshot created: {backup_path}")
        except Exception as e:
            print(f"[ERROR] Failed to create VACUUM INTO snapshot: {e}", file=sys.stderr)
            conn.close()
            sys.exit(1)

    # Step 2: Query candidate tracks with 'hires' audio_quality
    query = """
        SELECT t.id AS track_id, t.title, t.audio_quality,
               d.id AS download_id, d.file_path, d.file_format, d.bit_depth, d.sample_rate,
               d.requested_quality, d.quality_decision
        FROM tracks t
        LEFT JOIN downloads d ON t.id = d.track_id
        WHERE t.audio_quality = 'hires'
    """
    cursor.execute(query)
    candidates = cursor.fetchall()
    print(f"[TASK-109] Found {len(candidates)} tracks currently labeled as 'hires'")

    reconciled_tracks = 0
    reconciled_downloads = 0
    shortfalls_updated = 0
    physical_files_inspected = 0

    for row in candidates:
        track_id = row["track_id"]
        download_id = row["download_id"]
        file_path = row["file_path"]
        bit_depth = row["bit_depth"]
        sample_rate = row["sample_rate"]
        file_format = row["file_format"]
        req_q = row["requested_quality"] or "hires"
        q_decision = row["quality_decision"]

        # If download entry exists
        if download_id is not None:
            # If requested and file exists on disk, inspect physical header
            if args.scan_physical_files and file_path and os.path.isfile(file_path):
                physical_files_inspected += 1
                if file_path.lower().endswith(".flac"):
                    si = inspect_flac_streaminfo(file_path)
                    if si:
                        bit_depth = si["bit_depth"]
                        sample_rate = si["sample_rate"]
                        file_format = "FLAC"

            # Check if this download is fake hires: <= 16 bit and <= 48000 Hz
            is_fake_hires = (
                (bit_depth is not None and bit_depth <= 16)
                and (sample_rate is not None and sample_rate <= 48000)
            )

            # Also check if format is lossy (e.g. AAC 16/44.1 labeled hires)
            is_lossy = (file_format or "").strip().upper() in ("AAC", "MP3", "M4A", "OGG", "OPUS")

            if is_fake_hires or is_lossy:
                target_tier = "lossy" if is_lossy else "lossless"

                # Update tracks.audio_quality
                if not args.dry_run:
                    cursor.execute(
                        "UPDATE tracks SET audio_quality = ? WHERE id = ?",
                        (target_tier, track_id),
                    )
                reconciled_tracks += 1

                # Update downloads ledger
                new_decision = q_decision
                reason = None
                fallback_used = 0
                if "hires" in req_q.lower() or "max" in req_q.lower():
                    new_decision = "CompletedWithQualityShortfall"
                    fallback_used = 1
                    reason = f"Quality shortfall: requested Hi-Res ({req_q}), but verified CD quality ({bit_depth}bit/{sample_rate / 1000.0 if sample_rate else 44.1}kHz)"
                    shortfalls_updated += 1

                if not args.dry_run:
                    cursor.execute(
                        """
                        UPDATE downloads
                        SET bit_depth = COALESCE(?, bit_depth, 16),
                            sample_rate = COALESCE(?, sample_rate, 44100),
                            quality_decision = COALESCE(?, quality_decision),
                            quality_fallback_used = CASE WHEN ? = 1 THEN 1 ELSE quality_fallback_used END,
                            decision_reason = COALESCE(?, decision_reason)
                        WHERE id = ?
                        """,
                        (bit_depth, sample_rate, new_decision, fallback_used, reason, download_id),
                    )
                reconciled_downloads += 1

                # Synchronize download_queue if matching completed queue item exists
                if not args.dry_run:
                    cursor.execute(
                        """
                        UPDATE download_queue
                        SET quality_decision = COALESCE(?, quality_decision),
                            quality_fallback_used = CASE WHEN ? = 1 THEN 1 ELSE quality_fallback_used END,
                            decision_reason = COALESCE(?, decision_reason)
                        WHERE track_id = ? AND status = 'complete'
                        """,
                        (new_decision, fallback_used, reason, track_id),
                    )

    if not args.dry_run:
        conn.commit()
        print("[TASK-109] ✓ All changes successfully committed to database.")
    else:
        print("[TASK-109] (Dry run completed: no database modifications made)")

    conn.close()

    print("\n" + "=" * 55)
    print("TASK-109 HI-RES RECONCILIATION SUMMARY")
    print("=" * 55)
    action_label = "Identified" if args.dry_run else "Reconciled"
    print(f"Tracks recategorized from 'hires' to 'lossless'/'lossy': {reconciled_tracks}")
    print(f"Downloads ledger records updated:                         {reconciled_downloads}")
    print(f"Quality shortfall decisions recorded:                    {shortfalls_updated}")
    if args.scan_physical_files:
        print(f"Physical files inspected on disk:                        {physical_files_inspected}")
    print("=" * 55)


if __name__ == "__main__":
    main()

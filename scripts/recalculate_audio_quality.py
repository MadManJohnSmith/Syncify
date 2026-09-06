#!/usr/bin/env python3
"""
Audio Quality Reconciliation Script (TASK-72)
Inspects physical audio files on disk, reconciles discrepancies in `downloads`
ledger (sample_rate, bit_depth) and synchronizes `tracks.audio_quality` to canonical
values ('hires', 'lossless', 'lossy').
Also flags quality shortfalls (Hi-Res requested but CD-quality verified).
"""

import os
import sys
import argparse
import sqlite3
import subprocess

def inspect_flac(file_path):
    """Read STREAMINFO from FLAC file using metaflac or raw header."""
    res = subprocess.run(
        ["metaflac", "--show-sample-rate", "--show-bps", "--show-channels", "--show-total-samples", file_path],
        capture_output=True,
        text=True,
        check=False
    )
    if res.returncode == 0:
        lines = [line.strip() for line in res.stdout.strip().splitlines() if line.strip()]
        if len(lines) >= 3:
            try:
                sample_rate = int(lines[0])
                bit_depth = int(lines[1])
                channels = int(lines[2])
                total_samples = int(lines[3]) if len(lines) >= 4 else 0
                duration_sec = total_samples / sample_rate if sample_rate > 0 else 0
                file_size = os.path.getsize(file_path) if os.path.exists(file_path) else 0
                bitrate = round((file_size * 8) / duration_sec / 1000) if duration_sec > 0 and file_size > 0 else None
                return {
                    "format": "FLAC",
                    "sample_rate": sample_rate,
                    "bit_depth": bit_depth,
                    "channels": channels,
                    "bitrate": bitrate,
                }
            except (ValueError, ZeroDivisionError):
                pass

    # Fallback: binary header inspection
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
                return {
                    "format": "FLAC",
                    "sample_rate": sample_rate,
                    "bit_depth": bit_depth,
                    "channels": channels,
                    "bitrate": None,
                }
    except Exception:
        pass

    return None

def inspect_m4a(file_path):
    """Inspect M4A / AAC file via ffprobe or defaults."""
    cmd = [
        "ffprobe", "-v", "error",
        "-show_entries", "stream=codec_name,sample_rate,channels,bit_rate",
        "-of", "default=noprint_wrappers=1:nokey=1",
        file_path
    ]
    res = subprocess.run(cmd, capture_output=True, text=True, check=False)
    if res.returncode == 0:
        parts = [p.strip() for p in res.stdout.strip().splitlines() if p.strip()]
        codec = parts[0].upper() if len(parts) > 0 else "AAC"
        sr = int(parts[1]) if len(parts) > 1 and parts[1].isdigit() else 44100
        ch = int(parts[2]) if len(parts) > 2 and parts[2].isdigit() else 2
        br = round(int(parts[3]) / 1000) if len(parts) > 3 and parts[3].isdigit() else 320
        return {
            "format": codec,
            "sample_rate": sr,
            "bit_depth": 16,
            "channels": ch,
            "bitrate": br,
        }
    return {
        "format": "AAC",
        "sample_rate": 44100,
        "bit_depth": 16,
        "channels": 2,
        "bitrate": 320,
    }

def inspect_file(file_path):
    if not os.path.isfile(file_path):
        return None
    ext = os.path.splitext(file_path)[1].lower()
    if ext == ".flac":
        return inspect_flac(file_path)
    elif ext in (".m4a", ".aac", ".mp4"):
        return inspect_m4a(file_path)
    elif ext == ".mp3":
        return {
            "format": "MP3",
            "sample_rate": 44100,
            "bit_depth": 16,
            "channels": 2,
            "bitrate": 320,
        }
    return None

def classify_audio_tier(bit_depth, sample_rate, codec):
    is_lossless = codec.upper() in ("FLAC", "ALAC", "WAV")
    if not is_lossless:
        return "lossy"
    if (bit_depth and bit_depth > 16) or (sample_rate and sample_rate > 48000):
        return "hires"
    return "lossless"

def is_hires_requested(req_q):
    if not req_q:
        return False
    norm = req_q.strip().lower()
    return any(x in norm for x in ["hires", "hi_res", "hi-res", "max", "24-96", "24-192", "24/96", "24/192"])

def main():
    parser = argparse.ArgumentParser(description="Recalculate audio quality from physical disk files.")
    parser.add_argument("--db", default=os.path.expanduser("~/.local/share/com.syncify.app/syncify.db"), help="Path to syncify.db")
    parser.add_argument("--dry-run", action="store_true", help="Report discrepancies without writing changes")
    args = parser.parse_args()

    if not os.path.exists(args.db):
        print(f"Error: Database not found at {args.db}", file=sys.stderr)
        sys.exit(1)

    conn = sqlite3.connect(args.db)
    conn.row_factory = sqlite3.Row
    cur = conn.cursor()

    cur.execute("""
        SELECT d.id AS download_id, d.track_id, d.file_path, d.bit_depth, d.sample_rate, d.file_format,
               d.requested_quality, d.quality_decision, t.audio_quality AS track_quality
        FROM downloads d
        JOIN tracks t ON t.id = d.track_id
    """)
    rows = cur.fetchall()
    print(f"Found {len(rows)} downloads to audit.")

    updated_downloads = 0
    updated_tracks = 0
    shortfalls_detected = 0

    for row in rows:
        fpath = row["file_path"]
        info = inspect_file(fpath)
        if not info:
            continue

        real_bd = info["bit_depth"]
        real_sr = info["sample_rate"]
        real_tier = classify_audio_tier(real_bd, real_sr, info["format"])

        # Check for discrepancies
        bd_diff = row["bit_depth"] != real_bd
        sr_diff = row["sample_rate"] != real_sr
        track_q_diff = (row["track_quality"] or "").lower() != real_tier

        # Shortfall check
        req_is_hires = is_hires_requested(row["requested_quality"])
        is_shortfall = req_is_hires and real_tier != "hires"

        new_decision = row["quality_decision"]
        new_q_fallback = 0
        dec_reason = None
        if is_shortfall:
            new_decision = "CompletedWithQualityShortfall"
            new_q_fallback = 1
            dec_reason = f"Quality shortfall: requested Hi-Res ({row['requested_quality']}), but verified CD quality ({real_bd}bit/{real_sr / 1000.0}kHz)"
            shortfalls_detected += 1

        if bd_diff or sr_diff or is_shortfall:
            updated_downloads += 1
            if not args.dry_run:
                cur.execute("""
                    UPDATE downloads
                    SET bit_depth = ?, sample_rate = ?, quality_decision = COALESCE(?, quality_decision),
                        quality_fallback_used = CASE WHEN ? = 1 THEN 1 ELSE quality_fallback_used END,
                        decision_reason = COALESCE(?, decision_reason)
                    WHERE id = ?
                """, (real_bd, real_sr, new_decision, new_q_fallback, dec_reason, row["download_id"]))

        if track_q_diff:
            updated_tracks += 1
            if not args.dry_run:
                cur.execute("""
                    UPDATE tracks SET audio_quality = ? WHERE id = ?
                """, (real_tier, row["track_id"]))

    if not args.dry_run:
        conn.commit()

    conn.close()

    action = "Identified" if args.dry_run else "Reconciled"
    print(f"=== RECONCILIATION SUMMARY ===")
    print(f"{action} {updated_downloads} downloads with physical bit_depth/sample_rate discrepancies.")
    print(f"{action} {updated_tracks} tracks with audio_quality tier mismatches.")
    print(f"{action} {shortfalls_detected} quality shortfalls (Hi-Res -> CD quality).")

if __name__ == "__main__":
    main()

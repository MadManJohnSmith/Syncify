#!/usr/bin/env python3
"""
Rhythm and Key Analyzer & Backfill Bridge (TASK-74)

Extracts and emits rhythmic and tonal metadata:
- BPM (Tempo)
- INITIALKEY / musical_key in standard Camelot notation (1A-12B)
- Energy (0.0 to 1.0 acoustic intensity)

Features:
- Preventative snapshot via VACUUM INTO in /tmp/
- Dry-run mode (--dry-run)
- File scanning & tag writing (FLAC BPM/TEMPO/TBPM/INITIALKEY/KEY)
- Database synchronization (tracks.bpm, tracks.musical_key, tracks.energy)
"""

import os
import sys
import math
import struct
import argparse
import sqlite3
import subprocess
from datetime import datetime

# Standard Camelot Wheel Mappings (1A - 12B)
# Major keys -> B
# Minor keys -> A
CAMELOT_MAJOR = {
    0: "8B",   # C
    1: "3B",   # C# / Db
    2: "10B",  # D
    3: "5B",   # D# / Eb
    4: "12B",  # E
    5: "7B",   # F
    6: "2B",   # F# / Gb
    7: "9B",   # G
    8: "4B",   # G# / Ab
    9: "11B",  # A
    10: "6B",  # A# / Bb
    11: "1B",  # B
}

CAMELOT_MINOR = {
    0: "5A",   # C
    1: "12A",  # C# / Db
    2: "7A",   # D
    3: "2A",   # D# / Eb
    4: "9A",   # E
    5: "4A",   # F
    6: "11A",  # F# / Gb
    7: "6A",   # G
    8: "1A",   # G# / Ab
    9: "8A",   # A
    10: "3A",  # A# / Bb
    11: "10A", # B
}

# Krumhansl-Schmuckler Key Profiles (12-TET)
KRUMHANSL_MAJOR = [6.35, 2.23, 3.48, 2.33, 4.38, 4.09, 2.52, 5.19, 2.39, 3.66, 2.29, 2.88]
KRUMHANSL_MINOR = [6.33, 2.68, 3.52, 5.38, 2.60, 3.53, 2.54, 4.75, 3.98, 2.69, 3.34, 3.17]


def normalize_to_camelot(raw_str):
    """Normalize raw key string (Camelot, pitch name, minor/major) into standard Camelot notation (1A-12B)."""
    if not raw_str or not str(raw_str).strip():
        return None

    s = str(raw_str).strip()

    # 1. Check if already valid Camelot notation: 1A-12B or 01A-12B
    s_upper = s.upper()
    num_part = "".join(c for c in s_upper if c.isdigit())
    letter_part = "".join(c for c in s_upper if c.isalpha())

    if letter_part in ("A", "B") and num_part:
        try:
            n = int(num_part)
            if 1 <= n <= 12:
                return f"{n}{letter_part}"
        except ValueError:
            pass

    # 2. Parse pitch names
    s_lower = s.lower()
    is_minor = "min" in s_lower or "moll" in s_lower or (s_lower.endswith("m") and not s_lower.endswith("maj"))

    root = None
    if s_lower.startswith("c#") or s_lower.startswith("db"):
        root = 1
    elif s_lower.startswith("d#") or s_lower.startswith("eb"):
        root = 3
    elif s_lower.startswith("f#") or s_lower.startswith("gb"):
        root = 6
    elif s_lower.startswith("g#") or s_lower.startswith("ab"):
        root = 8
    elif s_lower.startswith("a#") or s_lower.startswith("bb"):
        root = 10
    elif s_lower.startswith("c"):
        root = 0
    elif s_lower.startswith("d"):
        root = 2
    elif s_lower.startswith("e"):
        root = 4
    elif s_lower.startswith("f"):
        root = 5
    elif s_lower.startswith("g"):
        root = 7
    elif s_lower.startswith("a"):
        root = 9
    elif s_lower.startswith("b"):
        root = 11

    if root is not None:
        return CAMELOT_MINOR[root] if is_minor else CAMELOT_MAJOR[root]

    return None


def pearson_correlation(x, y):
    """Compute Pearson correlation coefficient between two 12-dimensional vectors."""
    n = len(x)
    mean_x = sum(x) / n
    mean_y = sum(y) / n

    num = sum((x[i] - mean_x) * (y[i] - mean_y) for i in range(n))
    den_x = sum((x[i] - mean_x) ** 2 for i in range(n))
    den_y = sum((y[i] - mean_y) ** 2 for i in range(n))

    if den_x > 0 and den_y > 0:
        return num / math.sqrt(den_x * den_y)
    return 0.0


def decode_mono_pcm(file_path, sample_rate=22050, duration=30):
    """Decode audio snippet to mono f32le PCM using ffmpeg."""
    cmd = [
        "ffmpeg", "-v", "error",
        "-ss", "10",
        "-t", str(duration),
        "-i", file_path,
        "-f", "f32le",
        "-ac", "1",
        "-ar", str(sample_rate),
        "-"
    ]
    res = subprocess.run(cmd, capture_output=True, check=False)
    if res.returncode != 0 or len(res.stdout) == 0:
        # Fallback from offset 0
        cmd[3] = "0"
        res = subprocess.run(cmd, capture_output=True, check=False)
        if res.returncode != 0 or len(res.stdout) == 0:
            return None

    num_floats = len(res.stdout) // 4
    if num_floats == 0:
        return None
    samples = struct.unpack(f"<{num_floats}f", res.stdout[:num_floats * 4])
    return samples


def estimate_key_from_pcm(samples, sample_rate=22050):
    """Estimate musical key using Goertzel chromagram and Krumhansl-Schmuckler profiles."""
    if not samples or len(samples) < sample_rate:
        return None

    chroma = [0.0] * 12
    block_size = 4096
    num_blocks = len(samples) // block_size
    blocks_to_process = min(num_blocks, 16)
    if blocks_to_process == 0:
        return None

    step = max(1, num_blocks // blocks_to_process)

    # Analyze MIDI notes 36 (C2, ~65.4Hz) to 83 (B5, ~987.8Hz)
    for b_idx in range(blocks_to_process):
        start = b_idx * step * block_size
        block = samples[start:start + block_size]
        if len(block) < block_size:
            break

        for midi_note in range(36, 84):
            pitch_class = midi_note % 12
            freq = 440.0 * (2.0 ** ((midi_note - 69.0) / 12.0))
            omega = 2.0 * math.pi * freq / sample_rate
            coeff = 2.0 * math.cos(omega)

            s_prev = 0.0
            s_prev2 = 0.0
            for sample in block:
                s = sample + coeff * s_prev - s_prev2
                s_prev2 = s_prev
                s_prev = s

            power = max(0.0, s_prev * s_prev + s_prev2 * s_prev2 - coeff * s_prev * s_prev2)
            chroma[pitch_class] += power

    total_power = sum(chroma)
    if total_power < 1e-6:
        return None

    chroma = [c / total_power for c in chroma]

    best_root = 0
    best_is_major = True
    max_corr = -1.0

    for root in range(12):
        rot_maj = [KRUMHANSL_MAJOR[(i + 12 - root) % 12] for i in range(12)]
        rot_min = [KRUMHANSL_MINOR[(i + 12 - root) % 12] for i in range(12)]

        corr_maj = pearson_correlation(chroma, rot_maj)
        if corr_maj > max_corr:
            max_corr = corr_maj
            best_root = root
            best_is_major = True

        corr_min = pearson_correlation(chroma, rot_min)
        if corr_min > max_corr:
            max_corr = corr_min
            best_root = root
            best_is_major = False

    if max_corr < 0.15:
        return None

    return CAMELOT_MAJOR[best_root] if best_is_major else CAMELOT_MINOR[best_root]


def estimate_energy_from_pcm(samples):
    """Estimate normalized RMS energy (0.0 to 1.0)."""
    if not samples:
        return None
    sum_sq = sum(s * s for s in samples)
    rms = math.sqrt(sum_sq / len(samples))
    if rms < 1e-5:
        return None
    scaled = min(1.0, max(0.05, rms * 3.2))
    return round(scaled, 2)


def estimate_bpm_from_pcm(samples, sample_rate=22050):
    """Estimate tempo from PCM via frame energy onset envelope and autocorrelation."""
    if not samples or len(samples) < (sample_rate * 3):
        return None, 0.0

    hop_size = 512
    frame_size = 1024
    num_frames = (len(samples) - frame_size) // hop_size
    if num_frames < 64:
        return None, 0.0

    frame_energies = []
    for i in range(num_frames):
        start = i * hop_size
        frame = samples[start:start + frame_size]
        energy = math.sqrt(sum(s * s for s in frame))
        frame_energies.append(energy)

    onset_env = [max(0.0, frame_energies[i] - frame_energies[i - 1]) for i in range(1, len(frame_energies))]
    max_onset = max(onset_env) if onset_env else 0.0
    if max_onset < 1e-6:
        return None, 0.0
    onset_env = [o / max_onset for o in onset_env]

    fps = sample_rate / hop_size
    min_lag = int(round(fps * 60.0 / 220.0))
    max_lag = int(round(fps * 60.0 / 50.0))

    n = len(onset_env)
    best_lag = 0
    max_ac = 0.0

    for lag in range(min_lag, max_lag + 1):
        ac_sum = sum(onset_env[i] * onset_env[i + lag] for i in range(n - lag))
        count = n - lag
        val = (ac_sum / count) if count > 0 else 0.0

        # Prior weight centered at 120 BPM
        bpm_cand = (fps * 60.0) / lag
        ratio = math.log2(bpm_cand / 120.0)
        prior = math.exp(-0.5 * (ratio / 0.7) ** 2)
        weighted_val = val * (0.55 + 0.45 * prior)

        if weighted_val > max_ac:
            max_ac = weighted_val
            best_lag = lag

    if best_lag == 0 or max_ac < 0.02:
        return None, 0.0

    detected_bpm = round((fps * 60.0) / best_lag)
    confidence = min(1.0, max(0.1, max_ac * 1.5))
    return int(detected_bpm), round(confidence, 2)


def read_tags_from_file(file_path):
    """Read existing BPM and KEY tags from FLAC or M4A file."""
    ext = os.path.splitext(file_path)[1].lower()
    bpm = None
    key = None

    if ext == ".flac":
        res = subprocess.run(["metaflac", "--show-tag=BPM", "--show-tag=TEMPO", "--show-tag=TBPM",
                              "--show-tag=INITIALKEY", "--show-tag=KEY", file_path],
                             capture_output=True, text=True, check=False)
        if res.returncode == 0:
            for line in res.stdout.splitlines():
                if "=" in line:
                    k, v = line.split("=", 1)
                    k = k.strip().upper()
                    v = v.strip()
                    if k in ("BPM", "TEMPO", "TBPM") and not bpm:
                        try:
                            bpm = round(float(v))
                        except ValueError:
                            pass
                    elif k in ("INITIALKEY", "KEY") and not key:
                        key = normalize_to_camelot(v)

    elif ext in (".m4a", ".aac", ".mp4"):
        res = subprocess.run(["ffprobe", "-v", "error", "-show_entries", "format_tags",
                              "-of", "default=noprint_wrappers=1", file_path],
                             capture_output=True, text=True, check=False)
        if res.returncode == 0:
            for line in res.stdout.splitlines():
                if "=" in line:
                    k, v = line.split("=", 1)
                    k = k.strip().lower()
                    v = v.strip()
                    if ("bpm" in k or "tempo" in k or "tmpo" in k) and not bpm:
                        try:
                            bpm = round(float(v))
                        except ValueError:
                            pass
                    elif "initialkey" in k or "key" in k:
                        key = normalize_to_camelot(v)

    return bpm, key


def retag_flac_file(file_path, bpm, key):
    """Update FLAC tags with BPM and INITIALKEY via metaflac."""
    args = ["metaflac"]
    if bpm and bpm > 0:
        args.extend([
            f"--set-tag=BPM={bpm}",
            f"--set-tag=TEMPO={bpm}",
            f"--set-tag=TBPM={bpm}"
        ])
    if key:
        args.extend([
            f"--set-tag=INITIALKEY={key}",
            f"--set-tag=KEY={key}"
        ])
    args.append(file_path)
    subprocess.run(args, capture_output=True, check=False)


def perform_preventative_backup(db_path):
    """Creates a preventative VACUUM INTO database snapshot before running changes."""
    ts = datetime.now().strftime("%Y%m%d_%H%M%S")
    backup_path = f"/tmp/syncify_backup_pre_repair_TASK-74_{ts}.db"
    conn = sqlite3.connect(db_path)
    try:
        conn.execute(f"VACUUM INTO '{backup_path}'")
        print(f"✓ Created preventative snapshot: {backup_path}")
        return backup_path
    finally:
        conn.close()


def main():
    parser = argparse.ArgumentParser(
        description="Rhythm & Key Analyzer (TASK-74): Extract BPM and Camelot Key for Harmonic Mixing & Radio"
    )
    parser.add_argument(
        "--db-path",
        default=os.path.expanduser("~/.local/share/com.syncify.app/syncify.db"),
        help="Path to SQLite database (syncify.db)"
    )
    parser.add_argument(
        "--dry-run",
        action="store_true",
        help="Dry run: scan and report without modifying database or tags"
    )
    parser.add_argument(
        "--scan-files",
        action="store_true",
        help="Perform DSP audio analysis on downloaded audio files"
    )
    parser.add_argument(
        "--track-id",
        type=int,
        default=None,
        help="Analyze a single specific track ID"
    )
    parser.add_argument(
        "--force",
        action="store_true",
        help="Overwrite existing BPM and Key values"
    )

    args = parser.parse_args()

    print(f"=== Syncify Rhythm & Key Analyzer (TASK-74) ===")
    print(f"Database: {args.db_path}")
    print(f"Dry Run: {args.dry_run}")
    print(f"Scan Files: {args.scan_files}")

    if not os.path.exists(args.db_path):
        print(f"Error: Database file does not exist: {args.db_path}", file=sys.stderr)
        sys.exit(1)

    if not args.dry_run:
        perform_preventative_backup(args.db_path)

    if args.dry_run:
        try:
            conn = sqlite3.connect(f"file:{os.path.abspath(args.db_path)}?immutable=1", uri=True)
        except Exception:
            conn = sqlite3.connect(args.db_path)
    else:
        conn = sqlite3.connect(args.db_path)
    conn.row_factory = sqlite3.Row
    cursor = conn.cursor()

    query = """
        SELECT t.id, t.title, t.bpm, t.musical_key, t.energy, d.file_path
        FROM tracks t
        LEFT JOIN downloads d ON d.track_id = t.id
    """
    params = []
    if args.track_id:
        query += " WHERE t.id = ?"
        params.append(args.track_id)
    elif not args.force:
        query += " WHERE (t.bpm IS NULL OR t.musical_key IS NULL OR t.energy IS NULL)"

    query += " ORDER BY t.id ASC"

    cursor.execute(query, params)
    rows = cursor.fetchall()
    print(f"Found {len(rows)} candidate tracks for rhythm & key processing.\n")

    processed = 0
    updated = 0
    errors = 0

    for row in rows:
        track_id = row["id"]
        title = row["title"]
        file_path = row["file_path"]

        current_bpm = row["bpm"]
        current_key = row["musical_key"]
        current_energy = row["energy"]

        new_bpm = None
        new_key = None
        new_energy = None
        confidence = 0.85

        if file_path and os.path.exists(file_path):
            # 1. Read existing tags from file
            tag_bpm, tag_key = read_tags_from_file(file_path)

            if tag_bpm and not current_bpm:
                new_bpm = tag_bpm
            if tag_key and not current_key:
                new_key = tag_key

            # 2. Run DSP if requested and missing
            if args.scan_files and (new_bpm is None or new_key is None or current_energy is None):
                samples = decode_mono_pcm(file_path)
                if samples:
                    if new_bpm is None:
                        dsp_bpm, conf = estimate_bpm_from_pcm(samples)
                        if dsp_bpm and conf >= 0.35:
                            new_bpm = dsp_bpm
                            confidence = conf

                    if new_key is None:
                        dsp_key = estimate_key_from_pcm(samples)
                        if dsp_key:
                            new_key = dsp_key

                    if current_energy is None:
                        new_energy = estimate_energy_from_pcm(samples)

            # 3. Retag file if not dry run
            if not args.dry_run and file_path.lower().endswith(".flac"):
                eff_bpm = new_bpm or (int(current_bpm) if current_bpm else None)
                eff_key = new_key or current_key
                if eff_bpm or eff_key:
                    retag_flac_file(file_path, eff_bpm, eff_key)

        elif current_key:
            # Re-normalize existing key if needed
            norm = normalize_to_camelot(current_key)
            if norm and norm != current_key:
                new_key = norm

        # Determine if track needs DB update
        has_change = False
        final_bpm = new_bpm if new_bpm is not None else current_bpm
        final_key = new_key if new_key is not None else current_key
        final_energy = new_energy if new_energy is not None else current_energy

        if final_bpm != current_bpm or final_key != current_key or final_energy != current_energy:
            has_change = True

        if has_change:
            print(f"[{track_id}] '{title}': BPM: {current_bpm} -> {final_bpm} | Key: {current_key} -> {final_key} | Energy: {current_energy} -> {final_energy}")
            if not args.dry_run:
                try:
                    conn.execute("""
                        UPDATE tracks SET
                            bpm = ?,
                            musical_key = ?,
                            energy = ?,
                            tempo_confidence = ?,
                            tempo_source = 'LocalAudioAnalysis',
                            tempo_analyzed_at = CURRENT_TIMESTAMP
                        WHERE id = ?
                    """, (final_bpm, final_key, final_energy, confidence, track_id))
                    updated += 1
                except Exception as e:
                    print(f"  Error updating track {track_id}: {e}", file=sys.stderr)
                    errors += 1
            else:
                updated += 1

        processed += 1

    if not args.dry_run and updated > 0:
        conn.commit()

    conn.close()

    print("\n=== Summary ===")
    print(f"Tracks Processed: {processed}")
    print(f"Tracks Updated: {updated}")
    print(f"Errors: {errors}")


if __name__ == "__main__":
    main()

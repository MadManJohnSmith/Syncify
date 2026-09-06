#!/usr/bin/env python3
"""
Reconcile Downloads Storage - Physical Disk vs SQLite Reconciliation

Scans physical audio files in the music library, extracts Vorbis/ID3/MP4 metadata
(including ISRC, stream attributes, and SHA-256), resolves matching tracks in SQLite,
and transactionally upserts missing records into the 'downloads' table.
Also cleans orphaned staging residuals (.part, .partial) in .staging directories.

Usage:
    python3 scripts/reconcile_downloads_storage.py --help
    python3 scripts/reconcile_downloads_storage.py --dry-run
    python3 scripts/reconcile_downloads_storage.py --music-dir ~/Music/Syncify --db ~/.local/share/com.syncify.app/syncify.db
"""

import argparse
import hashlib
import json
import os
import re
import sqlite3
import subprocess
import sys
import time
from pathlib import Path
from typing import Dict, List, Optional, Tuple, Any

AUDIO_EXTENSIONS = {".flac", ".m4a", ".mp3", ".wav", ".alac", ".aac", ".ogg", ".opus"}
STAGING_EXTENSIONS = {".part", ".partial", ".tmp"}

try:
    from mutagen import File as MutagenFile
    from mutagen.easyid3 import EasyID3
    from mutagen.flac import FLAC
    from mutagen.mp4 import MP4
    from mutagen.id3 import ID3
    MUTAGEN_AVAILABLE = True
except ImportError:
    MUTAGEN_AVAILABLE = False


def compute_sha256(file_path: str) -> str:
    """Compute SHA-256 checksum for physical file."""
    h = hashlib.sha256()
    with open(file_path, "rb") as f:
        while chunk := f.read(65536):
            h.update(chunk)
    return h.hexdigest()


def probe_with_ffprobe(file_path: str) -> Optional[Dict[str, Any]]:
    """Extract stream metrics and metadata tags using ffprobe."""
    try:
        cmd = [
            "ffprobe",
            "-v", "quiet",
            "-print_format", "json",
            "-show_format",
            "-show_streams",
            file_path,
        ]
        res = subprocess.run(cmd, capture_output=True, text=True, timeout=15)
        if res.returncode != 0:
            return None
        data = json.loads(res.stdout)
        format_info = data.get("format", {})
        raw_tags = format_info.get("tags", {})
        tags = {k.upper(): str(v).strip() for k, v in raw_tags.items()}

        streams = data.get("streams", [])
        audio_stream = next((s for s in streams if s.get("codec_type") == "audio"), None)
        if not audio_stream and streams:
            audio_stream = streams[0]

        bit_depth = None
        sample_rate = None
        bitrate = None

        if audio_stream:
            raw_bits = audio_stream.get("bits_per_raw_sample") or tags.get("BITDEPTH")
            if raw_bits:
                try:
                    bit_depth = int(raw_bits)
                except Exception:
                    pass
            if "sample_rate" in audio_stream:
                try:
                    sample_rate = int(audio_stream["sample_rate"])
                except Exception:
                    pass
            if "bit_rate" in audio_stream:
                try:
                    bitrate = int(audio_stream["bit_rate"]) // 1000
                except Exception:
                    pass

        if not sample_rate and "SAMPLINGRATE" in tags:
            try:
                sample_rate = int(tags["SAMPLINGRATE"])
            except Exception:
                pass

        if not bitrate and "bit_rate" in format_info:
            try:
                bitrate = int(format_info["bit_rate"]) // 1000
            except Exception:
                pass

        return {
            "tags": tags,
            "sample_rate": sample_rate,
            "bit_depth": bit_depth,
            "bitrate": bitrate,
            "duration": float(format_info.get("duration", 0.0)) if "duration" in format_info else None,
        }
    except Exception:
        return None


def probe_audio_file(file_path: str) -> Dict[str, Any]:
    """Inspect audio file and extract normalized metadata, audio attributes, and SHA-256."""
    ext = os.path.splitext(file_path)[1].lower()
    file_size = os.path.getsize(file_path)
    sha256 = compute_sha256(file_path)

    format_name = "FLAC"
    if ext in (".m4a", ".aac", ".mp4"):
        format_name = "M4A"
    elif ext == ".mp3":
        format_name = "MP3"
    elif ext == ".wav":
        format_name = "WAV"
    elif ext == ".alac":
        format_name = "ALAC"
    elif ext == ".ogg":
        format_name = "OGG"
    elif ext == ".opus":
        format_name = "OPUS"

    isrc = None
    title = None
    artist = None
    album = None
    sample_rate = None
    bit_depth = None
    bitrate = None
    audio_source = None
    service_track_id = None

    # Check filename pattern for Tidal / Qobuz IDs
    # e.g. "01 - Tidal Track 134683067.flac" or "[Tidal-134683067].flac"
    base_name = os.path.basename(file_path)
    tidal_match = re.search(r"\[Tidal-(\d+)\]", base_name, re.IGNORECASE) or re.search(r"Tidal Track (\d+)", base_name, re.IGNORECASE)
    if tidal_match:
        service_track_id = tidal_match.group(1)
        audio_source = "tidal"

    qobuz_match = re.search(r"\[Qobuz-(\d+)\]", base_name, re.IGNORECASE)
    if qobuz_match:
        service_track_id = qobuz_match.group(1)
        audio_source = "qobuz"

    # 1. Try mutagen if available
    if MUTAGEN_AVAILABLE:
        try:
            if ext == ".flac":
                flac = FLAC(file_path)
                title = flac.get("title", [None])[0]
                artist = flac.get("artist", [None])[0]
                album = flac.get("album", [None])[0]
                isrc = flac.get("isrc", [None])[0]
                audio_source = flac.get("syncify_audio_source", [None])[0] or flac.get("source", [None])[0] or audio_source
                stid = flac.get("syncify_source_track_id", [None])[0] or flac.get("syncify_service_track_id", [None])[0]
                if stid:
                    service_track_id = stid

                if hasattr(flac, "info") and flac.info:
                    sample_rate = flac.info.sample_rate
                    bit_depth = flac.info.bits_per_sample
                    if flac.info.length and flac.info.length > 0:
                        bitrate = int((file_size * 8) / flac.info.length / 1000)
            elif ext in (".m4a", ".aac", ".mp4"):
                mp4 = MP4(file_path)
                if mp4.tags:
                    title = mp4.tags.get("\xa9nam", [None])[0]
                    artist = mp4.tags.get("\xa9ART", [None])[0]
                    album = mp4.tags.get("\xa9alb", [None])[0]
                    isrc_tag = mp4.tags.get("----:com.apple.iTunes:ISRC", [None])[0]
                    if isrc_tag:
                        isrc = isrc_tag.decode("utf-8", errors="ignore") if isinstance(isrc_tag, bytes) else str(isrc_tag)
                    src_tag = mp4.tags.get("----:com.apple.iTunes:SOURCE", [None])[0]
                    if src_tag:
                        audio_source = src_tag.decode("utf-8", errors="ignore") if isinstance(src_tag, bytes) else str(src_tag)
                sample_rate = 44100
                bit_depth = 16
                if hasattr(mp4, "info") and mp4.info and hasattr(mp4.info, "bitrate"):
                    bitrate = mp4.info.bitrate // 1000
            elif ext == ".mp3":
                try:
                    easy = EasyID3(file_path)
                    title = easy.get("title", [None])[0]
                    artist = easy.get("artist", [None])[0]
                    album = easy.get("album", [None])[0]
                except Exception:
                    pass
                try:
                    id3 = ID3(file_path)
                    tsrc = id3.get("TSRC")
                    if tsrc and hasattr(tsrc, "text") and tsrc.text:
                        isrc = str(tsrc.text[0])
                except Exception:
                    pass
                sample_rate = 44100
                bit_depth = 16
                bitrate = 320
        except Exception:
            pass

    # 2. Fallback to ffprobe if tags or stream metrics are still missing
    if not isrc or not title or not sample_rate:
        ff_info = probe_with_ffprobe(file_path)
        if ff_info:
            tags = ff_info.get("tags", {})
            if not isrc:
                isrc = tags.get("ISRC") or tags.get("TRACKISRC")
            if not title:
                title = tags.get("TITLE")
            if not artist:
                artist = tags.get("ARTIST")
            if not album:
                album = tags.get("ALBUM")
            if not audio_source:
                audio_source = tags.get("SYNCIFY_AUDIO_SOURCE") or tags.get("SOURCE")
            if not service_track_id:
                service_track_id = tags.get("SYNCIFY_SOURCE_TRACK_ID") or tags.get("SYNCIFY_SERVICE_TRACK_ID")
            if not sample_rate and ff_info.get("sample_rate"):
                sample_rate = ff_info["sample_rate"]
            if not bit_depth and ff_info.get("bit_depth"):
                bit_depth = ff_info["bit_depth"]
            if not bitrate and ff_info.get("bitrate"):
                bitrate = ff_info["bitrate"]

    # Defaults if stream metrics unresolved
    if not sample_rate:
        sample_rate = 44100
    if not bit_depth:
        bit_depth = 16 if format_name != "FLAC" else 24

    return {
        "file_path": file_path,
        "file_format": format_name,
        "file_size": file_size,
        "file_hash": sha256,
        "isrc": isrc.strip() if isrc else None,
        "title": title.strip() if title else None,
        "artist": artist.strip() if artist else None,
        "album": album.strip() if album else None,
        "sample_rate": sample_rate,
        "bit_depth": bit_depth,
        "bitrate": bitrate,
        "audio_source": audio_source.lower() if audio_source else "qobuz",
        "service_track_id": service_track_id,
    }


def find_track_id(cursor: sqlite3.Cursor, metadata: Dict[str, Any]) -> Tuple[Optional[int], str]:
    """
    Resolve track_id in SQLite tracks table using unambiguous exact matching.
    Priority:
    1. ISRC exact match
    2. service_track_id match in track_sources
    3. Unambiguous canonical Title + Artist match
    """
    isrc = metadata.get("isrc")
    if isrc:
        cursor.execute("SELECT id FROM tracks WHERE UPPER(TRIM(isrc)) = UPPER(TRIM(?))", (isrc,))
        rows = cursor.fetchall()
        if len(rows) == 1:
            return rows[0][0], "isrc"
        elif len(rows) > 1:
            return rows[0][0], "isrc_multi"

    stid = metadata.get("service_track_id")
    if stid:
        cursor.execute("SELECT track_id FROM track_sources WHERE service_track_id = ? AND available = 1", (stid,))
        rows = cursor.fetchall()
        if len(rows) == 1:
            return rows[0][0], "service_track_id"

    title = metadata.get("title")
    artist = metadata.get("artist")
    if title and artist:
        clean_title = title.strip(' "')
        clean_artist = artist.strip(' "')
        cursor.execute(
            """
            SELECT t.id FROM tracks t
            JOIN track_artists ta ON t.id = ta.track_id
            JOIN artists a ON ta.artist_id = a.id
            WHERE (LOWER(TRIM(t.title)) = LOWER(?) OR LOWER(TRIM(t.title)) = LOWER(?))
              AND (LOWER(TRIM(a.name)) = LOWER(?) OR LOWER(TRIM(a.name)) = LOWER(?))
            """,
            (title, clean_title, artist, clean_artist)
        )
        rows = cursor.fetchall()
        if len(rows) == 1:
            return rows[0][0], "title_artist"

    return None, "none"


def scan_storage_files(music_dir: Path) -> Tuple[List[str], List[str]]:
    """Scan music directory for valid audio files and orphaned .staging residual files."""
    audio_files = []
    staging_residuals = []

    for root, dirs, files in os.walk(music_dir):
        # Check if currently inside a .staging folder
        path_parts = Path(root).parts
        in_staging = any(p.lower() == ".staging" for p in path_parts)

        for f in files:
            full_path = os.path.join(root, f)
            ext = os.path.splitext(f)[1].lower()

            if in_staging:
                if ext in STAGING_EXTENSIONS or ext in AUDIO_EXTENSIONS or ext in (".webp", ".jpg", ".png"):
                    staging_residuals.append(full_path)
            else:
                if ext in AUDIO_EXTENSIONS:
                    audio_files.append(full_path)
                elif ext in STAGING_EXTENSIONS:
                    staging_residuals.append(full_path)

    return sorted(audio_files), sorted(staging_residuals)


def reconcile_downloads(
    music_dir: str,
    db_path: str,
    dry_run: bool = False,
    purge_staging: bool = True,
    verbose: bool = False,
) -> Dict[str, Any]:
    """Execute end-to-end reconciliation between storage and SQLite database."""
    music_path = Path(os.path.expanduser(music_dir)).resolve()
    db_file = Path(os.path.expanduser(db_path)).resolve()

    if not music_path.exists():
        raise FileNotFoundError(f"Music directory not found: {music_path}")
    if not db_file.exists():
        raise FileNotFoundError(f"SQLite database not found: {db_file}")

    start_time = time.time()
    audio_files, staging_files = scan_storage_files(music_path)

    if dry_run:
        try:
            conn = sqlite3.connect(f"file:{db_file}?immutable=1", uri=True)
            conn.execute("PRAGMA foreign_keys = ON;")
            cursor = conn.cursor()
            cursor.execute("PRAGMA table_info(downloads);")
        except Exception:
            conn = sqlite3.connect(str(db_file))
            conn.execute("PRAGMA foreign_keys = ON;")
            cursor = conn.cursor()
    else:
        conn = sqlite3.connect(str(db_file))
        conn.execute("PRAGMA foreign_keys = ON;")
        cursor = conn.cursor()

    # Discover available columns in downloads table
    cursor.execute("PRAGMA table_info(downloads);")
    download_cols = {row[1] for row in cursor.fetchall()}

    has_effective_service = "effective_service" in download_cols
    has_effective_track_id = "effective_service_track_id" in download_cols
    has_match_method = "match_method" in download_cols
    has_match_confidence = "match_confidence" in download_cols
    has_requested_quality = "requested_quality" in download_cols
    has_effective_quality = "effective_quality" in download_cols
    has_requested_format = "requested_format" in download_cols
    has_effective_format = "effective_format" in download_cols
    has_quality_decision = "quality_decision" in download_cols
    has_decision_reason = "decision_reason" in download_cols

    matched_by_isrc = 0
    matched_by_service_id = 0
    matched_by_title_artist = 0
    ambiguous_files = []
    relinked_records = 0
    staging_purged = 0

    try:
        if not dry_run:
            conn.execute("BEGIN IMMEDIATE;")

        for fpath in audio_files:
            meta = probe_audio_file(fpath)
            track_id, method = find_track_id(cursor, meta)

            if not track_id:
                ambiguous_files.append({
                    "file_path": fpath,
                    "isrc": meta.get("isrc"),
                    "title": meta.get("title"),
                    "artist": meta.get("artist"),
                    "reason": "No exact match found in tracks table",
                })
                if verbose:
                    print(f"[UNMATCHED] {fpath} (ISRC: {meta.get('isrc')}, Title: {meta.get('title')})")
                continue

            if method.startswith("isrc"):
                matched_by_isrc += 1
            elif method == "service_track_id":
                matched_by_service_id += 1
            elif method == "title_artist":
                matched_by_title_artist += 1

            # Determine service id (1: Spotify, 2: Qobuz, 3: Tidal)
            svc_name = meta.get("audio_source", "qobuz")
            service_id = 2
            if "tidal" in svc_name:
                service_id = 3
                svc_name = "tidal"
            elif "spotify" in svc_name:
                service_id = 1
                svc_name = "spotify"

            # Determine quality tier
            quality_tier = "lossless"
            if meta["file_format"] == "FLAC":
                if (meta["bit_depth"] and meta["bit_depth"] > 16) or (meta["sample_rate"] and meta["sample_rate"] > 44100):
                    quality_tier = "hires"
            else:
                quality_tier = "high"

            if not dry_run:
                # Construct query dynamically based on table columns
                cols = [
                    "track_id", "source_service_id", "file_path", "file_format",
                    "file_size_bytes", "file_hash", "bit_depth", "sample_rate",
                    "metadata_completeness", "downloaded_at"
                ]
                vals = [
                    track_id, service_id, fpath, meta["file_format"],
                    meta["file_size"], meta["file_hash"], meta["bit_depth"], meta["sample_rate"],
                    100
                ]
                placeholders = ["?", "?", "?", "?", "?", "?", "?", "?", "?", "CURRENT_TIMESTAMP"]

                if has_effective_service:
                    cols.append("effective_service")
                    vals.append(svc_name)
                    placeholders.append("?")
                if has_effective_track_id:
                    cols.append("effective_service_track_id")
                    vals.append(meta.get("service_track_id"))
                    placeholders.append("?")
                if has_match_method:
                    cols.append("match_method")
                    vals.append(method)
                    placeholders.append("?")
                if has_match_confidence:
                    cols.append("match_confidence")
                    vals.append(1.0)
                    placeholders.append("?")
                if has_requested_quality:
                    cols.append("requested_quality")
                    vals.append(quality_tier)
                    placeholders.append("?")
                if has_effective_quality:
                    cols.append("effective_quality")
                    vals.append(quality_tier)
                    placeholders.append("?")
                if has_requested_format:
                    cols.append("requested_format")
                    vals.append(meta["file_format"])
                    placeholders.append("?")
                if has_effective_format:
                    cols.append("effective_format")
                    vals.append(meta["file_format"])
                    placeholders.append("?")
                if has_quality_decision:
                    cols.append("quality_decision")
                    vals.append("direct_match")
                    placeholders.append("?")
                if has_decision_reason:
                    cols.append("decision_reason")
                    vals.append("storage_reconciliation")
                    placeholders.append("?")

                cols_str = ", ".join(cols)
                placeholders_str = ", ".join(placeholders)

                update_clauses = [
                    "source_service_id = excluded.source_service_id",
                    "file_path = excluded.file_path",
                    "file_format = excluded.file_format",
                    "file_size_bytes = excluded.file_size_bytes",
                    "file_hash = excluded.file_hash",
                    "bit_depth = excluded.bit_depth",
                    "sample_rate = excluded.sample_rate",
                    "metadata_completeness = excluded.metadata_completeness",
                ]
                if has_effective_service:
                    update_clauses.append("effective_service = excluded.effective_service")
                if has_effective_track_id:
                    update_clauses.append("effective_service_track_id = excluded.effective_service_track_id")
                if has_match_method:
                    update_clauses.append("match_method = excluded.match_method")

                sql = f"""
                    INSERT INTO downloads ({cols_str})
                    VALUES ({placeholders_str})
                    ON CONFLICT(track_id) DO UPDATE SET
                    {', '.join(update_clauses)}
                """

                cursor.execute(sql, vals)

            relinked_records += 1
            if verbose:
                print(f"[RELINKED] track_id={track_id} <- {fpath} ({method})")

        # Purge staging residuals
        if purge_staging:
            for sfile in staging_files:
                if not dry_run:
                    try:
                        os.remove(sfile)
                        staging_purged += 1
                        if verbose:
                            print(f"[PURGED STAGING] {sfile}")
                    except Exception as e:
                        if verbose:
                            print(f"[ERROR PURGING] {sfile}: {e}", file=sys.stderr)
                else:
                    staging_purged += 1
                    if verbose:
                        print(f"[DRY-RUN PURGE] {sfile}")

        if not dry_run:
            conn.commit()

    except Exception:
        if not dry_run:
            conn.rollback()
        raise
    finally:
        conn.close()

    duration = time.time() - start_time

    return {
        "success": True,
        "dry_run": dry_run,
        "scanned_audio_files": len(audio_files),
        "relinked_downloads": relinked_records,
        "matched_by_isrc": matched_by_isrc,
        "matched_by_service_id": matched_by_service_id,
        "matched_by_title_artist": matched_by_title_artist,
        "ambiguous_count": len(ambiguous_files),
        "ambiguous_files": ambiguous_files,
        "staging_residuals_scanned": len(staging_files),
        "staging_residuals_purged": staging_purged,
        "duration_seconds": round(duration, 3),
    }


def main():
    parser = argparse.ArgumentParser(
        description="Reconcile physical storage audio files with Syncify SQLite downloads ledger."
    )
    parser.add_argument(
        "--music-dir", "-m",
        default="~/Music/Syncify",
        help="Path to root music directory (default: ~/Music/Syncify)",
    )
    parser.add_argument(
        "--db", "-d",
        default="~/.local/share/com.syncify.app/syncify.db",
        help="Path to SQLite database (default: ~/.local/share/com.syncify.app/syncify.db)",
    )
    parser.add_argument(
        "--dry-run",
        action="store_true",
        help="Simulate scan and report planned actions without modifying SQLite or disk",
    )
    parser.add_argument(
        "--no-purge-staging",
        action="store_true",
        help="Disable automatic purging of .staging residual files",
    )
    parser.add_argument(
        "--json",
        action="store_true",
        help="Output results strictly formatted as JSON",
    )
    parser.add_argument(
        "--verbose", "-v",
        action="store_true",
        help="Print verbose logs for each scanned and resolved file",
    )

    args = parser.parse_args()

    try:
        res = reconcile_downloads(
            music_dir=args.music_dir,
            db_path=args.db,
            dry_run=args.dry_run,
            purge_staging=not args.no_purge_staging,
            verbose=args.verbose,
        )
        if args.json:
            print(json.dumps(res, indent=2))
        else:
            print("=" * 65)
            print(f"SYNCIFY STORAGE RECONCILIATION SUMMARY {'(DRY-RUN)' if res['dry_run'] else ''}")
            print("=" * 65)
            print(f"Physical Audio Files Scanned : {res['scanned_audio_files']}")
            print(f"Successfully Relinked        : {res['relinked_downloads']}")
            print(f"  - Matched by ISRC          : {res['matched_by_isrc']}")
            print(f"  - Matched by Service ID    : {res['matched_by_service_id']}")
            print(f"  - Matched by Title+Artist  : {res['matched_by_title_artist']}")
            print(f"Ambiguous / Unmatched Files  : {res['ambiguous_count']}")
            print(f"Staging Residuals Purged     : {res['staging_residuals_purged']}")
            print(f"Execution Duration           : {res['duration_seconds']}s")
            print("=" * 65)
        sys.exit(0)
    except Exception as e:
        if args.json:
            print(json.dumps({"success": False, "error": str(e)}))
        else:
            print(f"Error during reconciliation: {e}", file=sys.stderr)
        sys.exit(1)


if __name__ == "__main__":
    main()

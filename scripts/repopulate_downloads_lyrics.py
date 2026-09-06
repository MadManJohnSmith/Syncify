#!/usr/bin/env python3
"""
Repopulate 'downloads' and 'lyrics' tables in syncify.db from physical audio
and lyric files located in /home/alan/Music/Syncify, and materialize missing sidecars (.lrc and covers).
Tasks: F0.7 & TASK-111 (Completitud de Sidecars).
"""

import os
import sys
import json
import hashlib
import sqlite3
import subprocess
import argparse
from datetime import datetime
import urllib.request

MUSIC_DIR = "/home/alan/Music/Syncify"
DB_PATH = os.path.expanduser("~/.local/share/com.syncify.app/syncify.db")


def compute_sha256(file_path: str) -> str:
    h = hashlib.sha256()
    with open(file_path, "rb") as f:
        while chunk := f.read(65536):
            h.update(chunk)
    return h.hexdigest()


def probe_file(file_path: str):
    cmd = [
        "ffprobe",
        "-v", "quiet",
        "-print_format", "json",
        "-show_format",
        "-show_streams",
        file_path,
    ]
    res = subprocess.run(cmd, capture_output=True, text=True)
    if res.returncode != 0:
        return None
    try:
        return json.loads(res.stdout)
    except Exception:
        return None


def create_backup_snapshot(db_path: str) -> str:
    """Create a preventative VACUUM INTO snapshot in /tmp before modifying the database."""
    timestamp = datetime.now().strftime("%Y%m%d_%H%M%S")
    backup_path = f"/tmp/syncify_backup_pre_repair_TASK-111_{timestamp}.db"
    try:
        conn = sqlite3.connect(db_path)
        conn.execute(f"VACUUM INTO '{backup_path}';")
        conn.close()
        print(f"[Snapshot] Preventative backup created: {backup_path}")
        return backup_path
    except Exception as e:
        print(f"[Snapshot Warning] Failed to create VACUUM INTO snapshot: {e}", file=sys.stderr)
        return ""


def repopulate_from_audio(conn, music_dir: str, dry_run: bool = False):
    """Scan music_dir for audio and LRC files and repopulate downloads and lyrics tables."""
    c = conn.cursor()

    audio_files = []
    for root, _, files in os.walk(music_dir):
        for f in files:
            if f.lower().endswith((".flac", ".m4a")):
                audio_files.append(os.path.join(root, f))

    print(f"Found {len(audio_files)} physical audio files in {music_dir}")

    processed_audios = 0
    matched_tracks = 0
    downloads_inserted = 0
    path_to_track_id = {}

    for path in audio_files:
        processed_audios += 1
        probe_data = probe_file(path)
        if not probe_data:
            print(f"Warning: Failed to probe {path}", file=sys.stderr)
            continue

        format_info = probe_data.get("format", {})
        streams = probe_data.get("streams", [])
        raw_tags = format_info.get("tags", {})
        tags = {k.upper(): v for k, v in raw_tags.items()}

        audio_stream = None
        for s in streams:
            if s.get("codec_type") == "audio":
                audio_stream = s
                break
        if not audio_stream and streams:
            audio_stream = streams[0]

        codec = (audio_stream.get("codec_name") or "").lower() if audio_stream else ""

        if path.lower().endswith(".flac") or codec == "flac":
            file_format = "FLAC"
        elif codec == "alac":
            file_format = "ALAC"
        elif path.lower().endswith(".m4a") or codec in ("aac", "mp4a"):
            file_format = "AAC"
        else:
            file_format = None

        file_size_bytes = os.path.getsize(path)
        file_hash = compute_sha256(path)

        raw_bits = (audio_stream.get("bits_per_raw_sample") or tags.get("BITDEPTH")) if audio_stream else tags.get("BITDEPTH")
        try:
            bit_depth = int(raw_bits) if raw_bits and int(raw_bits) in (16, 24, 32) else None
        except (ValueError, TypeError):
            bit_depth = None

        raw_sr = (audio_stream.get("sample_rate") or tags.get("SAMPLINGRATE")) if audio_stream else tags.get("SAMPLINGRATE")
        try:
            sample_rate = int(raw_sr) if raw_sr and int(raw_sr) > 0 else None
        except (ValueError, TypeError):
            sample_rate = None

        isrc = tags.get("ISRC", "").strip()
        title = tags.get("TITLE", "").strip()
        artist = tags.get("ARTIST", "").strip()
        src_tag = tags.get("SYNCIFY_AUDIO_SOURCE") or tags.get("SOURCE") or ""
        mb_release = tags.get("MUSICBRAINZ_ALBUMID") or tags.get("MUSICBRAINZ_RELEASEGROUPID")

        track_row = None
        match_method = None

        # A: by ISRC
        if isrc:
            c.execute(
                "SELECT id, title, isrc, qobuz_id, spotify_id, file_disambiguator FROM tracks WHERE UPPER(isrc) = UPPER(?)",
                (isrc,)
            )
            track_row = c.fetchone()
            if track_row:
                match_method = "isrc"

        # B: by Title and Artist
        if not track_row and title and artist:
            clean_title = title.strip(' "')
            c.execute(
                """
                SELECT t.id, t.title, t.isrc, t.qobuz_id, t.spotify_id, t.file_disambiguator
                FROM tracks t
                JOIN track_artists ta ON t.id = ta.track_id
                JOIN artists a ON ta.artist_id = a.id
                WHERE (t.title = ? OR t.title = ? OR LOWER(t.title) = LOWER(?))
                  AND (a.name = ? OR LOWER(a.name) = LOWER(?))
                """,
                (title, clean_title, clean_title, artist, artist)
            )
            track_row = c.fetchone()
            if track_row:
                match_method = "title_artist"

        # C: by Title / Album
        if not track_row and title:
            clean_title = title.strip(' "')
            album = tags.get("ALBUM", "").strip()
            if album:
                c.execute(
                    """
                    SELECT t.id, t.title, t.isrc, t.qobuz_id, t.spotify_id, t.file_disambiguator
                    FROM tracks t
                    JOIN albums al ON t.album_id = al.id
                    WHERE (t.title = ? OR t.title = ? OR LOWER(t.title) = LOWER(?))
                      AND (al.title = ? OR LOWER(al.title) = LOWER(?))
                    """,
                    (title, clean_title, clean_title, album, album)
                )
                track_row = c.fetchone()
                if track_row:
                    match_method = "title_album"

        if not track_row:
            print(f"Warning: Could not match track in database for {path} (ISRC: {isrc}, Title: {title})", file=sys.stderr)
            continue

        matched_tracks += 1
        track_id, t_title, t_isrc, qobuz_id, spotify_id, file_disambiguator = track_row
        path_to_track_id[path] = track_id

        if "qobuz" in src_tag.lower():
            source_service_id = 2
            origin_service = "qobuz"
            effective_service = "qobuz"
            effective_service_track_id = qobuz_id
        elif "tidal" in src_tag.lower():
            source_service_id = 3
            origin_service = "tidal"
            effective_service = "tidal"
            effective_service_track_id = None
        else:
            source_service_id = 2
            origin_service = "qobuz"
            effective_service = "qobuz"
            effective_service_track_id = qobuz_id

        if file_format == "FLAC":
            if (bit_depth and bit_depth > 16) or (sample_rate and sample_rate > 44100):
                requested_quality = "hires"
                effective_quality = "hires"
            else:
                requested_quality = "lossless"
                effective_quality = "lossless"
        else:
            requested_quality = "high"
            effective_quality = "high"

        if not dry_run:
            c.execute(
                """
                INSERT INTO downloads (
                    track_id, source_service_id, file_path, file_format, file_size_bytes,
                    file_hash, bit_depth, sample_rate, metadata_completeness, downloaded_at,
                    only_available_on, not_streaming, musicbrainz_release_id, updated_at,
                    origin_service, origin_service_track_id, effective_service, effective_service_track_id,
                    fallback_reason, match_method, match_confidence, file_disambiguator,
                    requested_quality, effective_quality, requested_format, effective_format,
                    quality_decision, provider_fallback_used, quality_fallback_used, decision_reason, skip_reason
                ) VALUES (
                    ?, ?, ?, ?, ?,
                    ?, ?, ?, 100, CURRENT_TIMESTAMP,
                    NULL, 0, ?, CURRENT_TIMESTAMP,
                    ?, ?, ?, ?,
                    NULL, ?, 1.0, ?,
                    ?, ?, ?, ?,
                    'direct_match', 0, 0, 'reconciliation_backfill', NULL
                )
                ON CONFLICT(track_id) DO UPDATE SET
                    source_service_id = excluded.source_service_id,
                    file_path = excluded.file_path,
                    file_format = excluded.file_format,
                    file_size_bytes = excluded.file_size_bytes,
                    file_hash = excluded.file_hash,
                    bit_depth = excluded.bit_depth,
                    sample_rate = excluded.sample_rate,
                    metadata_completeness = excluded.metadata_completeness,
                    updated_at = CURRENT_TIMESTAMP,
                    origin_service = excluded.origin_service,
                    origin_service_track_id = excluded.origin_service_track_id,
                    effective_service = excluded.effective_service,
                    effective_service_track_id = excluded.effective_service_track_id,
                    match_method = excluded.match_method,
                    match_confidence = excluded.match_confidence,
                    requested_quality = excluded.requested_quality,
                    effective_quality = excluded.effective_quality,
                    requested_format = excluded.requested_format,
                    effective_format = excluded.effective_format,
                    quality_decision = excluded.quality_decision,
                    decision_reason = excluded.decision_reason
                """,
                (
                    track_id, source_service_id, path, file_format, file_size_bytes,
                    file_hash, bit_depth, sample_rate, mb_release,
                    origin_service, effective_service_track_id, effective_service, effective_service_track_id,
                    match_method, file_disambiguator,
                    requested_quality, effective_quality, file_format, file_format
                )
            )
        downloads_inserted += 1

    # Discover and insert all LRC files into lyrics table
    lrc_files = []
    for root, _, files in os.walk(music_dir):
        for f in files:
            if f.lower().endswith(".lrc"):
                lrc_files.append(os.path.join(root, f))

    print(f"Found {len(lrc_files)} physical .lrc files in {music_dir}")

    lyrics_inserted = 0
    for lrc_path in lrc_files:
        folder = os.path.dirname(lrc_path)
        base_name = os.path.splitext(os.path.basename(lrc_path))[0]

        flac_candidate = os.path.join(folder, base_name + ".flac")
        m4a_candidate = os.path.join(folder, base_name + ".m4a")

        target_audio = None
        if os.path.exists(flac_candidate):
            target_audio = flac_candidate
        elif os.path.exists(m4a_candidate):
            target_audio = m4a_candidate
        else:
            audios_in_dir = [
                os.path.join(folder, x)
                for x in os.listdir(folder)
                if x.lower().endswith((".flac", ".m4a"))
            ]
            if len(audios_in_dir) == 1:
                target_audio = audios_in_dir[0]

        if not target_audio or target_audio not in path_to_track_id:
            print(f"Warning: Could not associate LRC file {lrc_path} to an audio file", file=sys.stderr)
            continue

        resolved_track_id = path_to_track_id[target_audio]

        with open(lrc_path, "r", encoding="utf-8") as lf:
            content = lf.read()

        if not dry_run:
            c.execute(
                """
                INSERT INTO lyrics (
                    track_id, format, sync_level, source, content, embedded_in_file, created_at
                ) VALUES (
                    ?, 'lrc', 'line', 'local_lrc', ?, 0, CURRENT_TIMESTAMP
                )
                ON CONFLICT(track_id, format) DO UPDATE SET
                    sync_level = excluded.sync_level,
                    source = excluded.source,
                    content = excluded.content,
                    embedded_in_file = excluded.embedded_in_file
                """,
                (resolved_track_id, content)
            )
        lyrics_inserted += 1

    if not dry_run:
        conn.commit()

    return {
        "processed_audios": processed_audios,
        "matched_tracks": matched_tracks,
        "downloads_inserted": downloads_inserted,
        "lrc_files": len(lrc_files),
        "lyrics_inserted": lyrics_inserted,
    }


def materialize_lrc_sidecars(conn, dry_run: bool = False):
    """Materialize missing physical .lrc files from lyrics table for downloaded tracks."""
    c = conn.cursor()
    c.execute(
        """
        SELECT d.id, d.track_id, d.file_path, l.format, l.content, l.sync_level
        FROM downloads d
        JOIN lyrics l ON d.track_id = l.track_id
        WHERE d.file_path IS NOT NULL AND d.file_path != ''
        ORDER BY d.track_id,
            CASE l.format
                WHEN 'lrc' THEN 1
                WHEN 'ttml' THEN 2
                WHEN 'plain' THEN 3
                ELSE 4
            END
        """
    )
    rows = c.fetchall()
    seen_tracks = set()
    scanned = 0
    already_present = 0
    materialized = 0
    missing_audio = 0
    failed = 0

    for row in rows:
        dl_id, track_id, file_path, fmt, content, sync_level = row
        if track_id in seen_tracks:
            continue
        seen_tracks.add(track_id)
        scanned += 1

        if not os.path.exists(file_path):
            missing_audio += 1
            continue

        lrc_path = os.path.splitext(file_path)[0] + ".lrc"
        if os.path.exists(lrc_path) and os.path.getsize(lrc_path) > 0:
            already_present += 1
            continue

        if not content or not content.strip():
            continue

        if dry_run:
            print(f"[Dry-run] Would materialize LRC: {lrc_path} (track_id={track_id}, len={len(content)})")
            materialized += 1
        else:
            try:
                with open(lrc_path, "w", encoding="utf-8") as f:
                    f.write(content)
                print(f"[LRC Materialized] Wrote: {lrc_path}")
                materialized += 1
            except Exception as e:
                print(f"[Error] Failed to write {lrc_path}: {e}", file=sys.stderr)
                failed += 1

    summary = {
        "scanned": scanned,
        "already_present": already_present,
        "materialized": materialized,
        "missing_audio": missing_audio,
        "failed": failed,
    }
    print(f"LRC Materialization: scanned={scanned}, already_present={already_present}, materialized={materialized}, missing_audio={missing_audio}, failed={failed}")
    return summary


def materialize_covers(conn, dry_run: bool = False):
    """Materialize missing album covers from embedded tags or cover_art_url."""
    c = conn.cursor()
    c.execute(
        """
        SELECT DISTINCT al.id, al.title, al.cover_art_url, d.file_path
        FROM downloads d
        JOIN tracks t ON d.track_id = t.id
        JOIN albums al ON t.album_id = al.id
        WHERE d.file_path IS NOT NULL AND d.file_path != ''
        ORDER BY al.id
        """
    )
    rows = c.fetchall()

    folder_to_album = {}
    for al_id, al_title, cover_url, file_path in rows:
        folder = os.path.dirname(file_path)
        if folder not in folder_to_album:
            folder_to_album[folder] = {
                "album_id": al_id,
                "album_title": al_title,
                "cover_url": cover_url,
                "audio_files": []
            }
        folder_to_album[folder]["audio_files"].append(file_path)

    scanned_albums = len(folder_to_album)
    already_present = 0
    materialized_embedded = 0
    materialized_url = 0
    missing_url = 0
    failed = 0

    for folder, info in folder_to_album.items():
        if not os.path.isdir(folder):
            continue

        cover_jpg = os.path.join(folder, "cover.jpg")
        cover_webp = os.path.join(folder, "cover.webp")
        cover_png = os.path.join(folder, "cover.png")

        # INVARIANTE SYMFONIUM: If cover.webp or cover.jpg exists, NEVER overwrite
        has_valid_cover = False
        for cpath in (cover_webp, cover_jpg, cover_png):
            if os.path.exists(cpath) and os.path.getsize(cpath) > 0:
                has_valid_cover = True
                break

        if has_valid_cover:
            already_present += 1
            continue

        # Try 1: Extract embedded cover from audio files
        extracted = False
        for af in info["audio_files"]:
            if not os.path.exists(af):
                continue

            if dry_run:
                probe = probe_file(af)
                if probe:
                    for st in probe.get("streams", []):
                        if st.get("codec_name") in ("mjpeg", "png", "jpeg"):
                            print(f"[Dry-run] Would extract embedded cover from {af} -> {cover_jpg}")
                            extracted = True
                            materialized_embedded += 1
                            break
                if extracted:
                    break
            else:
                cmd = ["ffmpeg", "-y", "-i", af, "-an", "-vcodec", "copy", cover_jpg]
                res = subprocess.run(cmd, capture_output=True)
                if res.returncode == 0 and os.path.exists(cover_jpg) and os.path.getsize(cover_jpg) > 0:
                    print(f"[Cover Materialized] Extracted artwork from {af} -> {cover_jpg}")
                    materialized_embedded += 1
                    extracted = True
                    break

        if extracted:
            continue

        # Try 2: Download from cover_url
        cover_url = info.get("cover_url")
        if cover_url and cover_url.startswith("http"):
            if dry_run:
                print(f"[Dry-run] Would download cover from {cover_url} -> {cover_jpg}")
                materialized_url += 1
            else:
                try:
                    req = urllib.request.Request(cover_url, headers={"User-Agent": "Syncify/1.0"})
                    with urllib.request.urlopen(req, timeout=10) as response, open(cover_jpg, "wb") as out_file:
                        data = response.read()
                        if data:
                            out_file.write(data)
                            print(f"[Cover Materialized] Downloaded {cover_url} -> {cover_jpg}")
                            materialized_url += 1
                        else:
                            failed += 1
                except Exception as e:
                    print(f"[Warning] Failed to download cover for {folder} from {cover_url}: {e}", file=sys.stderr)
                    failed += 1
        else:
            missing_url += 1

    summary = {
        "scanned": scanned_albums,
        "already_present": already_present,
        "materialized_embedded": materialized_embedded,
        "materialized_url": materialized_url,
        "missing_url": missing_url,
        "failed": failed,
    }
    print(f"Cover Materialization: scanned={scanned_albums}, already_present={already_present}, materialized_embedded={materialized_embedded}, materialized_url={materialized_url}, missing_url={missing_url}, failed={failed}")
    return summary


def main():
    parser = argparse.ArgumentParser(
        description="Repopulate downloads & lyrics tables and materialize missing sidecars (.lrc and covers) [TASK-111]"
    )
    parser.add_argument(
        "db_arg",
        nargs="?",
        default=None,
        help="Optional positional database path (for backwards compatibility)"
    )
    parser.add_argument(
        "--db-path",
        default=None,
        help=f"Path to SQLite database (default: {DB_PATH})"
    )
    parser.add_argument(
        "--music-dir",
        default=MUSIC_DIR,
        help=f"Path to music library root directory (default: {MUSIC_DIR})"
    )
    parser.add_argument(
        "--dry-run",
        action="store_true",
        help="Report planned modifications without writing to disk or database"
    )
    parser.add_argument(
        "--materialize-lrc",
        action="store_true",
        help="Materialize missing physical .lrc sidecars from database lyrics"
    )
    parser.add_argument(
        "--materialize-covers",
        action="store_true",
        help="Materialize missing album covers from embedded tags or cover_art_url"
    )
    parser.add_argument(
        "--repopulate",
        action="store_true",
        help="Scan files and repopulate downloads and lyrics tables"
    )
    parser.add_argument(
        "--skip-backup",
        action="store_true",
        help="Skip creation of preventative VACUUM INTO snapshot"
    )

    args = parser.parse_args()

    db_target = args.db_path or args.db_arg or DB_PATH
    music_dir = args.music_dir

    if not os.path.isdir(music_dir) and (args.repopulate or (not args.materialize_lrc and not args.materialize_covers)):
        print(f"Warning: Music directory not found: {music_dir}", file=sys.stderr)

    if not os.path.isfile(db_target):
        print(f"Error: Database file not found: {db_target}", file=sys.stderr)
        sys.exit(1)

    print(f"Connecting to database: {db_target} (dry_run={args.dry_run})")

    # Snapshot preventivo con VACUUM INTO
    if not args.dry_run and not args.skip_backup:
        create_backup_snapshot(db_target)

    if args.dry_run:
        conn = sqlite3.connect(f"file:{os.path.abspath(db_target)}?immutable=1", uri=True)
    else:
        conn = sqlite3.connect(db_target)
        conn.execute("PRAGMA foreign_keys = ON;")

    # Determine actions to execute
    explicit_actions = args.materialize_lrc or args.materialize_covers or args.repopulate
    run_repopulate = args.repopulate or not explicit_actions
    run_lrc = args.materialize_lrc or not explicit_actions
    run_covers = args.materialize_covers or not explicit_actions

    if run_repopulate and os.path.isdir(music_dir):
        repopulate_from_audio(conn, music_dir, dry_run=args.dry_run)

    if run_lrc:
        materialize_lrc_sidecars(conn, dry_run=args.dry_run)

    if run_covers:
        materialize_covers(conn, dry_run=args.dry_run)

    # Foreign key integrity verification
    c = conn.cursor()
    c.execute("PRAGMA foreign_key_check;")
    fk_violations = c.fetchall()
    if fk_violations:
        print(f"Warning: Foreign key violations detected: {fk_violations}", file=sys.stderr)
    else:
        print("Foreign key check passed: 0 violations.")

    c.execute("SELECT COUNT(*) FROM downloads;")
    total_downloads = c.fetchone()[0]

    c.execute("SELECT COUNT(*) FROM lyrics;")
    total_lyrics = c.fetchone()[0]

    conn.close()

    print("=" * 60)
    print("TASK-111 COMPLETION STATUS:")
    print(f"  Downloads table count: {total_downloads}")
    print(f"  Lyrics table count: {total_lyrics}")
    print("=" * 60)


if __name__ == "__main__":
    main()

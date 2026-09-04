#!/usr/bin/env python3
"""
Repopulate 'downloads' and 'lyrics' tables in syncify.db from physical audio
and lyric files located in /home/alan/Music/Syncify.
Task: F0.7 / Backfill físico de descargas y letras.
"""

import os
import sys
import json
import hashlib
import sqlite3
import subprocess

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


def main():
    db_target = sys.argv[1] if len(sys.argv) > 1 else DB_PATH
    if not os.path.isdir(MUSIC_DIR):
        print(f"Error: Music directory not found: {MUSIC_DIR}", file=sys.stderr)
        sys.exit(1)

    if not os.path.isfile(db_target):
        print(f"Error: Database file not found: {db_target}", file=sys.stderr)
        sys.exit(1)

    print(f"Connecting to database: {db_target}")
    conn = sqlite3.connect(db_target)
    conn.execute("PRAGMA foreign_keys = ON;")
    c = conn.cursor()

    # Step 1: Discover all audio files
    audio_files = []
    for root, _, files in os.walk(MUSIC_DIR):
        for f in files:
            if f.lower().endswith((".flac", ".m4a")):
                audio_files.append(os.path.join(root, f))

    print(f"Found {len(audio_files)} physical audio files in {MUSIC_DIR}")

    processed_audios = 0
    matched_tracks = 0
    downloads_inserted = 0

    # Map to hold path -> track_id for lyric resolution
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

        # Cross reference against tracks
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

        # Determine services
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

        # Determine quality
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

        # Insert / Update into downloads table
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

    # Step 2: Discover and insert all LRC files
    lrc_files = []
    for root, _, files in os.walk(MUSIC_DIR):
        for f in files:
            if f.lower().endswith(".lrc"):
                lrc_files.append(os.path.join(root, f))

    print(f"Found {len(lrc_files)} physical .lrc files in {MUSIC_DIR}")

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

    # Commit transaction
    conn.commit()

    # Also export standalone SQL file for portability / direct execution
    sql_export_path = os.path.join(os.path.dirname(__file__), "repopulate_downloads_lyrics.sql")
    try:
        with open(sql_export_path, "w", encoding="utf-8") as sf:
            sf.write("-- Auto-generated backfill script for downloads and lyrics\n")
            sf.write("BEGIN TRANSACTION;\n\n")
            
            c.execute("SELECT track_id, source_service_id, file_path, file_format, file_size_bytes, file_hash, bit_depth, sample_rate, metadata_completeness, musicbrainz_release_id, origin_service, origin_service_track_id, effective_service, effective_service_track_id, match_method, file_disambiguator, requested_quality, effective_quality, requested_format, effective_format, quality_decision, provider_fallback_used, quality_fallback_used, decision_reason FROM downloads;")
            for row in c.fetchall():
                def sql_val(v):
                    if v is None:
                        return "NULL"
                    if isinstance(v, (int, float)):
                        return str(v)
                    v_str = str(v).replace("'", "''")
                    return f"'{v_str}'"
                
                vals = [sql_val(x) for x in row]
                sf.write(f"""INSERT INTO downloads (
    track_id, source_service_id, file_path, file_format, file_size_bytes,
    file_hash, bit_depth, sample_rate, metadata_completeness, downloaded_at,
    only_available_on, not_streaming, musicbrainz_release_id, updated_at,
    origin_service, origin_service_track_id, effective_service, effective_service_track_id,
    fallback_reason, match_method, match_confidence, file_disambiguator,
    requested_quality, effective_quality, requested_format, effective_format,
    quality_decision, provider_fallback_used, quality_fallback_used, decision_reason, skip_reason
) VALUES (
    {vals[0]}, {vals[1]}, {vals[2]}, {vals[3]}, {vals[4]},
    {vals[5]}, {vals[6]}, {vals[7]}, {vals[8]}, CURRENT_TIMESTAMP,
    NULL, 0, {vals[9]}, CURRENT_TIMESTAMP,
    {vals[10]}, {vals[11]}, {vals[12]}, {vals[13]},
    NULL, {vals[14]}, 1.0, {vals[15]},
    {vals[16]}, {vals[17]}, {vals[18]}, {vals[19]},
    {vals[20]}, {vals[21]}, {vals[22]}, {vals[23]}, NULL
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
    decision_reason = excluded.decision_reason;\n\n""")
                
            c.execute("SELECT track_id, format, sync_level, source, content, embedded_in_file FROM lyrics;")
            for row in c.fetchall():
                c_content = str(row[4]).replace("'", "''")
                sf.write(f"""INSERT INTO lyrics (track_id, format, sync_level, source, content, embedded_in_file, created_at)
VALUES ({row[0]}, '{row[1]}', '{row[2]}', '{row[3]}', '{c_content}', {row[5]}, CURRENT_TIMESTAMP)
ON CONFLICT(track_id, format) DO UPDATE SET
    sync_level = excluded.sync_level,
    source = excluded.source,
    content = excluded.content,
    embedded_in_file = excluded.embedded_in_file;\n\n""")
                
            sf.write("COMMIT;\n")
        print(f"Exported SQL script to {sql_export_path}")
    except Exception as e:
        print(f"Warning: Failed to export SQL script: {e}", file=sys.stderr)

    # Step 3: Verify foreign key integrity
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
    print("BACKFILL SUMMARY:")
    print(f"  Canciones (archivos de audio) procesadas: {processed_audios}")
    print(f"  Canciones asociadas con éxito en 'tracks': {matched_tracks}")
    print(f"  Filas insertadas/actualizadas en 'downloads': {downloads_inserted} (Total tabla: {total_downloads})")
    print(f"  Archivos LRC procesados: {len(lrc_files)}")
    print(f"  Filas insertadas/actualizadas en 'lyrics': {lyrics_inserted} (Total tabla: {total_lyrics})")
    print("=" * 60)


if __name__ == "__main__":
    main()

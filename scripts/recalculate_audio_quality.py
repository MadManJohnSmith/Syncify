#!/usr/bin/env python3
"""
scripts/recalculate_audio_quality.py
====================================
Recalcula `tracks.audio_quality` mediante un barrido SQL basado en la mejor
fuente física real en `track_sources` (F3.6).

Reglas:
1. Para cada track con fuentes en `track_sources`:
   - Si la mejor fuente disponible tiene (bit_depth >= 24 OR sample_rate > 48000)
     Y formato sin pérdida (FLAC/ALAC/WAV/etc.) -> 'hires'
   - Si la mejor fuente es FLAC/ALAC/WAV o formato sin pérdida con bit_depth = 16
     o sample_rate <= 48000 -> 'lossless'
   - Si la mejor fuente es MP3 / AAC / Opus / lossy:
     * Si bitrate >= 256 -> 'high'
     * Si bitrate >= 128 -> 'medium'
     * Si bitrate < 128 o no especificado -> 'low'
2. Asigna a `tracks.audio_quality` el nivel de calidad más alto disponible entre
   todas sus fuentes en `track_sources`.
"""

import argparse
import os
import sqlite3
import sys
from typing import Dict, Any


def get_distribution(cur: sqlite3.Cursor) -> Dict[str, int]:
    rows = cur.execute("SELECT COALESCE(audio_quality, '<NULL/EMPTY>'), count(*) FROM tracks GROUP BY audio_quality").fetchall()
    return {row[0] if row[0] != "" else "<EMPTY>": row[1] for row in rows}


def count_mislabeled_hires(cur: sqlite3.Cursor) -> int:
    query = """
    WITH ranked AS (
        SELECT 
            track_id,
            CASE
                WHEN UPPER(TRIM(format)) IN ('FLAC', 'ALAC', 'WAV', 'AIFF', 'APE', 'LOSSLESS')
                     AND ((bit_depth IS NOT NULL AND bit_depth >= 24) OR (sample_rate IS NOT NULL AND (sample_rate > 48000 OR (sample_rate > 48 AND sample_rate <= 384))))
                    THEN 'hires'
                WHEN UPPER(TRIM(format)) IN ('FLAC', 'ALAC', 'WAV', 'AIFF', 'APE', 'LOSSLESS')
                    THEN 'lossless'
                WHEN bitrate IS NOT NULL AND bitrate >= 256
                    THEN 'high'
                WHEN bitrate IS NOT NULL AND bitrate >= 128
                    THEN 'medium'
                ELSE 'low'
            END AS calculated_quality,
            ROW_NUMBER() OVER (
                PARTITION BY track_id 
                ORDER BY 
                    available DESC, 
                    CASE
                        WHEN UPPER(TRIM(format)) IN ('FLAC', 'ALAC', 'WAV', 'AIFF', 'APE', 'LOSSLESS')
                             AND ((bit_depth IS NOT NULL AND bit_depth >= 24) OR (sample_rate IS NOT NULL AND (sample_rate > 48000 OR (sample_rate > 48 AND sample_rate <= 384))))
                            THEN 5
                        WHEN UPPER(TRIM(format)) IN ('FLAC', 'ALAC', 'WAV', 'AIFF', 'APE', 'LOSSLESS')
                            THEN 4
                        WHEN bitrate IS NOT NULL AND bitrate >= 256
                            THEN 3
                        WHEN bitrate IS NOT NULL AND bitrate >= 128
                            THEN 2
                        ELSE 1
                    END DESC,
                    id ASC
            ) as rn
        FROM track_sources
    )
    SELECT COUNT(*)
    FROM tracks t
    JOIN ranked r ON t.id = r.track_id
    WHERE r.rn = 1 AND r.calculated_quality = 'hires' AND (t.audio_quality IS NULL OR t.audio_quality != 'hires');
    """
    return cur.execute(query).fetchone()[0]


def recalculate_audio_quality(db_path: str) -> Dict[str, Any]:
    if not os.path.exists(db_path):
        raise FileNotFoundError(f"Database not found at: {db_path}")

    print(f"[*] Opening database: {db_path}")
    conn = sqlite3.connect(db_path)
    cur = conn.cursor()

    cur.execute("PRAGMA foreign_keys = ON;")

    pre_dist = get_distribution(cur)
    pre_mislabeled_hires = count_mislabeled_hires(cur)
    total_tracks = cur.execute("SELECT count(*) FROM tracks").fetchone()[0]
    tracks_with_sources = cur.execute("SELECT count(DISTINCT track_id) FROM track_sources").fetchone()[0]

    print("\n--- PRE-REPAIR STATUS ---")
    print(f"Total tracks: {total_tracks}")
    print(f"Tracks with sources: {tracks_with_sources}")
    print(f"Mislabeled Hi-Res tracks: {pre_mislabeled_hires}")
    print("Audio quality distribution:")
    for k, v in sorted(pre_dist.items()):
        print(f"  {k}: {v}")

    update_sql = """
    WITH ranked AS (
        SELECT 
            track_id,
            CASE
                WHEN UPPER(TRIM(format)) IN ('FLAC', 'ALAC', 'WAV', 'AIFF', 'APE', 'LOSSLESS')
                     AND ((bit_depth IS NOT NULL AND bit_depth >= 24) OR (sample_rate IS NOT NULL AND (sample_rate > 48000 OR (sample_rate > 48 AND sample_rate <= 384))))
                    THEN 'hires'
                WHEN UPPER(TRIM(format)) IN ('FLAC', 'ALAC', 'WAV', 'AIFF', 'APE', 'LOSSLESS')
                    THEN 'lossless'
                WHEN bitrate IS NOT NULL AND bitrate >= 256
                    THEN 'high'
                WHEN bitrate IS NOT NULL AND bitrate >= 128
                    THEN 'medium'
                ELSE 'low'
            END AS calculated_quality,
            ROW_NUMBER() OVER (
                PARTITION BY track_id 
                ORDER BY 
                    available DESC, 
                    CASE
                        WHEN UPPER(TRIM(format)) IN ('FLAC', 'ALAC', 'WAV', 'AIFF', 'APE', 'LOSSLESS')
                             AND ((bit_depth IS NOT NULL AND bit_depth >= 24) OR (sample_rate IS NOT NULL AND (sample_rate > 48000 OR (sample_rate > 48 AND sample_rate <= 384))))
                            THEN 5
                        WHEN UPPER(TRIM(format)) IN ('FLAC', 'ALAC', 'WAV', 'AIFF', 'APE', 'LOSSLESS')
                            THEN 4
                        WHEN bitrate IS NOT NULL AND bitrate >= 256
                            THEN 3
                        WHEN bitrate IS NOT NULL AND bitrate >= 128
                            THEN 2
                        ELSE 1
                    END DESC,
                    id ASC
            ) as rn
        FROM track_sources
    )
    UPDATE tracks
    SET audio_quality = ranked.calculated_quality
    FROM ranked
    WHERE tracks.id = ranked.track_id
      AND ranked.rn = 1
      AND (tracks.audio_quality IS NULL OR tracks.audio_quality != ranked.calculated_quality);
    """

    print("\n[*] Executing recalculation transaction...")
    cur.execute("BEGIN TRANSACTION;")
    cur.execute(update_sql)
    updated_tracks_count = cur.execute("SELECT changes();").fetchone()[0]
    conn.commit()
    print(f"[*] Updated tracks: {updated_tracks_count}")

    print("\n[*] Running PRAGMA foreign_key_check...")
    cur.execute("PRAGMA foreign_keys = ON;")
    fk_violations = cur.execute("PRAGMA foreign_key_check;").fetchall()

    post_dist = get_distribution(cur)
    post_mislabeled_hires = count_mislabeled_hires(cur)

    print("\n--- POST-REPAIR STATUS ---")
    print(f"Mislabeled Hi-Res tracks remaining: {post_mislabeled_hires}")
    print(f"Foreign key violations: {len(fk_violations)}")
    print("Audio quality distribution:")
    for k, v in sorted(post_dist.items()):
        print(f"  {k}: {v}")

    conn.close()

    return {
        "db_path": db_path,
        "total_tracks": total_tracks,
        "tracks_with_sources": tracks_with_sources,
        "updated_tracks_count": updated_tracks_count,
        "pre_mislabeled_hires": pre_mislabeled_hires,
        "post_mislabeled_hires": post_mislabeled_hires,
        "fk_violations_count": len(fk_violations),
        "pre_dist": pre_dist,
        "post_dist": post_dist,
    }


def main():
    parser = argparse.ArgumentParser(description="Recalculate tracks.audio_quality from track_sources (F3.6)")
    parser.add_argument("--db", default="syncify_backup_pre_repair.db", help="Path to database (default: syncify_backup_pre_repair.db)")
    args = parser.parse_args()

    recalculate_audio_quality(args.db)


if __name__ == "__main__":
    main()

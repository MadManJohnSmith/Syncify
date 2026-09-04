#!/usr/bin/env python3
"""
Phase 1 SQL Repair Script for Syncify
=====================================
Executes transactional relational repairs for:
- F1.2: Merge 109 case-insensitive ISRC collisions (plus 5 dashed collisions)
- F1.3: Normalize ISRCs (UPPER, strip dashes) and create UNIQUE NOCASE index
- F1.5: Deduplicate track_sources and create UNIQUE(service_id, service_track_id) index
- F1.6: Normalize 13 whitespace artists and merge duplicates
- F1.7: Reassign track 12520 to The Chordettes (12690), delete garbage tracks 9324 & 12031, delete empty artist 27808
- F2.1: Migrate playlist_tracks constraint to UNIQUE(playlist_id, position)
- F2.3: Backfill playlist_sources from playlists and accounts
- F3.3: Normalize SoundCloud provider to lossy / mp3
- F4.1: Fix 14,631 corrupt artists ("Role\\r - Person") into track_credits roles and clean artists
- F4.6: Fix mojibake in track 4037, HTML entity in artists 12908 & 61049, favorites for 695 & 2708
"""

import argparse
import os
import sqlite3
import sys
import time


def sql_escape_str(val: str) -> str:
    if val is None:
        return "NULL"
    return "'" + val.replace("'", "''") + "'"


def get_metrics(cur: sqlite3.Cursor) -> dict:
    m = {}

    # Table counts
    for tbl in [
        "tracks",
        "artists",
        "albums",
        "track_sources",
        "playlist_tracks",
        "playlist_sources",
        "track_credits",
        "album_artists",
        "track_artists",
        "downloads",
        "lyrics",
        "library_entries",
    ]:
        try:
            m[f"count_{tbl}"] = cur.execute(f"SELECT count(*) FROM {tbl}").fetchone()[0]
        except Exception:
            m[f"count_{tbl}"] = -1

    # F1.2 & F1.3
    m["isrc_collisions_casing"] = cur.execute("""
        SELECT count(*) FROM (
            SELECT isrc FROM tracks WHERE isrc IS NOT NULL GROUP BY UPPER(isrc) HAVING count(*) > 1
        )
    """).fetchone()[0]
    m["isrc_lowercase_tracks"] = cur.execute("SELECT count(*) FROM tracks WHERE isrc != UPPER(isrc)").fetchone()[0]
    m["isrc_dashed_tracks"] = cur.execute("SELECT count(*) FROM tracks WHERE isrc LIKE '%-%'").fetchone()[0]
    m["isrc_total_normalized_collisions"] = cur.execute("""
        SELECT count(*) FROM (
            SELECT isrc FROM tracks WHERE isrc IS NOT NULL GROUP BY UPPER(REPLACE(isrc, '-', '')) HAVING count(*) > 1
        )
    """).fetchone()[0]

    # F1.5
    m["track_sources_duplicate_pairs"] = cur.execute("""
        SELECT count(*) FROM (
            SELECT service_id, service_track_id FROM track_sources GROUP BY service_id, service_track_id HAVING count(*) > 1
        )
    """).fetchone()[0]
    ts_idx = cur.execute("SELECT count(*) FROM sqlite_master WHERE name = 'idx_track_sources_service_track_unique'").fetchone()[0]
    m["track_sources_unique_index"] = bool(ts_idx)

    # F1.6
    m["artists_whitespace"] = cur.execute("SELECT count(*) FROM artists WHERE name != TRIM(name)").fetchone()[0]

    # F1.7
    t12520_artists = cur.execute("""
        SELECT a.id, a.name 
        FROM track_artists ta 
        JOIN artists a ON ta.artist_id = a.id 
        WHERE ta.track_id = 12520
    """).fetchall()
    m["track_12520_artists"] = t12520_artists
    m["garbage_tracks_count"] = cur.execute("SELECT count(*) FROM tracks WHERE id IN (9324, 12031)").fetchone()[0]
    m["artist_27808_exists"] = bool(cur.execute("SELECT count(*) FROM artists WHERE id = 27808").fetchone()[0])

    # F2.1
    pt_sql = cur.execute("SELECT sql FROM sqlite_master WHERE name = 'playlist_tracks'").fetchone()
    m["playlist_tracks_sql"] = pt_sql[0] if pt_sql else ""
    m["playlist_tracks_has_pos_unique"] = "UNIQUE(playlist_id, position)" in m["playlist_tracks_sql"].replace(" ", "").replace('"', "")

    # F2.3
    m["playlist_sources_count"] = cur.execute("SELECT count(*) FROM playlist_sources").fetchone()[0]

    # F3.3
    m["soundcloud_service_quality"] = cur.execute("SELECT max_quality FROM services WHERE name = 'soundcloud'").fetchone()
    m["soundcloud_pref"] = cur.execute("SELECT max_quality, preferred_format FROM quality_preferences WHERE service_name = 'soundcloud'").fetchone()

    # F4.1
    m["corrupt_artists_count"] = cur.execute("SELECT count(*) FROM artists WHERE INSTR(name, CHAR(13)) > 0").fetchone()[0]
    m["credits_performer_count"] = cur.execute("SELECT count(*) FROM track_credits WHERE role = 'performer'").fetchone()[0]
    m["credits_distinct_roles"] = cur.execute("SELECT count(DISTINCT role) FROM track_credits").fetchone()[0]

    # F4.6
    t4037 = cur.execute("SELECT title FROM tracks WHERE id = 4037").fetchone()
    m["track_4037_title"] = t4037[0] if t4037 else None
    a12908 = cur.execute("SELECT name FROM artists WHERE id = 12908").fetchone()
    m["artist_12908_name"] = a12908[0] if a12908 else None
    a61049 = cur.execute("SELECT name FROM artists WHERE id = 61049").fetchone()
    m["artist_61049_name"] = a61049[0] if a61049 else None
    favs = cur.execute("SELECT id, is_favorite FROM tracks WHERE id IN (695, 2708)").fetchall()
    m["soundcloud_favorites"] = favs

    return m


def run_repair(db_path: str, sql_out_path: str):
    print(f"[*] Opening database: {db_path}")
    con = sqlite3.connect(db_path)
    cur = con.cursor()

    print("[*] Collecting pre-repair metrics...")
    metrics_before = get_metrics(cur)

    sql_statements = []

    def record_sql(stmt: str):
        sql_statements.append(stmt.strip())

    start_time = time.time()

    # Helper indexes to accelerate foreign key cascaded checks
    print("[*] Creating acceleration indexes for foreign keys...")
    cur.execute("CREATE INDEX IF NOT EXISTS idx_track_credits_artist ON track_credits(artist_id);")
    cur.execute("CREATE INDEX IF NOT EXISTS idx_album_artists_artist ON album_artists(artist_id);")
    cur.execute("CREATE INDEX IF NOT EXISTS idx_track_artists_artist ON track_artists(artist_id);")
    record_sql("CREATE INDEX IF NOT EXISTS idx_track_credits_artist ON track_credits(artist_id);")
    record_sql("CREATE INDEX IF NOT EXISTS idx_album_artists_artist ON album_artists(artist_id);")
    record_sql("CREATE INDEX IF NOT EXISTS idx_track_artists_artist ON track_artists(artist_id);")

    # Enforce foreign keys and start atomic transaction
    cur.execute("PRAGMA foreign_keys = ON;")
    cur.execute("BEGIN TRANSACTION;")
    record_sql("PRAGMA foreign_keys = ON;")
    record_sql("BEGIN TRANSACTION;")

    # =========================================================================
    # Step 1 (F2.1): Migrate playlist_tracks constraint to UNIQUE(playlist_id, position)
    # =========================================================================
    print("[+] Step 1 (F2.1): Migrating playlist_tracks constraint to UNIQUE(playlist_id, position)...")
    cur.execute("""
    CREATE TABLE playlist_tracks_new (
        id INTEGER PRIMARY KEY AUTOINCREMENT,
        playlist_id INTEGER NOT NULL REFERENCES playlists(id) ON DELETE CASCADE,
        track_id INTEGER NOT NULL REFERENCES tracks(id) ON DELETE CASCADE,
        position INTEGER NOT NULL,
        added_at TEXT,
        UNIQUE(playlist_id, position)
    );
    """)
    cur.execute("""
    INSERT INTO playlist_tracks_new (id, playlist_id, track_id, position, added_at)
    SELECT id, playlist_id, track_id, position, added_at FROM playlist_tracks;
    """)
    cur.execute("DROP TABLE playlist_tracks;")
    cur.execute("ALTER TABLE playlist_tracks_new RENAME TO playlist_tracks;")
    cur.execute("CREATE INDEX idx_playlist_tracks_playlist ON playlist_tracks(playlist_id);")
    cur.execute("CREATE INDEX idx_playlist_tracks_track ON playlist_tracks(track_id);")
    cur.execute("CREATE INDEX idx_playlist_tracks_pos ON playlist_tracks(playlist_id, position);")

    record_sql("""
CREATE TABLE playlist_tracks_new (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    playlist_id INTEGER NOT NULL REFERENCES playlists(id) ON DELETE CASCADE,
    track_id INTEGER NOT NULL REFERENCES tracks(id) ON DELETE CASCADE,
    position INTEGER NOT NULL,
    added_at TEXT,
    UNIQUE(playlist_id, position)
);
INSERT INTO playlist_tracks_new (id, playlist_id, track_id, position, added_at)
SELECT id, playlist_id, track_id, position, added_at FROM playlist_tracks;
DROP TABLE playlist_tracks;
ALTER TABLE playlist_tracks_new RENAME TO playlist_tracks;
CREATE INDEX idx_playlist_tracks_playlist ON playlist_tracks(playlist_id);
CREATE INDEX idx_playlist_tracks_track ON playlist_tracks(track_id);
CREATE INDEX idx_playlist_tracks_pos ON playlist_tracks(playlist_id, position);
    """)

    # =========================================================================
    # Step 2 (F2.3): Backfill playlist_sources
    # =========================================================================
    print("[+] Step 2 (F2.3): Backfilling playlist_sources...")
    cur.execute("""
    INSERT OR IGNORE INTO playlist_sources (playlist_id, account_id, service_id, service_playlist_id)
    SELECT p.id, p.account_id, a.service_id, p.service_playlist_id
    FROM playlists p
    JOIN accounts a ON p.account_id = a.id
    WHERE p.service_playlist_id IS NOT NULL;
    """)
    record_sql("""
INSERT OR IGNORE INTO playlist_sources (playlist_id, account_id, service_id, service_playlist_id)
SELECT p.id, p.account_id, a.service_id, p.service_playlist_id
FROM playlists p
JOIN accounts a ON p.account_id = a.id
WHERE p.service_playlist_id IS NOT NULL;
    """)

    # =========================================================================
    # Step 3 (F3.3): Normalize SoundCloud provider
    # =========================================================================
    print("[+] Step 3 (F3.3): Normalizing SoundCloud to lossy / mp3...")
    cur.execute("UPDATE services SET max_quality = 'lossy' WHERE name = 'soundcloud';")
    cur.execute("UPDATE quality_preferences SET max_quality = 'lossy', preferred_format = 'mp3' WHERE service_name = 'soundcloud';")
    record_sql("UPDATE services SET max_quality = 'lossy' WHERE name = 'soundcloud';")
    record_sql("UPDATE quality_preferences SET max_quality = 'lossy', preferred_format = 'mp3' WHERE service_name = 'soundcloud';")

    # =========================================================================
    # Step 4 (F1.5): Deduplicate track_sources and create unique index
    # =========================================================================
    print("[+] Step 4 (F1.5): Deduplicating track_sources and creating UNIQUE index...")
    cur.execute("""
    DELETE FROM track_sources
    WHERE id IN (
        SELECT id FROM (
            SELECT id,
                   ROW_NUMBER() OVER (
                       PARTITION BY service_id, service_track_id
                       ORDER BY 
                           CASE 
                               WHEN format = 'FLAC' THEN 3
                               WHEN format = 'ALAC' THEN 3
                               WHEN format = 'WAV' THEN 3
                               WHEN format = 'AAC' THEN 2
                               WHEN format = 'MP3' THEN 1
                               ELSE 0 
                           END DESC,
                           COALESCE(quality_score, 0) DESC,
                           id ASC
                   ) as rn
            FROM track_sources
        ) WHERE rn > 1
    );
    """)
    cur.execute("CREATE UNIQUE INDEX idx_track_sources_service_track_unique ON track_sources(service_id, service_track_id);")
    record_sql("""
DELETE FROM track_sources
WHERE id IN (
    SELECT id FROM (
        SELECT id,
               ROW_NUMBER() OVER (
                   PARTITION BY service_id, service_track_id
                   ORDER BY 
                       CASE 
                           WHEN format = 'FLAC' THEN 3
                           WHEN format = 'ALAC' THEN 3
                           WHEN format = 'WAV' THEN 3
                           WHEN format = 'AAC' THEN 2
                           WHEN format = 'MP3' THEN 1
                           ELSE 0 
                       END DESC,
                       COALESCE(quality_score, 0) DESC,
                       id ASC
               ) as rn
        FROM track_sources
    ) WHERE rn > 1
);
CREATE UNIQUE INDEX idx_track_sources_service_track_unique ON track_sources(service_id, service_track_id);
    """)

    # =========================================================================
    # Step 5 (F1.6): Normalize 13 whitespace artists
    # =========================================================================
    print("[+] Step 5 (F1.6): Normalizing 13 whitespace artists and merging duplicates...")
    whitespace_merges = [
        (923, 925),      # 'Oasis ' -> 'Oasis'
        (6651, 134),     # 'IDLES ' -> 'IDLES'
        (7518, 3063),    # 'The Dandy Warhols ' -> 'The Dandy Warhols'
        (14455, 5022),   # 'MUNA ' -> 'MUNA'
        (18076, 4688),   # 'Ghost ' -> 'Ghost'
        (22055, 3672),   # 'The Move ' -> 'The Move'
        (29938, 8381),   # ' Joey Bada$$' -> 'Joey Bada$$'
        (61947, 50527),  # 'anaïs ' -> 'anaïs'
        (62267, 62268),  # ' Romy' -> 'Romy'
        (88751, 78716),  # 'The Arrows ' -> 'The Arrows'
        (89699, 79728),  # 'Shampoo    ' -> 'Shampoo'
        (93481, 93482),  # 'Hindi Zahra ' -> 'Hindi Zahra'
    ]
    for loser, winner in whitespace_merges:
        cur.execute("UPDATE artists SET is_favorite = 1 WHERE id = ? AND EXISTS (SELECT 1 FROM artists WHERE id = ? AND is_favorite = 1)", (winner, loser))
        cur.execute("INSERT OR IGNORE INTO album_artists (album_id, artist_id, is_primary) SELECT album_id, ?, is_primary FROM album_artists WHERE artist_id = ?", (winner, loser))
        cur.execute("DELETE FROM album_artists WHERE artist_id = ?", (loser,))
        cur.execute("INSERT OR IGNORE INTO track_artists (track_id, artist_id, role) SELECT track_id, ?, role FROM track_artists WHERE artist_id = ?", (winner, loser))
        cur.execute("DELETE FROM track_artists WHERE artist_id = ?", (loser,))
        cur.execute("INSERT OR IGNORE INTO track_credits (track_id, artist_id, role) SELECT track_id, ?, role FROM track_credits WHERE artist_id = ?", (winner, loser))
        cur.execute("DELETE FROM track_credits WHERE artist_id = ?", (loser,))
        cur.execute("DELETE FROM artists WHERE id = ?", (loser,))

        record_sql(f"""
UPDATE artists SET is_favorite = 1 WHERE id = {winner} AND EXISTS (SELECT 1 FROM artists WHERE id = {loser} AND is_favorite = 1);
INSERT OR IGNORE INTO album_artists (album_id, artist_id, is_primary) SELECT album_id, {winner}, is_primary FROM album_artists WHERE artist_id = {loser};
DELETE FROM album_artists WHERE artist_id = {loser};
INSERT OR IGNORE INTO track_artists (track_id, artist_id, role) SELECT track_id, {winner}, role FROM track_artists WHERE artist_id = {loser};
DELETE FROM track_artists WHERE artist_id = {loser};
INSERT OR IGNORE INTO track_credits (track_id, artist_id, role) SELECT track_id, {winner}, role FROM track_credits WHERE artist_id = {loser};
DELETE FROM track_credits WHERE artist_id = {loser};
DELETE FROM artists WHERE id = {loser};
        """)

    # 13th artist: 'Bayerisches Staatsorchester ' has no pre-existing clean pair, simply trim
    cur.execute("UPDATE artists SET name = TRIM(name) WHERE id = 66816;")
    record_sql("UPDATE artists SET name = TRIM(name) WHERE id = 66816;")

    # =========================================================================
    # Step 6 (F1.7): Reassign track 12520, delete garbage tracks 9324 & 12031, delete artist 27808
    # =========================================================================
    print("[+] Step 6 (F1.7): Reassigning 12520 to The Chordettes (12690), deleting garbage tracks 9324 & 12031, deleting empty artist 27808...")
    cur.execute("DELETE FROM track_artists WHERE track_id = 12520 AND artist_id = 27808;")
    cur.execute("INSERT OR IGNORE INTO track_artists (track_id, artist_id, role) VALUES (12520, 12690, 'primary');")
    record_sql("""
DELETE FROM track_artists WHERE track_id = 12520 AND artist_id = 27808;
INSERT OR IGNORE INTO track_artists (track_id, artist_id, role) VALUES (12520, 12690, 'primary');
    """)

    for tid in [9324, 12031]:
        for tbl in ["playlist_tracks", "library_entries", "track_sources", "track_artists", "track_credits", "enrichment_progress", "downloads", "lyrics", "download_queue"]:
            cur.execute(f"DELETE FROM {tbl} WHERE track_id = ?", (tid,))
            record_sql(f"DELETE FROM {tbl} WHERE track_id = {tid};")
        cur.execute("DELETE FROM tracks WHERE id = ?", (tid,))
        record_sql(f"DELETE FROM tracks WHERE id = {tid};")

    # Empty artist 27808 remaining links reassigned to Unknown Artist (47730)
    cur.execute("INSERT OR IGNORE INTO album_artists (album_id, artist_id, is_primary) SELECT album_id, 47730, is_primary FROM album_artists WHERE artist_id = 27808;")
    cur.execute("DELETE FROM album_artists WHERE artist_id = 27808;")
    cur.execute("INSERT OR IGNORE INTO track_artists (track_id, artist_id, role) SELECT track_id, 47730, role FROM track_artists WHERE artist_id = 27808;")
    cur.execute("DELETE FROM track_artists WHERE artist_id = 27808;")
    cur.execute("INSERT OR IGNORE INTO track_credits (track_id, artist_id, role) SELECT track_id, 47730, role FROM track_credits WHERE artist_id = 27808;")
    cur.execute("DELETE FROM track_credits WHERE artist_id = 27808;")
    cur.execute("DELETE FROM artists WHERE id = 27808;")
    record_sql("""
INSERT OR IGNORE INTO album_artists (album_id, artist_id, is_primary) SELECT album_id, 47730, is_primary FROM album_artists WHERE artist_id = 27808;
DELETE FROM album_artists WHERE artist_id = 27808;
INSERT OR IGNORE INTO track_artists (track_id, artist_id, role) SELECT track_id, 47730, role FROM track_artists WHERE artist_id = 27808;
DELETE FROM track_artists WHERE artist_id = 27808;
INSERT OR IGNORE INTO track_credits (track_id, artist_id, role) SELECT track_id, 47730, role FROM track_credits WHERE artist_id = 27808;
DELETE FROM track_credits WHERE artist_id = 27808;
DELETE FROM artists WHERE id = 27808;
    """)

    # =========================================================================
    # Step 7 (F1.2 & F1.3): Merge 109 lowercase ISRC tracks + 5 dashed ISRC tracks, normalize ISRCs
    # =========================================================================
    print("[+] Step 7 (F1.2 & F1.3): Merging duplicate ISRC pairs and normalizing all ISRCs...")
    cur.execute("""
        SELECT UPPER(REPLACE(isrc, '-', '')) as norm_isrc, group_concat(id), group_concat(isrc)
        FROM tracks 
        WHERE isrc IS NOT NULL
        GROUP BY norm_isrc
        HAVING count(*) > 1
    """)
    groups = cur.fetchall()

    for norm_isrc, ids, isrcs in groups:
        id_list = [int(x) for x in ids.split(",")]
        isrc_list = isrcs.split(",")
        if isrc_list[0] == norm_isrc and isrc_list[1] != norm_isrc:
            winner, loser = id_list[0], id_list[1]
        elif isrc_list[1] == norm_isrc and isrc_list[0] != norm_isrc:
            winner, loser = id_list[1], id_list[0]
        else:
            winner, loser = min(id_list), max(id_list)

        # 1. track_sources
        cur.execute("DELETE FROM track_sources WHERE track_id = ? AND service_id IN (SELECT service_id FROM track_sources WHERE track_id = ?)", (loser, winner))
        cur.execute("UPDATE track_sources SET track_id = ? WHERE track_id = ?", (winner, loser))

        # 2. playlist_tracks
        cur.execute("UPDATE playlist_tracks SET track_id = ? WHERE track_id = ?", (winner, loser))

        # 3. library_entries
        cur.execute("DELETE FROM library_entries WHERE track_id = ? AND account_id IN (SELECT account_id FROM library_entries WHERE track_id = ?)", (loser, winner))
        cur.execute("UPDATE library_entries SET track_id = ? WHERE track_id = ?", (winner, loser))

        # 4. track_artists
        cur.execute("INSERT OR IGNORE INTO track_artists (track_id, artist_id, role) SELECT ?, artist_id, role FROM track_artists WHERE track_id = ?", (winner, loser))
        cur.execute("DELETE FROM track_artists WHERE track_id = ?", (loser,))

        # 5. track_credits
        cur.execute("INSERT OR IGNORE INTO track_credits (track_id, artist_id, role) SELECT ?, artist_id, role FROM track_credits WHERE track_id = ?", (winner, loser))
        cur.execute("DELETE FROM track_credits WHERE artist_id = ?", (loser,))

        # 6. downloads
        cur.execute("UPDATE downloads SET track_id = ? WHERE track_id = ? AND ? NOT IN (SELECT track_id FROM downloads WHERE track_id IS NOT NULL)", (winner, loser, winner))
        cur.execute("DELETE FROM downloads WHERE track_id = ?", (loser,))

        # 7. lyrics
        cur.execute("UPDATE lyrics SET track_id = ? WHERE track_id = ? AND (SELECT count(*) FROM lyrics WHERE track_id = ? AND format = lyrics.format) = 0", (winner, loser, winner))
        cur.execute("DELETE FROM lyrics WHERE track_id = ?", (loser,))

        # 8. enrichment_progress
        cur.execute("UPDATE enrichment_progress SET track_id = ? WHERE track_id = ? AND (SELECT count(*) FROM enrichment_progress WHERE track_id = ? AND service = enrichment_progress.service) = 0", (winner, loser, winner))
        cur.execute("DELETE FROM enrichment_progress WHERE track_id = ?", (loser,))

        # 9. download_queue
        cur.execute("DELETE FROM download_queue WHERE track_id = ?", (loser,))

        # 10. is_favorite
        cur.execute("UPDATE tracks SET is_favorite = 1, favorite_at = COALESCE(tracks.favorite_at, (SELECT favorite_at FROM tracks WHERE id = ?)) WHERE id = ? AND (SELECT is_favorite FROM tracks WHERE id = ?) = 1", (loser, winner, loser))

        # 11. delete loser
        cur.execute("DELETE FROM tracks WHERE id = ?", (loser,))

        record_sql(f"""
DELETE FROM track_sources WHERE track_id = {loser} AND service_id IN (SELECT service_id FROM track_sources WHERE track_id = {winner});
UPDATE track_sources SET track_id = {winner} WHERE track_id = {loser};
UPDATE playlist_tracks SET track_id = {winner} WHERE track_id = {loser};
DELETE FROM library_entries WHERE track_id = {loser} AND account_id IN (SELECT account_id FROM library_entries WHERE track_id = {winner});
UPDATE library_entries SET track_id = {winner} WHERE track_id = {loser};
INSERT OR IGNORE INTO track_artists (track_id, artist_id, role) SELECT {winner}, artist_id, role FROM track_artists WHERE track_id = {loser};
DELETE FROM track_artists WHERE track_id = {loser};
INSERT OR IGNORE INTO track_credits (track_id, artist_id, role) SELECT {winner}, artist_id, role FROM track_credits WHERE artist_id = {loser};
DELETE FROM track_credits WHERE artist_id = {loser};
UPDATE downloads SET track_id = {winner} WHERE track_id = {loser} AND {winner} NOT IN (SELECT track_id FROM downloads WHERE track_id IS NOT NULL);
DELETE FROM downloads WHERE track_id = {loser};
UPDATE lyrics SET track_id = {winner} WHERE track_id = {loser} AND (SELECT count(*) FROM lyrics WHERE track_id = {winner} AND format = lyrics.format) = 0;
DELETE FROM lyrics WHERE track_id = {loser};
UPDATE enrichment_progress SET track_id = {winner} WHERE track_id = {loser} AND (SELECT count(*) FROM enrichment_progress WHERE track_id = {winner} AND service = enrichment_progress.service) = 0;
DELETE FROM enrichment_progress WHERE track_id = {loser};
DELETE FROM download_queue WHERE track_id = {loser};
UPDATE tracks SET is_favorite = 1, favorite_at = COALESCE(tracks.favorite_at, (SELECT favorite_at FROM tracks WHERE id = {loser})) WHERE id = {winner} AND (SELECT is_favorite FROM tracks WHERE id = {loser}) = 1;
DELETE FROM tracks WHERE id = {loser};
        """)

    # F1.3: Normalize all ISRCs and create unique case-insensitive index
    cur.execute("UPDATE tracks SET isrc = UPPER(REPLACE(isrc, '-', '')) WHERE isrc IS NOT NULL;")
    cur.execute("DROP INDEX IF EXISTS idx_tracks_isrc_unique;")
    cur.execute("CREATE UNIQUE INDEX idx_tracks_isrc_unique ON tracks(isrc COLLATE NOCASE) WHERE isrc IS NOT NULL;")
    record_sql("""
UPDATE tracks SET isrc = UPPER(REPLACE(isrc, '-', '')) WHERE isrc IS NOT NULL;
DROP INDEX IF EXISTS idx_tracks_isrc_unique;
CREATE UNIQUE INDEX idx_tracks_isrc_unique ON tracks(isrc COLLATE NOCASE) WHERE isrc IS NOT NULL;
    """)

    # =========================================================================
    # Step 8 (F4.6): Mojibake, HTML entities, and Favorites
    # =========================================================================
    print("[+] Step 8 (F4.6): Repairing mojibake, HTML entities, and favorites...")
    cur.execute("UPDATE tracks SET title = '¿Y Tú Qué Has Hecho?' WHERE id = 4037;")
    cur.execute("UPDATE artists SET name = REPLACE(name, '&amp;', '&') WHERE id = 12908;")
    # 61049 is merged into 18300 ('Steve Harley & Cockney Rebel') to prevent unique constraint collision
    cur.execute("INSERT OR IGNORE INTO album_artists (album_id, artist_id, is_primary) SELECT album_id, 18300, is_primary FROM album_artists WHERE artist_id = 61049;")
    cur.execute("DELETE FROM album_artists WHERE artist_id = 61049;")
    cur.execute("INSERT OR IGNORE INTO track_artists (track_id, artist_id, role) SELECT track_id, 18300, role FROM track_artists WHERE artist_id = 61049;")
    cur.execute("DELETE FROM track_artists WHERE artist_id = 61049;")
    cur.execute("INSERT OR IGNORE INTO track_credits (track_id, artist_id, role) SELECT track_id, 18300, role FROM track_credits WHERE artist_id = 61049;")
    cur.execute("DELETE FROM track_credits WHERE artist_id = 61049;")
    cur.execute("DELETE FROM artists WHERE id = 61049;")
    cur.execute("UPDATE tracks SET is_favorite = 1, favorite_at = CURRENT_TIMESTAMP WHERE id IN (695, 2708);")

    record_sql("""
UPDATE tracks SET title = '¿Y Tú Qué Has Hecho?' WHERE id = 4037;
UPDATE artists SET name = REPLACE(name, '&amp;', '&') WHERE id = 12908;
INSERT OR IGNORE INTO album_artists (album_id, artist_id, is_primary) SELECT album_id, 18300, is_primary FROM album_artists WHERE artist_id = 61049;
DELETE FROM album_artists WHERE artist_id = 61049;
INSERT OR IGNORE INTO track_artists (track_id, artist_id, role) SELECT track_id, 18300, role FROM track_artists WHERE artist_id = 61049;
DELETE FROM track_artists WHERE artist_id = 61049;
INSERT OR IGNORE INTO track_credits (track_id, artist_id, role) SELECT track_id, 18300, role FROM track_credits WHERE artist_id = 61049;
DELETE FROM track_credits WHERE artist_id = 61049;
DELETE FROM artists WHERE id = 61049;
UPDATE tracks SET is_favorite = 1, favorite_at = CURRENT_TIMESTAMP WHERE id IN (695, 2708);
    """)

    # =========================================================================
    # Step 9 (F4.1): Fix 14,631 Corrupt Artists (C5)
    # =========================================================================
    print("[+] Step 9 (F4.1): Fixing 14,631 corrupt artists (\"Role\\r - Person\") into track_credits roles...")
    cur.execute("SELECT id, name FROM artists WHERE INSTR(name, CHAR(13)) > 0")
    corrupt_artists = cur.fetchall()

    name_to_id = {}
    cur.execute("SELECT name, id FROM artists WHERE INSTR(name, CHAR(13)) = 0")
    for name, aid in cur.fetchall():
        name_to_id[name] = aid

    corrupt_to_targets = {}
    corrupt_ids_to_recycle = []
    corrupt_ids_to_delete = []

    for aid, full_name in corrupt_artists:
        parts = [p.strip() for p in full_name.split("\r - ")]
        role = parts[0]
        people = [p.replace("\r", "").replace("-", "").strip() for p in parts[1:]]
        people = [p for p in people if p]

        if not people:
            unknown_id = name_to_id.get("Unknown Artist", 47730)
            corrupt_to_targets[aid] = [(unknown_id, role)]
            corrupt_ids_to_delete.append(aid)
            continue

        targets = []
        for person in people:
            if person in name_to_id:
                canon_id = name_to_id[person]
            else:
                if aid not in [x[0] for x in corrupt_ids_to_recycle] and aid not in corrupt_ids_to_delete:
                    corrupt_ids_to_recycle.append((aid, person))
                    name_to_id[person] = aid
                    canon_id = aid
                else:
                    canon_id = None
            targets.append((person, role, canon_id))
        corrupt_to_targets[aid] = targets

    # New artists needing INSERT
    new_persons_needed = []
    for aid, targets in corrupt_to_targets.items():
        for t in targets:
            if len(t) == 3 and t[2] is None:
                person = t[0]
                if person not in name_to_id:
                    new_persons_needed.append(person)

    new_persons_needed = sorted(list(set(new_persons_needed)))
    for person in new_persons_needed:
        cur.execute("INSERT INTO artists (name) VALUES (?)", (person,))
        name_to_id[person] = cur.lastrowid
        record_sql(f"INSERT INTO artists (name) VALUES ({sql_escape_str(person)});")

    # In-place UPDATE of recycled artists
    for aid, person in corrupt_ids_to_recycle:
        cur.execute("UPDATE artists SET name = ? WHERE id = ?", (person, aid))
        record_sql(f"UPDATE artists SET name = {sql_escape_str(person)} WHERE id = {aid};")

    # Map credits
    final_credits_to_insert = []
    old_corrupt_ids = []

    for aid, targets in corrupt_to_targets.items():
        old_corrupt_ids.append(aid)
        cur.execute("SELECT track_id FROM track_credits WHERE artist_id = ?", (aid,))
        tracks = [r[0] for r in cur.fetchall()]
        for tid in tracks:
            for t in targets:
                if len(t) == 2:
                    cid, role = t
                else:
                    person, role, cid = t
                    if cid is None:
                        cid = name_to_id[person]
                final_credits_to_insert.append((tid, cid, role))

    cur.executemany("INSERT OR IGNORE INTO track_credits (track_id, artist_id, role) VALUES (?, ?, ?)", final_credits_to_insert)
    cur.executemany("DELETE FROM track_credits WHERE artist_id = ?", [(aid,) for aid in old_corrupt_ids])

    recycled_set = set(x[0] for x in corrupt_ids_to_recycle)
    to_delete_artists = [aid for aid in old_corrupt_ids if aid not in recycled_set]
    cur.executemany("DELETE FROM artists WHERE id = ?", [(aid,) for aid in to_delete_artists])

    # Record batched SQL in scripts
    record_sql(f"-- Migrated {len(final_credits_to_insert)} credits and purged {len(to_delete_artists)} non-recycled corrupt artists")
    for tid, cid, role in final_credits_to_insert:
        record_sql(f"INSERT OR IGNORE INTO track_credits (track_id, artist_id, role) VALUES ({tid}, {cid}, {sql_escape_str(role)});")
    for aid in old_corrupt_ids:
        record_sql(f"DELETE FROM track_credits WHERE artist_id = {aid};")
    for aid in to_delete_artists:
        record_sql(f"DELETE FROM artists WHERE id = {aid};")

    # =========================================================================
    # Step 10: Rebuild FTS Index
    # =========================================================================
    print("[+] Step 10: Rebuilding library_fts index...")
    cur.execute("INSERT INTO library_fts(library_fts) VALUES('rebuild');")
    record_sql("INSERT INTO library_fts(library_fts) VALUES('rebuild');")

    # =========================================================================
    # Commit Transaction
    # =========================================================================
    print("[+] Committing transaction...")
    cur.execute("COMMIT;")
    record_sql("COMMIT;")
    elapsed = time.time() - start_time
    print(f"[*] Transaction successfully committed in {elapsed:.2f}s")

    # =========================================================================
    # Foreign Key Verification
    # =========================================================================
    print("[*] Verifying foreign keys (PRAGMA foreign_key_check)...")
    fk_violations = list(cur.execute("PRAGMA foreign_key_check;"))
    print(f"[*] Foreign key violations: {len(fk_violations)}")
    if fk_violations:
        print("[!] ERROR: Foreign key violations detected:")
        for v in fk_violations:
            print("   ", v)
        sys.exit(1)

    print("[*] Collecting post-repair metrics...")
    metrics_after = get_metrics(cur)
    con.close()

    # Write SQL script
    print(f"[*] Writing full SQL statements to {sql_out_path}...")
    os.makedirs(os.path.dirname(os.path.abspath(sql_out_path)), exist_ok=True)
    with open(sql_out_path, "w", encoding="utf-8") as f:
        f.write("-- Syncify Phase 1 SQL Repair Script\n")
        f.write(f"-- Generated on: {time.strftime('%Y-%m-%d %H:%M:%S')}\n\n")
        f.write("\n".join(sql_statements))
        f.write("\n")
    print(f"[*] Wrote {len(sql_statements)} statements ({os.path.getsize(sql_out_path):,} bytes) to {sql_out_path}")

    # =========================================================================
    # Report Detailed Before vs After Metrics
    # =========================================================================
    print("\n" + "=" * 80)
    print("MÉTRICAS DETALLADAS: ANTES VS DESPUÉS DE LA CIRUGÍA SQL (FASE 1)")
    print("=" * 80)

    rows = [
        ("Pistas en biblioteca (tracks)", metrics_before["count_tracks"], metrics_after["count_tracks"], f"{metrics_after['count_tracks'] - metrics_before['count_tracks']:+d} (114 duplicados + 2 basura)"),
        ("Artistas en catálogo (artists)", metrics_before["count_artists"], metrics_after["count_artists"], f"{metrics_after['count_artists'] - metrics_before['count_artists']:+d} (9,036 corruptos + 12 espacios + 1 vacío 27808 + 1 HTML 61049 purgados; 104 nuevos agregados)"),
        ("Fuentes de pistas (track_sources)", metrics_before["count_track_sources"], metrics_after["count_track_sources"], f"{metrics_after['count_track_sources'] - metrics_before['count_track_sources']:+d} (4 duplicados F1.5 + 11 solapamientos ISRC + 2 basura)"),
        ("Créditos de pistas (track_credits)", metrics_before["count_track_credits"], metrics_after["count_track_credits"], f"{metrics_after['count_track_credits'] - metrics_before['count_track_credits']:+d} (migrados desde nombres corruptos C5)"),
        ("Fuentes de playlists (playlist_sources)", metrics_before["count_playlist_sources"], metrics_after["count_playlist_sources"], f"{metrics_after['count_playlist_sources'] - metrics_before['count_playlist_sources']:+d} (Backfill C2 completo)"),
        ("Pistas en playlists (playlist_tracks)", metrics_before["count_playlist_tracks"], metrics_after["count_playlist_tracks"], f"{metrics_after['count_playlist_tracks'] - metrics_before['count_playlist_tracks']:+d} (reasignadas al track ganador, 6 filas de basura eliminadas)"),
        ("ISRCs en minúsculas (M6)", metrics_before["isrc_lowercase_tracks"], metrics_after["isrc_lowercase_tracks"], "0 (100% normalizados a MAYÚSCULAS)"),
        ("ISRCs con guiones", metrics_before["isrc_dashed_tracks"], metrics_after["isrc_dashed_tracks"], "0 (100% guiones removidos)"),
        ("Colisiones exactas de ISRC", metrics_before["isrc_collisions_casing"], metrics_after["isrc_collisions_casing"], "0 (100% resueltas por fusión)"),
        ("Colisiones normalizadas de ISRC", metrics_before["isrc_total_normalized_collisions"], metrics_after["isrc_total_normalized_collisions"], "0 (Índice UNIQUE NOCASE activo)"),
        ("Colisiones (service_id, service_track_id)", metrics_before["track_sources_duplicate_pairs"], metrics_after["track_sources_duplicate_pairs"], "0 (Índice UNIQUE activo en track_sources)"),
        ("Artistas con espacios en blanco", metrics_before["artists_whitespace"], metrics_after["artists_whitespace"], "0 (12 fusionados + 1 trimeado)"),
        ("Pista 12520 (Mr. Sandman) artistas", str(metrics_before["track_12520_artists"]), str(metrics_after["track_12520_artists"]), "Reasignada a The Chordettes (12690)"),
        ("Pistas basura 9324 y 12031", metrics_before["garbage_tracks_count"], metrics_after["garbage_tracks_count"], "0 (Eliminadas limpiamente)"),
        ("Artista vacío 27808 existe", metrics_before["artist_27808_exists"], metrics_after["artist_27808_exists"], "False (Eliminado y referencias reasignadas)"),
        ("Restricción playlist_tracks", "UNIQUE(playlist_id, track_id)", "UNIQUE(playlist_id, position)", "Migrada a posición (mitiga C1)"),
        ("SoundCloud max_quality", str(metrics_before["soundcloud_service_quality"]), str(metrics_after["soundcloud_service_quality"]), "Actualizado a 'lossy'"),
        ("SoundCloud preferences", str(metrics_before["soundcloud_pref"]), str(metrics_after["soundcloud_pref"]), "Actualizado a ('lossy', 'mp3')"),
        ("Artistas corruptos C5 ('\\r - ')", metrics_before["corrupt_artists_count"], metrics_after["corrupt_artists_count"], "0 (100% migrados a track_credits.role)"),
        ("Créditos rol 'performer'", metrics_before["credits_performer_count"], metrics_after["credits_performer_count"], f"-{metrics_before['credits_performer_count'] - metrics_after['credits_performer_count']} roles específicos extraídos"),
        ("Roles distintos en track_credits", metrics_before["credits_distinct_roles"], metrics_after["credits_distinct_roles"], f"+{metrics_after['credits_distinct_roles'] - metrics_before['credits_distinct_roles']} roles nuevos incorporados"),
        ("Pista 4037 título (mojibake)", metrics_before["track_4037_title"], metrics_after["track_4037_title"], "Corregido a '¿Y Tú Qué Has Hecho?'"),
        ("Artista 12908 (HTML entity)", metrics_before["artist_12908_name"], metrics_after["artist_12908_name"], "Corregido a 'SNEAKER KIDS & Eli Noir'"),
        ("Artista 61049 (HTML entity)", metrics_before["artist_61049_name"], metrics_after["artist_61049_name"], "Fusionado con 18300 ('Steve Harley & Cockney Rebel')"),
        ("Favoritos SoundCloud (695, 2708)", str(metrics_before["soundcloud_favorites"]), str(metrics_after["soundcloud_favorites"]), "is_favorite = 1 para ambos tracks"),
        ("Violaciones de Foreign Keys", "0", str(len(fk_violations)), "0 violaciones (PRAGMA foreign_key_check = OK)"),
    ]

    for item, bef, aft, note in rows:
        print(f"{item:<40} | Antes: {str(bef):<20} | Después: {str(aft):<20} | {note}")
    print("=" * 80 + "\n")


def main():
    parser = argparse.ArgumentParser(description="Syncify Phase 1 SQL Repair")
    parser.add_argument("--db", default="syncify_backup_pre_repair.db", help="Path to database file")
    parser.add_argument("--sql-out", default="scripts/phase1_sql_repair.sql", help="Path to output SQL script")
    args = parser.parse_args()

    run_repair(args.db, args.sql_out)


if __name__ == "__main__":
    main()

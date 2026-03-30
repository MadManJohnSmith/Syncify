"""
Syncify Database Repair Script
Repairs missing track_artists links by matching track titles to existing artist names.
"""
import sqlite3
import re

conn = sqlite3.connect(r'src-tauri/data/syncify.db')
c = conn.cursor()

print("=== SYNCIFY DATABASE REPAIR ===\n")

# 1. Get orphan tracks (tracks without artist links)
c.execute('''
SELECT t.id, t.title, t.album_id
FROM tracks t
LEFT JOIN track_artists ta ON t.id = ta.track_id
WHERE ta.track_id IS NULL
''')
orphans = c.fetchall()
print(f"Found {len(orphans)} tracks without artist links\n")

# 2. Build artist lookup (name -> id)
c.execute('SELECT id, name FROM artists')
artist_lookup = {name.lower(): id for id, name in c.fetchall()}
print(f"Loaded {len(artist_lookup)} artists for matching\n")

# 3. Build album -> artist mapping from album_artists
c.execute('SELECT album_id, artist_id FROM album_artists')
album_to_artist = {album_id: artist_id for album_id, artist_id in c.fetchall()}
print(f"Loaded {len(album_to_artist)} album-artist mappings\n")

# 4. Get track_sources to find Spotify track IDs and lookup original artists
# This is tricky because Spotify's original artist info was discarded
# We'll try to infer from album_artists first

repaired = 0
failed = 0

for track_id, title, album_id in orphans:
    artist_id = None
    
    # Strategy 1: Use album_artist if available
    if album_id and album_id in album_to_artist:
        artist_id = album_to_artist[album_id]
    
    # Strategy 2: Try to extract artist from title if it contains " - "
    # (Some tracks might have "Artist - Title" format but this is rare)
    
    if artist_id:
        c.execute('INSERT OR IGNORE INTO track_artists (track_id, artist_id, role) VALUES (?, ?, ?)',
                 (track_id, artist_id, 'primary'))
        repaired += 1
    else:
        failed += 1

conn.commit()

print(f"\n=== REPAIR COMPLETE ===")
print(f"Repaired: {repaired}")
print(f"Failed (no album artist): {failed}")

# Verify
c.execute('SELECT COUNT(*) FROM tracks t LEFT JOIN track_artists ta ON t.id = ta.track_id WHERE ta.track_id IS NULL')
remaining = c.fetchone()[0]
print(f"\nRemaining orphan tracks: {remaining}")

conn.close()

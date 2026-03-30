"""
Syncify Database Reset Script
Clears library data while preserving accounts and settings.
"""
import sqlite3

conn = sqlite3.connect(r'src-tauri/data/syncify.db')
c = conn.cursor()

print("=== SYNCIFY LIBRARY RESET ===\n")

# Order matters for referential integrity
tables_to_clear = [
    'library_entries',
    'playlist_tracks', 
    'playlists',
    'download_queue',
    'downloads',
    'lyrics',
    'track_sources',
    'track_artists',
    'album_artists',
    'tracks',
    'albums',
    'artists',
    'sync_log',
]

for table in tables_to_clear:
    try:
        c.execute(f'DELETE FROM {table}')
        print(f"  Cleared {table}: {c.rowcount} rows deleted")
    except Exception as e:
        print(f"  Error clearing {table}: {e}")

# Reset auto-increment counters
c.execute("DELETE FROM sqlite_sequence WHERE name IN ('tracks', 'albums', 'artists', 'playlists', 'download_queue', 'library_entries')")

conn.commit()
conn.close()

print("\n✅ Library data has been reset!")
print("Accounts and settings were preserved.")
print("\nYou can now re-import from Spotify in the app.")

import sqlite3
import sys

conn = sqlite3.connect(r'C:\Users\tardis\AppData\Local\com.syncify.app\syncify.db')
cursor = conn.cursor()
cursor.execute('SELECT count(*) FROM playlists')
count = cursor.fetchone()[0]
print(f"Spotify playlists import complete: {count} playlists")

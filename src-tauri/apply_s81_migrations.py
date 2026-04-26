import sqlite3
import os

db_path = r"C:\Users\tardis\AppData\Local\com.syncify.app\syncify.db"

if not os.path.exists(db_path):
    print(f"Error: Database not found at {db_path}")
    exit(1)

try:
    conn = sqlite3.connect(db_path)
    cursor = conn.cursor()
    
    # 1. Update albums
    print("Updating albums table...")
    cursor.execute("PRAGMA table_info(albums)")
    album_cols = [col[1] for col in cursor.fetchall()]
    if "label" not in album_cols:
        cursor.execute("ALTER TABLE albums ADD COLUMN label TEXT")
    if "upc" not in album_cols:
        cursor.execute("ALTER TABLE albums ADD COLUMN upc TEXT")
        
    # 2. Update tracks
    print("Updating tracks table...")
    cursor.execute("PRAGMA table_info(tracks)")
    track_cols = [col[1] for col in cursor.fetchall()]
    if "preview_url" not in track_cols:
        cursor.execute("ALTER TABLE tracks ADD COLUMN preview_url TEXT")
    if "audio_quality" not in track_cols:
        cursor.execute("ALTER TABLE tracks ADD COLUMN audio_quality TEXT")
        
    conn.commit()
    print("S81 schema updates applied successfully!")
    conn.close()
except Exception as e:
    print(f"Error applying S81 migrations: {e}")
    exit(1)

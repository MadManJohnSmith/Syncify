import sqlite3
import os
import hashlib
import time

db_path = r"C:\Users\tardis\AppData\Local\com.syncify.app\syncify.db"
migrations_dir = r"C:\Users\tardis\Documents\Syncify\migrations"

def get_checksum(filename):
    with open(os.path.join(migrations_dir, filename), "rb") as f:
        return hashlib.sha384(f.read()).digest()

try:
    conn = sqlite3.connect(db_path)
    cursor = conn.cursor()
    
    # Migrations to fake
    migrations = [
        (39, "0039_add_album_industry_fields.sql"),
        (40, "0040_add_track_preview_url.sql"),
        (41, "0041_add_track_audio_quality.sql")
    ]
    
    for version, filename in migrations:
        checksum = get_checksum(filename)
        now = int(time.time() * 1000000000) # Nanoseconds for sqlx
        
        print(f"Faking migration {version} ({filename})...")
        cursor.execute(
            "INSERT OR IGNORE INTO _sqlx_migrations (version, description, installed_on, success, checksum, execution_time) VALUES (?, ?, ?, 1, ?, 0)",
            (version, filename, now, checksum)
        )
        
    conn.commit()
    conn.close()
    print("Migration table synchronized. You can now run 'cargo tauri dev'.")
except Exception as e:
    print(f"Error fixing migration table: {e}")
    exit(1)

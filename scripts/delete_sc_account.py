
import sqlite3
import os

DB_PATH = "src-tauri/data/syncify.db"

def delete_soundcloud_account():
    if not os.path.exists(DB_PATH):
        print(f"Database not found at {DB_PATH}")
        return
    
    conn = sqlite3.connect(DB_PATH)
    cursor = conn.cursor()
    
    # Get soundcloud service ID
    cursor.execute("SELECT id FROM services WHERE name = 'soundcloud'")
    row = cursor.fetchone()
    if not row:
        print("SoundCloud service not found")
        conn.close()
        return
        
    service_id = row[0]
    print(f"SoundCloud service_id: {service_id}")
    
    # Delete account
    cursor.execute("DELETE FROM accounts WHERE service_id = ?", (service_id,))
    deleted = cursor.rowcount
    conn.commit()
    conn.close()
    
    print(f"Deleted {deleted} SoundCloud account(s) from database")
    print("Please click Connect for SoundCloud in the app to re-authenticate")

if __name__ == "__main__":
    delete_soundcloud_account()

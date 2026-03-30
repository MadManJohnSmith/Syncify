
import sqlite3
import os

DB_PATH = "src-tauri/data/syncify.db"

def delete_apple_music_account():
    if not os.path.exists(DB_PATH):
        print(f"Database not found at {DB_PATH}")
        return
    
    conn = sqlite3.connect(DB_PATH)
    cursor = conn.cursor()
    
    # Get apple_music service ID
    cursor.execute("SELECT id FROM services WHERE name = 'apple_music'")
    row = cursor.fetchone()
    if not row:
        print("Apple Music service not found")
        conn.close()
        return
        
    service_id = row[0]
    print(f"Apple Music service_id: {service_id}")
    
    # Delete account
    cursor.execute("DELETE FROM accounts WHERE service_id = ?", (service_id,))
    deleted = cursor.rowcount
    conn.commit()
    conn.close()
    
    print(f"Deleted {deleted} Apple Music account(s) from database")
    print("Please click Connect for Apple Music in the app to re-authenticate")

if __name__ == "__main__":
    delete_apple_music_account()


import sqlite3
import os

DB_PATH = "src-tauri/data/syncify.db"

def verify_metadata():
    if not os.path.exists(DB_PATH):
        print(f"Database not found at {DB_PATH}")
        return

    conn = sqlite3.connect(DB_PATH)
    cursor = conn.cursor()
    
    print("--- Metadata Preferences (ID 1) ---")
    try:
        cursor.execute("SELECT * FROM metadata_preferences WHERE id = 1")
        row = cursor.fetchone()
        
        if row:
            names = [description[0] for description in cursor.description]
            for i, val in enumerate(row):
                 print(f"{names[i]}: {val}")
        else:
            print("Metadata preferences row 1 not found")
            
    except Exception as e:
        print(f"Error: {e}")
    finally:
        conn.close()

if __name__ == "__main__":
    verify_metadata()

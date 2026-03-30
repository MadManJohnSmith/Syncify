import sqlite3
import os

# Correct path found in src-tauri/src/db.rs
db_path = r'C:\Users\madma\OneDrive\Documents\Syncify\src-tauri\data\syncify.db'

def verify_services():
    print(f"Checking database at: {db_path}")
    if not os.path.exists(db_path):
        print(f"ERROR: Database file not found at {db_path}")
        return

    try:
        conn = sqlite3.connect(db_path)
        cursor = conn.cursor()
        
        # Force WAL checkpoint
        cursor.execute("PRAGMA wal_checkpoint(FULL)")
        
        print("\n--- Service Preferences ---")
        cursor.execute("SELECT service_name, auto_import_enabled, priority, updated_at FROM service_preferences ORDER BY priority")
        rows = cursor.fetchall()
        for row in rows:
            print(f"Service: {row[0]:<12} | Auto-Import: {row[1]} | Priority: {row[2]} | Updated: {row[3]}")
        
        conn.close()
    except Exception as e:
        print(f"An error occurred: {e}")

if __name__ == "__main__":
    verify_services()

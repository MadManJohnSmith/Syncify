import sqlite3
import os
from datetime import datetime

db_path = r'C:\Users\madma\OneDrive\Documents\Syncify\src-tauri\data\syncify.db'

def verify_wal():
    print(f"Checking database at: {db_path}")
    if not os.path.exists(db_path):
        print(f"ERROR: Database file not found at {db_path}")
        return

    try:
        conn = sqlite3.connect(db_path)
        cursor = conn.cursor()
        
        # Force WAL checkpoint to ensure disk reflects recent UI changes
        cursor.execute("PRAGMA wal_checkpoint(FULL)")
        
        print("\n--- Service Preferences Audit ---")
        cursor.execute("SELECT service_name, auto_import_enabled, updated_at FROM service_preferences")
        rows = cursor.fetchall()
        
        today = datetime.now().strftime('%Y-%m-%d')
        found_today = False
        
        for row in rows:
            is_today = today in str(row[2])
            status = "[TODAY]" if is_today else "[OLD]"
            if is_today: found_today = True
            print(f"{status} Service: {row[0]:<12} | Auto-Import: {row[1]} | Updated: {row[2]}")
        
        if found_today:
            print("\nRESULT: PASA - Al menos un registro tiene fecha de hoy.")
        else:
            print("\nRESULT: FALLA - Ningun registro tiene fecha de hoy.")
            
        conn.close()
    except Exception as e:
        print(f"An error occurred: {e}")

if __name__ == "__main__":
    verify_wal()

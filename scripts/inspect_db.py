
import sqlite3
import json
import os

DB_PATH = "src-tauri/data/syncify.db"

def inspect_db():
    if not os.path.exists(DB_PATH):
        print(f"Database not found at {DB_PATH}")
        return

    conn = sqlite3.connect(DB_PATH)
    cursor = conn.cursor()
    
    print("--- Account ID 10 Debug ---")
    try:
        cursor.execute("SELECT * FROM accounts WHERE id = 10")
        row = cursor.fetchone()
        
        # Print column names from cursor description
        names = [description[0] for description in cursor.description]
        print(f"Columns: {names}")
        
        if row:
            for i, val in enumerate(row):
                 print(f"{names[i]}: {val}")
        else:
            print("Row 10 not found")
            
    except Exception as e:
        print(f"Error: {e}")
    except Exception as e:
        print(f"Error querying accounts: {e}")
    finally:
        conn.close()

if __name__ == "__main__":
    inspect_db()

import sqlite3
import os

db_path = r'C:\Users\madma\OneDrive\Documents\Syncify\src-tauri\syncify.db'

def list_tables():
    if not os.path.exists(db_path):
        print(f"ERROR: Database file not found at {db_path}")
        return

    try:
        conn = sqlite3.connect(db_path)
        cursor = conn.cursor()
        cursor.execute("SELECT name FROM sqlite_master WHERE type='table';")
        tables = cursor.fetchall()
        print("Tables in database:")
        for table in tables:
            print(f"- {table[0]}")
        conn.close()
    except Exception as e:
        print(f"An error occurred: {e}")

if __name__ == "__main__":
    list_tables()

import sqlite3
import os

db_path = r'src-tauri/data/syncify.db'
if not os.path.exists(db_path):
    print(f"Error: {db_path} not found")
    exit(1)

conn = sqlite3.connect(db_path)
cursor = conn.cursor()
try:
    cursor.execute("SELECT * FROM download_preferences;")
    rows = cursor.fetchall()
    if not rows:
        print("Table is empty.")
    else:
        colnames = [desc[0] for desc in cursor.description]
        print(" | ".join(colnames))
        print("-" * 50)
        for row in rows:
            print(" | ".join(str(val) for val in row))
except Exception as e:
    print(f"Error executing query: {e}")
finally:
    conn.close()

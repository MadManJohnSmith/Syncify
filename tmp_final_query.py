import sqlite3
import os

db_path = r'src-tauri/data/syncify.db'
if not os.path.exists(db_path):
    print(f"Error: {db_path} not found")
    exit(1)

conn = sqlite3.connect(db_path)
cursor = conn.cursor()

def print_table(table_name):
    print(f"\n--- Table: {table_name} ---")
    try:
        cursor.execute(f"SELECT * FROM {table_name};")
        rows = cursor.fetchall()
        if not rows:
            print("Table is empty.")
            return
        colnames = [desc[0] for desc in cursor.description]
        print(" | ".join(colnames))
        print("-" * 50)
        for row in rows:
            print(" | ".join(str(val) for val in row))
    except Exception as e:
        print(f"Error querying {table_name}: {e}")

print_table("sync_settings")
print_table("folder_settings")

print("\n--- Download Related Keys in 'settings' table ---")
try:
    cursor.execute("SELECT key, value FROM settings WHERE key LIKE 'dl_%';")
    rows = cursor.fetchall()
    for row in rows:
        print(f"{row[0]}: {row[1]}")
except Exception as e:
    print(f"Error querying settings table: {e}")

conn.close()

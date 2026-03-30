import sqlite3

db_path = 'C:/Users/madma/OneDrive/Documents/Syncify/src-tauri/data/syncify.db'

try:
    conn = sqlite3.connect(db_path)
    cursor = conn.cursor()
    
    print("--- 1. Query: metadata_preferences Table ---")
    cursor.execute("SELECT * FROM metadata_preferences LIMIT 1;")
    columns = [description[0] for description in cursor.description]
    row = cursor.fetchone()
    if row:
        for col, val in zip(columns, row):
            print(f"{col}: {val}")
    else:
        print("(No rows in metadata_preferences)")

    print("\n--- 2. Query: settings Table (Requested) ---")
    cursor.execute("SELECT key, value FROM settings WHERE key LIKE '%brainz%' OR key LIKE '%musicbrainz%';")
    rows = cursor.fetchall()
    if not rows:
        print("(No results found in settings table)")
    for row in rows:
        print(f"{row[0]}|{row[1]}")
        
    conn.close()
except Exception as e:
    print(f"Error accessing DB: {e}")

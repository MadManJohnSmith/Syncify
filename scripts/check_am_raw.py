"""Check raw credentials in database."""
import sqlite3
from pathlib import Path

DB_PATH = Path(__file__).parent.parent / "src-tauri" / "data" / "syncify.db"

conn = sqlite3.connect(DB_PATH)
cursor = conn.cursor()

cursor.execute("""
    SELECT a.id, a.display_name, a.credentials_json, s.name
    FROM accounts a
    JOIN services s ON a.service_id = s.id
    WHERE s.name = 'apple_music'
""")

for row in cursor.fetchall():
    account_id, display_name, creds, service = row
    print(f"Account ID: {account_id}")
    print(f"Display Name: {display_name}")
    print(f"Service: {service}")
    print(f"Credentials type: {type(creds)}")
    print(f"Credentials length: {len(creds) if creds else 0}")
    print(f"Credentials (first 200 chars): {creds[:200] if creds else 'None'}")
    print()

conn.close()

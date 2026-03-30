
import sqlite3
import json
import os
import sys
sys.path.insert(0, "src-tauri")

# Import crypto module to decrypt
# Since we can't easily import Rust crypto, let's check if there's a Python equivalent
# For now, just print the encrypted value length to see if it's populated

DB_PATH = "src-tauri/data/syncify.db"

def check_am_creds():
    conn = sqlite3.connect(DB_PATH)
    cursor = conn.cursor()
    
    cursor.execute("SELECT id, credentials_json FROM accounts WHERE service_id = 6")
    row = cursor.fetchone()
    
    if not row:
        print("No Apple Music account found")
        return
        
    acc_id, creds = row
    print(f"Account ID: {acc_id}")
    print(f"Credentials length: {len(creds) if creds else 0}")
    print(f"Credentials (first 100 chars): {creds[:100] if creds else 'None'}...")
    
    conn.close()

if __name__ == "__main__":
    check_am_creds()

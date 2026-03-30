import sqlite3
import sys
import time

db_path = r"c:\Users\tardis\Documents\Syncify\src-tauri\data\syncify.db"
conn = sqlite3.connect(db_path)
c = conn.cursor()

def setup_mock():
    # Insert a fake track if not exists
    c.execute("INSERT OR IGNORE INTO tracks (id, title) VALUES (9999, 'Mock Track')")
    
    # Insert a fake active account with bad crypto logic
    c.execute("INSERT INTO accounts (service_id, email, credentials_json, is_active) VALUES (1, 'mock@aead.com', 'bad_crypto_string_123', 1)")
    account_id = c.lastrowid
    
    # Link track to the account's library
    c.execute("INSERT INTO library_entries (account_id, track_id) VALUES (?, 9999)", (account_id,))
    conn.commit()
    
    print(f"--- BEFORE STARTUP PURGE ---")
    print("Mock Account created:", account_id)
    count = c.execute("SELECT COUNT(*) FROM library_entries WHERE account_id = ?", (account_id,)).fetchone()[0]
    print(f"Library entries for account {account_id}:", count)
    return account_id

def verify_mock(account_id):
    print(f"\n--- AFTER STARTUP PURGE ---")
    
    acct = c.execute("SELECT id, credentials_invalid, is_active FROM accounts WHERE id = ?", (account_id,)).fetchone()
    if acct:
        print(f"Account {account_id} exists: credentials_invalid={acct[1]}, is_active={acct[2]}")
    else:
        print(f"FAIL: Account {account_id} WAS DELETED!")
        
    count = c.execute("SELECT COUNT(*) FROM library_entries WHERE account_id = ?", (account_id,)).fetchone()[0]
    print(f"Library entries for account {account_id}:", count)
    
    if count > 0 and acct and acct[1] == 1:
        print("RESULT: SUCCESS. Data preserved, account marked invalid.")
    else:
        print("RESULT: FAILED.")

if __name__ == "__main__":
    if len(sys.argv) > 1 and sys.argv[1] == "verify":
        verify_mock(int(sys.argv[2]))
    else:
        account_id = setup_mock()
        print(account_id)

import sqlite3
import json
import os

db_path = r'C:\Users\tardis\AppData\Local\com.syncify.app\syncify.db'
db = sqlite3.connect(db_path)
try:
    cursor = db.execute("SELECT credentials_json FROM accounts a JOIN services s ON s.id = a.service_id WHERE s.name = 'qobuz' AND a.is_active = 1")
    res = cursor.fetchone()
    if res:
        creds = json.loads(res[0])
        token = creds.get('user_auth_token') or creds.get('auth_token')
        print(token)
    else:
        print("No active Qobuz account found")
finally:
    db.close()

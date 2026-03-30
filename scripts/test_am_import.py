"""Test Apple Music API directly to debug import issues."""
import sqlite3
import json
import requests
from pathlib import Path

# Database path
DB_PATH = Path(__file__).parent.parent / "src-tauri" / "data" / "syncify.db"

def get_apple_music_creds():
    """Get Apple Music credentials from database."""
    conn = sqlite3.connect(DB_PATH)
    cursor = conn.cursor()
    
    # Get the Apple Music account
    cursor.execute("""
        SELECT a.id, a.credentials_json, s.name
        FROM accounts a
        JOIN services s ON a.service_id = s.id
        WHERE s.name = 'apple_music'
        AND a.is_active = 1
    """)
    
    row = cursor.fetchone()
    conn.close()
    
    if not row:
        return None, None, None
    
    account_id, creds_json, service_name = row
    creds = json.loads(creds_json) if creds_json else {}
    return account_id, creds, service_name

def test_library_songs(music_user_token: str, developer_token: str):
    """Test fetching library songs from Apple Music API."""
    url = "https://api.music.apple.com/v1/me/library/songs?limit=10"
    
    headers = {
        "Authorization": f"Bearer {developer_token}",
        "Music-User-Token": music_user_token,
    }
    
    print(f"Request URL: {url}")
    print(f"Developer Token (first 50 chars): {developer_token[:50]}...")
    print(f"Music User Token (first 50 chars): {music_user_token[:50]}...")
    print()
    
    response = requests.get(url, headers=headers)
    
    print(f"Status Code: {response.status_code}")
    print(f"Response Headers: {dict(response.headers)}")
    print()
    
    if response.status_code == 200:
        data = response.json()
        songs = data.get("data", [])
        print(f"Number of songs returned: {len(songs)}")
        if songs:
            for i, song in enumerate(songs[:3]):  # Show first 3
                attrs = song.get("attributes", {})
                print(f"  {i+1}. {attrs.get('name')} - {attrs.get('artistName')}")
        else:
            print("No songs in library!")
    else:
        print(f"Error response: {response.text}")

def main():
    account_id, creds, service = get_apple_music_creds()
    
    if not creds:
        print("No Apple Music credentials found in database!")
        return
    
    print(f"Found Apple Music account ID: {account_id}")
    print(f"Credentials keys: {list(creds.keys())}")
    print()
    
    music_user_token = creds.get("music_user_token")
    developer_token = creds.get("developer_token")
    
    if not music_user_token:
        print("ERROR: music_user_token is missing!")
        return
    if not developer_token:
        print("ERROR: developer_token is missing!")
        return
    
    test_library_songs(music_user_token, developer_token)

if __name__ == "__main__":
    main()

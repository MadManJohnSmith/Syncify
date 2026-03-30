
import json
import requests
import re
from pathlib import Path

SETTINGS_PATH = Path("adjacent_tools/Syncify-test/.gui_settings.json")

def fetch_developer_token():
    """Fetch the developer token from Apple Music website."""
    print("Fetching developer token from Apple Music...")
    try:
        response = requests.get('https://music.apple.com/us/browse')
        if response.status_code != 200:
            print(f"Failed to get Apple Music page: {response.status_code}")
            return None
        
        # Find the JS bundle URL
        match = re.search(r'(?<=index)(.*?)(?=\.js")', response.text)
        if not match:
            print("Failed to find JS bundle in page")
            return None
        
        index_js = match.group(1)
        response = requests.get(f'https://music.apple.com/assets/index{index_js}.js')
        if response.status_code != 200:
            print("Failed to get JS bundle")
            return None
        
        # Extract token from JS
        match = re.search(r'(?=eyJh)(.*?)(?=")', response.text)
        if not match:
            print("Failed to find developer token in JS")
            return None
        
        return match.group(1)
    except Exception as e:
        print(f"Error: {e}")
        return None

def test_apple_music():
    if not SETTINGS_PATH.exists():
        print(f"Settings file not found at {SETTINGS_PATH}")
        return
    
    settings = json.loads(SETTINGS_PATH.read_text(encoding='utf-8'))
    music_user_token = settings.get("apple_music_token", "")
    
    if not music_user_token:
        print("No apple_music_token found in settings")
        return
    
    print(f"Music User Token: {music_user_token[:30]}...")
    
    # Fetch developer token
    dev_token = fetch_developer_token()
    if not dev_token:
        print("Could not fetch developer token")
        return
    
    print(f"Developer Token: {dev_token[:30]}...")
    
    # Test API call
    url = "https://api.music.apple.com/v1/me/library/songs?limit=5"
    headers = {
        "Authorization": f"Bearer {dev_token}",
        "Music-User-Token": music_user_token
    }
    
    print(f"\nTesting API: {url}")
    resp = requests.get(url, headers=headers)
    print(f"Status: {resp.status_code}")
    print(f"Response: {resp.text[:300]}...")

if __name__ == "__main__":
    test_apple_music()

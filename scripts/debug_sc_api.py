
import json
import requests
import os
from pathlib import Path

# Path used by soundcloud_auth.py
CACHE_PATH = Path("adjacent_tools/Syncify-test/.gui_credentials_cache.json")

def debug_sc_api():
    if not CACHE_PATH.exists():
        print(f"Cache file not found at {CACHE_PATH}")
        return

    try:
        data = json.loads(CACHE_PATH.read_text(encoding='utf-8'))
        sc_data = data.get("soundcloud", {})
        token = sc_data.get("token") or sc_data.get("oauth_token")
        
        if not token:
            print("No SoundCloud token found in cache")
            return
            
        print(f"Testing API with token: {token[:15]}...")
        
        # Test /me
        headers = {"Authorization": token} # token usually includes "OAuth " prefix
        if not token.startswith("OAuth"):
             headers["Authorization"] = f"OAuth {token}"
             
        url = "https://api-v2.soundcloud.com/me"
        print(f"Requesting {url}...")
        resp = requests.get(url, headers=headers)
        
        print(f"Status: {resp.status_code}")
        print(f"Response: {resp.text[:200]}...")
        
        if resp.status_code != 200:
            print("\nTrying with explicit client_id query param (scraping needed?)")
            
    except Exception as e:
        print(f"Error: {e}")

if __name__ == "__main__":
    debug_sc_api()

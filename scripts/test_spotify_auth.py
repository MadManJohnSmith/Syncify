#!/usr/bin/env python3
"""Test Spotify OAuth directly."""
import os
import sys
from pathlib import Path

# Load .env
from dotenv import load_dotenv
load_dotenv(Path(__file__).parent.parent / ".env")

print("=== Spotify Auth Test ===")
print(f"CLIENT_ID: {os.getenv('SPOTIPY_CLIENT_ID', 'NOT SET')}")
print(f"CLIENT_SECRET: {'SET' if os.getenv('SPOTIPY_CLIENT_SECRET') else 'NOT SET'}")
print(f"REDIRECT_URI: {os.getenv('SPOTIPY_REDIRECT_URI', 'NOT SET')}")

try:
    import spotipy
    from spotipy.oauth2 import SpotifyOAuth
    
    auth_manager = SpotifyOAuth(
        client_id=os.getenv('SPOTIPY_CLIENT_ID'),
        client_secret=os.getenv('SPOTIPY_CLIENT_SECRET'),
        redirect_uri=os.getenv('SPOTIPY_REDIRECT_URI'),
        scope='user-library-read user-read-private user-read-email',
        cache_path=str(Path(__file__).parent / '.spotify_cache.json'),
        open_browser=True
    )
    
    print("\nAttempting to get auth token...")
    # This will open browser if no cached token
    sp = spotipy.Spotify(auth_manager=auth_manager)
    
    # Test the connection
    user = sp.current_user()
    print(f"\n✅ SUCCESS! Connected as: {user.get('display_name')}")
    print(f"   Email: {user.get('email')}")
    print(f"   User ID: {user.get('id')}")
    
except spotipy.SpotifyOauthError as e:
    print(f"\n❌ OAuth Error: {e}")
    print("\nMake sure your Spotify Developer Dashboard settings match:")
    print(f"  - Redirect URI: {os.getenv('SPOTIPY_REDIRECT_URI')}")
except Exception as e:
    print(f"\n❌ Error: {e}")
    import traceback
    traceback.print_exc()

#!/usr/bin/env python3
"""
Auth Bridge - CLI interface to Syncify-test auth services.

Usage:
    python auth_bridge.py <service> <action>

Services: spotify, tidal, qobuz, deezer, soundcloud
Actions: login, status, logout

Returns JSON:
    {"success": true/false, "data": {...}, "error": "..."}
"""

import json
import sys
import os
from pathlib import Path
from contextlib import contextmanager
from io import StringIO

# Add local services to path (S43: relocated from adjacent_tools/Syncify-test)
SCRIPTS_DIR = Path(__file__).parent
if str(SCRIPTS_DIR) not in sys.path:
    sys.path.insert(0, str(SCRIPTS_DIR))
# Project root for credential cache files
PROJECT_ROOT = SCRIPTS_DIR.parent

# Load .env from project root
from dotenv import load_dotenv
load_dotenv(Path(__file__).parent.parent / ".env")

@contextmanager
def suppress_stdout():
    """Temporarily suppress stdout to prevent library output from polluting JSON."""
    old_stdout = sys.stdout
    sys.stdout = StringIO()
    try:
        yield
    finally:
        sys.stdout = old_stdout


def json_response(success: bool, data=None, error=None):
    """Output JSON response and exit."""
    result = {"success": success}
    if data:
        result["data"] = data
    if error:
        result["error"] = error
    print(json.dumps(result))
    sys.exit(0 if success else 1)


def is_viable_qobuz_token(token) -> bool:
    if token is None:
        return False
    value = str(token).strip()
    if not value or value in ("null", "undefined", "browser_cookies"):
        return False
    if value.startswith("{") or value.startswith("["):
        return False
    if len(value) < 16:
        return False
    if any(ch.isspace() for ch in value):
        return False
    return True


def handle_spotify(action: str):
    """Handle Spotify auth actions."""
    from services.spotify_api import get_spotify_connection
    
    if action == "login":
        # spotipy handles the full OAuth flow
        # Suppress stdout to prevent rich console output from polluting JSON
        with suppress_stdout():
            sp = get_spotify_connection(
                scopes="user-library-read user-library-modify user-read-private user-read-email playlist-read-private",
                verbose_flag=False
            )
        if sp:
            user = sp.current_user()
            # Get access token from spotipy's auth manager
            token_info = sp.auth_manager.get_cached_token() if hasattr(sp, 'auth_manager') else None
            access_token = token_info.get("access_token") if token_info else None
            
            json_response(True, {
                "user_id": user.get("id"),
                "display_name": user.get("display_name"),
                "email": user.get("email"),
                "access_token": access_token,
                "refresh_token": token_info.get("refresh_token") if token_info else None,
            })
        else:
            json_response(False, error="Spotify auth failed")
            
    elif action == "status":
        sp = get_spotify_connection(scopes=None, verbose_flag=False)
        if sp:
            try:
                user = sp.current_user()
                json_response(True, {"connected": True, "user": user.get("display_name")})
            except:
                json_response(True, {"connected": False})
        else:
            json_response(True, {"connected": False})
            
    elif action == "logout":
        # Remove cached token
        cache_path = PROJECT_ROOT / ".spotify_token_cache.json"
        if cache_path.exists():
            cache_path.unlink()
        json_response(True, {"message": "Logged out"})


def handle_tidal(action: str):
    """Handle Tidal auth actions via device code flow."""
    from services.tidal_auth import tidal_login, tidal_status, tidal_logout, tidal_refresh, TidalAuth
    
    if action == "login":
        result = tidal_login()
        # tidal_login returns {"status": "success"} not {"success": true}
        if result.get("status") == "success":
            # Get the stored tokens to return for Rust to save
            auth = TidalAuth(verbose=False)
            tokens = auth.get_stored_tokens()
            if tokens:
                json_response(True, {
                    "message": result.get("message", "Connected to Tidal"),
                    "access_token": tokens.get("access_token"),
                    "refresh_token": tokens.get("refresh_token"),
                    "user_id": str(tokens.get("user_id", tokens.get("user", {}).get("userId", ""))),
                    "country_code": tokens.get("user", {}).get("countryCode", "US"),
                    "email": tokens.get("user", {}).get("email"),
                })
            else:
                json_response(True, {"message": result.get("message", "Connected to Tidal")})
        else:
            json_response(False, error=result.get("message", "Tidal login failed"))
            
    elif action == "status":
        result = tidal_status()
        json_response(True, result)
        
    elif action == "logout":
        result = tidal_logout()
        json_response(True, result)
        
    elif action == "refresh":
        result = tidal_refresh()
        if result.get("status") == "success":
            # Get refreshed tokens
            auth = TidalAuth(verbose=False)
            tokens = auth.get_stored_tokens()
            if tokens:
                json_response(True, {
                    "message": result.get("message", "Token refreshed"),
                    "access_token": tokens.get("access_token"),
                })
            else:
                json_response(True, {"message": result.get("message", "Token refreshed")})
        else:
            json_response(False, error=result.get("message", "Token refresh failed"))


def handle_qobuz(action: str):
    """Handle Qobuz auth actions via browser automation."""
    from services.qobuz_auth import QobuzAuth
    import asyncio
    
    auth = QobuzAuth(verbose=True)
    
    if action == "login":
        loop = asyncio.new_event_loop()
        success, result = loop.run_until_complete(auth.login_with_browser())
        loop.close()
        
        if success:
            # Get stored session for Rust to save
            session = auth.get_stored_session()
            # Session stores "auth_token" not "user_auth_token"
            auth_token = session.get("auth_token") if session else None
            if not is_viable_qobuz_token(auth_token):
                auth_token = None
            
            # Also get username/password from "qobuz" cache for API fallback
            qobuz_creds = {
                "username": session.get("username") if session else None,
                "password": session.get("password") if session else None,
            }
            try:
                import json
                cache_file = auth.credentials_file
                if cache_file.exists():
                    with open(cache_file) as f:
                        cache = json.load(f)
                        cache_creds = cache.get("qobuz", {})
                        if not qobuz_creds.get("username"):
                            qobuz_creds["username"] = cache_creds.get("username")
                        if not qobuz_creds.get("password"):
                            qobuz_creds["password"] = cache_creds.get("password")
            except:
                pass

            username = (qobuz_creds.get("username") or "").strip() or None
            password = (qobuz_creds.get("password") or "").strip() or None
            if not auth_token and not (username and password):
                json_response(
                    False,
                    error=(
                        "Qobuz login finished in browser but no API token or fallback credentials were captured. "
                        "Please log out from Qobuz in the browser first, then reconnect and enter email/password manually."
                    ),
                )
            
            json_response(True, {
                "user_id": result,
                "user_auth_token": auth_token,  # Key expected by Rust
                "auth_token": auth_token,
                "display_name": result,
                "username": username,
                "password": password,
            })

        else:
            json_response(False, error=result)
            
    elif action == "status":
        status = auth.get_status()
        json_response(True, status)
        
    elif action == "logout":
        auth.clear_session()
        json_response(True, {"message": "Logged out"})


def handle_deezer(action: str):
    """Handle Deezer auth actions via browser automation."""
    from services.deezer_auth import deezer_login, deezer_status, deezer_logout, DeezerAuth
    
    if action == "login":
        result = deezer_login()
        # deezer_login returns {"status": "success"} not {"success": true}
        if result.get("status") == "success":
            # Get stored ARL for Rust to save
            auth = DeezerAuth(verbose=False)
            arl = auth.get_stored_arl()
            json_response(True, {
                "message": result.get("message", "Connected to Deezer"),
                "arl": arl,
                "access_token": arl,  # Use ARL as access_token for compatibility
            })
        else:
            json_response(False, error=result.get("message", "Deezer login failed"))
            
    elif action == "status":
        result = deezer_status()
        json_response(True, result)
        
    elif action == "logout":
        result = deezer_logout()
        json_response(True, result)


def handle_soundcloud(action: str):
    """Handle SoundCloud auth actions."""
    from services.soundcloud_auth import SoundCloudAuth
    import asyncio
    import requests
    
    auth = SoundCloudAuth(verbose=True)
    
    if action == "login":
        loop = asyncio.new_event_loop()
        success, result = loop.run_until_complete(auth.login_with_browser())
        loop.close()
        
        if success:
            oauth_token = result
            # Fetch user info to store in DB
            headers = {
                "Authorization": oauth_token,
                "User-Agent": "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36"
            }
            try:
                # Get user ID and profile
                resp = requests.get("https://api-v2.soundcloud.com/me", headers=headers)
                if resp.status_code == 200:
                    user_data = resp.json()
                    json_response(True, {
                        "oauth_token": oauth_token,
                        "access_token": oauth_token.replace("OAuth ", ""),
                        "user_id": user_data.get("id"),
                        "username": user_data.get("username"),
                        "display_name": user_data.get("username"),
                        "avatar_url": user_data.get("avatar_url")
                    })
                else:
                     # Fallback if API fails - log full response
                     error_msg = f"Failed to fetch user info: {resp.status_code} {resp.text[:100]}"
                     json_response(True, {
                        "oauth_token": oauth_token, 
                        "access_token": oauth_token.replace("OAuth ", ""),
                        "user_id": 0, # Placeholder
                        "display_name": "SoundCloud User",
                        "error": error_msg
                     })
            except Exception as e:
                json_response(True, {
                    "oauth_token": oauth_token, 
                    "access_token": oauth_token.replace("OAuth ", ""),
                    "error": str(e)
                })
        else:
            json_response(False, error=result)
            
    elif action == "status":
        status = auth.get_status()
        json_response(True, status)
        
    elif action == "logout":
        auth.clear_token()
        json_response(True, {"message": "Logged out"})


def handle_apple_music(action: str):
    """Handle Apple Music auth actions via browser automation."""
    from services.apple_music_auth import AppleMusicAuth, apple_music_status, apple_music_logout
    import asyncio
    
    if action == "login":
        auth = AppleMusicAuth(verbose=True)
        loop = asyncio.new_event_loop()
        success, result = loop.run_until_complete(auth.login_with_browser())
        loop.close()
        
        if success:
            # result is the music user token
            music_user_token = result
            # fetch developer token from auth instance
            dev_token = auth._access_token
            
            # Double check if dev token is missing (might happen if cached)
            if not dev_token:
                dev_token = auth._fetch_access_token()
                
            json_response(True, {
                "message": "Connected to Apple Music",
                "music_user_token": music_user_token,
                "developer_token": dev_token
            })
        else:
            json_response(False, error=str(result))
            
    elif action == "status":
        result = apple_music_status()
        json_response(True, result)
        
    elif action == "logout":
        result = apple_music_logout()
        json_response(True, result)


HANDLERS = {
    "spotify": handle_spotify,
    "tidal": handle_tidal,
    "qobuz": handle_qobuz,
    "deezer": handle_deezer,
    "soundcloud": handle_soundcloud,
    "apple_music": handle_apple_music,
}


def main():
    if len(sys.argv) < 3:
        json_response(False, error="Usage: auth_bridge.py <service> <action>")
    
    service = sys.argv[1].lower()
    action = sys.argv[2].lower()
    
    if service not in HANDLERS:
        json_response(False, error=f"Unknown service: {service}. Valid: {list(HANDLERS.keys())}")
    
    if action not in ("login", "status", "logout"):
        json_response(False, error=f"Unknown action: {action}. Valid: login, status, logout")
    
    try:
        HANDLERS[service](action)
    except Exception as e:
        json_response(False, error=str(e))


if __name__ == "__main__":
    main()

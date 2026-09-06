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
try:
    from dotenv import load_dotenv
    load_dotenv(Path(__file__).parent.parent / ".env")
except ImportError:
    pass

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


def handle_spotify(args=None, *extra_args, **kwargs) -> dict:
    """Handle Spotify auth actions.

    Spotify authentication is handled natively in Rust via OAuth PKCE in Tauri.
    The Python bridge is deprecated for Spotify auth.
    """
    # Clean up legacy cache file if logout requested
    action = None
    if isinstance(args, str):
        action = args.lower()
    elif isinstance(args, dict):
        action = args.get("action", "").lower()
    elif hasattr(args, "action"):
        action = getattr(args, "action", "").lower()

    if action == "logout":
        cache_path = PROJECT_ROOT / ".spotify_token_cache.json"
        if cache_path.exists():
            try:
                cache_path.unlink()
            except OSError:
                pass

    return {
        "success": False,
        "service": "spotify",
        "message": "Spotify authentication is handled natively in Rust via OAuth PKCE in Tauri. Python bridge is deprecated for Spotify auth.",
        "native": True,
    }


def handle_tidal(action: str):
    """Handle Tidal auth actions via device code flow."""
    from services.tidal_auth import tidal_login, tidal_status, tidal_logout, tidal_refresh, TidalAuth
    
    if action == "login":
        auth = TidalAuth(verbose=False)
        result = tidal_login(auth)
        # tidal_login returns {"status": "success"} not {"success": true}
        if result.get("status") == "success":
            tokens = result.get("tokens") or auth.get_stored_tokens()
            if tokens:
                json_response(True, {
                    "message": result.get("message", "Connected to Tidal"),
                    "access_token": tokens.get("access_token"),
                    "refresh_token": tokens.get("refresh_token"),
                    "token_expiry": tokens.get("token_expiry"),
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
            
            # Also get username/password from captured creds or session for API fallback
            captured_creds = getattr(auth, "_captured_credentials", None) or {}
            qobuz_creds = {
                "username": session.get("username") if session else None,
                "password": session.get("password") if session else None,
            }
            if not qobuz_creds.get("username"):
                qobuz_creds["username"] = captured_creds.get("username")
            if not qobuz_creds.get("password"):
                qobuz_creds["password"] = captured_creds.get("password")

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
            arl = result.get("arl")
            if not arl:
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
    service = None
    action = None

    # Support flag-based options: --service <svc> [--action <act>]
    # as well as positional: <svc> <act>
    if "--service" in sys.argv or "-s" in sys.argv:
        import argparse
        parser = argparse.ArgumentParser(description="Syncify Auth Bridge CLI")
        parser.add_argument("--service", "-s", required=True, help="Service name")
        parser.add_argument("--action", "-a", default="status", help="Action name")
        args, _ = parser.parse_known_args()
        service = args.service.lower()
        action = args.action.lower()
    elif len(sys.argv) >= 2 and not sys.argv[1].startswith("-"):
        service = sys.argv[1].lower()
        action = sys.argv[2].lower() if len(sys.argv) >= 3 else "status"
    else:
        json_response(False, error="Usage: auth_bridge.py <service> <action> or --service <service> [--action <action>]")

    if service not in HANDLERS:
        json_response(False, error=f"Unknown service: {service}. Valid: {list(HANDLERS.keys())}")

    if action not in ("login", "status", "logout", "refresh"):
        json_response(False, error=f"Unknown action: {action}. Valid: login, status, logout, refresh")

    try:
        res = HANDLERS[service](action)
        if isinstance(res, dict):
            print(json.dumps(res))
            sys.exit(0)
    except Exception as e:
        json_response(False, error=str(e))


if __name__ == "__main__":
    main()

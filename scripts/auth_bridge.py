#!/usr/bin/env python3
"""
Auth Bridge - CLI interface to Syncify-test auth services.

Usage:
    python auth_bridge.py <service> <action>

Services: spotify, tidal, qobuz, deezer, soundcloud
Actions: login, status, logout, refresh (refresh only for spotify and tidal; the other
services answer with an explicit {"success": false, "error": ...})

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

# .env is loaded only in development runs (SYNCIFY_ENV=development);
# release builds rely on the OS environment / keyring for credentials.
if os.getenv("SYNCIFY_ENV", "").strip().lower() in ("development", "dev"):
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


from services.bridge_protocol import BridgeFailure, FailureEnvelope, run  # noqa: E402


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


def qobuz_visible_display_name(user_id, username) -> str:
    """Qobuz account's visible display name, NEVER the provider's numeric id.

    `login_with_browser` returns the captured id ("browser_session" when it
    found none), which the handler stored as-is in `display_name`: the UI
    showed it as "Connected as 123456789" and two accounts of the same
    person became indistinguishable. Priority: session marker or opaque id ->
    captured email; if there is no email either, a service generic.

    The real id still comes out in `user_id`: Rust uses it as `external_user_id`
    (deduplicated identity), never as a visible name.
    """
    email = (username or "").strip()
    if email and "@" in email and "." in email.split("@")[-1]:
        return email
    return "Qobuz User"


def extract_sp_dc() -> str:
    """Securely retrieve sp_dc session cookie from SYNCIFY_SP_DC or stdin.

    Strictly avoids reading from sys.argv to prevent leaking sensitive session
    cookies into local process listing tables (/proc/<pid>/cmdline, ps aux, etc.).
    """
    # 1. Check environment variable (SYNCIFY_SP_DC or SPOTIFY_SP_DC)
    env_sp_dc = os.environ.get("SYNCIFY_SP_DC") or os.environ.get("SPOTIFY_SP_DC")
    if env_sp_dc and env_sp_dc.strip():
        return env_sp_dc.strip()

    # 2. Check standard input (stdin)
    try:
        if not sys.stdin.isatty():
            content = sys.stdin.read().strip()
            if content:
                # Support JSON payload: {"sp_dc": "..."}
                try:
                    data = json.loads(content)
                    if isinstance(data, dict):
                        for key in ("sp_dc", "cookie", "token", "session_cookie"):
                            val = data.get(key)
                            if val and isinstance(val, str) and val.strip():
                                return val.strip()
                except Exception:
                    pass
                # Support direct raw string
                return content
    except Exception:
        pass

    return None


def exchange_sp_dc_for_token(sp_dc: str):
    """Exchange Spotify sp_dc session cookie for Web Player access token."""
    if not sp_dc or not sp_dc.strip():
        return None

    clean_sp_dc = sp_dc.strip()

    # Mock / deterministic hook for tests and offline validation
    if os.environ.get("SYNCIFY_AUTH_MOCK") == "1" or clean_sp_dc.startswith("mock_"):
        return {
            "accessToken": f"mock_spotify_access_token_{abs(hash(clean_sp_dc)):08x}",
            "accessTokenExpirationTimestampMs": 1750000000000,
            "isAnonymous": False,
        }

    import urllib.request
    import urllib.error

    url = "https://open.spotify.com/get_access_token?reason=transport&productType=web_player"
    headers = {
        "Cookie": f"sp_dc={clean_sp_dc}",
        "User-Agent": "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/124.0.0.0 Safari/537.36",
        "Referer": "https://open.spotify.com/",
    }

    req = urllib.request.Request(url, headers=headers)
    try:
        with urllib.request.urlopen(req, timeout=10) as resp:
            raw_body = resp.read().decode("utf-8")
            data = json.loads(raw_body)
            if data and data.get("accessToken") and not data.get("isAnonymous", False):
                return data
            return None
    except Exception:
        return None


def handle_spotify(args=None, *extra_args, sp_dc: str = None, **kwargs) -> dict:
    """Handle Spotify auth actions.

    Spotify login and status are handled natively in Rust via OAuth PKCE in Tauri.
    Session token refresh via sp_dc is handled securely via stdin / SYNCIFY_SP_DC (SEC-022).
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

    if action == "refresh":
        # SEC-022: Enforce that sp_dc is never passed via sys.argv
        if len(sys.argv) > 3 or any("--sp" in arg or "sp_dc=" in arg for arg in sys.argv):
            raise BridgeFailure("Passing sensitive credentials (sp_dc) via CLI arguments is strictly prohibited. Provide sp_dc via stdin or SYNCIFY_SP_DC environment variable.")

        cookie_val = sp_dc or kwargs.get("sp_dc") or extract_sp_dc()
        if not cookie_val:
            raise BridgeFailure("No sp_dc session cookie provided. Provide via stdin (JSON or raw text) or SYNCIFY_SP_DC environment variable.")

        token_data = exchange_sp_dc_for_token(cookie_val)
        if token_data and "accessToken" in token_data:
            return {
                "accessToken": token_data["accessToken"],
                "accessTokenExpirationTimestampMs": token_data.get("accessTokenExpirationTimestampMs", 0),
                "isAnonymous": token_data.get("isAnonymous", False),
            }
        else:
            raise BridgeFailure("sp_dc token refresh failed — cookie may be expired or invalid")

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
        # The verification URL travels in the response even when the flow fails: it is
        # the only way the user can complete the connection by hand
        # when the desktop does not open the browser.
        verification_url = (result.get("device_info") or {}).get("verification_url")
        # tidal_login returns {"status": "success"} not {"success": true}
        if result.get("status") == "success":
            tokens = result.get("tokens") or auth.get_stored_tokens()
            if tokens:
                # Tidal's device-flow token response carries a `user` object;
                # whichever identity fields it includes travel up so the account
                # row is never a generic name. Rust validates every value
                # (`sanitize_visible_display_name`) before storing it.
                user_obj = tokens.get("user") or {}
                tidal_name = (
                    user_obj.get("username")
                    or user_obj.get("profileName")
                    or user_obj.get("userName")
                    or user_obj.get("displayName")
                )
                return {
                    "message": result.get("message", "Connected to Tidal"),
                    "access_token": tokens.get("access_token"),
                    "refresh_token": tokens.get("refresh_token"),
                    "token_expiry": tokens.get("token_expiry"),
                    "user_id": str(tokens.get("user_id", user_obj.get("userId", ""))),
                    "country_code": user_obj.get("countryCode", "US"),
                    "email": user_obj.get("email"),
                    "display_name": tidal_name,
                    "verification_url": verification_url,
                }
            else:
                return {
                    "message": result.get("message", "Connected to Tidal"),
                    "verification_url": verification_url,
                }
        else:
            return FailureEnvelope({
                "success": False,
                 "data": {"verification_url": verification_url},
                "error": result.get("message", "Tidal login failed"),
            })
            
    elif action == "status":
        result = tidal_status()
        return result
        
    elif action == "logout":
        result = tidal_logout()
        return result
        
    elif action == "refresh":
        result = tidal_refresh()
        if result.get("status") == "success":
            # Get refreshed tokens
            auth = TidalAuth(verbose=False)
            tokens = auth.get_stored_tokens()
            if tokens:
                return {
                    "message": result.get("message", "Token refreshed"),
                    "access_token": tokens.get("access_token"),
                }
            else:
                return {"message": result.get("message", "Token refreshed")}
        else:
            raise BridgeFailure(result.get("message", "Token refresh failed"))


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
                raise BridgeFailure("Qobuz login finished in browser but no API token or fallback credentials were captured. "
                        "Please log out from Qobuz in the browser first, then reconnect and enter email/password manually.")

            # VISIBLE identity, never the provider's numeric id or a session
            # marker: `display_name` is shown in the UI and stored in
            # accounts.display_name, so an opaque id ("123456789") or the
            # bridge's placeholder are indistinguishable between two accounts of the
            # same person and stop being readable. The real id still travels as
            # `user_id` (Rust maps it to external_user_id, the deduplicated
            # identity), and `username` is only propagated as an email when it is a
            # plausible email (decided by Rust, not this bridge).
            session_name = (session.get("display_name") if session else None) or None
            session_email = (session.get("email") if session else None) or None
            return {
                "user_id": result,
                "user_auth_token": auth_token,  # Key expected by Rust
                "auth_token": auth_token,
                "display_name": session_name or qobuz_visible_display_name(result, username),
                "email": session_email,
                "username": username,
                "password": password,
            }

        else:
            raise BridgeFailure(result)
            
    elif action == "status":
        status = auth.get_status()
        return status
        
    elif action == "logout":
        auth.clear_session()
        return {"message": "Logged out"}


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
            return {
                "message": result.get("message", "Connected to Deezer"),
                "arl": arl,
                "access_token": arl,  # Use ARL as access_token for compatibility
            }
        else:
            raise BridgeFailure(result.get("message", "Deezer login failed"))
            
    elif action == "status":
        result = deezer_status()
        return result
        
    elif action == "logout":
        result = deezer_logout()
        return result


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
                    return {
                        "oauth_token": oauth_token,
                        "access_token": oauth_token.replace("OAuth ", ""),
                        "user_id": user_data.get("id"),
                        "username": user_data.get("username"),
                        "display_name": user_data.get("username"),
                        "avatar_url": user_data.get("avatar_url")
                    }
                else:
                     # Fallback if API fails - log full response
                     error_msg = f"Failed to fetch user info: {resp.status_code} {resp.text[:100]}"
                     return {
                        "oauth_token": oauth_token, 
                        "access_token": oauth_token.replace("OAuth ", ""),
                        "user_id": 0, # Placeholder
                        "display_name": "SoundCloud User",
                        "error": error_msg
                     }
            except Exception as e:
                return {
                    "oauth_token": oauth_token, 
                    "access_token": oauth_token.replace("OAuth ", ""),
                    "error": str(e)
                }
        else:
            raise BridgeFailure(result)
            
    elif action == "status":
        status = auth.get_status()
        return status
        
    elif action == "logout":
        auth.clear_token()
        return {"message": "Logged out"}


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
                
            return {
                "message": "Connected to Apple Music",
                "music_user_token": music_user_token,
                "developer_token": dev_token,
                # Catalog storefront of the account: the Rust client stores it with
                # the credentials and uses it for every /catalog/{storefront}/ call.
                "storefront": auth.storefront,
                # Apple ID typed at sign-in when the form was readable; Apple's
                # APIs expose no profile, so this is the only identity available.
                "email": getattr(auth, "_captured_apple_id", None),
            }
        else:
            raise BridgeFailure(str(result))
            
    elif action == "status":
        result = apple_music_status()
        return result
        
    elif action == "logout":
        result = apple_music_logout()
        return result


HANDLERS = {
    "spotify": handle_spotify,
    "tidal": handle_tidal,
    "qobuz": handle_qobuz,
    "deezer": handle_deezer,
    "soundcloud": handle_soundcloud,
    "apple_music": handle_apple_music,
}

# Services whose provider actually issues renewable tokens. The rest (Qobuz,
# Deezer, SoundCloud, Apple Music) authenticate with long-lived credentials
# (session token / ARL cookie / OAuth token / music user token) that this bridge
# cannot renew, so `refresh` fails explicitly instead of silently doing nothing.
REFRESH_CAPABLE_SERVICES = ("spotify", "tidal")


def _parse(argv):
    service = None
    action = None

    # Flag-based: --service <svc> [--action <act>]; también posicional: <svc> <act>
    if "--service" in argv or "-s" in argv:
        import argparse
        parser = argparse.ArgumentParser(description="Syncify Auth Bridge CLI")
        parser.add_argument("--service", "-s", required=True, help="Service name")
        parser.add_argument("--action", "-a", default="status", help="Action name")
        args, extra = parser.parse_known_args(argv)
        service = args.service.lower()
        action = args.action.lower()
        # SEC-022: prohibit sensitive tokens in extra CLI arguments
        if service == "spotify" and action == "refresh" and extra:
            raise BridgeFailure("Passing sensitive credentials (sp_dc) via CLI arguments is strictly prohibited. Provide sp_dc via stdin or SYNCIFY_SP_DC environment variable.")
    elif argv and not argv[0].startswith("-"):
        service = argv[0].lower()
        action = argv[1].lower() if len(argv) >= 2 else "status"
        # SEC-022: prohibit sensitive tokens in positional CLI arguments
        if service == "spotify" and action == "refresh" and len(argv) > 2:
            raise BridgeFailure("Passing sensitive credentials (sp_dc) via CLI arguments is strictly prohibited. Provide sp_dc via stdin or SYNCIFY_SP_DC environment variable.")
    else:
        raise BridgeFailure("Usage: auth_bridge.py <service> <action> or --service <service> [--action <action>]")

    # SEC-022: prohibit sensitive flags or tokens anywhere in argv
    for arg in argv:
        lower_arg = arg.lower()
        if lower_arg.startswith("--sp-dc") or lower_arg.startswith("--sp_dc") or "sp_dc=" in lower_arg:
            raise BridgeFailure("Passing sensitive credentials (sp_dc) via CLI arguments is strictly prohibited. Provide sp_dc via stdin or SYNCIFY_SP_DC environment variable.")

    if service not in HANDLERS:
        raise BridgeFailure(f"Unknown service: {service}. Valid: {list(HANDLERS.keys())}")

    if action not in ("login", "status", "logout", "refresh"):
        raise BridgeFailure(f"Unknown action: {action}. Valid: login, status, logout, refresh")

    if action == "refresh" and service not in REFRESH_CAPABLE_SERVICES:
        raise BridgeFailure(f"Token refresh is not supported for '{service}': the provider does not issue "
                f"renewable tokens for this integration. Supported: {', '.join(REFRESH_CAPABLE_SERVICES)}. "
                f"Reconnect the service with '{service} login' instead.")

    return (service, action)


def _handle(pair):
    service, action = pair
    try:
        res = HANDLERS[service](action)
    except Exception as e:
        raise BridgeFailure(str(e))
    if isinstance(res, dict):
        return res  # envelope explícito histórico de auth (verbatim)
    raise BridgeFailure(f"No result produced by the '{service}' handler for action '{action}'")


def main():
    return run(_handle, parse=_parse)


if __name__ == "__main__":
    sys.exit(main())

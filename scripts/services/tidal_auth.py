"""
Tidal Authentication Service - Device Code Flow.

Based on streamrip's proven implementation.
Opens verification URL in user's browser for login.
"""

import asyncio
import base64
import json
import time
from pathlib import Path
from typing import Optional, Tuple, Dict, Any

import aiohttp


class TidalAuth:
    """
    Tidal authentication via OAuth device code flow.
    
    Uses credentials from tidal_service.py that support device authorization.
    """
    
    # OAuth 2.0 Credentials (Standard HiFi) - from existing tidal_service.py
    CLIENT_ID = "fX2JxdmntZWK0ixT"
    CLIENT_SECRET = "xeuPmY7nbpZ9IIbLAcQ93shka1VNheUAqN6IcszjTG8="
    
    # OAuth endpoints
    AUTH_URL = "https://auth.tidal.com/v1/oauth2"
    API_BASE = "https://api.tidal.com/v1"
    
    def __init__(self, credentials_file: Optional[Path] = None, verbose: bool = False):
        self.credentials_file = credentials_file or Path(__file__).parent.parent / ".gui_credentials_cache.json"
        self.verbose = verbose
        self.session: Optional[aiohttp.ClientSession] = None
    
    def _log(self, message: str):
        if self.verbose:
            print(f"[Tidal Auth] {message}", flush=True)
    
    def get_stored_tokens(self) -> Optional[Dict[str, Any]]:
        """Get stored tokens from credentials cache file."""
        try:
            if self.credentials_file.exists():
                with open(self.credentials_file, 'r') as f:
                    cache = json.load(f)
                    tidal_data = cache.get("tidal", {})
                    if tidal_data.get("access_token"):
                        return tidal_data
        except Exception as e:
            self._log(f"Error reading credentials: {e}")
        return None
    
    def save_tokens(self, tokens: Dict[str, Any]) -> bool:
        """Save tokens to credentials cache file."""
        try:
            cache = {}
            if self.credentials_file.exists():
                with open(self.credentials_file, 'r') as f:
                    cache = json.load(f)
            
            cache["tidal"] = tokens
            
            with open(self.credentials_file, 'w') as f:
                json.dump(cache, f, indent=2)
            
            self._log("Tokens saved successfully")
            return True
        except Exception as e:
            self._log(f"Error saving tokens: {e}")
            return False
    
    def clear_tokens(self) -> bool:
        """Clear stored tokens (logout)."""
        try:
            if self.credentials_file.exists():
                with open(self.credentials_file, 'r') as f:
                    cache = json.load(f)
                
                if "tidal" in cache:
                    del cache["tidal"]
                    
                    with open(self.credentials_file, 'w') as f:
                        json.dump(cache, f, indent=2)
            
            self._log("Tokens cleared")
            return True
        except Exception as e:
            self._log(f"Error clearing tokens: {e}")
            return False
    
    async def refresh_access_token(self) -> Tuple[bool, str]:
        """Refresh the access token using stored refresh_token."""
        tokens = self.get_stored_tokens()
        if not tokens or not tokens.get("refresh_token"):
            return False, "No refresh token available"
        
        self._log("Refreshing access token...")
        
        try:
            async with aiohttp.ClientSession() as session:
                data = {
                    "client_id": self.CLIENT_ID,
                    "refresh_token": tokens["refresh_token"],
                    "grant_type": "refresh_token",
                    "scope": "r_usr+w_usr+w_sub",
                }
                auth = aiohttp.BasicAuth(login=self.CLIENT_ID, password=self.CLIENT_SECRET)
                
                async with session.post(f"{self.AUTH_URL}/token", data=data, auth=auth) as resp:
                    result = await resp.json()
                
                self._log(f"Refresh response: {result}")
                
                if "access_token" in result:
                    # Update stored tokens
                    tokens["access_token"] = result["access_token"]
                    tokens["token_expiry"] = result.get("expires_in", 14400) + time.time()
                    if "refresh_token" in result:
                        tokens["refresh_token"] = result["refresh_token"]
                    
                    self.save_tokens(tokens)
                    return True, "Token refreshed successfully"
                else:
                    return False, result.get("error_description", "Refresh failed")
        except Exception as e:
            self._log(f"Refresh error: {e}")
            return False, str(e)
    
    async def _api_post(self, url: str, data: dict, auth: Optional[aiohttp.BasicAuth] = None) -> dict:
        """Post to the Tidal API."""
        async with self.session.post(url, data=data, auth=auth) as resp:
            return await resp.json()
    
    async def _get_device_code(self) -> Tuple[str, str]:
        """Get the device code and verification URL."""
        data = {
            "client_id": self.CLIENT_ID,
            "scope": "r_usr+w_usr+w_sub",
        }
        resp = await self._api_post(f"{self.AUTH_URL}/device_authorization", data)
        
        if resp.get("status", 200) != 200:
            raise Exception(f"Device authorization failed: {resp}")
        
        verification_url = resp["verificationUriComplete"]
        # Ensure URL has https:// prefix
        if not verification_url.startswith("http"):
            verification_url = f"https://{verification_url}"
        
        return resp["deviceCode"], verification_url
    
    async def _get_auth_status(self, device_code: str) -> Tuple[int, Dict[str, Any]]:
        """
        Check if the user has logged in.
        
        Returns:
            (status_code, auth_info)
            status_code: 0 = success, 1 = error, 2 = pending
        """
        data = {
            "client_id": self.CLIENT_ID,
            "device_code": device_code,
            "grant_type": "urn:ietf:params:oauth:grant-type:device_code",
            "scope": "r_usr+w_usr+w_sub",
        }
        
        auth = aiohttp.BasicAuth(login=self.CLIENT_ID, password=self.CLIENT_SECRET)
        resp = await self._api_post(f"{self.AUTH_URL}/token", data, auth)
        
        self._log(f"Token response: {resp}")
        
        if "status" in resp and resp["status"] != 200:
            if resp["status"] == 400 and resp.get("sub_status") == 1002:
                # Authorization pending
                return 2, {}
            else:
                return 1, {}
        
        # Success!
        ret = {
            "user_id": resp["user"]["userId"],
            "country_code": resp["user"]["countryCode"],
            "access_token": resp["access_token"],
            "refresh_token": resp["refresh_token"],
            "token_expiry": resp["expires_in"] + time.time(),
        }
        return 0, ret
    
    async def login_with_device_code(self, timeout_seconds: int = 300) -> Tuple[bool, str, Dict[str, Any]]:
        """
        Authenticate using OAuth device code flow.
        
        Opens verification URL in user's real browser (no bot detection).
        
        Returns:
            (success, message, device_info)
        """
        import webbrowser
        
        self._log("Starting device code authorization flow...")
        
        try:
            self.session = aiohttp.ClientSession()
            
            # Step 1: Get device code
            device_code, verification_url = await self._get_device_code()
            
            device_info = {
                "verification_url": verification_url,
                "device_code": device_code,
            }
            
            self._log(f"Got device code. URL: {verification_url}")
            
            # Step 2: Open browser IMMEDIATELY so user can log in while we poll
            try:
                self._log("Opening browser for Tidal login...")
                webbrowser.open(verification_url)
            except Exception as e:
                self._log(f"Failed to open browser: {e}")
            
            # Step 3: Poll for authorization
            interval = 2  # seconds
            elapsed = 0
            self._log(f"Polling for authorization (timeout: {timeout_seconds}s)...")
            
            while elapsed < timeout_seconds:
                await asyncio.sleep(interval)
                elapsed += interval
                
                status, auth_info = await self._get_auth_status(device_code)
                
                if status == 0:
                    # Success!
                    self.save_tokens(auth_info)
                    await self.session.close()
                    return True, "Successfully connected to Tidal", device_info
                elif status == 1:
                    # Error
                    await self.session.close()
                    return False, "Authorization failed", device_info
                # status == 2: pending, continue polling
            
            await self.session.close()
            return False, "Authorization timed out", device_info
            
        except Exception as e:
            self._log(f"Device code flow error: {e}")
            if self.session:
                await self.session.close()
            return False, f"Error: {str(e)}", {}
    
    def get_status(self) -> Dict[str, Any]:
        """Get current Tidal connection status."""
        tokens = self.get_stored_tokens()
        
        if not tokens or not tokens.get("access_token"):
            return {
                "status": "success",
                "connected": False,
                "message": "Not connected",
                "user_id": None
            }
        
        # Check if token is expired
        token_expiry = tokens.get("token_expiry", 0)
        if token_expiry and time.time() > token_expiry:
            return {
                "status": "success",
                "connected": False,
                "message": "Token expired",
                "user_id": None
            }
        
        return {
            "status": "success",
            "connected": True,
            "message": "Connected",
            "user_id": tokens.get("user_id")
        }


# Convenience functions for GUI bridge
async def _async_tidal_login() -> dict:
    """Async login helper."""
    auth = TidalAuth(verbose=True)
    success, message, device_info = await auth.login_with_device_code()
    
    # Browser is opened inside login_with_device_code() now
    
    return {
        "status": "success" if success else "error",
        "message": message,
        "device_info": device_info
    }


def tidal_login() -> dict:
    """Start Tidal device code login flow."""
    loop = asyncio.new_event_loop()
    asyncio.set_event_loop(loop)
    try:
        return loop.run_until_complete(_async_tidal_login())
    finally:
        loop.close()


def tidal_status() -> dict:
    """Get Tidal connection status."""
    auth = TidalAuth(verbose=False)
    return auth.get_status()


def tidal_logout() -> dict:
    """Log out from Tidal (clear tokens)."""
    auth = TidalAuth(verbose=True)
    success = auth.clear_tokens()
    return {
        "status": "success" if success else "error",
        "message": "Logged out from Tidal" if success else "Failed to log out"
    }


def tidal_refresh() -> dict:
    """Refresh Tidal access token."""
    loop = asyncio.new_event_loop()
    asyncio.set_event_loop(loop)
    try:
        auth = TidalAuth(verbose=True)
        success, message = loop.run_until_complete(auth.refresh_access_token())
        return {
            "status": "success" if success else "error",
            "message": message
        }
    finally:
        loop.close()

if __name__ == "__main__":
    import sys
    
    if len(sys.argv) > 1 and sys.argv[1] == "login":
        print("Starting Tidal login...")
        result = tidal_login()
        print(f"Result: {result}")
    else:
        print("Checking Tidal status...")
        result = tidal_status()
        print(f"Status: {result}")

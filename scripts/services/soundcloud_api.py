"""
SoundCloud API Client - Based on streamrip approach.

Uses public API with dynamically discovered client_id.
No browser automation needed - only supports public tracks.
"""

import asyncio
import json
import re
from pathlib import Path
from typing import Optional, Dict, Any, List, Tuple

import aiohttp


class SoundCloudClient:
    """
    SoundCloud API client using public endpoints.
    
    Discovers client_id dynamically from SoundCloud's web assets,
    then uses the API to search, resolve URLs, and get track info.
    """
    
    BASE_URL = "https://api-v2.soundcloud.com"
    STOCK_URL = "https://soundcloud.com/"
    
    def __init__(self, credentials_file: Optional[Path] = None, verbose: bool = False):
        self.credentials_file = credentials_file or Path(__file__).parent.parent / ".gui_credentials_cache.json"
        self.verbose = verbose
        self.session: Optional[aiohttp.ClientSession] = None
        self.client_id: Optional[str] = None
        self.app_version: Optional[str] = None
        self._logged_in = False
    
    def _log(self, message: str):
        if self.verbose:
            print(f"[SoundCloud API] {message}", flush=True)
    
    def get_stored_credentials(self) -> Optional[Dict[str, str]]:
        """Get stored client_id and app_version from cache."""
        try:
            if self.credentials_file.exists():
                with open(self.credentials_file, 'r') as f:
                    cache = json.load(f)
                    return cache.get("soundcloud_api")
        except Exception as e:
            self._log(f"Error reading credentials: {e}")
        return None
    
    def save_credentials(self, client_id: str, app_version: str) -> bool:
        """Save client_id and app_version to cache."""
        try:
            cache = {}
            if self.credentials_file.exists():
                with open(self.credentials_file, 'r') as f:
                    cache = json.load(f)
            
            cache["soundcloud_api"] = {
                "client_id": client_id,
                "app_version": app_version
            }
            # Also set the old soundcloud key for status checks
            cache["soundcloud"] = {
                "token": f"api:{client_id}",
                "remember": "true"
            }
            
            with open(self.credentials_file, 'w') as f:
                json.dump(cache, f, indent=2)
            
            self._log("Credentials saved successfully")
            return True
        except Exception as e:
            self._log(f"Error saving credentials: {e}")
            return False
    
    async def login(self) -> Tuple[bool, str]:
        """
        Initialize the client by discovering or loading client_id.
        
        Returns:
            (success, message)
        """
        try:
            # Create session if needed
            if self.session is None or self.session.closed:
                self.session = aiohttp.ClientSession()
            
            # Try to load from cache first
            cached = self.get_stored_credentials()
            if cached:
                self.client_id = cached.get("client_id")
                self.app_version = cached.get("app_version")
                self._log(f"Loaded cached credentials: client_id={self.client_id[:10]}...")
            
            # Validate or refresh
            if self.client_id and self.app_version:
                if await self._validate_credentials():
                    self._logged_in = True
                    return True, "Connected using cached credentials"
            
            # Need to refresh tokens
            self._log("Refreshing tokens from SoundCloud...")
            success, result = await self._refresh_tokens()
            
            if success:
                self._logged_in = True
                self.save_credentials(self.client_id, self.app_version)
                return True, f"Connected with new client_id"
            else:
                return False, result
                
        except Exception as e:
            return False, f"Login error: {str(e)}"
    
    async def _validate_credentials(self) -> bool:
        """Check if current credentials are valid."""
        try:
            url = f"{self.BASE_URL}/announcements"
            params = {
                "client_id": self.client_id,
                "app_version": self.app_version,
                "app_locale": "en",
            }
            async with self.session.get(url, params=params) as resp:
                return resp.status == 200
        except:
            return False
    
    async def _refresh_tokens(self) -> Tuple[bool, str]:
        """Discover client_id and app_version from SoundCloud's web page."""
        try:
            # Fetch the main page
            self._log("Fetching soundcloud.com...")
            async with self.session.get(self.STOCK_URL) as resp:
                if resp.status != 200:
                    return False, f"Failed to fetch SoundCloud: {resp.status}"
                page_text = await resp.text(encoding="utf-8")
            
            # Find app_version
            app_version_match = re.search(
                r'<script>window\.__sc_version="(\d+)"</script>',
                page_text
            )
            if not app_version_match:
                return False, "Could not find app version"
            self.app_version = app_version_match.group(1)
            self._log(f"Found app_version: {self.app_version}")
            
            # Find the JS bundle URL that contains client_id
            script_matches = list(re.finditer(
                r'<script\s+crossorigin\s+src="([^"]+)"',
                page_text
            ))
            
            if not script_matches:
                return False, "Could not find script URLs"
            
            # The client_id is usually in the last script
            client_id_url = script_matches[-1].group(1)
            self._log(f"Fetching JS bundle for client_id...")
            
            async with self.session.get(client_id_url) as resp:
                if resp.status != 200:
                    return False, f"Failed to fetch JS bundle: {resp.status}"
                js_text = await resp.text(encoding="utf-8")
            
            # Find client_id in the JS
            client_id_match = re.search(r'client_id:\s*"(\w+)"', js_text)
            if not client_id_match:
                return False, "Could not find client_id in JS bundle"
            
            self.client_id = client_id_match.group(1)
            self._log(f"Found client_id: {self.client_id[:10]}...")
            
            return True, "Tokens refreshed successfully"
            
        except Exception as e:
            return False, f"Token refresh error: {str(e)}"
    
    async def _api_request(self, path: str, params: dict = None) -> Tuple[dict, int]:
        """Make an API request to SoundCloud."""
        url = f"{self.BASE_URL}/{path}"
        _params = {
            "client_id": self.client_id,
            "app_version": self.app_version,
            "app_locale": "en",
        }
        if params:
            _params.update(params)
        
        async with self.session.get(url, params=_params) as resp:
            if resp.status == 200:
                return await resp.json(), resp.status
            else:
                return {}, resp.status
    
    async def resolve_url(self, url: str) -> Optional[dict]:
        """Resolve a SoundCloud URL to track/playlist metadata."""
        resp, status = await self._api_request("resolve", params={"url": url})
        if status == 200:
            return resp
        return None
    
    async def get_track(self, track_id: str) -> Optional[dict]:
        """Get track metadata by ID."""
        resp, status = await self._api_request(f"tracks/{track_id}")
        if status == 200:
            return resp
        return None
    
    async def search_tracks(self, query: str, limit: int = 50) -> List[dict]:
        """Search for tracks."""
        params = {
            "q": query,
            "facet": "genre",
            "limit": limit,
            "offset": 0,
            "linked_partitioning": "1",
        }
        resp, status = await self._api_request("search/tracks", params=params)
        if status == 200:
            return resp.get("collection", [])
        return []
    
    async def get_stream_url(self, track: dict) -> Optional[str]:
        """Get the stream URL for a track."""
        if not track.get("streamable") or track.get("policy") == "BLOCK":
            return None
        
        # Check for original download
        if track.get("downloadable") and track.get("has_downloads_left"):
            resp, status = await self._api_request(f"tracks/{track['id']}/download")
            if status == 200:
                return resp.get("redirectUri")
        
        # Get from transcodings
        media = track.get("media", {})
        transcodings = media.get("transcodings", [])
        
        for tc in transcodings:
            fmt = tc.get("format", {})
            if fmt.get("protocol") == "progressive":
                # Get the actual URL
                url = tc.get("url")
                if url:
                    resp, status = await self._api_request(url.replace(self.BASE_URL + "/", ""))
                    if status == 200:
                        return resp.get("url")
        
        # Try HLS as fallback
        for tc in transcodings:
            fmt = tc.get("format", {})
            if fmt.get("protocol") == "hls" and fmt.get("mime_type") == "audio/mpeg":
                url = tc.get("url")
                if url:
                    resp, status = await self._api_request(url.replace(self.BASE_URL + "/", ""))
                    if status == 200:
                        return resp.get("url")
        
        return None
    
    def get_status(self) -> Dict[str, Any]:
        """Get current connection status."""
        if self._logged_in and self.client_id:
            return {
                "status": "success",
                "connected": True,
                "message": "Connected (API)",
                "client_id": self.client_id[:10] + "..."
            }
        
        # Check for cached credentials
        cached = self.get_stored_credentials()
        if cached and cached.get("client_id"):
            return {
                "status": "success",
                "connected": True,
                "message": "Connected (cached)",
            }
        
        return {
            "status": "success",
            "connected": False,
            "message": "Not connected"
        }
    
    async def close(self):
        """Close the session."""
        if self.session and not self.session.closed:
            await self.session.close()


# Convenience function for GUI
async def soundcloud_api_login(verbose: bool = True) -> Tuple[bool, str]:
    """Connect to SoundCloud using the public API."""
    client = SoundCloudClient(verbose=verbose)
    try:
        success, message = await client.login()
        return success, message
    finally:
        await client.close()


if __name__ == "__main__":
    import sys
    
    async def main():
        client = SoundCloudClient(verbose=True)
        success, message = await client.login()
        print(f"Login: {success}, {message}")
        
        if success:
            # Test search
            tracks = await client.search_tracks("electronic music", limit=5)
            print(f"\nFound {len(tracks)} tracks:")
            for t in tracks[:5]:
                print(f"  - {t.get('title')} by {t.get('user', {}).get('username')}")
        
        await client.close()
    
    asyncio.run(main())

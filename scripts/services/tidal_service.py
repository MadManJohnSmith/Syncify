"""
Tidal music streaming service integration.

This module provides access to Tidal's high-fidelity music catalog via OAuth 2.0 and PKCE authentication.
Supports up to 24-bit/192kHz FLAC downloads with HiFi+ subscription.
"""

import asyncio
import base64
import hashlib
import logging
import os
import random
import time
from typing import List, Optional, Callable
from urllib.parse import parse_qs, urlsplit, urlencode

import aiohttp

from services.service_base import (
    MusicService,
    ServiceCredentials,
    ServiceType,
    SearchResult,
    TrackMetadata,
    AlbumMetadata,
    PlaylistMetadata,
    DownloadResult,
    DownloadQuality
)


class TidalService(MusicService):
    """
    Tidal music streaming service implementation.
    
    Supports OAuth 2.0 and PKCE authentication methods:
    - OAuth: Standard HiFi (FLAC CD quality, 16-bit/44.1kHz)
    - PKCE: HiFi+ (Hi-Res FLAC, up to 24-bit/192kHz)
    
    Features:
    - Device authorization flow for easy login
    - Token refresh for persistent sessions
    - ISRC-based duplicate detection
    - Progress callbacks for downloads
    - SHA256 file hashing
    """
    
    # OAuth 2.0 Credentials (Standard HiFi)
    CLIENT_ID = "fX2JxdmntZWK0ixT"
    CLIENT_SECRET = "xeuPmY7nbpZ9IIbLAcQ93shka1VNheUAqN6IcszjTG8="
    
    # PKCE Credentials (Hi-Res / HiFi+)
    CLIENT_ID_PKCE = "6BDSRdpK9hqEBTgU"
    CLIENT_SECRET_PKCE = "xeuPmY7nbpZ9IIbLAcQ93shka1VNheUAqN6IcszjTG8="
    
    # API Endpoints
    API_BASE = "https://api.tidal.com/v1"
    AUTH_BASE = "https://auth.tidal.com/v1/oauth2"
    PKCE_AUTH_BASE = "https://login.tidal.com/authorize"
    
    # Quality mapping
    QUALITY_MAP = {
        DownloadQuality.LOSSY_LOW: "LOW",          # AAC 96kbps
        DownloadQuality.LOSSY_STANDARD: "HIGH",     # AAC 320kbps
        DownloadQuality.LOSSLESS_CD: "LOSSLESS",    # FLAC 16/44.1
        DownloadQuality.LOSSLESS_HIRES: "HI_RES"    # FLAC 24-bit MQA/Hi-Res
    }
    
    def __init__(self, credentials: ServiceCredentials, use_pkce: bool = True, verbose: bool = False):
        """
        Initialize Tidal service.
        
        Args:
            credentials: Service credentials (username/password not used for Tidal, 
                        but kept for interface compatibility)
            use_pkce: Use PKCE authentication (True) or standard OAuth (False).
                     PKCE is recommended for Hi-Res quality and simpler downloads.
            verbose: Enable verbose logging
        """
        super().__init__(credentials, verbose)
        self.session: Optional[aiohttp.ClientSession] = None
        self.use_pkce = use_pkce
        
        # Authentication state
        self.access_token: Optional[str] = None
        self.refresh_token: Optional[str] = None
        self.token_type: str = "Bearer"
        self.session_id: Optional[str] = None
        self.user_id: Optional[int] = None
        self.country_code: Optional[str] = None
        
        # PKCE state
        self.client_unique_key: Optional[str] = None
        self.code_verifier: Optional[str] = None
        self.code_challenge: Optional[str] = None
        
        if self.verbose:
            logging.basicConfig(level=logging.DEBUG)
        self.logger = logging.getLogger(__name__)
    
    # ==========================================
    # ABSTRACT PROPERTY IMPLEMENTATIONS
    # ==========================================
    
    @property
    def service_name(self) -> str:
        """Human-readable service name."""
        return "Tidal"
    
    @property
    def service_type(self) -> ServiceType:
        """Service type enum."""
        return ServiceType.TIDAL
    
    @property
    def supports_lossless(self) -> bool:
        """Whether service supports lossless audio."""
        return True
    
    def _init_pkce_params(self):
        """Initialize PKCE parameters for authorization."""
        # Generate random client unique key (64-bit hex)
        self.client_unique_key = format(random.getrandbits(64), "02x")
        
        # Generate code_verifier (random 32 bytes, base64url-encoded without padding)
        verifier_bytes = os.urandom(32)
        self.code_verifier = base64.urlsafe_b64encode(verifier_bytes).decode('utf-8').rstrip('=')
        
        # Generate code_challenge = SHA256(code_verifier), base64url-encoded without padding
        challenge_bytes = hashlib.sha256(self.code_verifier.encode('utf-8')).digest()
        self.code_challenge = base64.urlsafe_b64encode(challenge_bytes).decode('utf-8').rstrip('=')
    
    async def authenticate(self) -> bool:
        """
        Authenticate with Tidal.
        
        Priority:
        1. Use token passed via credentials (from Syncify database/UI login)
        2. Load cached tokens from browser-based login (.gui_credentials_cache.json)
        3. Fall back to PKCE or OAuth interactive flows
        
        Returns:
            True if authentication successful, False otherwise
        """
        try:
            if self.session is None:
                self.session = aiohttp.ClientSession()
            
            # First, try using token passed via credentials (from Syncify database)
            if self.credentials.token:
                self.access_token = self.credentials.token
                self.refresh_token = self.credentials.refresh_token
                self.token_type = "Bearer"
                
                # Validate tokens by getting session info
                if await self._validate_session():
                    self.logger.info("✓ Authenticated using credentials from database")
                    return True
                else:
                    self.logger.info("Credentials from database invalid, trying other methods")
                    self.access_token = None
                    self.refresh_token = None
            
            # Try loading cached tokens from browser-based login
            cached_tokens = self._load_cached_tokens()
            if cached_tokens:
                self.access_token = cached_tokens.get('access_token')
                self.refresh_token = cached_tokens.get('refresh_token')
                self.token_type = cached_tokens.get('token_type', 'Bearer')
                
                # Validate tokens by getting session info
                if await self._validate_session():
                    self.logger.info("✓ Authenticated using cached tokens")
                    return True
                else:
                    self.logger.info("Cached tokens invalid, will need re-authentication")
                    self.access_token = None
                    self.refresh_token = None
            
            # Fall back to interactive authentication
            if self.use_pkce:
                return await self._authenticate_pkce()
            else:
                return await self._authenticate_oauth()
        
        except Exception as e:
            self.logger.error(f"Authentication failed: {e}")
            return False
    
    async def _authenticate_oauth(self) -> bool:
        """
        Authenticate using OAuth 2.0 device authorization flow.
        
        This method:
        1. Requests device authorization code
        2. Displays verification URL to user
        3. Polls for token until user completes authorization
        4. Stores access_token and refresh_token
        
        Returns:
            True if successful, False otherwise
        """
        self.logger.info("Starting OAuth device authorization flow...")
        
        # Step 1: Request device authorization
        device_auth_url = f"{self.AUTH_BASE}/device_authorization"
        params = {
            "client_id": self.CLIENT_ID,
            "scope": "r_usr w_usr w_sub"
        }
        
        async with self.session.post(device_auth_url, params=params) as response:
            if not response.ok:
                error_text = await response.text()
                self.logger.error(f"Device authorization failed: {error_text}")
                return False
            
            data = await response.json()
        
        # Extract authorization details
        device_code = data["deviceCode"]
        user_code = data["userCode"]
        verification_uri = data["verificationUriComplete"]
        expires_in = int(data["expiresIn"])
        interval = float(data["interval"])
        
        # Step 2: Display verification URL to user
        print("\n" + "="*70)
        print("TIDAL AUTHENTICATION")
        print("="*70)
        print(f"\nPlease visit this URL to authorize the application:")
        print(f"\n    {verification_uri}\n")
        print(f"The code will expire in {expires_in} seconds.")
        print(f"Waiting for authorization...\n")
        
        # Step 3: Poll for token
        token_url = f"{self.AUTH_BASE}/token"
        token_params = {
            "client_id": self.CLIENT_ID,
            "client_secret": self.CLIENT_SECRET,
            "device_code": device_code,
            "grant_type": "urn:ietf:params:oauth:grant-type:device_code",
            "scope": "r_usr w_usr w_sub"
        }
        
        expiry = expires_in
        while expiry > 0:
            await asyncio.sleep(interval)
            expiry -= interval
            
            async with self.session.post(token_url, params=token_params) as response:
                result = await response.json()
                
                if response.ok:
                    # Success! Process token
                    return await self._process_auth_token(result)
                
                # Check error codes
                error = result.get("error", "")
                if error == "authorization_pending":
                    # Still waiting for user
                    continue
                elif error == "expired_token":
                    self.logger.error("Authorization code expired")
                    return False
                else:
                    self.logger.error(f"Token request failed: {result}")
                    return False
        
        self.logger.error("Authorization timed out")
        return False
    
    async def _authenticate_pkce(self) -> bool:
        """
        Authenticate using PKCE (Proof Key for Code Exchange).
        
        This method:
        1. Generates PKCE parameters (code_verifier, code_challenge)
        2. Displays authorization URL with PKCE challenge
        3. Prompts user to paste redirect URL from "Oops" page
        4. Exchanges authorization code for tokens
        
        Returns:
            True if successful, False otherwise
        """
        self.logger.info("Starting PKCE authorization flow...")
        
        # Step 1: Initialize PKCE parameters
        self._init_pkce_params()
        
        # Step 2: Build authorization URL
        auth_params = {
            "response_type": "code",
            "redirect_uri": "https://tidal.com/android/login/auth",
            "client_id": self.CLIENT_ID_PKCE,
            "lang": "EN",
            "appMode": "android",
            "client_unique_key": self.client_unique_key,
            "code_challenge": self.code_challenge,
            "code_challenge_method": "S256",
            "restrict_signup": "true"
        }
        auth_url = f"{self.PKCE_AUTH_BASE}?{urlencode(auth_params)}"
        
        # Step 3: Display instructions to user
        print("\n" + "="*70)
        print("TIDAL PKCE AUTHENTICATION (Hi-Res / HiFi+)")
        print("="*70)
        print("\nREAD CAREFULLY!")
        print("---------------")
        print("1. Open this URL in your browser:")
        print(f"\n   {auth_url}\n")
        print("2. Log in with your Tidal username and password")
        print("3. You will be redirected to an 'Oops' error page - THIS IS EXPECTED")
        print("4. Copy the FULL URL from the 'Oops' page")
        print("5. Paste it below\n")
        
        # Step 4: Get redirect URL from user
        redirect_url = input("Paste the 'Oops' page URL here and press ENTER: ").strip()
        
        if not redirect_url or "https://" not in redirect_url:
            self.logger.error("Invalid redirect URL provided")
            return False
        
        # Step 5: Extract authorization code
        try:
            parsed_qs = parse_qs(urlsplit(redirect_url).query)
            auth_code = parsed_qs["code"][0]
        except (KeyError, IndexError) as e:
            self.logger.error(f"Failed to extract authorization code from URL: {e}")
            return False
        
        # Step 6: Exchange code for tokens
        token_url = f"{self.AUTH_BASE}/token"
        token_data = {
            "code": auth_code,
            "client_id": self.CLIENT_ID_PKCE,
            "grant_type": "authorization_code",
            "redirect_uri": "https://tidal.com/android/login/auth",
            "scope": "r_usr+w_usr+w_sub",
            "code_verifier": self.code_verifier,
            "client_unique_key": self.client_unique_key
        }
        
        async with self.session.post(token_url, data=token_data) as response:
            if not response.ok:
                error_text = await response.text()
                self.logger.error(f"Token exchange failed: {error_text}")
                return False
            
            token_result = await response.json()
        
        # Step 7: Process tokens
        return await self._process_auth_token(token_result, is_pkce=True)
    
    async def _process_auth_token(self, token_data: dict, is_pkce: bool = False) -> bool:
        """
        Process authentication token response and retrieve session info.
        
        Args:
            token_data: Token response from Tidal API
            is_pkce: Whether this is a PKCE token
        
        Returns:
            True if session established successfully
        """
        # Extract tokens
        self.access_token = token_data["access_token"]
        self.refresh_token = token_data.get("refresh_token")
        self.token_type = token_data.get("token_type", "Bearer")
        
        # Get session info
        session_url = f"{self.API_BASE}/sessions"
        headers = {"Authorization": f"{self.token_type} {self.access_token}"}
        
        async with self.session.get(session_url, headers=headers) as response:
            if not response.ok:
                self.logger.error("Failed to retrieve session info")
                return False
            
            session_data = await response.json()
        
        # Store session details
        self.session_id = session_data["sessionId"]
        self.user_id = session_data["userId"]
        self.country_code = session_data["countryCode"]
        
        self.logger.info(f"✓ Authenticated as user {self.user_id} ({self.country_code})")
        self.logger.info(f"  Session ID: {self.session_id}")
        self.logger.info(f"  Auth method: {'PKCE (Hi-Res)' if is_pkce else 'OAuth (Standard)'}")
        
        return True
    
    def is_authenticated(self) -> bool:
        """Check if service is currently authenticated."""
        return self.access_token is not None and self.session_id is not None
    
    def _load_cached_tokens(self) -> Optional[dict]:
        """Load cached tokens from browser-based login."""
        import json
        from pathlib import Path
        
        cache_file = Path(__file__).parent.parent / ".gui_credentials_cache.json"
        try:
            if cache_file.exists():
                with open(cache_file, 'r') as f:
                    cache = json.load(f)
                    tidal_data = cache.get("tidal", {})
                    if tidal_data.get("access_token"):
                        self.logger.debug("Found cached Tidal tokens")
                        return tidal_data
        except Exception as e:
            self.logger.debug(f"Could not load cached tokens: {e}")
        return None
    
    async def _validate_session(self) -> bool:
        """Validate current tokens by getting session info."""
        if not self.access_token:
            return False
        
        try:
            session_url = f"{self.API_BASE}/sessions"
            headers = {"Authorization": f"{self.token_type} {self.access_token}"}
            
            async with self.session.get(session_url, headers=headers) as response:
                if response.ok:
                    session_data = await response.json()
                    self.session_id = session_data["sessionId"]
                    self.user_id = session_data["userId"]
                    self.country_code = session_data["countryCode"]
                    return True
                else:
                    return False
        except Exception as e:
            self.logger.debug(f"Session validation failed: {e}")
            return False
    
    async def search(self, query: str, result_type: str = "track", limit: int = 50) -> List[SearchResult]:
        """
        Search Tidal catalog.
        
        Args:
            query: Search term
            result_type: "track", "album", or "all"
            limit: Maximum results to return (default 50, max 300)
        
        Returns:
            List of SearchResult objects
        """
        if not self.is_authenticated():
            self.logger.error("Not authenticated. Call authenticate() first.")
            return []
        
        # Map result_type to Tidal API types
        type_map = {
            "track": "TRACKS",
            "album": "ALBUMS",
            "all": ""
        }
        tidal_type = type_map.get(result_type.lower(), "TRACKS")
        
        search_url = f"{self.API_BASE}/search"
        params = {
            "query": query,
            "type": tidal_type,
            "limit": min(limit, 300),
            "offset": 0,
            "countryCode": self.country_code
        }
        headers = {"Authorization": f"{self.token_type} {self.access_token}"}
        
        try:
            async with self.session.get(search_url, params=params, headers=headers) as response:
                if not response.ok:
                    error_text = await response.text()
                    self.logger.error(f"Search failed: {error_text}")
                    return []
                
                data = await response.json()
            
            results = []
            
            # Process track results
            if "tracks" in data and data["tracks"].get("items"):
                for track in data["tracks"]["items"]:
                    results.append(SearchResult(
                        service="tidal",
                        id=str(track["id"]),
                        title=track["title"],
                        artist=track["artist"]["name"],
                        album=track["album"]["title"],
                        duration_seconds=track["duration"],
                        quality=track.get("audioQuality", "UNKNOWN"),
                        result_type="track"
                    ))
            
            # Process album results
            if "albums" in data and data["albums"].get("items"):
                for album in data["albums"]["items"]:
                    # Get primary artist name
                    artist_name = album["artist"]["name"] if "artist" in album else "Unknown Artist"
                    
                    results.append(SearchResult(
                        service="tidal",
                        id=str(album["id"]),
                        title=album["title"],
                        artist=artist_name,
                        album=album["title"],
                        duration_seconds=album.get("duration", 0),
                        quality=album.get("audioQuality", "UNKNOWN"),
                        result_type="album"
                    ))
            
            self.logger.info(f"Found {len(results)} results for '{query}'")
            return results
        
        except Exception as e:
            self.logger.error(f"Search error: {e}")
            return []
    
    async def get_track_metadata(self, track_id: str) -> Optional[TrackMetadata]:
        """
        Retrieve detailed metadata for a track.
        
        Args:
            track_id: Tidal track ID
        
        Returns:
            TrackMetadata object or None if not found
        """
        if not self.is_authenticated():
            self.logger.error("Not authenticated. Call authenticate() first.")
            return None
        
        metadata_url = f"{self.API_BASE}/tracks/{track_id}"
        params = {"countryCode": self.country_code}
        headers = {"Authorization": f"{self.token_type} {self.access_token}"}
        
        try:
            async with self.session.get(metadata_url, params=params, headers=headers) as response:
                if not response.ok:
                    error_text = await response.text()
                    self.logger.error(f"Failed to get metadata: {error_text}")
                    return None
                
                track = await response.json()
            
            # Extract artist names
            artist_names = [artist["name"] for artist in track.get("artists", [])]
            if not artist_names and "artist" in track:
                artist_names = [track["artist"]["name"]]
            
            # Build metadata object
            metadata = TrackMetadata(
                service_id=str(track["id"]),
                service_type=ServiceType.TIDAL,
                title=track["title"],
                artists=artist_names,
                album=track["album"]["title"],
                album_artist=track["artist"]["name"] if "artist" in track else artist_names[0] if artist_names else "Unknown",
                track_number=track.get("trackNumber", 0),
                disc_number=track.get("volumeNumber", 1),
                duration_ms=track["duration"] * 1000,  # Convert seconds to milliseconds
                isrc=track.get("isrc"),
                upc=track["album"].get("upc"),
                release_date=track["album"].get("releaseDate"),
                genres=[],  # Tidal doesn't provide genres in track metadata
                sample_rate=None,  # Requires stream URL to determine
                bit_depth=None,
                quality=None,  # Quality enum, not string
                custom_tags={
                    "tidal_id": str(track["id"]),
                    "audio_quality": track.get("audioQuality", ""),
                    "audio_modes": ",".join(track.get("audioModes", [])),
                    "explicit": str(track.get("explicit", False)),
                    "copyright": track.get("copyright", ""),
                }
            )
            
            self.logger.info(f"✓ Retrieved metadata for '{metadata.title}' by {metadata.artists[0]}")
            if metadata.isrc:
                self.logger.info(f"  ISRC: {metadata.isrc}")
            
            return metadata
        
        except Exception as e:
            self.logger.error(f"Metadata retrieval error: {e}")
            return None
    
    async def download_track(
        self,
        track_id: str,
        output_path: str,
        quality: DownloadQuality = DownloadQuality.LOSSLESS_CD,
        progress_callback: Optional[Callable[[int, int], None]] = None
    ) -> DownloadResult:
        """
        Download a track from Tidal.
        
        Args:
            track_id: Tidal track ID
            output_path: Full path where file should be saved
            quality: Desired download quality
            progress_callback: Optional callback(downloaded_bytes, total_bytes)
        
        Returns:
            DownloadResult with success status and metadata
        """
        start_time = time.time()
        
        # Step 1: Get track metadata
        metadata = await self.get_track_metadata(track_id)
        if not metadata:
            return DownloadResult(
                success=False,
                error_message="Failed to retrieve track metadata",
                filepath=None,
                file_size_bytes=0,
                download_duration_seconds=0.0,
                track_metadata=None
            )
        
        # Step 2: Get stream URL
        stream_url = await self._get_stream_url(track_id, quality)
        if not stream_url:
            return DownloadResult(
                success=False,
                error_message=f"Failed to get stream URL (quality: {quality.value})",
                filepath=None,
                file_size_bytes=0,
                download_duration_seconds=0.0,
                track_metadata=metadata
            )
        
        # Step 3: Create output directory
        os.makedirs(os.path.dirname(output_path), exist_ok=True)
        
        # Step 4: Download file
        try:
            headers = {"Authorization": f"{self.token_type} {self.access_token}"}
            
            async with self.session.get(stream_url, headers=headers) as response:
                if not response.ok:
                    error_text = await response.text()
                    return DownloadResult(
                        success=False,
                        error_message=f"Download failed: {error_text}",
                        filepath=None,
                        file_size_bytes=0,
                        download_duration_seconds=time.time() - start_time,
                        track_metadata=metadata
                    )
                
                total_size = int(response.headers.get('content-length', 0))
                downloaded = 0
                hasher = hashlib.sha256()
                
                with open(output_path, 'wb') as f:
                    async for chunk in response.content.iter_chunked(8192):
                        f.write(chunk)
                        hasher.update(chunk)
                        downloaded += len(chunk)
                        
                        if progress_callback:
                            progress_callback(downloaded, total_size)
            
            file_hash = hasher.hexdigest()
            download_time = time.time() - start_time
            
            self.logger.info(f"✓ Downloaded '{metadata.title}' ({downloaded} bytes in {download_time:.2f}s)")
            self.logger.info(f"  SHA256: {file_hash}")
            
            return DownloadResult(
                success=True,
                track_metadata=metadata,
                filepath=output_path,
                file_size_bytes=downloaded,
                download_duration_seconds=download_time
            )
        
        except Exception as e:
            self.logger.error(f"Download error: {e}")
            return DownloadResult(
                success=False,
                error_message=f"Download error: {str(e)}",
                filepath=None,
                file_size_bytes=0,
                download_duration_seconds=time.time() - start_time,
                track_metadata=metadata
            )
    
    async def _get_stream_url(self, track_id: str, quality: DownloadQuality) -> Optional[str]:
        """
        Get streaming URL for a track.
        
        Args:
            track_id: Tidal track ID
            quality: Desired quality level
        
        Returns:
            Direct download URL or None if unavailable
        """
        if not self.is_authenticated():
            self.logger.error("Not authenticated")
            return None
        
        # Map quality to Tidal format
        tidal_quality = self.QUALITY_MAP.get(quality, "LOSSLESS")
        
        stream_url = f"{self.API_BASE}/tracks/{track_id}/streamUrl"
        params = {
            "soundQuality": tidal_quality,
            "playbackMode": "STREAM",
            "assetPresentation": "FULL",
            "countryCode": self.country_code
        }
        headers = {"Authorization": f"{self.token_type} {self.access_token}"}
        
        try:
            async with self.session.get(stream_url, params=params, headers=headers) as response:
                if not response.ok:
                    error_text = await response.text()
                    self.logger.error(f"Stream URL request failed: {error_text}")
                    return None
                
                data = await response.json()
            
            # PKCE returns direct URLs, OAuth returns BTS manifests
            if "url" in data:
                # Direct URL (PKCE / Hi-Res)
                url = data["url"]
                self.logger.info(f"✓ Got stream URL (quality: {data.get('audioQuality', tidal_quality)})")
                return url
            elif "manifest" in data:
                # BTS manifest (OAuth) - not yet implemented
                self.logger.warning("BTS manifest received - OAuth downloads not yet supported")
                self.logger.warning("Please use PKCE authentication (use_pkce=True) for downloads")
                return None
            else:
                self.logger.error("No URL or manifest in stream response")
                return None
        
        except Exception as e:
            self.logger.error(f"Error getting stream URL: {e}")
            return None
    
    # ==========================================
    # STUB METHODS (required by abstract base)
    # ==========================================
    
    async def get_album_metadata(self, album_id: str) -> Optional[AlbumMetadata]:
        """Retrieve album metadata - stub implementation."""
        self.logger.warning("get_album_metadata not yet implemented")
        return None
    
    async def get_album_tracks(self, album_id: str) -> List[TrackMetadata]:
        """Get all tracks in an album - stub implementation."""
        self.logger.warning("get_album_tracks not yet implemented")
        return []
    
    async def get_user_playlists(self) -> List[dict]:
        """
        Get all playlists for the authenticated user.
        
        Returns:
            List of playlist dicts with uuid, title, numberOfTracks, etc.
        """
        if not self.is_authenticated():
            self.logger.error("Not authenticated")
            return []
        
        try:
            url = f"{self.API_BASE}/users/{self.user_id}/playlists"
            params = {
                "countryCode": self.country_code,
                "limit": 50,
                "offset": 0
            }
            headers = {"Authorization": f"{self.token_type} {self.access_token}"}
            
            playlists = []
            
            async with self.session.get(url, params=params, headers=headers) as response:
                if not response.ok:
                    error_text = await response.text()
                    self.logger.error(f"Failed to get playlists: {error_text}")
                    return []
                
                data = await response.json()
            
            for item in data.get("items", []):
                playlists.append({
                    "id": item.get("uuid"),
                    "name": item.get("title", "Unknown"),
                    "description": item.get("description", ""),
                    "track_count": item.get("numberOfTracks", 0),
                    "owner": item.get("creator", {}).get("name", "Unknown"),
                    "public": item.get("publicPlaylist", False),
                    "duration": item.get("duration", 0),
                })
            
            self.logger.info(f"Found {len(playlists)} Tidal playlists")
            return playlists
            
        except Exception as e:
            self.logger.error(f"Failed to get playlists: {e}")
            return []
    
    async def get_playlist_metadata(self, playlist_id: str) -> Optional[PlaylistMetadata]:
        """
        Retrieve playlist metadata.
        
        Args:
            playlist_id: Tidal playlist UUID
        
        Returns:
            PlaylistMetadata object or None if not found
        """
        if not self.is_authenticated():
            self.logger.error("Not authenticated")
            return None
        
        try:
            url = f"{self.API_BASE}/playlists/{playlist_id}"
            params = {"countryCode": self.country_code}
            headers = {"Authorization": f"{self.token_type} {self.access_token}"}
            
            async with self.session.get(url, params=params, headers=headers) as response:
                if not response.ok:
                    error_text = await response.text()
                    self.logger.error(f"Failed to get playlist: {error_text}")
                    return None
                
                data = await response.json()
            
            return PlaylistMetadata(
                service_id=data.get("uuid", playlist_id),
                service_type=ServiceType.TIDAL,
                name=data.get("title", "Unknown"),
                description=data.get("description", ""),
                owner=data.get("creator", {}).get("name", ""),
                track_count=data.get("numberOfTracks", 0),
                duration_ms=data.get("duration", 0) * 1000,
                is_public=data.get("publicPlaylist", False),
            )
            
        except Exception as e:
            self.logger.error(f"Failed to get playlist metadata: {e}")
            return None
    
    async def get_playlist_tracks(self, playlist_id: str) -> List[TrackMetadata]:
        """
        Get all tracks in a playlist.
        
        Args:
            playlist_id: Tidal playlist UUID
        
        Returns:
            List of TrackMetadata objects
        """
        if not self.is_authenticated():
            self.logger.error("Not authenticated")
            return []
        
        try:
            url = f"{self.API_BASE}/playlists/{playlist_id}/items"
            params = {
                "countryCode": self.country_code,
                "limit": 100,
                "offset": 0
            }
            headers = {"Authorization": f"{self.token_type} {self.access_token}"}
            
            all_tracks = []
            
            while True:
                async with self.session.get(url, params=params, headers=headers) as response:
                    if not response.ok:
                        error_text = await response.text()
                        self.logger.error(f"Failed to get playlist tracks: {error_text}")
                        break
                    
                    data = await response.json()
                
                for item in data.get("items", []):
                    track = item.get("item", {})
                    if not track or item.get("type") != "track":
                        continue
                    
                    # Extract artist names
                    artist_names = [a.get("name", "Unknown") for a in track.get("artists", [])]
                    if not artist_names and track.get("artist"):
                        artist_names = [track["artist"].get("name", "Unknown")]
                    
                    all_tracks.append(TrackMetadata(
                        service_id=str(track.get("id")),
                        service_type=ServiceType.TIDAL,
                        title=track.get("title", "Unknown"),
                        artists=artist_names,
                        album=track.get("album", {}).get("title", ""),
                        album_artist=track.get("artist", {}).get("name") if track.get("artist") else (artist_names[0] if artist_names else "Unknown"),
                        track_number=track.get("trackNumber"),
                        disc_number=track.get("volumeNumber", 1),
                        duration_ms=track.get("duration", 0) * 1000,
                        isrc=track.get("isrc"),
                        release_date=track.get("album", {}).get("releaseDate"),
                    ))
                
                # Check for more pages
                total_items = data.get("totalNumberOfItems", 0)
                if params["offset"] + len(data.get("items", [])) >= total_items:
                    break
                params["offset"] += 100
            
            self.logger.info(f"Found {len(all_tracks)} tracks in Tidal playlist {playlist_id}")
            return all_tracks
            
        except Exception as e:
            self.logger.error(f"Failed to get playlist tracks: {e}")
            return []
    
    async def get_available_qualities(self, track_id: str) -> List[DownloadQuality]:
        """Get available quality options for a track."""
        # Tidal supports these quality levels depending on subscription
        return [
            DownloadQuality.LOSSY_LOW,
            DownloadQuality.LOSSY_STANDARD,
            DownloadQuality.LOSSLESS_CD,
            DownloadQuality.LOSSLESS_HIRES
        ]
    
    async def close(self):
        """Close the aiohttp session."""
        if self.session:
            await self.session.close()
            self.session = None

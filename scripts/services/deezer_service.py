"""
Deezer service integration - Phase 1 implementation.
Based on streamrip and orpheusdl-deezer architecture.

Deezer uses ARL cookie authentication and requires Blowfish decryption
of downloaded tracks (every 3rd 2048-byte chunk).
"""

import asyncio
import aiohttp
import hashlib
import json
import os
from typing import List, Optional, Callable, Dict, Any
from pathlib import Path
import logging
from random import randint
import binascii

try:
    from Cryptodome.Cipher import Blowfish, AES
    CRYPTODOME_AVAILABLE = True
except ImportError:
    try:
        from Crypto.Cipher import Blowfish, AES
        CRYPTODOME_AVAILABLE = True
    except ImportError:
        Blowfish = None  # type: ignore
        AES = None  # type: ignore
        CRYPTODOME_AVAILABLE = False

try:
    from services.service_base import (
        MusicService, ServiceType, ServiceCredentials, TrackMetadata,
        AlbumMetadata, PlaylistMetadata, SearchResult, DownloadResult,
        DownloadQuality, DownloadStatus
    )
except ImportError:
    from .service_base import (
        MusicService, ServiceType, ServiceCredentials, TrackMetadata,
        AlbumMetadata, PlaylistMetadata, SearchResult, DownloadResult,
        DownloadQuality, DownloadStatus
    )


class DeezerService(MusicService):
    """
    Deezer music service implementation.
    
    Authentication: ARL cookie (from browser session)
    API Base: https://www.deezer.com/ajax/gw-light.php
    
    Quality Levels:
    - MP3_128: 128kbps MP3 (free tier)
    - MP3_320: 320kbps MP3 (premium)
    - FLAC: 16bit/44.1kHz FLAC (HiFi subscription)
    
    Key Features:
    - ARL cookie authentication (long-lived session token)
    - Blowfish decryption of downloaded tracks
    - ISRC support for cross-service matching
    - Gateway API (gw-light.php) for all operations
    """
    
    # API Configuration
    GW_LIGHT_URL = "https://www.deezer.com/ajax/gw-light.php"
    MEDIA_URL = "https://media.deezer.com/v1/get_url"
    
    # API Credentials and Decryption Key Configuration
    CLIENT_ID = "447462"
    CLIENT_SECRET = ""

    # Blowfish decryption key fallback for development/testing when not configured.
    # Production environments must provide the key via `DEEZER_BLOWFISH_KEY` environment
    # variable or via injected `ServiceCredentials.extra["blowfish_key"]`.
    DEFAULT_BLOWFISH_KEY_FALLBACK = b"dev_placeholder_blowfish_key_16b"

    @classmethod
    def resolve_blowfish_key(cls, credentials: Optional[ServiceCredentials] = None) -> bytes:
        """
        Resolve Deezer Blowfish decryption key dynamically.
        Priority:
        1. DEEZER_BLOWFISH_KEY environment variable.
        2. Injected credentials extra['blowfish_key'].
        3. Default development fallback placeholder.
        """
        env_key = os.environ.get("DEEZER_BLOWFISH_KEY")
        if env_key:
            return env_key.encode("utf-8")
        if credentials and credentials.extra:
            extra_key = credentials.extra.get("blowfish_key")
            if extra_key:
                if isinstance(extra_key, bytes):
                    return extra_key
                return str(extra_key).encode("utf-8")
        return cls.DEFAULT_BLOWFISH_KEY_FALLBACK

    @property
    def BLOWFISH_SECRET(self) -> bytes:
        """Dynamic Blowfish secret resolved from environment or credentials."""
        return self.resolve_blowfish_key(self.credentials)
    
    # Quality mapping: DownloadQuality → Deezer format
    QUALITY_MAP = {
        DownloadQuality.LOSSY_LOW: 'MP3_128',
        DownloadQuality.LOSSY_STANDARD: 'MP3_320',
        DownloadQuality.LOSSLESS_CD: 'FLAC',
        # Deezer doesn't support Hi-Res beyond 16/44.1
        DownloadQuality.LOSSLESS_HIRES: 'FLAC',
    }
    
    # Format numbers for encrypted URL generation
    FORMAT_NUMBERS = {
        'MP3_128': 1,
        'MP3_320': 3,
        'FLAC': 9
    }
    
    def __init__(self, credentials: Optional[ServiceCredentials] = None, verbose: bool = False):
        if credentials is None:
            credentials = ServiceCredentials(service_type=ServiceType.DEEZER)
        elif not hasattr(credentials, "service_type") or credentials.service_type is None:
            credentials.service_type = ServiceType.DEEZER
        super().__init__(credentials, verbose)
        self.session: Optional[aiohttp.ClientSession] = None
        self.arl: Optional[str] = None
        self.api_token: Optional[str] = None
        self.license_token: Optional[str] = None
        self.country: Optional[str] = None
        self.user_id: Optional[str] = None
        self.available_formats: List[str] = ['MP3_128']
        self.logger = logging.getLogger(__name__)
        if not CRYPTODOME_AVAILABLE:
            self.logger.warning(
                "pycryptodome not installed. Deezer track decryption will fail. Run: pip install pycryptodome"
            )
    
    # ==========================================
    # ABSTRACT PROPERTY IMPLEMENTATIONS
    # ==========================================
    
    @property
    def service_name(self) -> str:
        """Human-readable service name."""
        return "Deezer"
    
    @property
    def service_type(self) -> ServiceType:
        """Service type enum."""
        return ServiceType.DEEZER
    
    @property
    def supports_lossless(self) -> bool:
        """Whether service supports lossless audio."""
        return True
    
    async def _api_call(self, method: str, payload: Optional[Dict] = None) -> Dict[str, Any]:
        """
        Make a call to the Deezer Gateway API.
        
        Args:
            method: API method (e.g., 'deezer.getUserData', 'search.music')
            payload: Request payload dict
        
        Returns:
            Response results dict
        
        Raises:
            Exception: If API returns error
        """
        if payload is None:
            payload = {}
        
        # Empty api_token for initial calls (getUserData, user.getArl)
        api_token = self.api_token if method not in ('deezer.getUserData', 'user.getArl') else ''
        
        params = {
            'method': method,
            'input': 3,
            'api_version': '1.0',
            'api_token': api_token,
            'cid': randint(0, 1_000_000_000),
        }
        
        headers = {
            'accept': '*/*',
            'user-agent': 'Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36',
            'content-type': 'text/plain;charset=UTF-8',
            'origin': 'https://www.deezer.com',
            'referer': 'https://www.deezer.com/',
        }
        
        async with self.session.post(self.GW_LIGHT_URL, params=params, json=payload, headers=headers) as response:
            response.raise_for_status()
            data = await response.json()
        
        # Check for errors
        if data.get('error'):
            error_type = list(data['error'].keys())[0]
            error_msg = list(data['error'].values())[0]
            raise Exception(f"Deezer API Error: {error_type} - {error_msg}")
        
        # Update session info for getUserData calls
        if method == 'deezer.getUserData':
            results = data['results']
            self.api_token = results['checkForm']
            self.country = results['COUNTRY']
            self.license_token = results['USER']['OPTIONS']['license_token']
            self.user_id = results['USER']['USER_ID']
            
            # Detect available formats based on subscription
            self.available_formats = ['MP3_128']
            if results['USER']['OPTIONS'].get('web_hq'):
                self.available_formats.append('MP3_320')
            if results['USER']['OPTIONS'].get('web_lossless'):
                self.available_formats.append('FLAC')
        
        return data['results']
    
    async def authenticate(self) -> bool:
        """
        Authenticate with Deezer using ARL cookie.
        
        ARL cookie should be extracted from browser after logging in to deezer.com:
        1. Log in to https://www.deezer.com
        2. Open Developer Tools (F12) > Application > Cookies
        3. Copy the 'arl' cookie value
        4. Set it in credentials.token
        
        Returns:
            True if authentication successful, False otherwise
        """
        # Check for ARL in credentials.token or credentials.extra['arl']
        if self.credentials.token:
            self.arl = self.credentials.token
        elif self.credentials.extra and 'arl' in self.credentials.extra:
            self.arl = self.credentials.extra['arl']
        else:
            self._log("Missing ARL cookie. See documentation for extraction instructions.", "error")
            return False
        
        if not self.session:
            self.session = aiohttp.ClientSession()
        
        try:
            # Set ARL cookie properly using URL object
            from yarl import URL
            self.session.cookie_jar.update_cookies(
                {'arl': self.arl},
                response_url=URL('https://www.deezer.com')
            )
            
            # Call getUserData to validate ARL and get session info
            user_data = await self._api_call('deezer.getUserData')
            
            # Check if ARL is valid (USER_ID should be present)
            if not user_data['USER']['USER_ID']:
                self._log("Invalid ARL cookie", "error")
                return False
            
            self._log(f"Authenticated as user ID: {self.user_id}", "info")
            self._log(f"Country: {self.country}", "info")
            self._log(f"Available formats: {', '.join(self.available_formats)}", "info")
            
            return True
            
        except Exception as e:
            self._log(f"Authentication failed: {e}", "error")
            return False
    
    def is_authenticated(self) -> bool:
        """Check if currently authenticated."""
        return self.arl is not None and self.api_token is not None and self.user_id is not None
    
    async def search(
        self,
        query: str,
        result_type: str = "track",
        limit: int = 50,
        offset: int = 0
    ) -> List[SearchResult]:
        """
        Search Deezer catalog.
        
        Args:
            query: Search query string
            result_type: Type of results ('track', 'album', 'artist', 'playlist')
            limit: Maximum number of results
            offset: Offset for pagination
        
        Returns:
            List of SearchResult objects
        """
        if not self.is_authenticated():
            self._log("Not authenticated", "error")
            return []
        
        try:
            payload = {
                'query': query,
                'start': offset,
                'nb': limit,
                'filter': 'ALL',
                'output': result_type.upper()
            }
            
            data = await self._api_call('search.music', payload)
            
            results = []
            for item in data.get('data', []):
                if result_type == 'track':
                    # Build full title with version if present
                    title = item['SNG_TITLE']
                    if item.get('VERSION'):
                        title = f"{title} {item['VERSION']}"
                    
                    # Extract artist names
                    artists = [a['ART_NAME'] for a in item.get('ARTISTS', [])]
                    artist_str = ', '.join(artists) if artists else ''
                    
                    # Detect quality
                    quality = None
                    if item.get('FILESIZE_FLAC', '0') != '0':
                        quality = DownloadQuality.LOSSLESS_CD
                    elif item.get('FILESIZE_MP3_320', '0') != '0':
                        quality = DownloadQuality.LOSSY_STANDARD
                    elif item.get('FILESIZE_MP3_128', '0') != '0':
                        quality = DownloadQuality.LOSSY_LOW
                    
                    results.append(SearchResult(
                        result_type='track',
                        service_id=item['SNG_ID'],
                        service_type=ServiceType.DEEZER,
                        title=title,
                        artist=artist_str,
                        album=item.get('ALB_TITLE', ''),
                        duration_ms=int(item.get('DURATION', 0)) * 1000,
                        quality=quality
                    ))
                
                elif result_type == 'album':
                    artists = [a['ART_NAME'] for a in item.get('ARTISTS', [])]
                    artist_str = ', '.join(artists) if artists else ''
                    
                    # Extract year from release date
                    year = None
                    if item.get('PHYSICAL_RELEASE_DATE'):
                        year = int(item['PHYSICAL_RELEASE_DATE'].split('-')[0])
                    
                    results.append(SearchResult(
                        result_type='album',
                        service_id=item['ALB_ID'],
                        service_type=ServiceType.DEEZER,
                        title=item['ALB_TITLE'],
                        artist=artist_str,
                        year=year
                    ))
            
            self._log(f"Found {len(results)} {result_type}(s) for '{query}'", "info")
            return results
            
        except Exception as e:
            self._log(f"Search failed: {e}", "error")
            return []
    
    async def get_track_metadata(self, track_id: str) -> Optional[TrackMetadata]:
        """
        Get detailed metadata for a track.
        
        Args:
            track_id: Deezer track ID
        
        Returns:
            TrackMetadata object or None if track not found
        """
        if not self.is_authenticated():
            self._log("Not authenticated", "error")
            return None
        
        try:
            payload = {'sng_id': track_id}
            data = await self._api_call('deezer.pageTrack', payload)
            
            if not data or 'DATA' not in data:
                self._log(f"Track {track_id} not found", "error")
                return None
            
            track = data['DATA']
            
            # Handle FALLBACK if track not available
            if 'FALLBACK' in track:
                track = track['FALLBACK']
            
            # Build full title with version
            title = track['SNG_TITLE']
            if track.get('VERSION'):
                title = f"{title} {track['VERSION']}"
            
            # Extract artists
            artists = [a['ART_NAME'] for a in track.get('ARTISTS', [])]
            
            # Detect maximum available quality
            quality = None
            if track.get('FILESIZE_FLAC', '0') != '0':
                quality = DownloadQuality.LOSSLESS_CD
            elif track.get('FILESIZE_MP3_320', '0') != '0':
                quality = DownloadQuality.LOSSY_STANDARD
            elif track.get('FILESIZE_MP3_128', '0') != '0':
                quality = DownloadQuality.LOSSY_LOW
            
            # Parse release date
            year = None
            release_date = track.get('PHYSICAL_RELEASE_DATE')
            if release_date:
                year = int(release_date.split('-')[0])
            
            # Build cover URL
            artwork_url = None
            if track.get('ALB_PICTURE'):
                artwork_url = f"https://cdn-images.dzcdn.net/images/cover/{track['ALB_PICTURE']}/1200x0-000000-80-0-0.jpg"
            
            return TrackMetadata(
                service_id=track['SNG_ID'],
                service_type=ServiceType.DEEZER,
                title=title,
                artists=artists,
                album=track.get('ALB_TITLE', ''),
                album_artist=track.get('ART_NAME'),
                track_number=int(track.get('TRACK_NUMBER', 0)) if track.get('TRACK_NUMBER') else None,
                disc_number=int(track.get('DISK_NUMBER', 1)) if track.get('DISK_NUMBER') else 1,
                duration_ms=int(track.get('DURATION', 0)) * 1000,
                release_date=release_date,
                year=year,
                label=None,  # Not included in basic track metadata
                genres=[],   # Not included in basic track metadata
                quality=quality,
                sample_rate=44100 if quality == DownloadQuality.LOSSLESS_CD else None,
                bit_depth=16 if quality == DownloadQuality.LOSSLESS_CD else None,
                artwork_url=artwork_url,
                isrc=track.get('ISRC'),  # Critical for cross-service matching
                custom_tags={
                    'track_token': track.get('TRACK_TOKEN'),
                    'track_token_expiry': track.get('TRACK_TOKEN_EXPIRE'),
                    'md5_origin': track.get('MD5_ORIGIN'),
                    'media_version': track.get('MEDIA_VERSION'),
                    'explicit': str(track.get('EXPLICIT_LYRICS') == '1')
                }
            )
            
        except Exception as e:
            self._log(f"Failed to get metadata for track {track_id}: {e}", "error")
            return None
    
    def _generate_blowfish_key(self, track_id: str) -> bytes:
        """
        Generate per-track Blowfish decryption key.
        
        Deezer uses a unique key for each track based on:
        - MD5 hash of track ID
        - XOR with BLOWFISH_SECRET
        
        Args:
            track_id: Deezer track ID
        
        Returns:
            16-byte Blowfish key
        """
        # MD5 hash of track ID
        md5_hash = hashlib.md5(track_id.encode()).hexdigest()
        
        # XOR first 16 chars with last 16 chars with BF_SECRET
        # This is Deezer's key derivation function (KDF)
        key = ''.join(
            chr(ord(md5_hash[i]) ^ ord(md5_hash[i + 16]) ^ self.BLOWFISH_SECRET[i])
            for i in range(16)
        ).encode()
        
        return key
    
    async def _get_download_url(self, track_id: str, track_token: str, format_str: str) -> Optional[str]:
        """
        Get download URL for a track.
        
        Args:
            track_id: Deezer track ID
            track_token: Track token from metadata
            format_str: Format string ('MP3_128', 'MP3_320', 'FLAC')
        
        Returns:
            Download URL or None if not available
        """
        try:
            headers = {
                'Content-Type': 'application/json'
            }
            
            json_payload = {
                'license_token': self.license_token,
                'media': [
                    {
                        'type': 'FULL',
                        'formats': [
                            {
                                'cipher': 'BF_CBC_STRIPE',
                                'format': format_str
                            }
                        ]
                    }
                ],
                'track_tokens': [track_token]
            }
            
            async with self.session.post(self.MEDIA_URL, json=json_payload, headers=headers) as response:
                response.raise_for_status()
                data = await response.json()
            
            # Extract URL from response
            if data.get('data') and len(data['data']) > 0:
                media = data['data'][0].get('media', [])
                if media and len(media) > 0:
                    sources = media[0].get('sources', [])
                    if sources and len(sources) > 0:
                        return sources[0].get('url')
            
            return None
            
        except Exception as e:
            self._log(f"Failed to get download URL: {e}", "error")
            return None
    
    async def download_track(
        self,
        track_id: str,
        output_path: Path,
        quality: DownloadQuality = DownloadQuality.LOSSLESS_CD,
        progress_callback: Optional[Callable[[int, int], None]] = None
    ) -> DownloadResult:
        """
        Download a track with Blowfish decryption.
        
        Deezer encrypts tracks with Blowfish CBC:
        - Every 3rd chunk of 2048 bytes is encrypted
        - Cipher must be reset for each encrypted chunk
        - IV is fixed: [0, 1, 2, 3, 4, 5, 6, 7]
        
        Args:
            track_id: Deezer track ID
            output_path: Path to save downloaded file
            quality: Desired quality level
            progress_callback: Optional callback(downloaded_bytes, total_bytes)
        
        Returns:
            DownloadResult with success status and details
        """
        if not self.is_authenticated():
            return DownloadResult(
                success=False,
                error_message="Not authenticated",
                track_metadata=None,
                filepath=None,
                file_size_bytes=0
            )
        
        try:
            # Get track metadata
            metadata = await self.get_track_metadata(track_id)
            if not metadata:
                return DownloadResult(
                    success=False,
                    error_message="Track not found",
                    track_metadata=None,
                    filepath=None,
                    file_size_bytes=0
                )
            
            # Extract download parameters
            track_token = metadata.custom_tags.get('track_token')
            if not track_token:
                return DownloadResult(
                    success=False,
                    error_message="Track token not available",
                    track_metadata=metadata,
                    filepath=None,
                    file_size_bytes=0
                )
            
            # Map quality to Deezer format
            format_str = self.QUALITY_MAP[quality]
            
            # Check if format is available with subscription
            if format_str not in self.available_formats:
                self._log(f"Format {format_str} not available with subscription, falling back", "warning")
                # Fallback to best available
                if 'MP3_320' in self.available_formats:
                    format_str = 'MP3_320'
                else:
                    format_str = 'MP3_128'
            
            # Get download URL
            download_url = await self._get_download_url(track_id, track_token, format_str)
            if not download_url:
                return DownloadResult(
                    success=False,
                    error_message="Download URL not available",
                    track_metadata=metadata,
                    filepath=None,
                    file_size_bytes=0
                )
            
            # Determine if URL requires decryption
            is_encrypted = '/mobile/' in download_url or '/media/' in download_url
            if is_encrypted and (not CRYPTODOME_AVAILABLE or Blowfish is None):
                raise RuntimeError(
                    "pycryptodome is required for decrypting Deezer tracks. Run: pip install pycryptodome"
                )
            
            # Generate Blowfish key for decryption
            bf_key = self._generate_blowfish_key(track_id)
            
            # Download and decrypt
            self._log(f"Downloading track {track_id} as {format_str} (encrypted: {is_encrypted})", "info")
            
            async with self.session.get(download_url, allow_redirects=True) as response:
                response.raise_for_status()
                total_size = int(response.headers.get('Content-Length', 0))
                
                # Check if response is too small (error response)
                if total_size < 20000 and not download_url.endswith('.jpg'):
                    error_data = await response.json()
                    return DownloadResult(
                        success=False,
                        error_message=f"Download failed: {error_data}",
                        track_metadata=metadata,
                        filepath=None,
                        file_size_bytes=0
                    )
                
                # Ensure output directory exists
                output_path.parent.mkdir(parents=True, exist_ok=True)
                
                downloaded_bytes = 0
                chunk_index = 0
                
                # Fixed IV for Blowfish CBC
                blowfish_iv = b'\x00\x01\x02\x03\x04\x05\x06\x07'
                
                with open(output_path, 'wb') as f:
                    async for chunk in response.content.iter_chunked(2048):
                        # Every 3rd chunk of exactly 2048 bytes is encrypted
                        if is_encrypted and chunk_index % 3 == 0 and len(chunk) == 2048:
                            # Reset cipher for each encrypted chunk (Deezer DRM requirement)
                            cipher = Blowfish.new(bf_key, Blowfish.MODE_CBC, blowfish_iv)
                            chunk = cipher.decrypt(chunk)
                        
                        f.write(chunk)
                        downloaded_bytes += len(chunk)
                        chunk_index += 1
                        
                        # Progress callback
                        if progress_callback and total_size > 0:
                            progress_callback(downloaded_bytes, total_size)
            
            # Calculate SHA256 hash for verification
            file_hash = hashlib.sha256()
            with open(output_path, 'rb') as f:
                for chunk in iter(lambda: f.read(8192), b''):
                    file_hash.update(chunk)
            
            self._log(f"Successfully downloaded track {track_id} to {output_path}", "info")
            
            return DownloadResult(
                success=True,
                track_metadata=metadata,
                filepath=str(output_path),
                file_size_bytes=os.path.getsize(output_path),
                download_duration_seconds=0.0
            )
            
        except Exception as e:
            self._log(f"Download failed: {e}", "error")
            return DownloadResult(
                success=False,
                error_message=str(e),
                track_metadata=metadata if 'metadata' in locals() else None,
                filepath=None,
                file_size_bytes=0
            )
    
    # ==========================================
    # STUB METHODS (required by abstract base)
    # ==========================================
    
    async def get_album_metadata(self, album_id: str) -> Optional[AlbumMetadata]:
        """Retrieve album metadata - stub implementation."""
        self._log("get_album_metadata not yet implemented", "warning")
        return None
    
    async def get_album_tracks(self, album_id: str) -> List[TrackMetadata]:
        """Get all tracks in an album - stub implementation."""
        self._log("get_album_tracks not yet implemented", "warning")
        return []
    
    async def get_user_playlists(self) -> List[Dict[str, Any]]:
        """
        Get all playlists for the authenticated user.
        
        Returns:
            List of playlist dicts with id, name, description, track_count, etc.
        """
        if not self.is_authenticated():
            self._log("Not authenticated", "error")
            return []
        
        try:
            # Get user playlists via gateway API
            payload = {'user_id': self.user_id, 'tab': 'playlists', 'nb': 100}
            data = await self._api_call('deezer.pageProfile', payload)
            
            playlists = []
            playlist_tab = data.get('TAB', {}).get('playlists', {})
            
            for item in playlist_tab.get('data', []):
                playlists.append({
                    'id': item.get('PLAYLIST_ID'),
                    'name': item.get('TITLE', 'Unknown Playlist'),
                    'description': item.get('DESCRIPTION', ''),
                    'track_count': int(item.get('NB_SONG', 0)),
                    'owner': item.get('PARENT_USERNAME', ''),
                    'public': item.get('STATUS') == '1',
                    'image_url': f"https://cdn-images.dzcdn.net/images/playlist/{item.get('PLAYLIST_PICTURE')}/500x500.jpg" if item.get('PLAYLIST_PICTURE') else None,
                })
            
            self._log(f"Found {len(playlists)} playlists", "info")
            return playlists
            
        except Exception as e:
            self._log(f"Failed to get user playlists: {e}", "error")
            return []
    
    async def get_playlist_metadata(self, playlist_id: str) -> Optional[PlaylistMetadata]:
        """
        Retrieve playlist metadata.
        
        Args:
            playlist_id: Deezer playlist ID
        
        Returns:
            PlaylistMetadata object or None if not found
        """
        if not self.is_authenticated():
            self._log("Not authenticated", "error")
            return None
        
        try:
            payload = {'playlist_id': playlist_id, 'lang': 'en'}
            data = await self._api_call('deezer.pagePlaylist', payload)
            
            playlist_data = data.get('DATA', {})
            
            return PlaylistMetadata(
                service_id=str(playlist_data.get('PLAYLIST_ID', playlist_id)),
                service_type=ServiceType.DEEZER,
                name=playlist_data.get('TITLE', 'Unknown'),
                description=playlist_data.get('DESCRIPTION', ''),
                owner=playlist_data.get('PARENT_USERNAME', ''),
                track_count=int(playlist_data.get('NB_SONG', 0)),
                duration_ms=int(playlist_data.get('DURATION', 0)) * 1000,
                is_public=playlist_data.get('STATUS') == '1',
            )
            
        except Exception as e:
            self._log(f"Failed to get playlist metadata: {e}", "error")
            return None
    
    async def get_playlist_tracks(self, playlist_id: str) -> List[TrackMetadata]:
        """
        Get all tracks in a playlist.
        
        Args:
            playlist_id: Deezer playlist ID
        
        Returns:
            List of TrackMetadata objects
        """
        if not self.is_authenticated():
            self._log("Not authenticated", "error")
            return []
        
        try:
            payload = {'playlist_id': playlist_id, 'lang': 'en', 'nb': 2000}
            data = await self._api_call('deezer.pagePlaylist', payload)
            
            tracks = []
            songs_data = data.get('SONGS', {}).get('data', [])
            
            for track in songs_data:
                # Build full title with version
                title = track.get('SNG_TITLE', 'Unknown')
                if track.get('VERSION'):
                    title = f"{title} {track['VERSION']}"
                
                # Extract artists
                artists = [a.get('ART_NAME', 'Unknown') for a in track.get('ARTISTS', [])]
                if not artists:
                    artists = [track.get('ART_NAME', 'Unknown Artist')]
                
                # Detect quality
                quality = None
                if track.get('FILESIZE_FLAC', '0') != '0':
                    quality = DownloadQuality.LOSSLESS_CD
                elif track.get('FILESIZE_MP3_320', '0') != '0':
                    quality = DownloadQuality.LOSSY_STANDARD
                else:
                    quality = DownloadQuality.LOSSY_LOW
                
                tracks.append(TrackMetadata(
                    service_id=str(track.get('SNG_ID')),
                    service_type=ServiceType.DEEZER,
                    title=title,
                    artists=artists,
                    album=track.get('ALB_TITLE', ''),
                    album_artist=track.get('ART_NAME'),
                    track_number=int(track.get('TRACK_NUMBER', 0)) if track.get('TRACK_NUMBER') else None,
                    duration_ms=int(track.get('DURATION', 0)) * 1000,
                    isrc=track.get('ISRC'),
                    quality=quality,
                ))
            
            self._log(f"Found {len(tracks)} tracks in playlist {playlist_id}", "info")
            return tracks
            
        except Exception as e:
            self._log(f"Failed to get playlist tracks: {e}", "error")
            return []
    
    async def get_available_qualities(self, track_id: str) -> List[DownloadQuality]:
        """Get available quality options for a track."""
        # Deezer supports these quality levels depending on subscription
        return [
            DownloadQuality.LOSSY_LOW,
            DownloadQuality.LOSSY_STANDARD,
            DownloadQuality.LOSSLESS_CD
        ]
    
    async def close(self):
        """Close the aiohttp session."""
        if self.session:
            await self.session.close()
            self.session = None

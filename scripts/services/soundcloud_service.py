"""
SoundCloud API integration for Syncify.

This module provides complete SoundCloud API functionality including:
- Token-based authentication (client_id + app_version)
- Automatic token extraction from web app
- Track and playlist search
- HLS segment streaming downloads
- Original file downloads (when available)

Note: SoundCloud does not provide ISRC in API responses, so cross-service
matching will rely on artist + title matching instead.
"""

import asyncio
import functools
import hashlib
import logging
import os
import random
import re
import tempfile
from dataclasses import dataclass
from pathlib import Path
from typing import Optional, Callable, Any

import aiohttp

try:
    import m3u8
    M3U8_AVAILABLE = True
except ImportError:
    m3u8 = None  # type: ignore
    M3U8_AVAILABLE = False

try:
    from .service_base import (
        MusicService,
        ServiceCredentials,
        ServiceType,
        SearchResult,
        TrackMetadata,
        AlbumMetadata,
        PlaylistMetadata,
        DownloadResult,
        DownloadQuality,
    )
except ImportError:
    from services.service_base import (
        MusicService,
        ServiceCredentials,
        ServiceType,
        SearchResult,
        TrackMetadata,
        AlbumMetadata,
        PlaylistMetadata,
        DownloadResult,
        DownloadQuality,
    )

logger = logging.getLogger(__name__)


@dataclass
class SoundCloudConfig:
    """Configuration for SoundCloud API access."""
    
    client_id: Optional[str] = None
    app_version: Optional[str] = None


class SoundCloudService(MusicService):
    """
    SoundCloud music service implementation.
    
    Provides access to SoundCloud's free streaming platform with MP3 downloads.
    Unlike subscription services, SoundCloud focuses on user-uploaded content
    and independent artists.
    
    Features:
    - Free streaming (no subscription required)
    - MP3 128kbps standard quality
    - Original file downloads (when uploader enables it)
    - HLS segmented streaming
    - No ISRC support (uses artist + title matching)
    
    Authentication:
    Uses client_id and app_version tokens extracted from SoundCloud web app.
    Tokens are automatically refreshed when they expire.
    """
    
    # API endpoints
    BASE_URL = "https://api-v2.soundcloud.com"
    STOCK_URL = "https://soundcloud.com/"
    
    # Download status markers
    NON_STREAMABLE = "_non_streamable"
    ORIGINAL_DOWNLOAD = "_original_download"
    NOT_RESOLVED = "_not_resolved"
    
    # Constants
    MAX_BATCH_SIZE = 50  # Maximum track IDs per batch request
    
    def __init__(self, credentials: Optional[ServiceCredentials] = None, verbose: bool = False):
        """
        Initialize SoundCloud service.
        
        Args:
            credentials: Service credentials (optional - tokens auto-extracted)
            verbose: Enable verbose logging
        """
        if credentials is None:
            credentials = ServiceCredentials(service_type=ServiceType.SOUNDCLOUD)
        elif not hasattr(credentials, "service_type") or credentials.service_type is None:
            credentials.service_type = ServiceType.SOUNDCLOUD

        super().__init__(credentials, verbose=verbose)
        
        # Extract config from credentials if provided
        if credentials and credentials.extra:
            self.client_id = credentials.extra.get("client_id")
            self.app_version = credentials.extra.get("app_version")
        elif credentials and credentials.token:
            self.client_id = credentials.token
            self.app_version = None
        else:
            self.client_id = None
            self.app_version = None
        
        # Generate random user ID for API requests
        self.user_id = "-".join(
            str(random.randint(111111, 999999)) for _ in range(4)
        )
        
        self.session: Optional[aiohttp.ClientSession] = None

    # ==========================================
    # ABSTRACT PROPERTY IMPLEMENTATIONS
    # ==========================================

    @property
    def service_name(self) -> str:
        """Human-readable service name."""
        return "SoundCloud"

    @property
    def service_type(self) -> ServiceType:
        """Service type enum."""
        return ServiceType.SOUNDCLOUD

    @property
    def supports_lossless(self) -> bool:
        """Whether service supports lossless audio."""
        return False

    async def is_authenticated(self) -> bool:
        """Check if currently authenticated."""
        return bool(self._authenticated or (self.client_id and self.app_version))

    async def get_available_qualities(self, track_id: str) -> list[DownloadQuality]:
        """Get available quality options for a track."""
        return [DownloadQuality.LOSSY_LOW, DownloadQuality.LOSSY_STANDARD]

    async def get_album_metadata(self, album_id: str) -> Optional[AlbumMetadata]:
        """SoundCloud does not use traditional albums."""
        return None

    async def get_album_tracks(self, album_id: str) -> list[TrackMetadata]:
        """SoundCloud does not use traditional albums."""
        return []

    async def get_playlist_metadata(self, playlist_id: str) -> Optional[PlaylistMetadata]:
        """Retrieve playlist metadata."""
        try:
            if not self.session:
                await self.authenticate()
            url = f"{self.BASE_URL}/playlists/{playlist_id}"
            params = {
                "client_id": self.client_id,
                "app_version": self.app_version,
            }
            async with self.session.get(url, params=params) as resp:
                if resp.status != 200:
                    return None
                data = await resp.json()
                user = data.get("user") or {}
                return PlaylistMetadata(
                    service_id=str(data.get("id")),
                    service_type=ServiceType.SOUNDCLOUD,
                    name=data.get("title", "Unknown Playlist"),
                    description=data.get("description"),
                    owner=user.get("username", "Unknown"),
                    track_count=data.get("track_count", len(data.get("tracks", []))),
                    is_public=data.get("public", True),
                    artwork_url=data.get("artwork_url"),
                )
        except Exception as e:
            logger.error(f"Failed to get playlist metadata for {playlist_id}: {e}")
            return None
    
    async def authenticate(self) -> bool:
        """
        Authenticate with SoundCloud API.
        
        Validates existing tokens or extracts new ones from the web app.
        No user credentials required - tokens are public.
        
        Returns:
            bool: True if authentication successful
            
        Raises:
            Exception: If token extraction fails
        """
        logger.info("Authenticating with SoundCloud API...")
        
        # Create session if not exists
        if not self.session:
            self.session = aiohttp.ClientSession(headers={
                "User-Agent": "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36"
            })
        
        # Try existing tokens first
        if self.client_id and self.app_version:
            logger.debug(f"Validating existing tokens: client_id={self.client_id[:8]}...")
            if await self._validate_tokens():
                logger.info("Existing tokens are valid")
                return True
            logger.info("Existing tokens expired, extracting new ones...")
        
        # Extract new tokens from web app
        logger.info("Extracting tokens from SoundCloud web app...")
        self.client_id, self.app_version = await self._refresh_tokens()
        
        logger.info(f"Successfully extracted tokens: client_id={self.client_id[:8]}..., app_version={self.app_version}")
        return True
    
    async def search(
        self,
        query: str,
        result_type: str = "track",
        limit: int = 50,
        search_type: Optional[str] = None
    ) -> list[SearchResult]:
        """
        Search for tracks or playlists on SoundCloud.
        
        Args:
            query: Search query string
            result_type: "track" or "playlist" (service_base standard)
            limit: Maximum number of results (default: 50)
            search_type: Legacy alias for result_type
            
        Returns:
            list[SearchResult]: List of search results
            
        Raises:
            ValueError: If search_type is invalid
            Exception: If API request fails
        """
        effective_type = search_type if search_type is not None else result_type
        if effective_type not in ("track", "playlist"):
            raise ValueError(f"Invalid search_type: {effective_type}. Must be 'track' or 'playlist'")
        
        logger.info(f"Searching SoundCloud: query='{query}', type={effective_type}, limit={limit}")
        
        params = {
            "q": query,
            "facet": "genre",
            "user_id": self.user_id,
            "limit": limit,
            "offset": 0,
            "linked_partitioning": "1",
            "client_id": self.client_id,
            "app_version": self.app_version,
            "app_locale": "en",
        }
        
        url = f"{self.BASE_URL}/search/{effective_type}s"
        async with self.session.get(url, params=params) as resp:
            resp.raise_for_status()
            data = await resp.json()
        
        results = []
        for item in data.get("collection", []):
            user = item.get("user") or {}
            result = SearchResult(
                result_type=effective_type,
                service_id=str(item.get("id")),
                service_type=ServiceType.SOUNDCLOUD,
                title=item.get("title", "Unknown"),
                artist=user.get("username", "Unknown"),
                album=None,
                duration_ms=item.get("duration", 0),
                artwork_url=item.get("artwork_url"),
                quality=DownloadQuality.LOSSY_LOW,
            )
            results.append(result)
        
        logger.info(f"Found {len(results)} {effective_type}(s)")
        return results
    
    async def get_track_metadata(self, track_id: str) -> TrackMetadata:
        """
        Get detailed metadata for a specific track.
        
        Args:
            track_id: SoundCloud track ID
            
        Returns:
            TrackMetadata: Track metadata
            
        Raises:
            Exception: If track not found or API request fails
        """
        logger.info(f"Fetching metadata for track: {track_id}")
        
        # Parse custom ID format if present (track_id|download_status)
        if "|" in track_id:
            track_id, _ = track_id.split("|", 1)
        
        params = {
            "client_id": self.client_id,
            "app_version": self.app_version,
            "app_locale": "en",
        }
        
        url = f"{self.BASE_URL}/tracks/{track_id}"
        async with self.session.get(url, params=params) as resp:
            resp.raise_for_status()
            track = await resp.json()
        
        # Determine available quality
        quality = DownloadQuality.LOSSY_LOW  # Default MP3 128k
        if track.get("downloadable") and track.get("has_downloads_left"):
            quality = DownloadQuality.LOSSLESS_CD  # Original file (may be FLAC/WAV)
        
        user = track.get("user") or {}
        artist_name = user.get("username", "Unknown")
        genre = track.get("genre")
        metadata = TrackMetadata(
            service_id=str(track["id"]),
            service_type=ServiceType.SOUNDCLOUD,
            title=track.get("title", "Unknown"),
            artists=[artist_name],
            album=track.get("title", ""),
            album_artist=artist_name,
            duration_ms=track.get("duration", 0),
            track_number=None,
            disc_number=None,
            year=None,
            genres=[genre] if genre else [],
            isrc=None,
            quality=quality,
            artwork_url=track.get("artwork_url"),
        )
        
        logger.debug(f"Retrieved metadata: {metadata.title} by {metadata.artist}")
        return metadata
    
    async def download_track(
        self,
        track_id: str,
        output_path: str,
        quality: DownloadQuality = DownloadQuality.LOSSY_LOW,
        audio_config: Optional[Any] = None,
        metadata_config: Optional[Any] = None,
        progress_callback: Optional[Callable[[int, int], None]] = None
    ) -> DownloadResult:
        """
        Download a track from SoundCloud.
        
        Downloads either the original file (if available) or HLS stream segments.
        Progress is reported per segment for HLS downloads.
        
        Args:
            track_id: SoundCloud track ID
            output_path: Path to save the downloaded file
            quality: Desired quality (LOSSLESS for original, HIGH for MP3)
            progress_callback: Optional callback for progress updates (current, total)
            
        Returns:
            DownloadResult: Download result with file info
            
        Raises:
            Exception: If track not streamable or download fails
        """
        logger.info(f"Downloading track: {track_id} to {output_path}")
        
        # Parse custom ID format if present
        if "|" in track_id:
            track_id, _ = track_id.split("|", 1)
        
        # Get track metadata
        params = {
            "client_id": self.client_id,
            "app_version": self.app_version,
            "app_locale": "en",
        }
        
        url = f"{self.BASE_URL}/tracks/{track_id}"
        async with self.session.get(url, params=params) as resp:
            resp.raise_for_status()
            track = await resp.json()
        
        # Check if track is streamable
        if not track.get("streamable") or track.get("policy") == "BLOCK":
            raise Exception(f"Track {track_id} is not streamable")
        
        # Determine download method
        if (quality == DownloadQuality.LOSSLESS and
            track.get("downloadable") and
            track.get("has_downloads_left")):
            # Download original file
            logger.info("Downloading original file (high quality)")
            result = await self._download_original_file(
                track_id, output_path, progress_callback
            )
        else:
            # Download HLS stream
            logger.info("Downloading HLS stream (MP3 128k)")
            result = await self._download_hls_stream(
                track, output_path, progress_callback
            )
        
        logger.info(f"Download completed: {result.file_path}")
        return result
    
    async def get_user_playlists(self, user_id: Optional[str] = None) -> list[dict]:
        """
        Get playlists for a user (defaults to likes if no user_id provided).
        
        Args:
            user_id: SoundCloud user ID (optional, uses API v2 me/playlists or likes)
        
        Returns:
            List of playlist dicts
        """
        logger.info("Fetching SoundCloud playlists...")
        
        if not self.client_id:
            logger.error("Not authenticated")
            return []
        
        try:
            params = {
                "client_id": self.client_id,
                "app_version": self.app_version,
                "app_locale": "en",
                "limit": 50,
                "offset": 0,
            }
            
            # Get user's liked playlists
            url = f"{self.BASE_URL}/me/library/playlists_without_albums"
            
            async with self.session.get(url, params=params) as resp:
                if resp.status == 401:
                    # Not logged in, can't get user playlists
                    logger.info("Not logged in, cannot fetch user playlists")
                    return []
                
                resp.raise_for_status()
                data = await resp.json()
            
            playlists = []
            for item in data.get("collection", []):
                playlist = item.get("playlist", item)  # Handle both wrapped and unwrapped
                playlists.append({
                    "id": str(playlist.get("id")),
                    "name": playlist.get("title", "Unknown"),
                    "description": playlist.get("description", ""),
                    "track_count": playlist.get("track_count", 0),
                    "owner": playlist.get("user", {}).get("username", "Unknown"),
                    "public": playlist.get("public", True),
                    "duration": playlist.get("duration", 0) // 1000,  # ms to s
                })
            
            logger.info(f"Found {len(playlists)} SoundCloud playlists")
            return playlists
            
        except Exception as e:
            logger.error(f"Failed to get playlists: {e}")
            return []
    
    async def get_playlist_tracks(self, playlist_id: str) -> list[TrackMetadata]:
        """
        Get all tracks in a playlist.
        
        Args:
            playlist_id: SoundCloud playlist ID
        
        Returns:
            List of TrackMetadata objects
        """
        logger.info(f"Fetching tracks for playlist: {playlist_id}")
        
        if not self.client_id:
            logger.error("Not authenticated")
            return []
        
        try:
            params = {
                "client_id": self.client_id,
                "app_version": self.app_version,
                "app_locale": "en",
            }
            
            url = f"{self.BASE_URL}/playlists/{playlist_id}"
            
            async with self.session.get(url, params=params) as resp:
                resp.raise_for_status()
                data = await resp.json()
            
            tracks = []
            for track in data.get("tracks", []):
                # Skip unavailable tracks
                if not track.get("streamable"):
                    continue
                
                # Determine quality
                quality = DownloadQuality.LOSSY_LOW  # MP3 128k default
                if track.get("downloadable") and track.get("has_downloads_left"):
                    quality = DownloadQuality.LOSSLESS_CD
                
                user = track.get("user") or {}
                artist_name = user.get("username", "Unknown")
                genre = track.get("genre")
                tracks.append(TrackMetadata(
                    service_id=str(track.get("id")),
                    service_type=ServiceType.SOUNDCLOUD,
                    title=track.get("title", "Unknown"),
                    artists=[artist_name],
                    album=track.get("title", ""),
                    album_artist=artist_name,
                    duration_ms=track.get("duration", 0),
                    track_number=None,
                    disc_number=None,
                    year=None,
                    genres=[genre] if genre else [],
                    isrc=None,
                    quality=quality,
                    artwork_url=track.get("artwork_url"),
                ))
            
            logger.info(f"Found {len(tracks)} tracks in playlist {playlist_id}")
            return tracks
            
        except Exception as e:
            logger.error(f"Failed to get playlist tracks: {e}")
            return []
    
    async def close(self):
        """Close the HTTP session."""
        if self.session:
            await self.session.close()
            self.session = None
    
    # Private helper methods
    
    async def _validate_tokens(self) -> bool:
        """
        Validate current tokens by making a test request.
        
        Returns:
            bool: True if tokens are valid
        """
        try:
            params = {
                "client_id": self.client_id,
                "app_version": self.app_version,
                "app_locale": "en",
            }
            url = f"{self.BASE_URL}/announcements"
            async with self.session.get(url, params=params) as resp:
                return resp.status == 200
        except Exception as e:
            logger.debug(f"Token validation failed: {e}")
            return False
    
    async def _refresh_tokens(self) -> tuple[str, str]:
        """
        Extract fresh client_id and app_version from SoundCloud web app.
        
        Returns:
            tuple[str, str]: (client_id, app_version)
            
        Raises:
            Exception: If token extraction fails
        """
        # 1. Fetch main SoundCloud page
        async with self.session.get(self.STOCK_URL) as resp:
            resp.raise_for_status()
            page_html = await resp.text(encoding="utf-8")
        
        # 2. Find the main JavaScript bundle URL (last script with crossorigin)
        script_matches = list(re.finditer(
            r'<script\s+crossorigin\s+src="([^"]+)"',
            page_html
        ))
        
        if not script_matches:
            raise Exception("Could not find script URLs in SoundCloud page")
        
        client_id_url = script_matches[-1].group(1)  # Use last match
        logger.debug(f"Found script URL: {client_id_url}")
        
        # 3. Extract app_version from page
        app_version_match = re.search(
            r'<script>window\.__sc_version="(\d+)"</script>',
            page_html
        )
        
        if not app_version_match:
            raise Exception("Could not find app version in SoundCloud page")
        
        app_version = app_version_match.group(1)
        logger.debug(f"Found app_version: {app_version}")
        
        # 4. Fetch JavaScript bundle and extract client_id
        async with self.session.get(client_id_url) as resp:
            resp.raise_for_status()
            js_content = await resp.text(encoding="utf-8")
        
        client_id_match = re.search(r'client_id:\s*"(\w+)"', js_content)
        
        if not client_id_match:
            raise Exception("Could not find client_id in JavaScript bundle")
        
        client_id = client_id_match.group(1)
        logger.debug(f"Found client_id: {client_id}")
        
        return client_id, app_version
    
    async def _download_original_file(
        self,
        track_id: str,
        output_path: str,
        progress_callback: Optional[Callable[[int, int], None]]
    ) -> DownloadResult:
        """
        Download the original uploaded file (high quality).
        
        Args:
            track_id: Track ID
            output_path: Output file path
            progress_callback: Progress callback
            
        Returns:
            DownloadResult: Download result
        """
        # Get download URL
        params = {
            "client_id": self.client_id,
            "app_version": self.app_version,
            "app_locale": "en",
        }
        
        url = f"{self.BASE_URL}/tracks/{track_id}/download"
        async with self.session.get(url, params=params) as resp:
            resp.raise_for_status()
            data = await resp.json()
        
        download_url = data["redirectUri"]
        logger.debug(f"Got original file URL: {download_url[:50]}...")
        
        # Download file
        async with self.session.get(download_url) as resp:
            resp.raise_for_status()
            total_size = int(resp.headers.get("Content-Length", 0))
            
            downloaded = 0
            with open(output_path, "wb") as f:
                async for chunk in resp.content.iter_chunked(8192):
                    f.write(chunk)
                    downloaded += len(chunk)
                    
                    if progress_callback and total_size > 0:
                        progress_callback(downloaded, total_size)
        
        # Determine file format from extension or content
        file_extension = Path(output_path).suffix or ".mp3"
        
        return DownloadResult(
            file_path=output_path,
            file_size=os.path.getsize(output_path),
            format=file_extension.lstrip(".").upper(),
            quality=DownloadQuality.LOSSLESS
        )
    
    async def _download_hls_stream(
        self,
        track: dict,
        output_path: str,
        progress_callback: Optional[Callable[[int, int], None]]
    ) -> DownloadResult:
        """
        Download HLS stream segments and concatenate.
        
        Args:
            track: Track metadata dict
            output_path: Output file path
            progress_callback: Progress callback (reports segments completed)
            
        Returns:
            DownloadResult: Download result
        """
        # Find HLS transcoding URL
        transcoding_url = None
        for tc in track["media"]["transcodings"]:
            fmt = tc["format"]
            if fmt["protocol"] == "hls" and fmt["mime_type"] == "audio/mpeg":
                transcoding_url = tc["url"]
                break
        
        if not transcoding_url:
            raise Exception("No HLS transcoding available for track")
        
        logger.debug(f"Found HLS transcoding URL: {transcoding_url[:50]}...")
        
        # Get M3U8 playlist URL
        params = {
            "client_id": self.client_id,
            "app_version": self.app_version,
            "app_locale": "en",
        }
        
        async with self.session.get(transcoding_url, params=params) as resp:
            resp.raise_for_status()
            data = await resp.json()
        
        m3u8_url = data["url"]
        logger.debug(f"Got M3U8 playlist URL: {m3u8_url[:50]}...")
        
        # Fetch and parse M3U8 playlist
        if not M3U8_AVAILABLE or m3u8 is None:
            raise RuntimeError("m3u8 library is required for SoundCloud HLS streaming. Run: pip install m3u8")
        async with self.session.get(m3u8_url) as resp:
            resp.raise_for_status()
            m3u8_content = await resp.text(encoding="utf-8")
        
        parsed_m3u8 = m3u8.loads(m3u8_content)
        total_segments = len(parsed_m3u8.segments)
        logger.info(f"Downloading {total_segments} HLS segments...")
        
        # Download all segments to temporary files
        temp_dir = tempfile.mkdtemp(prefix="soundcloud_")
        segment_files = []
        
        try:
            for i, segment in enumerate(parsed_m3u8.segments):
                segment_url = segment.uri
                temp_file = os.path.join(temp_dir, f"segment_{i:04d}.mp3")
                
                async with self.session.get(segment_url) as resp:
                    resp.raise_for_status()
                    content = await resp.read()
                
                with open(temp_file, "wb") as f:
                    f.write(content)
                
                segment_files.append(temp_file)
                
                if progress_callback:
                    progress_callback(i + 1, total_segments)
            
            # Concatenate segments directly (no ffmpeg needed for MP3)
            logger.info("Concatenating segments...")
            with open(output_path, "wb") as output_file:
                for segment_file in segment_files:
                    with open(segment_file, "rb") as f:
                        output_file.write(f.read())
            
            logger.info("Successfully concatenated all segments")
            
        finally:
            # Clean up temporary files
            for segment_file in segment_files:
                try:
                    os.remove(segment_file)
                except Exception as e:
                    logger.warning(f"Failed to remove temp file {segment_file}: {e}")
            
            try:
                os.rmdir(temp_dir)
            except Exception as e:
                logger.warning(f"Failed to remove temp directory {temp_dir}: {e}")
        
        return DownloadResult(
            file_path=output_path,
            file_size=os.path.getsize(output_path),
            format="MP3",
            quality=DownloadQuality.LOSSY_LOW
        )
    
    def _get_custom_id(self, track: dict) -> str:
        """
        Generate custom ID format that includes download availability.
        
        Format: {track_id}|{download_status}
        
        Args:
            track: Track metadata dict
            
        Returns:
            str: Custom ID string
        """
        track_id = track["id"]
        
        # Check if track is streamable
        if not track.get("streamable") or track.get("policy") == "BLOCK":
            return f"{track_id}|{self.NON_STREAMABLE}"
        
        # Check for original download
        if track.get("downloadable") and track.get("has_downloads_left"):
            return f"{track_id}|{self.ORIGINAL_DOWNLOAD}"
        
        # Find HLS transcoding URL
        transcoding_url = None
        for tc in track.get("media", {}).get("transcodings", []):
            fmt = tc["format"]
            if fmt["protocol"] == "hls" and fmt["mime_type"] == "audio/mpeg":
                transcoding_url = tc["url"]
                break
        
        if transcoding_url:
            return f"{track_id}|{transcoding_url}"
        
        return f"{track_id}|{self.NON_STREAMABLE}"


# Example usage
if __name__ == "__main__":
    async def main():
        # Initialize service (no credentials needed)
        service = SoundCloudService()
        
        try:
            # Authenticate (auto-extracts tokens)
            await service.authenticate()
            
            # Search for tracks
            results = await service.search("Daft Punk", search_type="track", limit=5)
            print(f"\nFound {len(results)} tracks:")
            for i, result in enumerate(results, 1):
                print(f"{i}. {result.title} by {result.artist} ({result.duration}s)")
            
            if results:
                # Get metadata for first result
                track_id = results[0].id
                metadata = await service.get_track_metadata(track_id)
                print(f"\nMetadata for '{metadata.title}':")
                print(f"  Artist: {metadata.artist}")
                print(f"  Duration: {metadata.duration}s")
                print(f"  Genre: {metadata.genre}")
                print(f"  Quality: {metadata.quality}")
                print(f"  ISRC: {metadata.isrc or 'Not available'}")
                
                # Optional: Download track
                # def progress(current, total):
                #     percent = (current / total) * 100
                #     print(f"\rDownload progress: {percent:.1f}%", end="")
                # 
                # print("\nDownloading track...")
                # result = await service.download_track(
                #     track_id,
                #     "output.mp3",
                #     quality=DownloadQuality.HIGH,
                #     progress_callback=progress
                # )
                # print(f"\nDownloaded: {result.file_path}")
                # print(f"Size: {result.file_size / 1024 / 1024:.2f} MB")
                # print(f"Format: {result.format}")
        
        finally:
            await service.close()
    
    asyncio.run(main())

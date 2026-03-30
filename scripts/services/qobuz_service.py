"""
Qobuz service integration - Phase 1 implementation.
Based on QobuzDownloaderX-MOD architecture.

TODO: Week 3-4 Implementation Tasks
- [ ] Extract API credentials from QobuzDownloaderX-MOD
- [ ] Implement complete authentication flow
- [ ] Add search functionality
- [ ] Implement track metadata retrieval
- [ ] Add download with progress tracking
- [ ] Test with real Qobuz account
"""

import asyncio
import aiohttp
import hashlib
import time
from typing import List, Optional, Callable, TYPE_CHECKING
from pathlib import Path
import logging

if TYPE_CHECKING:
    from core_logic.migration_engine import AudioQualityConfig, MetadataTagConfig

from services.service_base import (
    MusicService, ServiceType, ServiceCredentials, TrackMetadata,
    AlbumMetadata, PlaylistMetadata, SearchResult, DownloadResult,
    DownloadQuality, DownloadStatus
)
from services.metadata_enrichment import enrich_metadata, EnrichedMetadata
from services.lyrics_service import LyricsService


class QobuzService(MusicService):
    """
    Qobuz music service implementation.
    
    Authentication: Username/password → User auth token
    API Base: https://www.qobuz.com/api.json/0.2
    
    Quality Levels:
    - 5: MP3 320kbps (lossy_standard)
    - 7: CD Quality 16bit/44.1kHz (lossless_cd)
    - 27: Hi-Res 24bit/96kHz+ (lossless_hires)
    """
    
    # API Configuration
    BASE_URL = "https://www.qobuz.com/api.json/0.2"
    
    # Credentials extracted from Qobuz Web Player via streamrip's spoofer
    # These are validated working credentials as of November 2025
    APP_ID = "798273057"
    APP_SECRET = ""  # Working secret (verified)
    
    # Quality mapping: DownloadQuality → Qobuz format_id
    # Qobuz format IDs (verified from QobuzDownloaderX-MOD):
    #   5  = MP3 320kbps
    #   6  = CD Quality FLAC 16-bit/44.1kHz  
    #   7  = Hi-Res FLAC 24-bit/96kHz
    #   27 = Hi-Res FLAC 24-bit/192kHz
    QUALITY_MAP = {
        DownloadQuality.LOSSY_LOW: 5,         # MP3 320kbps
        DownloadQuality.LOSSY_STANDARD: 5,    # MP3 320kbps  
        DownloadQuality.LOSSLESS_CD: 6,       # CD Quality 16-bit/44.1kHz
        DownloadQuality.LOSSLESS_HIRES_96: 7, # Hi-Res 24-bit/96kHz
        DownloadQuality.LOSSLESS_HIRES: 27    # Hi-Res 24-bit/192kHz
    }
    
    def __init__(
        self, 
        credentials: ServiceCredentials, 
        verbose: bool = False,
        enable_metadata_enrichment: bool = True,
        lastfm_api_key: Optional[str] = None
    ):
        super().__init__(credentials, verbose)
        self.session: Optional[aiohttp.ClientSession] = None
        self.user_auth_token: Optional[str] = None
        self.user_id: Optional[str] = None
        self.logger = logging.getLogger(__name__)
        
        # Metadata enrichment configuration
        self.enable_metadata_enrichment = enable_metadata_enrichment
        self.lastfm_api_key = lastfm_api_key
    
    def _get_max_quality(self, track_data: dict) -> str:
        """Determine maximum available quality from track data."""
        if track_data.get('maximum_bit_depth'):
            bit_depth = track_data.get('maximum_bit_depth')
            sample_rate = track_data.get('maximum_sampling_rate')
            return f"Hi-Res {bit_depth}bit/{sample_rate}kHz"
        return "CD Quality"
    
    async def authenticate(self) -> bool:
        """
        Authenticate with Qobuz API.
        
        Qobuz requires MD5-hashed passwords for legacy compatibility.
        Returns user_auth_token used for all subsequent API calls.
        
        Returns:
            True if authentication successful, False otherwise
        """
        if not self.credentials.username or not self.credentials.password:
            self._log("Missing username or password", "error")
            return False
        
        if not self.session:
            # Set X-App-Id header in session like streamrip does
            headers = {"X-App-Id": self.APP_ID}
            self.session = aiohttp.ClientSession(headers=headers)
        
        try:
            # Qobuz requires MD5-hashed password (legacy requirement)
            password_md5 = hashlib.md5(self.credentials.password.encode()).hexdigest()
            
            # POST data for login
            data = {
                "email": self.credentials.username,
                "password": password_md5,
                "app_id": self.APP_ID
            }
            
            self._log(f"Authenticating as {self.credentials.username}...")
            
            async with self.session.post(
                f"{self.BASE_URL}/user/login", 
                data=data,
                timeout=aiohttp.ClientTimeout(total=30)
            ) as response:
                if response.status == 200:
                    result = await response.json()
                    
                    self.user_auth_token = result.get('user_auth_token')
                    user_data = result.get('user', {})
                    self.user_id = str(user_data.get('id', ''))
                    
                    if self.user_auth_token and self.user_id:
                        self._authenticated = True
                        self._log(f"✓ Authentication successful! User ID: {self.user_id}")
                        self._log(f"  Display name: {user_data.get('display_name', 'N/A')}")
                        self._log(f"  Subscription: {user_data.get('credential', {}).get('label', 'Unknown')}")
                        return True
                    else:
                        self._log("Authentication response missing required fields", "error")
                        return False
                else:
                    error_data = await response.json()
                    error_msg = error_data.get('message', 'Unknown error')
                    self._log(f"Authentication failed (HTTP {response.status}): {error_msg}", "error")
                    return False
        
        except asyncio.TimeoutError:
            self._log("Authentication timeout", "error")
            return False
        except Exception as e:
            self._log(f"Authentication error: {e}", "error")
            self.logger.exception("Authentication failed")
            return False
    
    async def is_authenticated(self) -> bool:
        """Check if currently authenticated."""
        return self._authenticated and self.user_auth_token is not None
    
    # Mapping between MetadataTagConfig fields and FLAC tags
    TAG_CONFIG_MAP = {
        'musicbrainz': ['MUSICBRAINZ_TRACKID', 'MUSICBRAINZ_ALBUMID', 'MUSICBRAINZ_ARTISTID', 
                        'MUSICBRAINZ_RELEASEGROUPID', 'MUSICBRAINZ_ALBUMARTISTID'],
        'isrc': ['ISRC'],
        'upc': ['UPC', 'BARCODE'],
        'label': ['LABEL', 'ORGANIZATION'],
        'composer': ['COMPOSER'],
        'producer': ['PRODUCER'],
        'compilation': ['COMPILATION'],
        'mediatype': ['MEDIA', 'MEDIATYPE'],
        'albumversion': ['VERSION', 'ALBUMVERSION'],
        'originaldate': ['ORIGINALDATE', 'ORIGINALYEAR'],
        'bpm': ['BPM', 'TEMPO'],
        'mood': ['MOOD'],
        'occasion': ['OCCASION'],
        'style': ['STYLE'],
        'language': ['LANGUAGE'],
        'country': ['COUNTRY', 'RELEASECOUNTRY'],
        'work': ['WORK'],
        'movement': ['MOVEMENT'],
        'movementnumber': ['MOVEMENTNUMBER'],
        'personnel': ['PERFORMER', 'PERSONNEL', 'COMMENT'],
        'copyright': ['COPYRIGHT']
    }
    
    def _is_tag_enabled(self, tag_name: str, metadata_config: Optional['MetadataTagConfig']) -> bool:
        """
        Check if a specific tag should be embedded based on config.
        
        Args:
            tag_name: The FLAC tag name to check (e.g., 'MUSICBRAINZ_TRACKID', 'ISRC')
            metadata_config: Optional metadata configuration
            
        Returns:
            True if tag should be embedded, False otherwise
        """
        # If no config provided, embed all tags (backwards compatibility)
        if metadata_config is None:
            return True
        
        # Find which config option controls this tag
        tag_upper = tag_name.upper()
        for config_field, tag_list in self.TAG_CONFIG_MAP.items():
            if tag_upper in tag_list:
                # Get the config value for this field
                return getattr(metadata_config, config_field, True)
        
        # Tag not in the controlled list - always embed (standard tags)
        return True
    
    def _embed_metadata_conditional(
        self,
        audio_file_path: str,
        metadata: TrackMetadata,
        metadata_config: Optional['MetadataTagConfig'] = None,
        artwork_data: Optional[tuple] = None
    ) -> None:
        """
        Embed metadata tags conditionally based on MetadataTagConfig.
        
        Standard tags (TITLE, ARTIST, ALBUM, etc.) are always embedded.
        Optional tags are only embedded if enabled in metadata_config.
        
        Args:
            audio_file_path: Path to the FLAC audio file
            metadata: Track metadata to embed
            metadata_config: Optional config specifying which tags to embed
            artwork_data: Optional tuple of (image_bytes, mime_type) for album art
        """
        from mutagen.flac import FLAC, Picture
        
        audio = FLAC(audio_file_path)
        
        # Embed album artwork if available (always embedded)
        if artwork_data:
            picture = Picture()
            picture.data = artwork_data[0]
            picture.type = 3  # Cover (front)
            picture.mime = artwork_data[1]
            picture.desc = 'Cover'
            audio.add_picture(picture)
            self._log("  ✓ Album artwork embedded")
        
        # === STANDARD TAGS - ALWAYS EMBEDDED (not controlled by config) ===
        audio['TITLE'] = metadata.title
        audio['ALBUM'] = metadata.album
        audio['ALBUMARTIST'] = metadata.album_artist or (metadata.artists[0] if metadata.artists else 'Unknown')
        audio['TRACKNUMBER'] = str(metadata.track_number) if metadata.track_number else '1'
        audio['DISCNUMBER'] = str(metadata.disc_number) if metadata.disc_number else '1'
        
        # DATE in ISO 8601 format (YYYY-MM-DD or YYYY)
        if metadata.custom_tags and metadata.custom_tags.get('release_date'):
            audio['DATE'] = str(metadata.custom_tags['release_date'])
        elif metadata.year:
            audio['DATE'] = str(metadata.year)
        else:
            audio['DATE'] = '1900'
        
        # TRACKTOTAL and DISCTOTAL (required for navigation)
        if metadata.custom_tags and metadata.custom_tags.get('tracktotal'):
            audio['TRACKTOTAL'] = str(metadata.custom_tags['tracktotal'])
        else:
            audio['TRACKTOTAL'] = '1'
        audio['DISCTOTAL'] = '1'
        
        # ARTISTS (multi-valued - always embedded)
        if metadata.artists:
            audio['ARTISTS'] = metadata.artists
            audio['ARTIST'] = metadata.album_artist or metadata.artists[0]
        else:
            audio['ARTIST'] = metadata.album_artist or 'Unknown'
        
        # GENRE (always embedded)
        if metadata.genres:
            audio['GENRE'] = metadata.genres if isinstance(metadata.genres, list) else [metadata.genres]
        
        # Audio quality tags (always embedded - technical info)
        if metadata.sample_rate:
            audio['SAMPLERATE'] = str(int(metadata.sample_rate))
        if metadata.bit_depth:
            audio['BITSPERSAMPLE'] = str(int(metadata.bit_depth))
        
        # URL to Qobuz page (always embedded)
        if metadata.service_id:
            audio['URL'] = f'https://www.qobuz.com/track/{metadata.service_id}'
        
        # === CONDITIONAL TAGS - Only embed if enabled in config ===
        
        # MusicBrainz identifiers
        if metadata.custom_tags:
            if self._is_tag_enabled('MUSICBRAINZ_TRACKID', metadata_config):
                if metadata.custom_tags.get('musicbrainz_recording_id'):
                    audio['MUSICBRAINZ_TRACKID'] = str(metadata.custom_tags['musicbrainz_recording_id'])
            if self._is_tag_enabled('MUSICBRAINZ_ALBUMID', metadata_config):
                if metadata.custom_tags.get('musicbrainz_release_id'):
                    audio['MUSICBRAINZ_ALBUMID'] = str(metadata.custom_tags['musicbrainz_release_id'])
            if self._is_tag_enabled('MUSICBRAINZ_RELEASEGROUPID', metadata_config):
                if metadata.custom_tags.get('musicbrainz_releasegroup_id'):
                    audio['MUSICBRAINZ_RELEASEGROUPID'] = str(metadata.custom_tags['musicbrainz_releasegroup_id'])
            if self._is_tag_enabled('MUSICBRAINZ_ARTISTID', metadata_config):
                if metadata.custom_tags.get('musicbrainz_artist_id'):
                    audio['MUSICBRAINZ_ARTISTID'] = str(metadata.custom_tags['musicbrainz_artist_id'])
            if self._is_tag_enabled('MUSICBRAINZ_ALBUMARTISTID', metadata_config):
                if metadata.custom_tags.get('musicbrainz_albumartist_id'):
                    audio['MUSICBRAINZ_ALBUMARTISTID'] = str(metadata.custom_tags['musicbrainz_albumartist_id'])
        
        # ISRC
        if self._is_tag_enabled('ISRC', metadata_config) and metadata.isrc:
            audio['ISRC'] = metadata.isrc
        
        # UPC/Barcode
        if self._is_tag_enabled('BARCODE', metadata_config) and metadata.upc:
            audio['BARCODE'] = metadata.upc
        
        # Label
        if self._is_tag_enabled('LABEL', metadata_config) and metadata.label:
            audio['LABEL'] = metadata.label
        
        # Composer
        if self._is_tag_enabled('COMPOSER', metadata_config) and metadata.custom_tags:
            if 'composer' in metadata.custom_tags and metadata.custom_tags['composer']:
                composers = metadata.custom_tags['composer']
                audio['COMPOSER'] = composers if isinstance(composers, list) else [str(composers)]
        
        # Producer
        if self._is_tag_enabled('PRODUCER', metadata_config) and metadata.custom_tags:
            if 'producer' in metadata.custom_tags and metadata.custom_tags['producer']:
                producers = metadata.custom_tags['producer']
                audio['PRODUCER'] = producers if isinstance(producers, list) else [str(producers)]
        
        # Compilation
        if self._is_tag_enabled('COMPILATION', metadata_config):
            if metadata.album_artist and metadata.album_artist.lower() in ['various artists', 'various']:
                audio['COMPILATION'] = '1'
        
        # MediaType
        if self._is_tag_enabled('MEDIATYPE', metadata_config) and metadata.custom_tags:
            if metadata.custom_tags.get('tracktotal'):
                track_total = int(metadata.custom_tags['tracktotal'])
                audio['MEDIATYPE'] = 'Album' if track_total > 1 else 'Single'
            else:
                audio['MEDIATYPE'] = 'Single'
        
        # Album Version
        if self._is_tag_enabled('ALBUMVERSION', metadata_config) and metadata.custom_tags:
            if metadata.custom_tags.get('album_version'):
                audio['ALBUMVERSION'] = str(metadata.custom_tags['album_version'])
        
        # Original Date
        if self._is_tag_enabled('ORIGINALDATE', metadata_config) and metadata.custom_tags:
            if metadata.custom_tags.get('original_release_date'):
                audio['ORIGINALDATE'] = str(metadata.custom_tags['original_release_date'])
        
        # BPM
        if self._is_tag_enabled('BPM', metadata_config) and metadata.custom_tags:
            if metadata.custom_tags.get('bpm'):
                audio['BPM'] = str(metadata.custom_tags['bpm'])
        
        # Mood
        if self._is_tag_enabled('MOOD', metadata_config) and metadata.custom_tags:
            if metadata.custom_tags.get('mood'):
                mood = metadata.custom_tags['mood']
                if isinstance(mood, list):
                    audio['MOOD'] = mood[:5]
                else:
                    audio['MOOD'] = [str(mood)]
        
        # Occasion
        if self._is_tag_enabled('OCCASION', metadata_config) and metadata.custom_tags:
            if metadata.custom_tags.get('occasion'):
                occasion = metadata.custom_tags['occasion']
                if isinstance(occasion, list):
                    audio['OCCASION'] = occasion[:5]
                else:
                    audio['OCCASION'] = [str(occasion)]
        
        # Style (detailed genres)
        if self._is_tag_enabled('STYLE', metadata_config) and metadata.custom_tags:
            if metadata.custom_tags.get('detailed_genres'):
                detailed = metadata.custom_tags['detailed_genres']
                if isinstance(detailed, list) and detailed:
                    audio['STYLE'] = detailed
                    audio['GENRE'] = detailed  # Also update GENRE with full hierarchy
            
            # Last.fm style tags
            if metadata.custom_tags.get('lastfm_style'):
                style = metadata.custom_tags['lastfm_style']
                if isinstance(style, list) and style:
                    if audio.get('STYLE'):
                        existing = audio['STYLE'] if isinstance(audio['STYLE'], list) else [audio['STYLE']]
                        audio['STYLE'] = existing + [s for s in style[:10] if s not in existing]
                    else:
                        audio['STYLE'] = style[:10]
        
        # Language
        if self._is_tag_enabled('LANGUAGE', metadata_config) and metadata.custom_tags:
            if metadata.custom_tags.get('language'):
                audio['LANGUAGE'] = str(metadata.custom_tags['language']).upper()
        
        # Country
        if self._is_tag_enabled('RELEASECOUNTRY', metadata_config) and metadata.custom_tags:
            if metadata.custom_tags.get('country'):
                audio['RELEASECOUNTRY'] = str(metadata.custom_tags['country'])
        
        # Classical music tags
        if self._is_tag_enabled('WORK', metadata_config) and metadata.custom_tags:
            if metadata.custom_tags.get('work'):
                audio['WORK'] = str(metadata.custom_tags['work'])
        
        if self._is_tag_enabled('MOVEMENT', metadata_config) and metadata.custom_tags:
            if metadata.custom_tags.get('movement'):
                audio['MOVEMENT'] = str(metadata.custom_tags['movement'])
        
        if self._is_tag_enabled('MOVEMENTNUMBER', metadata_config) and metadata.custom_tags:
            if metadata.custom_tags.get('movementnumber'):
                audio['MOVEMENTNUMBER'] = str(metadata.custom_tags['movementnumber'])
        
        # Personnel/Credits
        if self._is_tag_enabled('COMMENT', metadata_config) and metadata.custom_tags:
            if metadata.custom_tags.get('personnel'):
                audio['COMMENT'] = f"Personnel: {metadata.custom_tags['personnel']}"
        
        # Copyright
        if self._is_tag_enabled('COPYRIGHT', metadata_config) and metadata.custom_tags:
            if metadata.custom_tags.get('copyright'):
                audio['COPYRIGHT'] = str(metadata.custom_tags['copyright'])
        
        # ReplayGain tags (always embedded - technical audio info)
        if metadata.custom_tags:
            if metadata.custom_tags.get('replaygain_track_gain'):
                audio['REPLAYGAIN_TRACK_GAIN'] = str(metadata.custom_tags['replaygain_track_gain'])
            if metadata.custom_tags.get('replaygain_track_peak'):
                audio['REPLAYGAIN_TRACK_PEAK'] = str(metadata.custom_tags['replaygain_track_peak'])
        
        # Release status (always embedded - organizational info)
        if metadata.custom_tags and metadata.custom_tags.get('release_type'):
            audio['RELEASESTATUS'] = str(metadata.custom_tags['release_type']).title()
        
        audio.save()
        
        # Verify tags were actually saved
        verify_audio = FLAC(audio_file_path)
        if not verify_audio.get('TITLE'):
            raise RuntimeError(f"Metadata verification failed - TITLE tag missing after save")
        
        self._log("  ✓ Metadata embedded and verified")
    
    def _detect_bpm(self, audio_file_path: str) -> Optional[float]:
        """
        Detect BPM (tempo) from audio file using librosa.
        Directly supports FLAC without conversion.
        
        Args:
            audio_file_path: Path to the audio file (FLAC, MP3, WAV, etc.)
            
        Returns:
            Detected BPM as float, or None if detection fails
        """
        try:
            import librosa
            
            # Load audio file (librosa handles FLAC natively)
            # We only need ~30 seconds for BPM detection to save time
            y, sr = librosa.load(audio_file_path, sr=None, duration=30, mono=True)
            
            # Detect tempo using librosa's beat tracking
            tempo, _ = librosa.beat.beat_track(y=y, sr=sr)
            
            # Convert from numpy scalar to float
            bpm = float(tempo)
            
            # Validate BPM is in reasonable range (40-240)
            if 40 <= bpm <= 240:
                return bpm
            else:
                return None
                
        except ImportError:
            self._log("  librosa not installed, skipping BPM detection", "warning")
            return None
        except Exception as e:
            self._log(f"  BPM detection error: {e}", "warning")
            return None
    
    async def search(
        self, 
        query: str, 
        result_type: str = "track", 
        limit: int = 50
    ) -> List[SearchResult]:
        """
        Search Qobuz catalog.
        
        Args:
            query: Search query string
            result_type: Type of results ("track", "album", "artist")
            limit: Maximum number of results (max 50 per request)
            
        Returns:
            List of search results
        """
        if not await self.is_authenticated():
            await self.authenticate()
        
        if not self.user_auth_token:
            self._log("Cannot search without authentication token", "error")
            return []
        
        try:
            params = {
                "query": query,
                "limit": min(limit, 50),  # API max is 50
                "offset": 0,
                "user_auth_token": self.user_auth_token,
                "app_id": self.APP_ID
            }
            
            self._log(f"Searching for: {query} (type: {result_type})")
            
            async with self.session.get(
                f"{self.BASE_URL}/catalog/search",
                params=params,
                timeout=aiohttp.ClientTimeout(total=30)
            ) as response:
                if response.status == 200:
                    data = await response.json()
                    results = []
                    
                    # Parse track results
                    if result_type == "track" or result_type == "all":
                        tracks = data.get('tracks', {}).get('items', [])
                        for track in tracks:
                            album_data = track.get('album', {})
                            results.append(SearchResult(
                                service_id=str(track['id']),
                                service_type=ServiceType.QOBUZ,
                                result_type="track",
                                title=track.get('title', 'Unknown'),
                                artist=track.get('performer', {}).get('name') if track.get('performer') else (
                                    ', '.join([a['name'] for a in track.get('performers', []) if isinstance(a, dict)]) 
                                    if isinstance(track.get('performers'), list) 
                                    else str(track.get('performers', 'Unknown'))
                                ),
                                album=album_data.get('title'),
                                year=album_data.get('release_date_original', '')[:4] if album_data.get('release_date_original') else None
                            ))
                    
                    # Parse album results
                    if result_type == "album" or result_type == "all":
                        albums = data.get('albums', {}).get('items', [])
                        for album in albums:
                            results.append(SearchResult(
                                service_id=str(album['id']),
                                service_type=ServiceType.QOBUZ,
                                result_type="album",
                                title=album.get('title', 'Unknown'),
                                artist=', '.join([artist['name'] for artist in album.get('artists', [])]),
                                album=album.get('title'),
                                year=album.get('release_date_original', '')[:4] if album.get('release_date_original') else None
                            ))
                    
                    self._log(f"✓ Found {len(results)} results")
                    return results
                else:
                    self._log(f"Search failed (HTTP {response.status})", "error")
                    return []
        
        except Exception as e:
            self._log(f"Search error: {e}", "error")
            self.logger.exception("Search failed")
            return []
    
    async def get_track_metadata(self, track_id: str) -> Optional[TrackMetadata]:
        """
        Retrieve detailed metadata for a track.
        
        Args:
            track_id: Qobuz track identifier
            
        Returns:
            TrackMetadata object or None if not found
        """
        if not await self.is_authenticated():
            await self.authenticate()
        
        try:
            params = {
                "track_id": track_id,
                "app_id": self.APP_ID,
            }
            
            self._log(f"Fetching metadata for track {track_id}...")
            
            async with self.session.get(
                f"{self.BASE_URL}/track/get",
                params=params,
                timeout=aiohttp.ClientTimeout(total=30)
            ) as response:
                if response.status == 200:
                    data = await response.json()
                    album = data.get('album', {})
                    
                    # Extract performer and composer information
                    performers = data.get('performers')
                    artist_names = []
                    composers_list = []
                    producers_list = []
                    personnel_list = []  # All credits for PERSONNEL/COMMENT field
                    
                    if isinstance(performers, str):
                        # Performers is a formatted string like "Michael Jackson, Vocal - Vincent Price, Speaker - ..."
                        # Parse it to extract ONLY actual performing artists
                        # Format: "Name1, Role1 - Name2, Role2 - ..."
                        credits = [c.strip() for c in performers.split(' - ') if c.strip()]
                        for credit in credits:
                            if ', ' in credit:
                                parts = credit.split(', ', 1)
                                name = parts[0].strip()
                                role = parts[1].strip() if len(parts) > 1 else ''
                                
                                if name and role:
                                    # Store all credits for PERSONNEL field
                                    personnel_list.append(f"{name}: {role}")
                                    
                                    # Extract specific roles
                                    role_lower = role.lower()
                                    
                                    # Only add to artist_names if they're actual performing artists
                                    if any(keyword in role_lower for keyword in ['mainartist', 'vocalist', 'artist', 'vocal']):
                                        if name not in artist_names:
                                            artist_names.append(name)
                                    
                                    # Extract composers
                                    if 'composer' in role_lower or 'lyricist' in role_lower:
                                        if name not in composers_list:
                                            composers_list.append(name)
                                    
                                    # Extract producers
                                    if 'producer' in role_lower:
                                        if name not in producers_list:
                                            producers_list.append(name)
                    
                    elif isinstance(performers, list):
                        # Performers is a list of dicts with name/role
                        for p in performers:
                            if isinstance(p, dict):
                                name = p.get('name', '')
                                role = p.get('role', '')
                                if name and role:
                                    # Store all credits for PERSONNEL field
                                    personnel_list.append(f"{name}: {role}")
                                    
                                    # Extract specific roles
                                    role_lower = role.lower()
                                    
                                    # Only add to artist_names if they're actual performing artists
                                    if any(keyword in role_lower for keyword in ['mainartist', 'vocalist', 'artist', 'vocal']):
                                        if name not in artist_names:
                                            artist_names.append(name)
                                    
                                    # Extract composers
                                    if 'composer' in role_lower or 'lyricist' in role_lower:
                                        if name not in composers_list:
                                            composers_list.append(name)
                                    
                                    # Extract producers
                                    if 'producer' in role_lower:
                                        if name not in producers_list:
                                            producers_list.append(name)
                    
                    # Check for separate composer field
                    composer = data.get('composer', {}).get('name') if data.get('composer') else None
                    if composer and composer not in composers_list:
                        composers_list.append(composer)
                    
                    # Convert duration from seconds to milliseconds
                    duration_ms = data.get('duration') * 1000 if data.get('duration') else None
                    
                    # Extract year and full release date
                    year_str = album.get('release_date_original', '')
                    year_int = int(year_str[:4]) if year_str and len(year_str) >= 4 else None
                    release_date = album.get('release_date_original', '')  # Full date like 2024-12-12
                    
                    # Get genre information (both simple and detailed)
                    genre_name = album.get('genre', {}).get('name')
                    genres = [genre_name] if genre_name else []
                    
                    # Get detailed genre path (e.g., "Pop/Rock→Rock→Alternatif et Indé")
                    genres_list = album.get('genres_list', [])
                    detailed_genres = genres_list if genres_list else []
                    
                    # Get album artwork URLs
                    image_data = album.get('image', {})
                    artwork_url = image_data.get('large') if image_data else None
                    
                    # Get BPM if available
                    bpm = data.get('bpm')
                    
                    # Get release type and product type
                    release_type = album.get('release_type')  # 'single', 'album', etc.
                    product_type = album.get('product_type')  # Additional type info
                    
                    # Get audio info (ReplayGain, etc.)
                    audio_info = data.get('audio_info', {})
                    
                    # Get period/era (for classical music)
                    period = album.get('period')
                    
                    # Get recording information
                    recording_info = album.get('recording_information')
                    
                    # Get area/country information if available
                    area = album.get('area')
                    
                    # Get track and disc totals
                    tracks_count = album.get('tracks_count', 1)
                    
                    # Build custom tags dictionary
                    custom_tags = {}
                    if composers_list:
                        custom_tags['composer'] = composers_list
                    if producers_list:
                        custom_tags['producer'] = producers_list
                    if personnel_list:
                        # Store full credits for COMMENT field (will be used in metadata embedding)
                        custom_tags['personnel'] = ' | '.join(personnel_list)
                    if data.get('copyright'):
                        custom_tags['copyright'] = data.get('copyright')
                    if data.get('parental_warning') is not None:
                        custom_tags['parental_warning'] = str(data.get('parental_warning', False))
                    if release_date:
                        custom_tags['release_date'] = release_date
                    if tracks_count:
                        custom_tags['tracktotal'] = str(tracks_count)
                    if bpm:
                        custom_tags['bpm'] = str(bpm)
                    if detailed_genres:
                        custom_tags['detailed_genres'] = detailed_genres
                    if release_type:
                        custom_tags['release_type'] = release_type
                    if product_type:
                        custom_tags['product_type'] = product_type
                    if audio_info:
                        if 'replaygain_track_gain' in audio_info:
                            custom_tags['replaygain_track_gain'] = str(audio_info['replaygain_track_gain'])
                        if 'replaygain_track_peak' in audio_info:
                            custom_tags['replaygain_track_peak'] = str(audio_info['replaygain_track_peak'])
                    if period:
                        custom_tags['period'] = period
                    if recording_info:
                        custom_tags['recording_information'] = recording_info
                    if area:
                        custom_tags['country'] = area
                    
                    # Get album artist for fallback
                    album_artist_name = album.get('artist', {}).get('name')
                    
                    # Use artist_names if found, otherwise fallback to album_artist, then 'Unknown Artist'
                    final_artists = artist_names if artist_names else (
                        [album_artist_name] if album_artist_name else ['Unknown Artist']
                    )
                    
                    metadata = TrackMetadata(
                        service_id=str(data['id']),
                        service_type=ServiceType.QOBUZ,
                        title=data.get('title', 'Unknown'),
                        artists=final_artists,
                        album=album.get('title', 'Unknown'),
                        album_artist=album_artist_name,
                        track_number=data.get('track_number'),
                        disc_number=data.get('media_number'),
                        duration_ms=duration_ms,
                        year=year_int,
                        genres=genres,
                        isrc=data.get('isrc'),  # Important for duplicate detection!
                        sample_rate=data.get('maximum_sampling_rate'),
                        bit_depth=data.get('maximum_bit_depth'),
                        label=album.get('label', {}).get('name'),
                        upc=album.get('upc'),
                        artwork_url=artwork_url,
                        custom_tags=custom_tags if custom_tags else None
                    )
                    
                    self._log(f"✓ Retrieved metadata: {metadata.title} by {', '.join(metadata.artists)}")
                    
                    # Enrich metadata with external sources if enabled
                    if self.enable_metadata_enrichment:
                        self._log("  Enriching metadata from external sources...")
                        # Use album_artist (clean name) instead of artists[0] (full performer string)
                        artist_for_enrichment = metadata.album_artist or (metadata.artists[0] if metadata.artists else "Unknown")
                        enriched = await enrich_metadata(
                            isrc=metadata.isrc,
                            artist=artist_for_enrichment,
                            title=metadata.title,
                            lastfm_api_key=self.lastfm_api_key
                        )
                        
                        # Merge enriched data into custom_tags
                        if not metadata.custom_tags:
                            metadata.custom_tags = {}
                        
                        # Add MusicBrainz data
                        if enriched.language:
                            metadata.custom_tags['language'] = enriched.language
                        if enriched.country and not metadata.custom_tags.get('country'):
                            metadata.custom_tags['country'] = enriched.country
                        if enriched.musicbrainz_recording_id:
                            metadata.custom_tags['musicbrainz_recording_id'] = enriched.musicbrainz_recording_id
                        
                        # Add Last.fm data
                        if enriched.mood_tags:
                            metadata.custom_tags['mood'] = enriched.mood_tags
                        if enriched.occasion_tags:
                            metadata.custom_tags['occasion'] = enriched.occasion_tags
                        if enriched.style_tags:
                            metadata.custom_tags['lastfm_style'] = enriched.style_tags
                        if enriched.lastfm_tags:
                            metadata.custom_tags['lastfm_tags'] = enriched.lastfm_tags
                        
                        # Add Spotify data
                        if enriched.bpm and not metadata.custom_tags.get('bpm'):
                            metadata.custom_tags['bpm'] = str(int(enriched.bpm))
                        if enriched.key:
                            metadata.custom_tags['key'] = enriched.key
                        if enriched.energy is not None:
                            metadata.custom_tags['energy'] = str(enriched.energy)
                        if enriched.danceability is not None:
                            metadata.custom_tags['danceability'] = str(enriched.danceability)
                        if enriched.valence is not None:
                            metadata.custom_tags['valence'] = str(enriched.valence)
                        if enriched.acousticness is not None:
                            metadata.custom_tags['acousticness'] = str(enriched.acousticness)
                        if enriched.instrumentalness is not None:
                            metadata.custom_tags['instrumentalness'] = str(enriched.instrumentalness)
                        if enriched.liveness is not None:
                            metadata.custom_tags['liveness'] = str(enriched.liveness)
                        if enriched.speechiness is not None:
                            metadata.custom_tags['speechiness'] = str(enriched.speechiness)
                        if enriched.time_signature:
                            metadata.custom_tags['time_signature'] = str(enriched.time_signature)
                        
                        self._log("  ✓ Metadata enriched")
                    
                    return metadata
                else:
                    error_text = await response.text()
                    self._log(f"Failed to get track metadata (HTTP {response.status}): {error_text}", "error")
                    return None
        
        except Exception as e:
            self._log(f"Error getting track metadata: {e}", "error")
            self.logger.exception("Failed to retrieve track metadata")
            return None
    
    async def get_album_metadata(self, album_id: str) -> Optional[AlbumMetadata]:
        """Retrieve album metadata."""
        # TODO: Implement
        self._log("get_album_metadata not yet implemented")
        return None
    
    async def get_album_tracks(self, album_id: str) -> List[TrackMetadata]:
        """Get all tracks in an album."""
        # TODO: Implement
        self._log("get_album_tracks not yet implemented")
        return []
    
    async def get_user_playlists(self) -> List[dict]:
        """
        Get all playlists for the authenticated user.
        
        Returns:
            List of playlist dicts with id, name, description, tracks_count, etc.
        """
        if not await self.is_authenticated():
            await self.authenticate()
        
        if not self.user_auth_token:
            self._log("Cannot get playlists without authentication", "error")
            return []
        
        try:
            params = {
                "user_auth_token": self.user_auth_token,
                "app_id": self.APP_ID,
                "limit": 500,
                "offset": 0,
                "order": "last_updated"
            }
            
            self._log("Fetching user playlists...")
            
            async with self.session.get(
                f"{self.BASE_URL}/playlist/getUserPlaylists",
                params=params,
                timeout=aiohttp.ClientTimeout(total=30)
            ) as response:
                if response.status == 200:
                    data = await response.json()
                    playlists = []
                    
                    for item in data.get('playlists', {}).get('items', []):
                        playlists.append({
                            'id': str(item.get('id')),
                            'name': item.get('name', 'Unknown'),
                            'description': item.get('description', ''),
                            'track_count': item.get('tracks_count', 0),
                            'owner': item.get('owner', {}).get('name', 'Unknown'),
                            'public': item.get('is_public', False),
                            'duration': item.get('duration', 0),
                            'image_url': item.get('images300', [None])[0] if item.get('images300') else None,
                        })
                    
                    self._log(f"✓ Found {len(playlists)} Qobuz playlists")
                    return playlists
                else:
                    error_text = await response.text()
                    self._log(f"Failed to get playlists (HTTP {response.status}): {error_text}", "error")
                    return []
        
        except Exception as e:
            self._log(f"Error getting playlists: {e}", "error")
            return []
    
    async def get_playlist_metadata(self, playlist_id: str) -> Optional[PlaylistMetadata]:
        """Retrieve playlist metadata."""
        if not await self.is_authenticated():
            await self.authenticate()
        
        try:
            params = {
                "playlist_id": playlist_id,
                "app_id": self.APP_ID,
            }
            
            async with self.session.get(
                f"{self.BASE_URL}/playlist/get",
                params=params,
                timeout=aiohttp.ClientTimeout(total=30)
            ) as response:
                if response.status == 200:
                    data = await response.json()
                    return PlaylistMetadata(
                        service_id=str(data.get('id')),
                        service_type=ServiceType.QOBUZ,
                        name=data.get('name', 'Unknown'),
                        description=data.get('description', ''),
                        track_count=data.get('tracks_count', 0),
                        owner=data.get('owner', {}).get('name'),
                        public=data.get('is_public', False)
                    )
                else:
                    self._log(f"Failed to get playlist metadata (HTTP {response.status})", "error")
                    return None
        except Exception as e:
            self._log(f"Error getting playlist metadata: {e}", "error")
            return None
    
    async def get_playlist_tracks(self, playlist_id: str) -> List[TrackMetadata]:
        """Get all tracks in a playlist."""
        if not await self.is_authenticated():
            await self.authenticate()
        
        try:
            all_tracks = []
            offset = 0
            limit = 100
            
            while True:
                params = {
                    "playlist_id": playlist_id,
                    "app_id": self.APP_ID,
                    "offset": offset,
                    "limit": limit,
                    "extra": "tracks"
                }
                
                async with self.session.get(
                    f"{self.BASE_URL}/playlist/get",
                    params=params,
                    timeout=aiohttp.ClientTimeout(total=30)
                ) as response:
                    if response.status != 200:
                        self._log(f"Failed to get playlist tracks (HTTP {response.status})", "error")
                        break
                    
                    data = await response.json()
                    track_items = data.get('tracks', {}).get('items', [])
                    
                    if not track_items:
                        break
                    
                    for track in track_items:
                        album = track.get('album', {})
                        album_artist = album.get('artist', {}).get('name') if album.get('artist') else None
                        
                        all_tracks.append(TrackMetadata(
                            service_id=str(track.get('id')),
                            service_type=ServiceType.QOBUZ,
                            title=track.get('title', 'Unknown'),
                            artists=[album_artist] if album_artist else ['Unknown Artist'],
                            album=album.get('title', ''),
                            album_artist=album_artist,
                            track_number=track.get('track_number'),
                            disc_number=track.get('media_number'),
                            duration_ms=track.get('duration', 0) * 1000,
                            isrc=track.get('isrc'),
                        ))
                    
                    # Check for more pages
                    total_tracks = data.get('tracks', {}).get('total', 0)
                    if offset + len(track_items) >= total_tracks:
                        break
                    offset += limit
            
            self._log(f"✓ Found {len(all_tracks)} tracks in Qobuz playlist {playlist_id}")
            return all_tracks
            
        except Exception as e:
            self._log(f"Error getting playlist tracks: {e}", "error")
            return []
    
    async def _get_download_url(self, track_id: str, format_id: int) -> Optional[str]:
        """
        Get signed download URL for a track.
        
        Args:
            track_id: Qobuz track ID
            format_id: Quality format ID (5, 7, or 27)
            
        Returns:
            Download URL or None if not available
        """
        if not self.user_auth_token:
            self._log("Cannot get download URL without authentication", "error")
            return None
        
        try:
            # Generate request signature (using streamrip's proven algorithm)
            timestamp = time.time()  # Use float timestamp like streamrip
            format_id_str = str(format_id)
            track_id_str = str(track_id)
            
            # Signature string format from streamrip (verified working)
            sign_string = f"trackgetFileUrlformat_id{format_id_str}intentstreamtrack_id{track_id_str}{timestamp}{self.APP_SECRET}"
            request_sig = hashlib.md5(sign_string.encode()).hexdigest()
            
            params = {
                "track_id": track_id_str,
                "format_id": format_id_str,
                "intent": "stream",
                "request_ts": timestamp,
                "request_sig": request_sig
            }
            
            # Add user auth token header like streamrip
            headers = {"X-User-Auth-Token": self.user_auth_token}
            
            async with self.session.get(
                f"{self.BASE_URL}/track/getFileUrl",
                params=params,
                headers=headers,
                timeout=aiohttp.ClientTimeout(total=30)
            ) as response:
                if response.status == 200:
                    data = await response.json()
                    url = data.get('url')
                    # Log actual format returned by Qobuz
                    actual_format = data.get('format_id')
                    bit_depth = data.get('bit_depth')
                    sample_rate = data.get('sampling_rate')
                    print(f"[QOBUZ RESPONSE] Requested format_id={format_id}, Got format_id={actual_format}, {bit_depth}bit/{sample_rate}kHz", file=__import__('sys').stderr)
                    if url:
                        self._log(f"✓ Got download URL (format: {format_id})")
                        return url
                    else:
                        self._log("Response missing download URL", "error")
                        return None
                else:
                    error_data = await response.json()
                    error_msg = error_data.get('message', 'Unknown error')
                    self._log(f"Failed to get download URL (HTTP {response.status}): {error_msg}", "error")
                    return None
        
        except Exception as e:
            self._log(f"Error getting download URL: {e}", "error")
            return None
    
    async def download_track(
        self, 
        track_id: str, 
        output_path: str,
        quality: DownloadQuality = DownloadQuality.LOSSLESS_CD,
        audio_config: Optional['AudioQualityConfig'] = None,
        metadata_config: Optional['MetadataTagConfig'] = None,
        progress_callback: Optional[Callable[[int, int], None]] = None,
        apple_music_token: Optional[str] = None
    ) -> DownloadResult:
        """
        Download a track from Qobuz.
        
        Args:
            track_id: Qobuz track identifier
            output_path: Destination file path
            quality: Desired download quality
            audio_config: Optional audio quality configuration
            metadata_config: Optional metadata tag configuration
            progress_callback: Optional callback(bytes_downloaded, total_bytes)
            
        Returns:
            DownloadResult with status and metadata
        """
        if not await self.is_authenticated():
            await self.authenticate()
        
        start_time = time.time()
        
        try:
            # Step 1: Get track metadata
            self._log(f"Downloading track {track_id}...")
            metadata = await self.get_track_metadata(track_id)
            
            if not metadata:
                return DownloadResult(
                    success=False,
                    error_message="Failed to retrieve track metadata"
                )
            
            # Step 2: Get download URL with signed request
            format_id = self.QUALITY_MAP.get(quality, 7)  # Default to CD quality
            print(f"[QOBUZ DEBUG] Requesting quality={quality}, format_id={format_id} (5=MP3, 7=CD 16bit, 27=HiRes 24bit)", file=__import__('sys').stderr)
            download_url = await self._get_download_url(track_id, format_id)
            
            if not download_url:
                return DownloadResult(
                    success=False,
                    error_message="Failed to get download URL - check subscription tier"
                )
            
            # Step 3: Download file with progress tracking
            output_file = Path(output_path)
            output_file.parent.mkdir(parents=True, exist_ok=True)
            
            self._log(f"  Downloading to: {output_path}")
            
            async with self.session.get(download_url) as response:
                if response.status != 200:
                    return DownloadResult(
                        success=False,
                        error_message=f"Download failed with HTTP {response.status}"
                    )
                
                total_size = int(response.headers.get('content-length', 0))
                downloaded = 0
                
                with open(output_path, 'wb') as f:
                    async for chunk in response.content.iter_chunked(8192):
                        f.write(chunk)
                        downloaded += len(chunk)
                        
                        if progress_callback and total_size > 0:
                            progress_callback(downloaded, total_size)
            
            duration = time.time() - start_time
            
            # Step 4: Download and embed album artwork
            if metadata.artwork_url:
                try:
                    self._log("  Downloading album artwork...")
                    async with self.session.get(metadata.artwork_url) as img_response:
                        if img_response.status == 200:
                            from mutagen.flac import Picture
                            import base64
                            
                            image_data = await img_response.read()
                            
                            # Determine image format from URL or content-type
                            content_type = img_response.headers.get('Content-Type', 'image/jpeg')
                            if 'png' in content_type:
                                mime_type = 'image/png'
                            else:
                                mime_type = 'image/jpeg'
                            
                            # Store for later embedding
                            artwork_data = (image_data, mime_type)
                            self._log(f"  ✓ Downloaded artwork ({len(image_data)} bytes)")
                        else:
                            artwork_data = None
                            self._log(f"  Warning: Failed to download artwork (HTTP {img_response.status})", "warning")
                except Exception as e:
                    artwork_data = None
                    self._log(f"  Warning: Could not download artwork: {e}", "warning")
            else:
                artwork_data = None
            
            # Step 4: Detect BPM from audio file
            try:
                bpm = self._detect_bpm(output_path)
                if bpm:
                    if not metadata.custom_tags:
                        metadata.custom_tags = {}
                    metadata.custom_tags['bpm'] = str(int(round(bpm)))
                    self._log(f"  ✓ BPM detected: {int(round(bpm))}")
            except Exception as e:
                self._log(f"  Warning: BPM detection failed: {e}", "warning")
            
            # Step 5: Embed metadata tags conditionally based on config
            # Use retry logic to handle transient failures
            metadata_embedded = False
            last_embed_error = None
            for embed_attempt in range(3):  # Retry up to 3 times
                try:
                    self._log("  Embedding metadata..." if embed_attempt == 0 else f"  Retrying metadata embed (attempt {embed_attempt + 1})...")
                    self._embed_metadata_conditional(
                        audio_file_path=output_path,
                        metadata=metadata,
                        metadata_config=metadata_config,
                        artwork_data=artwork_data
                    )
                    metadata_embedded = True
                    break  # Success, exit retry loop
                except Exception as e:
                    last_embed_error = str(e)
                    self._log(f"  Warning: Metadata embed attempt {embed_attempt + 1} failed: {e}", "warning")
                    if embed_attempt < 2:  # Wait before retry (except on last attempt)
                        await asyncio.sleep(0.5 * (embed_attempt + 1))  # 0.5s, 1s backoff
            
            if not metadata_embedded:
                self._log(f"  ERROR: All metadata embedding attempts failed: {last_embed_error}", "error")
                # Don't fail the download, but record this issue
                import sys
                print(f"[METADATA ERROR] Failed to embed metadata for: {output_path} - {last_embed_error}", file=sys.stderr)
            
            # Step 6: Fetch and save synced lyrics (optional)
            try:
                lyrics_service = LyricsService(
                    apple_music_token=apple_music_token,
                    verbose=self.verbose
                )
                duration_seconds = int(metadata.duration_ms / 1000) if metadata.duration_ms else None
                
                lyrics_result = await lyrics_service.get_lyrics(
                    track_name=metadata.title,
                    artist_name=metadata.artists[0] if metadata.artists else "",
                    album_name=metadata.album,
                    duration_seconds=duration_seconds
                )
                
                # Save synced lyrics as .lrc file
                if lyrics_result.synced_lyrics:
                    lrc_path = LyricsService.save_lrc_file(lyrics_result.synced_lyrics, output_path)
                    if lrc_path:
                        source_info = f" (from {lyrics_result.source})"
                        word_info = " [word-synced]" if lyrics_result.word_synced else ""
                        self._log(f"  ✓ Saved synced lyrics: {Path(lrc_path).name}{source_info}{word_info}")
                # Fallback to plain lyrics as .txt if no synced available
                elif lyrics_result.plain_lyrics:
                    txt_path = LyricsService.save_txt_file(lyrics_result.plain_lyrics, output_path)
                    if txt_path:
                        self._log(f"  ✓ Saved plain lyrics: {Path(txt_path).name} (from {lyrics_result.source})")
                elif lyrics_result.instrumental:
                    self._log(f"  ℹ Track is instrumental (no lyrics)")
                else:
                    self._log(f"  ℹ No lyrics found")
                
                await lyrics_service.close()
            except Exception as e:
                self._log(f"  Warning: Could not fetch lyrics: {e}", "warning")
            
            # Step 7: Calculate file hash for duplicate detection
            file_hash = None
            try:
                with open(output_path, 'rb') as f:
                    sha256 = hashlib.sha256()
                    while chunk := f.read(8192):
                        sha256.update(chunk)
                    file_hash = sha256.hexdigest()
            except Exception as e:
                self._log(f"Warning: Could not calculate file hash: {e}", "warning")
            
            file_size = output_file.stat().st_size
            
            self._log(f"✓ Download complete ({file_size / 1024 / 1024:.2f} MB in {duration:.1f}s)")
            
            return DownloadResult(
                success=True,
                track_metadata=metadata,
                filepath=str(output_path),
                file_size_bytes=file_size,
                download_duration_seconds=duration,
                status=DownloadStatus.COMPLETED
            )
        
        except Exception as e:
            self._log(f"Download error: {e}", "error")
            self.logger.exception("Download failed")
            return DownloadResult(
                success=False,
                error_message=str(e)
            )
    
    async def get_available_qualities(self, track_id: str) -> List[DownloadQuality]:
        """
        Get available quality options for a track.
        
        Args:
            track_id: Qobuz track identifier
            
        Returns:
            List of available quality levels
        """
        # TODO: Implement - check user subscription level
        # Free tier: up to 320kbps MP3
        # Hi-Fi tier: up to CD quality
        # Studio tier: up to Hi-Res
        
        # For now, return all qualities
        return [
            DownloadQuality.LOSSY_STANDARD,
            DownloadQuality.LOSSLESS_CD,
            DownloadQuality.LOSSLESS_HIRES
        ]
    
    @property
    def service_name(self) -> str:
        """Human-readable service name."""
        return "Qobuz"
    
    @property
    def service_type(self) -> ServiceType:
        """Service type enum."""
        return ServiceType.QOBUZ
    
    @property
    def supports_lossless(self) -> bool:
        """Whether service supports lossless audio."""
        return True
    
    async def close(self):
        """Close HTTP session."""
        if self.session:
            await self.session.close()
            self.session = None
            self._log("Session closed")


# Example usage and testing
async def test_qobuz_service():
    """Test the Qobuz service implementation."""
    import os
    from dotenv import load_dotenv
    
    load_dotenv()
    
    credentials = ServiceCredentials(
        service_type=ServiceType.QOBUZ,
        username=os.getenv('QOBUZ_EMAIL'),
        password=os.getenv('QOBUZ_PASSWORD')
    )
    
    service = QobuzService(credentials, verbose=True)
    
    print("\n=== Testing Qobuz Service ===\n")
    
    # Test authentication
    print("1. Testing authentication...")
    auth_success = await service.authenticate()
    print(f"   Result: {'✓ SUCCESS' if auth_success else '✗ FAILED'}\n")
    
    if auth_success:
        # Test track metadata (example track ID)
        print("2. Testing track metadata retrieval...")
        # TODO: Use a real Qobuz track ID
        # metadata = await service.get_track_metadata("123456")
        # print(f"   Result: {metadata}\n")
        print("   (Skipped - need real track ID)\n")
        
        # Test available qualities
        print("3. Testing available qualities...")
        qualities = await service.get_available_qualities("123456")
        print(f"   Available: {[q.value for q in qualities]}\n")
    
    await service.close()
    print("=== Test Complete ===\n")


if __name__ == "__main__":
    # Run test if executed directly
    asyncio.run(test_qobuz_service())

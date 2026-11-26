"""
Qobuz service integration.
Based on QobuzDownloaderX-MOD architecture.
"""

import asyncio
import aiohttp
import hashlib
import os
from typing import List, Optional
import logging


from services.service_base import (
    MusicService, ServiceType, ServiceCredentials, TrackMetadata,
    AlbumMetadata, PlaylistMetadata, SearchResult, DownloadQuality
)
from services.metadata_enrichment import enrich_metadata, EnrichedMetadata


class QobuzService(MusicService):
    """
    Qobuz music service implementation.
    
    Authentication: Username/password → User auth token
    API Base: https://www.qobuz.com/api.json/0.2

    Este módulo conserva auth, búsqueda y metadatos. Los caminos de descarga
    (`track/getFileUrl` y el volcado a disco) se retiraron en D14 (#126): la
    descarga de Qobuz es nativa en Rust (`src-tauri/src/download/qobuz.rs`).
    La resolución de credenciales y el bundle público se mantienen porque
    fijan el contrato de auth que comparten los tests de seguridad.
    """
    
    # API Configuration
    BASE_URL = "https://www.qobuz.com/api.json/0.2"
    
    # Public Qobuz application id. It is a client identifier, not a secret, and
    # its presence in the tree is an accepted, documented risk
    # (docs/Deuda_Tecnica_y_UX.md). Override it per deployment with the
    # operator credentials or the QOBUZ_APP_ID environment variable.
    APP_ID = "798273057"
    # PUBLIC secret of the Qobuz API bundle: the same app_id/secret pair
    # that open-source clients ship and that the API itself requires (without it
    # no signature is possible). It is NOT anyone's personal secret and must NOT be
    # emptied again under the assumption that it is (that already happened twice).
    # The user's PERSONAL credentials (token, login) go through the keychain/accounts;
    # operator credentials and the env take priority over this value.
    APP_SECRET = "abb21364945c0583309667d13ca3d93a"
    
    
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
        self.subscription_credential: dict = {}
        self.logger = logging.getLogger(__name__)

        # Instance-level app credentials: the values supplied by the operator
        # win over the class placeholders (SYNC-AUD-062).
        self.APP_ID = self._resolve_app_id(credentials)
        self.APP_SECRET = self._resolve_app_secret(credentials)

        # Metadata enrichment configuration
        self.enable_metadata_enrichment = enable_metadata_enrichment
        self.lastfm_api_key = lastfm_api_key

    @staticmethod
    def _first_credential_value(credentials: ServiceCredentials, *keys: str) -> Optional[str]:
        """Return the first non-empty string among the credential aliases."""
        extra = getattr(credentials, "extra", None) or {}
        for key in keys:
            for candidate in (getattr(credentials, key, None), extra.get(key)):
                if isinstance(candidate, str) and candidate.strip():
                    return candidate.strip()
        return None

    @classmethod
    def _resolve_app_id(cls, credentials: ServiceCredentials) -> str:
        """Resolve the Qobuz app id: operator credentials, then env, then default."""
        return (
            cls._first_credential_value(credentials, "app_id", "client_id")
            or (os.getenv("QOBUZ_APP_ID") or "").strip()
            or cls.APP_ID
        )

    @classmethod
    def _resolve_app_secret(cls, credentials: ServiceCredentials) -> str:
        """Resolve the Qobuz app secret: operator credentials, then env, then the public bundle."""
        return (
            cls._first_credential_value(credentials, "app_secret", "client_secret")
            or (os.getenv("QOBUZ_APP_SECRET") or "").strip()
            or cls.APP_SECRET
        ).strip()


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
                    self.subscription_credential = user_data.get('credential') or {}
                    
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
    
    async def _get_album_payload(self, album_id: str) -> Optional[dict]:
        """Fetch one album payload from Qobuz without exposing transport details."""
        if not await self.is_authenticated() and not await self.authenticate():
            return None
        if not self.session:
            return None

        try:
            async with self.session.get(
                f"{self.BASE_URL}/album/get",
                params={"album_id": album_id, "app_id": self.APP_ID},
                timeout=aiohttp.ClientTimeout(total=30),
            ) as response:
                if response.status == 200:
                    return await response.json()
                self._log(
                    f"Failed to get album {album_id} (HTTP {response.status}): "
                    f"{await response.text()}",
                    "error",
                )
        except Exception as exc:
            self._log(f"Error getting album {album_id}: {exc}", "error")
            self.logger.exception("Failed to retrieve album")
        return None

    async def get_album_metadata(self, album_id: str) -> Optional[AlbumMetadata]:
        """Retrieve normalized album metadata."""
        data = await self._get_album_payload(album_id)
        if not data:
            return None

        artist = data.get("artist") or {}
        artist_name = artist.get("name") or "Unknown Artist"
        release_date = data.get("release_date_original") or data.get("released_at")
        image = data.get("image") or {}
        genre = data.get("genre") or {}
        year = None
        if release_date:
            try:
                year = int(str(release_date)[:4])
            except ValueError:
                pass

        return AlbumMetadata(
            service_id=str(data.get("id", album_id)),
            service_type=ServiceType.QOBUZ,
            title=data.get("title") or "Unknown Album",
            artist=artist_name,
            artists=[artist_name],
            release_date=release_date,
            year=year,
            label=(data.get("label") or {}).get("name"),
            genres=[genre["name"]] if genre.get("name") else [],
            track_count=data.get("tracks_count"),
            artwork_url=image.get("large") or image.get("extralarge"),
            upc=data.get("upc"),
        )

    async def get_album_tracks(self, album_id: str) -> List[TrackMetadata]:
        """Get every track in an album using the canonical track normalizer."""
        data = await self._get_album_payload(album_id)
        items = ((data or {}).get("tracks") or {}).get("items") or []
        tracks: List[TrackMetadata] = []
        for item in items:
            track_id = item.get("id")
            if track_id is None:
                continue
            metadata = await self.get_track_metadata(str(track_id))
            if metadata is not None:
                tracks.append(metadata)
        return tracks
    
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
    
    async def get_available_qualities(self, track_id: str) -> List[DownloadQuality]:
        """
        Get available quality options for a track.
        
        Args:
            track_id: Qobuz track identifier
            
        Returns:
            List of available quality levels
        """
        if not await self.is_authenticated() and not await self.authenticate():
            return []

        credential = self.subscription_credential
        qualities = [DownloadQuality.LOSSY_STANDARD]
        if credential.get("lossless_streaming"):
            qualities.append(DownloadQuality.LOSSLESS_CD)
        if credential.get("hires_streaming") or credential.get("hires_purchases_streaming"):
            qualities.extend([
                DownloadQuality.LOSSLESS_HIRES_96,
                DownloadQuality.LOSSLESS_HIRES,
            ])
        return qualities
    
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

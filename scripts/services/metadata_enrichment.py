"""
Metadata enrichment services for gathering additional metadata from external sources.
"""
import aiohttp
import asyncio
from typing import Optional, List, Dict, Any
from dataclasses import dataclass
import logging


@dataclass
class EnrichedMetadata:
    """Additional metadata from external sources."""
    # MusicBrainz data
    language: Optional[str] = None
    country: Optional[str] = None
    recording_location: Optional[str] = None
    musicbrainz_recording_id: Optional[str] = None
    musicbrainz_release_id: Optional[str] = None
    
    # Last.fm data
    mood_tags: Optional[List[str]] = None
    occasion_tags: Optional[List[str]] = None
    style_tags: Optional[List[str]] = None
    lastfm_tags: Optional[List[str]] = None
    
    # Spotify data
    bpm: Optional[float] = None
    key: Optional[str] = None
    musical_key: Optional[int] = None
    mode: Optional[int] = None  # 0 = minor, 1 = major
    time_signature: Optional[int] = None
    energy: Optional[float] = None
    danceability: Optional[float] = None
    valence: Optional[float] = None  # musical positiveness
    acousticness: Optional[float] = None
    instrumentalness: Optional[float] = None
    speechiness: Optional[float] = None
    liveness: Optional[float] = None
    loudness: Optional[float] = None
    spotify_popularity: Optional[int] = None


class MusicBrainzEnricher:
    """Enrich metadata using MusicBrainz API."""
    
    BASE_URL = "https://musicbrainz.org/ws/2"
    
    def __init__(self, app_name: str = "Syncify", app_version: str = "1.0", contact: str = ""):
        self.logger = logging.getLogger(__name__)
        self.headers = {
            "User-Agent": f"{app_name}/{app_version} ( {contact} )"
        }
        self.session: Optional[aiohttp.ClientSession] = None
    
    async def __aenter__(self):
        self.session = aiohttp.ClientSession(headers=self.headers)
        return self
    
    async def __aexit__(self, exc_type, exc_val, exc_tb):
        if self.session:
            await self.session.close()
    
    async def query_by_isrc(self, isrc: str) -> Optional[Dict[str, Any]]:
        """Query MusicBrainz by ISRC code."""
        if not self.session:
            self.session = aiohttp.ClientSession(headers=self.headers)
        
        try:
            params = {
                "query": f"isrc:{isrc}",
                "fmt": "json"
            }
            
            async with self.session.get(
                f"{self.BASE_URL}/recording",
                params=params,
                timeout=aiohttp.ClientTimeout(total=10)
            ) as response:
                if response.status == 200:
                    data = await response.json()
                    recordings = data.get('recordings', [])
                    if recordings:
                        recording = recordings[0]  # Take first match
                        
                        # Get additional release information
                        recording_id = recording.get('id')
                        releases = recording.get('releases', [])
                        
                        result = {
                            'recording_id': recording_id,
                            'title': recording.get('title'),
                            'length': recording.get('length'),
                            'releases': []
                        }
                        
                        # Extract data from releases
                        for release in releases:
                            release_info = {
                                'id': release.get('id'),
                                'title': release.get('title'),
                                'country': release.get('country'),
                                'date': release.get('date'),
                                'status': release.get('status')
                            }
                            
                            # Get text representation (language)
                            text_rep = release.get('text-representation', {})
                            if text_rep:
                                release_info['language'] = text_rep.get('language')
                                release_info['script'] = text_rep.get('script')
                            
                            result['releases'].append(release_info)
                        
                        return result
                
                elif response.status == 503:
                    self.logger.warning("MusicBrainz rate limit hit, waiting...")
                    await asyncio.sleep(1)
                    return None
                else:
                    self.logger.warning(f"MusicBrainz query failed: HTTP {response.status}")
                    return None
                    
        except asyncio.TimeoutError:
            self.logger.warning("MusicBrainz query timeout")
            return None
        except Exception as e:
            self.logger.error(f"MusicBrainz query error: {e}")
            return None
    
    async def get_recording_details(self, recording_id: str) -> Optional[Dict[str, Any]]:
        """Get detailed recording information including relationships."""
        if not self.session:
            self.session = aiohttp.ClientSession(headers=self.headers)
        
        try:
            params = {
                "inc": "artist-credits+releases+recordings+tags",
                "fmt": "json"
            }
            
            async with self.session.get(
                f"{self.BASE_URL}/recording/{recording_id}",
                params=params,
                timeout=aiohttp.ClientTimeout(total=10)
            ) as response:
                if response.status == 200:
                    return await response.json()
                elif response.status == 503:
                    await asyncio.sleep(1)
                    return None
                else:
                    return None
                    
        except Exception as e:
            self.logger.error(f"MusicBrainz recording details error: {e}")
            return None


class LastFmEnricher:
    """Enrich metadata using Last.fm API."""
    
    BASE_URL = "https://ws.audioscrobbler.com/2.0/"
    
    def __init__(self, api_key: str):
        self.api_key = api_key
        self.logger = logging.getLogger(__name__)
        self.session: Optional[aiohttp.ClientSession] = None
    
    async def __aenter__(self):
        self.session = aiohttp.ClientSession()
        return self
    
    async def __aexit__(self, exc_type, exc_val, exc_tb):
        if self.session:
            await self.session.close()
    
    async def get_track_tags(self, artist: str, track: str) -> Optional[Dict[str, Any]]:
        """Get track tags from Last.fm."""
        if not self.session:
            self.session = aiohttp.ClientSession()
        
        try:
            params = {
                "method": "track.getTopTags",
                "artist": artist,
                "track": track,
                "api_key": self.api_key,
                "format": "json"
            }
            
            self.logger.debug(f"Querying Last.fm for: {artist} - {track}")
            
            async with self.session.get(
                self.BASE_URL,
                params=params,
                timeout=aiohttp.ClientTimeout(total=10)
            ) as response:
                if response.status == 200:
                    data = await response.json()
                    self.logger.debug(f"Last.fm response: {data}")
                    tags = data.get('toptags', {}).get('tag', [])
                    
                    # Categorize tags - Expanded mood and occasion keywords
                    mood_keywords = [
                        # Basic emotions
                        'happy', 'sad', 'melancholic', 'cheerful', 'joyful', 'depressing', 'somber',
                        'emotional', 'sentimental', 'nostalgic', 'bittersweet', 'hopeful', 'optimistic',
                        'pessimistic', 'anxious', 'lonely', 'melancholy', 'euphoric', 'blissful',
                        # Energy levels
                        'energetic', 'upbeat', 'lively', 'dynamic', 'powerful', 'explosive', 'intense',
                        'aggressive', 'angry', 'furious', 'rage', 'violent', 'brutal', 'fierce',
                        'calm', 'peaceful', 'serene', 'tranquil', 'soothing', 'gentle', 'soft',
                        'relaxing', 'mellow', 'laid-back', 'easygoing', 'smooth', 'chill',
                        # Atmosphere
                        'dark', 'gloomy', 'mysterious', 'haunting', 'eerie', 'ominous', 'sinister',
                        'bright', 'uplifting', 'inspiring', 'motivating', 'empowering', 'triumphant',
                        'dreamy', 'ethereal', 'atmospheric', 'ambient', 'hypnotic', 'trippy',
                        'romantic', 'sensual', 'sexy', 'passionate', 'tender', 'loving', 'intimate',
                        'epic', 'dramatic', 'cinematic', 'majestic', 'grandiose', 'theatrical',
                        'fun', 'playful', 'silly', 'quirky', 'humorous', 'lighthearted', 'carefree',
                        'introspective', 'contemplative', 'meditative', 'reflective', 'thoughtful',
                        'rebellious', 'defiant', 'provocative', 'edgy', 'raw', 'gritty'
                    ]
                    
                    occasion_keywords = [
                        # Activities
                        'party', 'partying', 'clubbing', 'rave', 'festival', 'celebration', 'dancing',
                        'workout', 'exercise', 'running', 'jogging', 'gym', 'cardio', 'training',
                        'driving', 'road trip', 'cruising', 'commute', 'travel', 'journey',
                        'study', 'studying', 'focus', 'concentration', 'work', 'working', 'productive',
                        'sleep', 'sleeping', 'bedtime', 'insomnia', 'lullaby', 'rest',
                        'chill', 'chilling', 'hanging out', 'relax', 'relaxing', 'lounge', 'unwind',
                        'cooking', 'dinner', 'breakfast', 'brunch', 'dining',
                        'shower', 'bath', 'getting ready', 'makeup',
                        'gaming', 'video games', 'reading', 'writing',
                        # Times of day
                        'morning', 'wake up', 'sunrise', 'dawn', 'breakfast time',
                        'afternoon', 'midday', 'lunch', 'daytime',
                        'evening', 'sunset', 'dusk', 'dinner time',
                        'night', 'nighttime', 'late night', 'midnight', 'nocturnal',
                        # Seasons & weather
                        'summer', 'beach', 'sunshine', 'tropical', 'hot',
                        'winter', 'cold', 'snow', 'christmas', 'holiday',
                        'spring', 'autumn', 'fall', 'rainy', 'rain',
                        # Social contexts
                        'romantic', 'date night', 'wedding', 'love', 'romance',
                        'sad', 'breakup', 'heartbreak', 'crying', 'grief',
                        'meditation', 'yoga', 'spa', 'massage', 'zen',
                        'background', 'ambient', 'background music', 'instrumental',
                        # Special occasions
                        'halloween', 'spooky', 'scary', 'horror',
                        'birthday', 'anniversary', 'graduation', 'farewell'
                    ]
                    
                    mood_tags = []
                    occasion_tags = []
                    style_tags = []
                    all_tags = []
                    
                    for tag_obj in tags:
                        if isinstance(tag_obj, dict):
                            tag_name = tag_obj.get('name', '').lower()
                            all_tags.append(tag_name)
                            
                            if any(keyword in tag_name for keyword in mood_keywords):
                                mood_tags.append(tag_name)
                            elif any(keyword in tag_name for keyword in occasion_keywords):
                                occasion_tags.append(tag_name)
                            else:
                                style_tags.append(tag_name)
                    
                    return {
                        'mood': mood_tags,
                        'occasion': occasion_tags,
                        'style': style_tags,
                        'all': all_tags
                    }
                else:
                    self.logger.warning(f"Last.fm query failed: HTTP {response.status}")
                    return None
                    
        except asyncio.TimeoutError:
            self.logger.warning("Last.fm query timeout")
            return None
        except Exception as e:
            self.logger.error(f"Last.fm query error: {e}")
            return None


class SpotifyEnricher:
    """Enrich metadata using Spotify Web API."""
    
    BASE_URL = "https://api.spotify.com/v1"
    AUTH_URL = "https://accounts.spotify.com/api/token"
    
    def __init__(self, client_id: str, client_secret: str):
        self.client_id = client_id
        self.client_secret = client_secret
        self.logger = logging.getLogger(__name__)
        self.session: Optional[aiohttp.ClientSession] = None
        self.access_token: Optional[str] = None
    
    async def __aenter__(self):
        self.session = aiohttp.ClientSession()
        await self.authenticate()
        return self
    
    async def __aexit__(self, exc_type, exc_val, exc_tb):
        if self.session:
            await self.session.close()
    
    async def authenticate(self):
        """Get access token using client credentials flow."""
        if not self.session:
            self.session = aiohttp.ClientSession()
        
        try:
            import base64
            
            auth_str = f"{self.client_id}:{self.client_secret}"
            auth_bytes = auth_str.encode('ascii')
            auth_b64 = base64.b64encode(auth_bytes).decode('ascii')
            
            headers = {
                "Authorization": f"Basic {auth_b64}",
                "Content-Type": "application/x-www-form-urlencoded"
            }
            
            data = {"grant_type": "client_credentials"}
            
            async with self.session.post(
                self.AUTH_URL,
                headers=headers,
                data=data,
                timeout=aiohttp.ClientTimeout(total=10)
            ) as response:
                if response.status == 200:
                    result = await response.json()
                    self.access_token = result.get('access_token')
                    return True
                else:
                    self.logger.error(f"Spotify auth failed: HTTP {response.status}")
                    return False
                    
        except Exception as e:
            self.logger.error(f"Spotify authentication error: {e}")
            return False
    
    async def search_by_isrc(self, isrc: str) -> Optional[str]:
        """Search for track by ISRC and return Spotify track ID."""
        if not self.access_token:
            await self.authenticate()
        
        if not self.access_token:
            self.logger.error("No Spotify access token available")
            return None
        
        try:
            headers = {"Authorization": f"Bearer {self.access_token}"}
            params = {
                "q": f"isrc:{isrc}",
                "type": "track",
                "limit": 1
            }
            
            self.logger.debug(f"Searching Spotify for ISRC: {isrc}")
            
            async with self.session.get(
                f"{self.BASE_URL}/search",
                headers=headers,
                params=params,
                timeout=aiohttp.ClientTimeout(total=10)
            ) as response:
                if response.status == 200:
                    data = await response.json()
                    self.logger.debug(f"Spotify search response: {data}")
                    tracks = data.get('tracks', {}).get('items', [])
                    if tracks:
                        self.logger.info(f"Found Spotify track: {tracks[0]['id']}")
                        return tracks[0]['id']
                    else:
                        self.logger.warning(f"No Spotify track found for ISRC: {isrc}")
                    return None
                elif response.status == 401:
                    # Token expired, re-authenticate
                    self.logger.info("Spotify token expired, re-authenticating")
                    await self.authenticate()
                    return await self.search_by_isrc(isrc)
                else:
                    self.logger.warning(f"Spotify search failed: HTTP {response.status}")
                    return None
                    
        except Exception as e:
            self.logger.error(f"Spotify search error: {e}")
            return None
    
    async def get_audio_features(self, track_id: str) -> Optional[Dict[str, Any]]:
        """Get audio features for a track."""
        if not self.access_token:
            await self.authenticate()
        
        if not self.access_token:
            return None
        
        try:
            headers = {"Authorization": f"Bearer {self.access_token}"}
            
            self.logger.debug(f"Getting audio features for track: {track_id}")
            
            async with self.session.get(
                f"{self.BASE_URL}/audio-features/{track_id}",
                headers=headers,
                timeout=aiohttp.ClientTimeout(total=10)
            ) as response:
                if response.status == 200:
                    data = await response.json()
                    self.logger.debug(f"Audio features response: {data}")
                    return data
                elif response.status == 401:
                    self.logger.info("Audio features: Token expired, re-authenticating")
                    await self.authenticate()
                    return await self.get_audio_features(track_id)
                else:
                    self.logger.warning(f"Audio features failed: HTTP {response.status}")
                    return None
                    
        except Exception as e:
            self.logger.error(f"Spotify audio features error: {e}")
            return None
    
    async def get_track_info(self, track_id: str) -> Optional[Dict[str, Any]]:
        """Get track information including popularity."""
        if not self.access_token:
            await self.authenticate()
        
        if not self.access_token:
            return None
        
        try:
            headers = {"Authorization": f"Bearer {self.access_token}"}
            
            self.logger.debug(f"Getting track info for: {track_id}")
            
            async with self.session.get(
                f"{self.BASE_URL}/tracks/{track_id}",
                headers=headers,
                timeout=aiohttp.ClientTimeout(total=10)
            ) as response:
                if response.status == 200:
                    data = await response.json()
                    self.logger.debug(f"Track info response: {data.get('name')} - popularity: {data.get('popularity')}")
                    return data
                elif response.status == 401:
                    self.logger.info("Track info: Token expired, re-authenticating")
                    await self.authenticate()
                    return await self.get_track_info(track_id)
                else:
                    self.logger.warning(f"Track info failed: HTTP {response.status}")
                    return None
                    
        except Exception as e:
            self.logger.error(f"Spotify track info error: {e}")
            return None


async def enrich_metadata(
    isrc: Optional[str],
    artist: str,
    title: str,
    lastfm_api_key: Optional[str] = None
) -> EnrichedMetadata:
    """
    Enrich metadata using MusicBrainz and Last.fm.
    
    Args:
        isrc: ISRC code for the track
        artist: Artist name
        title: Track title
        lastfm_api_key: Last.fm API key (optional)
    
    Returns:
        EnrichedMetadata object with additional metadata
    """
    enriched = EnrichedMetadata()
    
    # Query MusicBrainz by ISRC
    if isrc:
        try:
            async with MusicBrainzEnricher() as mb:
                mb_data = await mb.query_by_isrc(isrc)
                if mb_data:
                    enriched.musicbrainz_recording_id = mb_data.get('recording_id')
                    
                    # Get language and country from first release
                    releases = mb_data.get('releases', [])
                    if releases:
                        first_release = releases[0]
                        enriched.country = first_release.get('country')
                        enriched.language = first_release.get('language')
                        enriched.musicbrainz_release_id = first_release.get('id')
        except Exception as e:
            logging.error(f"MusicBrainz enrichment failed: {e}")
    
    # Query Last.fm for tags
    if lastfm_api_key:
        try:
            async with LastFmEnricher(lastfm_api_key) as lastfm:
                tags_data = await lastfm.get_track_tags(artist, title)
                if tags_data:
                    enriched.mood_tags = tags_data.get('mood', [])
                    enriched.occasion_tags = tags_data.get('occasion', [])
                    enriched.style_tags = tags_data.get('style', [])
                    enriched.lastfm_tags = tags_data.get('all', [])
        except Exception as e:
            logging.error(f"Last.fm enrichment failed: {e}")
    
    return enriched

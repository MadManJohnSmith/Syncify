"""
Lyrics Service - Multi-source lyrics fetching with Apple Music word-sync support.

Sources (in priority order):
1. Apple Music - Syllable-level word-synced (requires media-user-token)
2. syncedlyrics (Musixmatch/Lrclib/NetEase) - Line-synced fallback
"""

import asyncio
import re
import json
from typing import Optional, Dict, Any
from pathlib import Path
import logging
import sys
import time
import random
import requests

try:
    import syncedlyrics
    SYNCEDLYRICS_AVAILABLE = True
except ImportError:
    syncedlyrics = None  # type: ignore
    SYNCEDLYRICS_AVAILABLE = False

logger = logging.getLogger(__name__)


class LyricsResult:
    """Result from lyrics fetch with metadata."""
    def __init__(
        self,
        synced_lyrics: Optional[str] = None,
        plain_lyrics: Optional[str] = None,
        word_synced: bool = False,
        instrumental: bool = False,
        source: str = "unknown"
    ):
        self.synced_lyrics = synced_lyrics
        self.plain_lyrics = plain_lyrics
        self.word_synced = word_synced
        self.instrumental = instrumental
        self.source = source
    
    @property
    def has_lyrics(self) -> bool:
        return bool(self.synced_lyrics or self.plain_lyrics)
    
    def to_dict(self) -> Dict[str, Any]:
        return {
            "synced_lyrics": self.synced_lyrics,
            "plain_lyrics": self.plain_lyrics,
            "word_synced": self.word_synced,
            "instrumental": self.instrumental,
            "source": self.source
        }


# Global cache for Apple Music access token (avoids refetching on every download)
_APPLE_MUSIC_TOKEN_CACHE = {
    "access_token": None,
    "expires_at": 0  # Token valid for ~1 hour
}


class AppleMusicLyricsProvider:
    """Fetch syllable-synced lyrics from Apple Music using dynamic token."""
    
    HEADERS = {
        'content-type': 'application/json;charset=utf-8',
        'connection': 'keep-alive',
        'accept': 'application/json',
        'origin': 'https://music.apple.com',
        'referer': 'https://music.apple.com/',
        'accept-encoding': 'gzip, deflate, br',
        'user-agent': 'Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36'
    }
    
    def __init__(self, media_user_token: Optional[str] = None, verbose: bool = False):
        self.media_user_token = media_user_token
        self.verbose = verbose
        self._session = requests.Session()
        self._session.headers.update(self.HEADERS)
        self._access_token = None
        self._storefront = "us"
        self._language = "en-US"
    
    def _log(self, message: str):
        if self.verbose:
            print(f"[Apple Music] {message}", file=sys.stderr)
    
    def _fetch_access_token(self) -> Optional[str]:
        """Fetch access token from Apple Music web page (with caching)."""
        global _APPLE_MUSIC_TOKEN_CACHE
        
        # Check cache first (token valid for ~1 hour)
        current_time = time.time()
        if (_APPLE_MUSIC_TOKEN_CACHE["access_token"] and 
            _APPLE_MUSIC_TOKEN_CACHE["expires_at"] > current_time):
            self._log("Using cached access token")
            return _APPLE_MUSIC_TOKEN_CACHE["access_token"]
        
        try:
            self._log("Fetching access token from web...")
            
            response = requests.get('https://music.apple.com/us/browse', timeout=10)
            if response.status_code != 200:
                self._log(f"Failed to get music.apple.com: {response.status_code}")
                return None
            
            # Find the JS bundle URL
            match = re.search(r'(?<=index)(.*?)(?=\.js")', response.text)
            if not match:
                self._log("Failed to find JS bundle")
                return None
            
            index_js = match.group(1)
            response = requests.get(f'https://music.apple.com/assets/index{index_js}.js', timeout=10)
            if response.status_code != 200:
                self._log("Failed to get JS bundle")
                return None
            
            # Extract token from JS
            match = re.search(r'(?=eyJh)(.*?)(?=")', response.text)
            if not match:
                self._log("Failed to find access token in JS")
                return None
            
            token = match.group(1)
            
            # Cache the token for 1 hour
            _APPLE_MUSIC_TOKEN_CACHE["access_token"] = token
            _APPLE_MUSIC_TOKEN_CACHE["expires_at"] = current_time + 3600  # 1 hour
            
            self._log("✓ Got and cached access token")
            return token
            
        except Exception as e:
            self._log(f"Error fetching token: {e}")
            return None
    
    def _ensure_auth(self) -> bool:
        """Ensure we have valid authentication."""
        if not self.media_user_token:
            return False
        
        if not self._access_token:
            self._access_token = self._fetch_access_token()
            if not self._access_token:
                return False
        
        self._session.headers.update({
            'authorization': f'Bearer {self._access_token}',
            'media-user-token': self.media_user_token
        })
        
        # Verify token and get storefront
        try:
            response = self._session.get("https://amp-api.music.apple.com/v1/me/storefront")
            if response.status_code == 200:
                data = response.json()
                self._storefront = data["data"][0].get("id", "us")
                self._language = data["data"][0]["attributes"].get("defaultLanguageTag", "en-US")
                self._session.headers.update({'accept-language': f'{self._language},en;q=0.9'})
                self._log(f"✓ Authenticated (storefront: {self._storefront})")
                return True
            else:
                self._log(f"Auth failed: {response.status_code}")
                return False
        except Exception as e:
            self._log(f"Auth error: {e}")
            return False
    
    def search_track(self, track_name: str, artist_name: str) -> Optional[str]:
        """Search for a track and return its Apple Music ID."""
        if not self._ensure_auth():
            return None
        
        try:
            query = f"{track_name} {artist_name}"
            self._log(f"Searching: {query}")
            
            response = self._session.get(
                f"https://amp-api.music.apple.com/v1/catalog/{self._storefront}/search",
                params={
                    "term": query,
                    "types": "songs",
                    "limit": 1,
                    "l": self._language
                },
                timeout=10
            )
            
            if response.status_code == 200:
                data = response.json()
                songs = data.get("results", {}).get("songs", {}).get("data", [])
                if songs:
                    song_id = songs[0]["id"]
                    song_name = songs[0]["attributes"]["name"]
                    artist = songs[0]["attributes"]["artistName"]
                    self._log(f"Found: {artist} - {song_name} (ID: {song_id})")
                    return song_id
            else:
                self._log(f"Search failed: {response.status_code}")
            
            return None
            
        except Exception as e:
            self._log(f"Search error: {e}")
            return None
    
    def get_lyrics(self, song_id: str) -> Optional[LyricsResult]:
        """Fetch syllable-synced lyrics for a song ID."""
        if not self._ensure_auth():
            return None
        
        try:
            self._log(f"Fetching lyrics for: {song_id}")
            
            # Fetch song with lyrics included
            response = self._session.get(
                f"https://amp-api.music.apple.com/v1/catalog/{self._storefront}/songs/{song_id}",
                params={
                    'include[songs]': 'lyrics,syllable-lyrics',
                    'l': self._language
                },
                timeout=10
            )
            
            if response.status_code == 200:
                data = response.json()
                song_data = data.get("data", [{}])[0]
                relationships = song_data.get("relationships", {})
                
                # Try syllable-lyrics first (word-synced)
                syllable_lyrics = relationships.get("syllable-lyrics", {}).get("data", [])
                if syllable_lyrics:
                    ttml = syllable_lyrics[0].get("attributes", {}).get("ttml", "")
                    if ttml:
                        lrc = self._ttml_to_enhanced_lrc(ttml)
                        if lrc:
                            self._log("✓ Found syllable-synced lyrics")
                            return LyricsResult(
                                synced_lyrics=lrc,
                                word_synced=True,
                                source="apple_music"
                            )
                
                # Fallback to regular lyrics (line-synced)
                lyrics = relationships.get("lyrics", {}).get("data", [])
                if lyrics:
                    ttml = lyrics[0].get("attributes", {}).get("ttml", "")
                    if ttml:
                        lrc = self._ttml_to_lrc(ttml)
                        if lrc:
                            self._log("✓ Found line-synced lyrics")
                            return LyricsResult(
                                synced_lyrics=lrc,
                                word_synced=False,
                                source="apple_music"
                            )
                
                self._log("No lyrics available for this track")
            else:
                self._log(f"Fetch failed: {response.status_code} - {response.text[:100]}")
            
            return None
            
        except Exception as e:
            self._log(f"Lyrics error: {e}")
            return None
    
    def _ttml_to_enhanced_lrc(self, ttml: str) -> Optional[str]:
        """Convert Apple Music TTML syllable format to Enhanced LRC."""
        try:
            lines = []
            
            # Parse each <p> element with syllables
            # Format: <p begin="00:10.500"><span begin="00:10.500" end="00:10.800">The </span>...</p>
            p_pattern = r'<p[^>]*begin="([^"]+)"[^>]*>(.*?)</p>'
            
            for p_match in re.finditer(p_pattern, ttml, re.DOTALL):
                line_start = p_match.group(1)
                line_content = p_match.group(2)
                
                # Extract word timing
                span_pattern = r'<span[^>]*begin="([^"]+)"[^>]*>([^<]*)</span>'
                words = []
                
                for span_match in re.finditer(span_pattern, line_content):
                    word_start = span_match.group(1)
                    word_text = span_match.group(2)
                    word_time = self._convert_timestamp(word_start, enhanced=True)
                    if word_time:
                        # Keep original text as-is - TTML already has correct spacing
                        # Syllables within a word have no trailing space
                        # Word boundaries have trailing space in the original text
                        words.append(f"{word_time}{word_text}")
                
                if words:
                    line_time = self._convert_timestamp(line_start)
                    line_text = ''.join(words)
                    lines.append(f"{line_time} {line_text}")
                else:
                    # Fallback: extract plain text
                    text = re.sub(r'<[^>]+>', '', line_content).strip()
                    if text:
                        line_time = self._convert_timestamp(line_start)
                        lines.append(f"{line_time} {text}")
            
            return "\n".join(lines) if lines else None
            
        except Exception as e:
            self._log(f"TTML parse error: {e}")
            return None
    
    def _ttml_to_lrc(self, ttml: str) -> Optional[str]:
        """Convert Apple Music TTML to standard LRC format."""
        try:
            lines = []
            p_pattern = r'<p[^>]*begin="([^"]+)"[^>]*>(.*?)</p>'
            
            for match in re.finditer(p_pattern, ttml, re.DOTALL):
                timestamp = match.group(1)
                content = match.group(2)
                
                # Strip HTML tags
                text = re.sub(r'<[^>]+>', '', content).strip()
                if text:
                    lrc_time = self._convert_timestamp(timestamp)
                    if lrc_time:
                        lines.append(f"{lrc_time} {text}")
            
            return "\n".join(lines) if lines else None
            
        except Exception as e:
            self._log(f"TTML parse error: {e}")
            return None
    
    def _convert_timestamp(self, ts: str, enhanced: bool = False) -> Optional[str]:
        """Convert TTML timestamp to LRC format."""
        try:
            # Format: HH:MM:SS.mmm or MM:SS.mmm or SS.mmm
            ts = ts.replace(",", ".")
            parts = ts.split(":")
            
            if len(parts) == 3:
                hours, mins, secs = parts
                total_mins = int(hours) * 60 + int(mins)
                secs_float = float(secs)
            elif len(parts) == 2:
                mins, secs = parts
                total_mins = int(mins)
                secs_float = float(secs)
            else:
                total_mins = 0
                secs_float = float(parts[0])
            
            if enhanced:
                return f"<{total_mins:02d}:{secs_float:05.2f}>"
            else:
                return f"[{total_mins:02d}:{secs_float:05.2f}]"
        except:
            return None


class LyricsService:
    """
    Multi-source lyrics service with Apple Music word-sync support.
    
    Priority order:
    1. Apple Music - Syllable-level word-synced
    2. syncedlyrics - Line-synced fallback
    """
    
    def __init__(
        self,
        apple_music_token: Optional[str] = None,
        verbose: bool = False
    ):
        self.verbose = verbose
        self.apple_music = AppleMusicLyricsProvider(apple_music_token, verbose) if apple_music_token else None
        # Circuit breaker for Musixmatch (syncedlyrics enhanced)
        # If we hit a 401 error once, we disable it for the rest of the session
        self._enable_musixmatch = True
        if not SYNCEDLYRICS_AVAILABLE:
            self._log("Warning: syncedlyrics not installed. Run: pip install syncedlyrics", "warning")
    
    def _log(self, message: str, level: str = "info"):
        if self.verbose or level == "error":
            print(f"[LyricsService] {message}", file=sys.stderr)
    
    async def close(self):
        """No cleanup needed."""
        pass
    async def get_lyrics(
        self,
        track_name: str,
        artist_name: str,
        album_name: Optional[str] = None,
        duration_seconds: Optional[int] = None,
        spotify_track_id: Optional[str] = None
    ) -> LyricsResult:
        """
        Fetch lyrics with priority: word-synced > line-synced > plain > none.
        
        Priority chain:
        1. Apple Music syllable-synced (word-level) - BEST QUALITY
        2. syncedlyrics enhanced (Musixmatch word-synced) - SAFE MODE (auto-disables on 401)
        3. syncedlyrics synced (line-level)
        4. Plain lyrics (no sync)
        
        Note: syncedlyrics enhanced is guarded by a circuit breaker to prevent 401 spam.
        """
        self._log(f"Fetching lyrics for: {artist_name} - {track_name}")
        
        search_term = f"{track_name} {artist_name}"
        
        # PRIORITY 1: Apple Music word-synced (syllable-level) - BEST QUALITY
        # Skip syncedlyrics enhanced since it requires Musixmatch token and spams 401 errors
        has_apple_music = self.apple_music and self.apple_music.media_user_token
        self._log(f"Apple Music available: {has_apple_music}")
        if has_apple_music:
            result = await asyncio.get_event_loop().run_in_executor(
                None,
                lambda: self._fetch_apple_music(track_name, artist_name)
            )
            if result and result.has_lyrics and result.word_synced:
                self._log("✓ Found word-synced lyrics (Apple Music)")
                return result
        
        # PRIORITY 2: syncedlyrics ENHANCED (Musixmatch word-synced) - GOOD QUALITY
        # Only try if circuit breaker is not tripped
        if self._enable_musixmatch:
            result = await asyncio.get_event_loop().run_in_executor(
                None,
                lambda: self._fetch_syncedlyrics_enhanced(search_term)
            )
            if result and result.synced_lyrics:
                self._log("✓ Found word-synced lyrics (Musixmatch)")
                return result
        
        # PRIORITY 3: syncedlyrics line-synced
        result = await asyncio.get_event_loop().run_in_executor(
            None,
            lambda: self._fetch_syncedlyrics_synced(search_term)
        )
        if result and result.synced_lyrics:
            self._log("✓ Found line-synced lyrics (syncedlyrics)")
            return result
            
        # PRIORITY 4: Plain lyrics (no sync)
        result = await asyncio.get_event_loop().run_in_executor(
            None,
            lambda: self._fetch_syncedlyrics_plain(search_term)
        )
        if result and result.plain_lyrics:
            self._log("✓ Found plain lyrics (syncedlyrics)")
            return result
        
        # PRIORITY 4: None
        self._log("✗ No lyrics found")
        return LyricsResult(source="none")
    
    def _fetch_apple_music(self, track_name: str, artist_name: str) -> Optional[LyricsResult]:
        """Fetch from Apple Music."""
        try:
            self._log(f"Apple Music: Searching for '{track_name}' by '{artist_name}'")
            song_id = self.apple_music.search_track(track_name, artist_name)
            if song_id:
                self._log(f"Apple Music: Found song ID {song_id}")
                result = self.apple_music.get_lyrics(song_id)
                if result:
                    self._log(f"Apple Music: Got lyrics (word_synced={result.word_synced})")
                else:
                    self._log("Apple Music: No lyrics returned from API")
                return result
            else:
                self._log("Apple Music: Song not found in catalog")
            return None
        except Exception as e:
            self._log(f"Apple Music error: {e}")
            return None
    
    def _fetch_syncedlyrics_enhanced(self, search_term: str) -> Optional[LyricsResult]:
        """Fetch word-synced lyrics (enhanced mode) from syncedlyrics."""
        if not self._enable_musixmatch or not SYNCEDLYRICS_AVAILABLE or syncedlyrics is None:
            return None
            
        try:
            import io
            from contextlib import redirect_stdout, redirect_stderr
            
            # Trap stdout/stderr to prevents spam and detect 401s
            f = io.StringIO()
            with redirect_stdout(f), redirect_stderr(f):
                lyrics = syncedlyrics.search(search_term, enhanced=True)
            
            output = f.getvalue()
            
            # Check for Auth errors in the trapped output
            if "401" in output or "unauthorized" in output.lower():
                self._log(f"Musixmatch Auth Error (401) detected. Disabling enhanced mode for this session.", "error")
                self._enable_musixmatch = False
                return None
            
            if lyrics:
                has_word_sync = '<' in lyrics and '>' in lyrics
                is_synced = lyrics.strip().startswith('[')
                if is_synced and has_word_sync:
                    return LyricsResult(
                        synced_lyrics=lyrics,
                        word_synced=True,
                        source="syncedlyrics"
                    )
        except Exception as e:
            error_str = str(e).lower()
            if "401" in error_str or "unauthorized" in error_str:
                self._log(f"Musixmatch Auth Error (401). Disabling enhanced mode for this session.", "error")
                self._enable_musixmatch = False
            else:
                self._log(f"syncedlyrics enhanced error: {e}")
        return None
    
    def _fetch_syncedlyrics_synced(self, search_term: str) -> Optional[LyricsResult]:
        """Fetch line-synced lyrics from syncedlyrics."""
        if not SYNCEDLYRICS_AVAILABLE or syncedlyrics is None:
            return None
        try:
            lyrics = syncedlyrics.search(search_term)
            if lyrics:
                is_synced = lyrics.strip().startswith('[')
                if is_synced:
                    has_word_sync = '<' in lyrics and '>' in lyrics
                    return LyricsResult(
                        synced_lyrics=lyrics,
                        word_synced=has_word_sync,
                        source="syncedlyrics"
                    )
        except Exception as e:
            self._log(f"syncedlyrics synced error: {e}")
        return None
    
    def _fetch_syncedlyrics_plain(self, search_term: str) -> Optional[LyricsResult]:
        """Fetch plain (unsynced) lyrics from syncedlyrics."""
        if not SYNCEDLYRICS_AVAILABLE or syncedlyrics is None:
            return None
        try:
            # Use plain_only if available, otherwise search and check
            lyrics = syncedlyrics.search(search_term)
            if lyrics:
                is_synced = lyrics.strip().startswith('[')
                if not is_synced:
                    return LyricsResult(
                        plain_lyrics=lyrics,
                        source="syncedlyrics"
                    )
                else:
                    # Extract plain text from synced lyrics
                    import re
                    plain = re.sub(r'\[[^\]]*\]', '', lyrics)
                    plain = re.sub(r'<[^>]*>', '', plain)
                    plain = '\n'.join(line.strip() for line in plain.split('\n') if line.strip())
                    if plain:
                        return LyricsResult(
                            plain_lyrics=plain,
                            source="syncedlyrics"
                        )
        except Exception as e:
            self._log(f"syncedlyrics plain error: {e}")
        return None
    
    @staticmethod
    def save_lrc_file(lyrics_content: str, audio_file_path: str) -> Optional[str]:
        """Save lyrics as .lrc file next to audio file."""
        if not lyrics_content:
            return None
        try:
            audio_path = Path(audio_file_path)
            lrc_path = audio_path.with_suffix(".lrc")
            with open(lrc_path, "w", encoding="utf-8") as f:
                f.write(lyrics_content)
            return str(lrc_path)
        except Exception as e:
            logger.error(f"Failed to save LRC file: {e}")
            return None
    
    @staticmethod
    def save_txt_file(lyrics_content: str, audio_file_path: str) -> Optional[str]:
        """Save plain lyrics as .txt file next to audio file."""
        if not lyrics_content:
            return None
        try:
            audio_path = Path(audio_file_path)
            txt_path = audio_path.with_suffix(".txt")
            with open(txt_path, "w", encoding="utf-8") as f:
                f.write(lyrics_content)
            return str(txt_path)
        except Exception as e:
            logger.error(f"Failed to save lyrics TXT file: {e}")
            return None

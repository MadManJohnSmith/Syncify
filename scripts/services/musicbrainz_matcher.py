"""
MusicBrainz Matcher - Match tracks to MusicBrainz database.

Provides ISRC-based and fuzzy metadata matching for enriching track metadata.
"""

import asyncio
import re
from dataclasses import dataclass
from typing import Optional, List, Dict, Any

try:
    import musicbrainzngs
    MUSICBRAINZ_AVAILABLE = True
except ImportError:
    musicbrainzngs = None  # type: ignore
    MUSICBRAINZ_AVAILABLE = False


@dataclass
class MBRecording:
    """MusicBrainz recording (track) data."""
    id: str  # MusicBrainz recording ID
    title: str
    artist: str
    artist_id: Optional[str] = None
    album: Optional[str] = None
    album_id: Optional[str] = None
    release_date: Optional[str] = None
    genres: List[str] = None
    isrc: Optional[str] = None
    duration_ms: Optional[int] = None
    score: int = 0  # Match confidence score (0-100)
    
    def __post_init__(self):
        if self.genres is None:
            self.genres = []


class MusicBrainzMatcher:
    """Match tracks to MusicBrainz database."""
    
    USER_AGENT = "Syncify/1.0.0 (https://github.com/syncify)"
    
    def __init__(self, verbose: bool = False):
        self.verbose = verbose
        self._initialized = False
        
        if not MUSICBRAINZ_AVAILABLE:
            self._log("Warning: musicbrainzngs not installed. Run: pip install musicbrainzngs")
    
    def _log(self, message: str):
        if self.verbose:
            print(f"[MusicBrainz] {message}", flush=True)
    
    def _ensure_initialized(self):
        """Initialize musicbrainzngs with user agent."""
        if not self._initialized and MUSICBRAINZ_AVAILABLE:
            musicbrainzngs.set_useragent(
                "Syncify", "1.0.0", "https://github.com/syncify"
            )
            self._initialized = True
    
    def match_by_isrc(self, isrc: str) -> Optional[MBRecording]:
        """Match a track by its ISRC.
        
        This is the most accurate matching method.
        """
        if not MUSICBRAINZ_AVAILABLE:
            return None
        
        self._ensure_initialized()
        self._log(f"Looking up ISRC: {isrc}")
        
        try:
            result = musicbrainzngs.get_recordings_by_isrc(
                isrc,
                includes=["artists", "releases", "tags"]
            )
            
            recordings = result.get("isrc", {}).get("recording-list", [])
            
            if not recordings:
                self._log(f"No recordings found for ISRC: {isrc}")
                return None
            
            # Use first (most relevant) recording
            rec = recordings[0]
            
            # Extract artist
            artists = rec.get("artist-credit", [])
            artist_name = ""
            artist_id = None
            for a in artists:
                if isinstance(a, dict) and "artist" in a:
                    artist_name += a["artist"].get("name", "")
                    if not artist_id:
                        artist_id = a["artist"].get("id")
                elif isinstance(a, str):
                    artist_name += a
            
            # Extract genres from tags
            genres = []
            tags = rec.get("tag-list", [])
            for tag in tags:
                if isinstance(tag, dict):
                    genres.append(tag.get("name", ""))
            
            # Get release info
            releases = rec.get("release-list", [])
            album = None
            album_id = None
            release_date = None
            if releases:
                release = releases[0]
                album = release.get("title")
                album_id = release.get("id")
                release_date = release.get("date")
            
            recording = MBRecording(
                id=rec.get("id", ""),
                title=rec.get("title", ""),
                artist=artist_name,
                artist_id=artist_id,
                album=album,
                album_id=album_id,
                release_date=release_date,
                genres=genres,
                isrc=isrc,
                duration_ms=int(rec.get("length", 0)) if rec.get("length") else None,
                score=100  # ISRC match is exact
            )
            
            self._log(f"Found: {recording.title} by {recording.artist}")
            return recording
            
        except musicbrainzngs.MusicBrainzError as e:
            self._log(f"MusicBrainz error: {e}")
            return None
        except Exception as e:
            self._log(f"Error: {e}")
            return None
    
    def match_by_metadata(
        self,
        title: str,
        artist: str,
        album: Optional[str] = None,
        limit: int = 5
    ) -> List[MBRecording]:
        """Fuzzy match by metadata (title, artist, album)."""
        if not MUSICBRAINZ_AVAILABLE:
            return []
        
        self._ensure_initialized()
        self._log(f"Searching: {title} by {artist}")
        
        try:
            # Build query
            query_parts = []
            if title:
                query_parts.append(f'recording:"{title}"')
            if artist:
                query_parts.append(f'artist:"{artist}"')
            if album:
                query_parts.append(f'release:"{album}"')
            
            query = " AND ".join(query_parts)
            
            result = musicbrainzngs.search_recordings(
                query=query,
                limit=limit
            )
            
            recordings = []
            for rec in result.get("recording-list", []):
                # Extract artist
                artists = rec.get("artist-credit", [])
                artist_name = ""
                artist_id = None
                for a in artists:
                    if isinstance(a, dict) and "artist" in a:
                        artist_name += a["artist"].get("name", "")
                        if not artist_id:
                            artist_id = a["artist"].get("id")
                    elif isinstance(a, str):
                        artist_name += a
                
                # Get release info
                releases = rec.get("release-list", [])
                album_name = None
                album_id = None
                if releases:
                    release = releases[0]
                    album_name = release.get("title")
                    album_id = release.get("id")
                
                # Score from MusicBrainz
                score = int(rec.get("ext:score", 0))
                
                recordings.append(MBRecording(
                    id=rec.get("id", ""),
                    title=rec.get("title", ""),
                    artist=artist_name,
                    artist_id=artist_id,
                    album=album_name,
                    album_id=album_id,
                    duration_ms=int(rec.get("length", 0)) if rec.get("length") else None,
                    score=score
                ))
            
            self._log(f"Found {len(recordings)} matches")
            return recordings
            
        except musicbrainzngs.MusicBrainzError as e:
            self._log(f"MusicBrainz error: {e}")
            return []
        except Exception as e:
            self._log(f"Error: {e}")
            return []
    
    def get_recording_details(self, recording_id: str) -> Optional[MBRecording]:
        """Get full details for a recording by ID."""
        if not MUSICBRAINZ_AVAILABLE:
            return None
        
        self._ensure_initialized()
        
        try:
            result = musicbrainzngs.get_recording_by_id(
                recording_id,
                includes=["artists", "releases", "tags", "isrcs"]
            )
            
            rec = result.get("recording", {})
            
            # Extract artist
            artists = rec.get("artist-credit", [])
            artist_name = ""
            artist_id = None
            for a in artists:
                if isinstance(a, dict) and "artist" in a:
                    artist_name += a["artist"].get("name", "")
                    if not artist_id:
                        artist_id = a["artist"].get("id")
                elif isinstance(a, str):
                    artist_name += a
            
            # Get ISRC
            isrcs = rec.get("isrc-list", [])
            isrc = isrcs[0] if isrcs else None
            
            # Get genres
            genres = [tag.get("name", "") for tag in rec.get("tag-list", []) if isinstance(tag, dict)]
            
            # Get release info
            releases = rec.get("release-list", [])
            album = None
            album_id = None
            release_date = None
            if releases:
                release = releases[0]
                album = release.get("title")
                album_id = release.get("id")
                release_date = release.get("date")
            
            return MBRecording(
                id=rec.get("id", ""),
                title=rec.get("title", ""),
                artist=artist_name,
                artist_id=artist_id,
                album=album,
                album_id=album_id,
                release_date=release_date,
                genres=genres,
                isrc=isrc,
                duration_ms=int(rec.get("length", 0)) if rec.get("length") else None,
                score=100
            )
            
        except Exception as e:
            self._log(f"Error getting recording details: {e}")
            return None
    
    def enrich_track(
        self,
        title: str,
        artist: str,
        isrc: Optional[str] = None,
        album: Optional[str] = None
    ) -> Optional[MBRecording]:
        """Enrich track metadata using best available method.
        
        Priority:
        1. ISRC (if available) - exact match
        2. Fuzzy metadata search - best match
        """
        # Try ISRC first
        if isrc:
            result = self.match_by_isrc(isrc)
            if result:
                return result
        
        # Fall back to fuzzy search
        results = self.match_by_metadata(title, artist, album, limit=1)
        if results:
            return results[0]
        
        return None


# Convenience function
def get_matcher(verbose: bool = False) -> MusicBrainzMatcher:
    """Get a MusicBrainzMatcher instance."""
    return MusicBrainzMatcher(verbose=verbose)


if __name__ == "__main__":
    # Test matcher
    matcher = MusicBrainzMatcher(verbose=True)
    
    # Test ISRC lookup
    result = matcher.match_by_isrc("USRC11600301")
    if result:
        print(f"\nISRC match: {result.title} by {result.artist}")
        print(f"  Genres: {result.genres}")
    
    # Test fuzzy search
    results = matcher.match_by_metadata("Bohemian Rhapsody", "Queen")
    for r in results[:3]:
        print(f"\nMatch ({r.score}%): {r.title} by {r.artist}")

"""
AcoustID Matcher - Audio fingerprinting for track identification.

Uses Chromaprint fingerprints to identify tracks without metadata.
"""

import os
import subprocess
import shutil
from dataclasses import dataclass
from pathlib import Path
from typing import Optional, List, Dict, Any, Tuple

try:
    import acoustid
    ACOUSTID_AVAILABLE = True
except ImportError:
    ACOUSTID_AVAILABLE = False


@dataclass
class AcoustIDResult:
    """Result from AcoustID lookup."""
    acoustid: str
    score: float
    recording_id: Optional[str] = None
    title: Optional[str] = None
    artist: Optional[str] = None
    # MusicBrainz Artist ID (MBID) of the matched artist. The AcoustID web service
    # returns artist objects whose `id` IS the MusicBrainz artist MBID; it is what
    # Syncify writes into MUSICBRAINZ_ARTISTID for discography navigation / radio.
    artist_mbid: Optional[str] = None
    album: Optional[str] = None
    duration: Optional[int] = None


class AcoustIDMatcher:
    """Identify tracks using audio fingerprinting."""
    
    def __init__(
        self,
        api_key: Optional[str] = None,
        fpcalc_path: Optional[str] = None,
        verbose: bool = False
    ):
        """Initialize the matcher.
        
        Args:
            api_key: AcoustID API key. If None, reads from ACOUSTID_API_KEY env var.
            fpcalc_path: Path to fpcalc binary. If None, searches PATH.
            verbose: Print progress information.
        """
        self.api_key = api_key if api_key is not None else self._get_api_key()
        self.fpcalc_path = fpcalc_path or self._find_fpcalc()
        self.verbose = verbose
        
        if not ACOUSTID_AVAILABLE:
            self._log("Warning: acoustid not installed. Run: pip install pyacoustid")
    
    def _log(self, message: str):
        if self.verbose:
            print(f"[AcoustID] {message}", flush=True)
    
    def _get_api_key(self) -> Optional[str]:
        """Get API key from environment variable ACOUSTID_API_KEY or None."""
        key = os.getenv("ACOUSTID_API_KEY")
        if key and key.strip():
            return key.strip()
        return None
    
    def _find_fpcalc(self) -> str:
        """Find fpcalc binary."""
        # Check system PATH
        fpcalc = shutil.which("fpcalc")
        if fpcalc:
            return fpcalc
        
        # Check bundled locations
        bundled_paths = [
            Path(__file__).parent.parent / "bin" / "fpcalc.exe",  # Windows
            Path(__file__).parent.parent / "bin" / "fpcalc",      # Linux/Mac
        ]
        
        for path in bundled_paths:
            if path.exists():
                return str(path)
        
        return "fpcalc"
    
    def is_available(self) -> bool:
        """Check if fpcalc is available."""
        try:
            result = subprocess.run(
                [self.fpcalc_path, "-v"],
                capture_output=True,
                timeout=5
            )
            return result.returncode == 0
        except:
            return False
    
    def get_fingerprint(self, audio_path: Path) -> Optional[Tuple[int, str]]:
        """Generate fingerprint for an audio file.
        
        Returns:
            (duration, fingerprint) or None on error
        """
        if not audio_path.exists():
            self._log(f"File not found: {audio_path}")
            return None
        
        try:
            result = subprocess.run(
                [self.fpcalc_path, str(audio_path)],
                capture_output=True,
                text=True,
                timeout=60
            )
            
            if result.returncode != 0:
                self._log(f"fpcalc error: {result.stderr}")
                return None
            
            # Parse output
            duration = None
            fingerprint = None
            for line in result.stdout.strip().split("\n"):
                if line.startswith("DURATION="):
                    duration = int(line.split("=")[1])
                elif line.startswith("FINGERPRINT="):
                    fingerprint = line.split("=")[1]
            
            if duration and fingerprint:
                self._log(f"Generated fingerprint ({duration}s)")
                return (duration, fingerprint)
            
            return None
            
        except subprocess.TimeoutExpired:
            self._log("Fingerprint generation timed out")
            return None
        except Exception as e:
            self._log(f"Error: {e}")
            return None
    
    def identify(self, audio_path: Path) -> List[AcoustIDResult]:
        """Identify a track using its audio fingerprint.
        
        Returns list of possible matches sorted by score.
        """
        if not ACOUSTID_AVAILABLE:
            self._log("acoustid library not available")
            return []
        
        if not self.api_key:
            self._log("AcoustID API key not configured - returning empty match results")
            return []
        
        self._log(f"Identifying: {audio_path.name}")
        
        try:
            # Use acoustid library which handles fpcalc internally
            results = acoustid.match(
                self.api_key,
                str(audio_path),
                parse=True
            )
            
            matches = []
            for score, recording_id, title, artist in results:
                matches.append(AcoustIDResult(
                    acoustid="",  # Not provided by this API
                    score=score,
                    recording_id=recording_id,
                    title=title,
                    artist=artist
                ))
            
            self._log(f"Found {len(matches)} matches")
            return sorted(matches, key=lambda x: x.score, reverse=True)
            
        except acoustid.NoBackendError:
            self._log("No audio decoder backend found. Install ffmpeg.")
            return []
        except acoustid.FingerprintGenerationError as e:
            self._log(f"Fingerprint error: {e}")
            return []
        except acoustid.WebServiceError as e:
            self._log(f"AcoustID API error: {e}")
            return []
        except Exception as e:
            self._log(f"Error: {e}")
            return []
    
    def identify_with_fingerprint(
        self,
        duration: int,
        fingerprint: str
    ) -> List[AcoustIDResult]:
        """Identify using pre-computed fingerprint."""
        if not ACOUSTID_AVAILABLE:
            return []
        
        if not self.api_key:
            self._log("AcoustID API key not configured - returning empty match results")
            return []
        
        try:
            import urllib.request
            import json
            
            url = f"https://api.acoustid.org/v2/lookup?client={self.api_key}&duration={duration}&fingerprint={fingerprint}&meta=recordings"
            
            with urllib.request.urlopen(url, timeout=30) as response:
                data = json.loads(response.read().decode())
            
            if data.get("status") != "ok":
                self._log(f"API error: {data.get('error', {}).get('message')}")
                return []
            
            matches = []
            for result in data.get("results", []):
                acoustid_id = result.get("id", "")
                score = result.get("score", 0)

                for recording in result.get("recordings", []):
                    artists = recording.get("artists", [])
                    artist = artists[0].get("name", "") if artists else ""
                    # Artist MBID: AcoustID artist objects are keyed by MusicBrainz id.
                    artist_mbid = artists[0].get("id") if artists else None

                    matches.append(AcoustIDResult(
                        acoustid=acoustid_id,
                        score=score,
                        recording_id=recording.get("id"),
                        title=recording.get("title"),
                        artist=artist,
                        artist_mbid=artist_mbid,
                        duration=recording.get("duration")
                    ))
            
            return sorted(matches, key=lambda x: x.score, reverse=True)
            
        except Exception as e:
            self._log(f"Error: {e}")
            return []
    
    def find_duplicates(self, audio_paths: List[Path]) -> Dict[str, List[Path]]:
        """Find duplicate audio files based on fingerprints.
        
        Returns dict mapping fingerprint to list of files with that fingerprint.
        """
        fingerprints: Dict[str, List[Path]] = {}
        
        for path in audio_paths:
            result = self.get_fingerprint(path)
            if result:
                duration, fp = result
                # Use first 100 chars as key (enough for comparison)
                fp_key = fp[:100] if len(fp) > 100 else fp
                
                if fp_key not in fingerprints:
                    fingerprints[fp_key] = []
                fingerprints[fp_key].append(path)
        
        # Return only duplicates
        return {fp: paths for fp, paths in fingerprints.items() if len(paths) > 1}


# Convenience function
def get_acoustid_matcher(verbose: bool = False) -> AcoustIDMatcher:
    """Get an AcoustIDMatcher instance."""
    return AcoustIDMatcher(verbose=verbose)


if __name__ == "__main__":
    import sys
    
    matcher = AcoustIDMatcher(verbose=True)
    print(f"fpcalc available: {matcher.is_available()}")
    
    if len(sys.argv) > 1:
        path = Path(sys.argv[1])
        results = matcher.identify(path)
        for r in results[:3]:
            print(f"Match ({r.score:.0%}): {r.title} by {r.artist}")

#!/usr/bin/env python3
"""
Lyrics Bridge - CLI interface to fetch and test lyrics for tracks.

Usage:
    python lyrics_bridge.py fetch <track> <artist> [album]
    python lyrics_bridge.py batch <json_file>
    python lyrics_bridge.py test --provider <provider_id>

Returns JSON:
    {"success": true/false, "data": {...}, "error": "..."}
"""

import argparse
import json
import sys
import os
from pathlib import Path
from typing import Optional

# Add local services to path (S43: relocated from adjacent_tools/Syncify-test)
SCRIPTS_DIR = Path(__file__).resolve().parent
REPO_ROOT = SCRIPTS_DIR.parent
if str(SCRIPTS_DIR) not in sys.path:
    sys.path.insert(0, str(SCRIPTS_DIR))

# If running outside venv, check for project .venv site-packages
for site in REPO_ROOT.glob(".venv/lib/python*/site-packages"):
    if site.is_dir() and str(site) not in sys.path:
        sys.path.insert(0, str(site))

# Load .env from project root
try:
    from dotenv import load_dotenv
    load_dotenv(REPO_ROOT / ".env")
except ImportError:
    pass


def json_response(success: bool, data=None, error=None, **extra):
    """Output JSON response and exit."""
    result = {"success": success}
    if data is not None:
        result["data"] = data
    if error is not None:
        result["error"] = error
    result.update(extra)
    print(json.dumps(result, ensure_ascii=False))
    sys.exit(0 if success else 1)


def fetch_lyrics(track: str, artist: str, album: Optional[str] = None):
    """Fetch lyrics for a single track."""
    import asyncio
    from services.lyrics_service import LyricsService
    
    # Get Apple Music token if available
    apple_token = os.getenv("APPLE_MUSIC_MEDIA_USER_TOKEN")
    
    service = LyricsService(
        apple_music_token=apple_token,
        verbose=True
    )
    
    async def _fetch():
        return await service.get_lyrics(
            track_name=track,
            artist_name=artist,
            album_name=album
        )
    
    try:
        result = asyncio.run(_fetch())
        
        if result.has_lyrics:
            json_response(True, {
                "synced_lyrics": result.synced_lyrics,
                "plain_lyrics": result.plain_lyrics,
                "word_synced": result.word_synced,
                "instrumental": result.instrumental,
                "source": result.source,
            })
        else:
            json_response(False, error="No lyrics found")
            
    except Exception as e:
        json_response(False, error=str(e))
    finally:
        service.close()


def batch_fetch(json_file: str):
    """Fetch lyrics for multiple tracks from a JSON file."""
    import asyncio
    from services.lyrics_service import LyricsService
    
    try:
        with open(json_file, 'r', encoding='utf-8') as f:
            tracks = json.load(f)
    except Exception as e:
        json_response(False, error=f"Failed to read JSON file: {e}")
        return
    
    apple_token = os.getenv("APPLE_MUSIC_MEDIA_USER_TOKEN")
    service = LyricsService(apple_music_token=apple_token, verbose=False)
    
    async def _fetch_all():
        results = []
        for track_info in tracks:
            track = track_info.get("track", "")
            artist = track_info.get("artist", "")
            album = track_info.get("album")
            track_id = track_info.get("id")
            
            try:
                result = await service.get_lyrics(
                    track_name=track,
                    artist_name=artist,
                    album_name=album
                )
                
                results.append({
                    "id": track_id,
                    "track": track,
                    "artist": artist,
                    "success": result.has_lyrics,
                    "synced_lyrics": result.synced_lyrics if result.has_lyrics else None,
                    "plain_lyrics": result.plain_lyrics if result.has_lyrics else None,
                    "word_synced": result.word_synced,
                    "source": result.source,
                })
            except Exception as e:
                results.append({
                    "id": track_id,
                    "track": track,
                    "artist": artist,
                    "success": False,
                    "error": str(e),
                })
        return results
    
    try:
        results = asyncio.run(_fetch_all())
        json_response(True, {"results": results, "total": len(results)})
    finally:
        service.close()


def test_provider(provider: str):
    """Test availability of a lyrics provider."""
    provider_clean = provider.strip()
    provider_lower = provider_clean.lower()

    def check_url(url: str, headers: Optional[dict] = None, timeout: float = 6.0) -> int:
        req_headers = {
            "User-Agent": "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36",
            "Accept": "*/*",
        }
        if headers:
            req_headers.update(headers)

        try:
            import requests
            resp = requests.get(url, headers=req_headers, timeout=timeout)
            return resp.status_code
        except ImportError:
            import urllib.request
            req = urllib.request.Request(url, headers=req_headers)
            try:
                with urllib.request.urlopen(req, timeout=timeout) as resp:
                    return resp.status
            except urllib.error.HTTPError as e:
                return e.code
            except Exception as e:
                raise RuntimeError(f"Connection failed: {e}")

    try:
        if provider_lower in ("apple_music", "applemusic", "apple"):
            apple_token = os.getenv("APPLE_MUSIC_MEDIA_USER_TOKEN")
            status_code = check_url("https://music.apple.com/us/browse")
            if status_code in (200, 301, 302):
                json_response(
                    True,
                    provider=provider_clean,
                    status="available",
                    has_user_token=bool(apple_token),
                )
            else:
                json_response(
                    False,
                    error=f"Apple Music returned HTTP {status_code}",
                    provider=provider_clean,
                    status="unavailable",
                )

        elif provider_lower in ("lrclib", "lrc"):
            status_code = check_url("https://lrclib.net/api/get?track_name=test&artist_name=test")
            if status_code in (200, 404):
                json_response(True, provider=provider_clean, status="available")
            else:
                json_response(
                    False,
                    error=f"LRCLIB returned HTTP {status_code}",
                    provider=provider_clean,
                    status="unavailable",
                )

        elif provider_lower == "genius":
            status_code = check_url("https://genius.com/api/search/multi?q=test")
            if status_code in (200, 403, 404):
                json_response(True, provider=provider_clean, status="available")
            else:
                json_response(
                    False,
                    error=f"Genius returned HTTP {status_code}",
                    provider=provider_clean,
                    status="unavailable",
                )

        elif provider_lower in ("netease", "163"):
            status_code = check_url("https://music.163.com")
            if status_code in (200, 301, 302):
                json_response(True, provider=provider_clean, status="available")
            else:
                json_response(
                    False,
                    error=f"NetEase returned HTTP {status_code}",
                    provider=provider_clean,
                    status="unavailable",
                )

        elif provider_lower == "musixmatch":
            status_code = check_url("https://www.musixmatch.com")
            if status_code in (200, 403):
                json_response(True, provider=provider_clean, status="available")
            else:
                json_response(
                    False,
                    error=f"Musixmatch returned HTTP {status_code}",
                    provider=provider_clean,
                    status="unavailable",
                )

        elif provider_lower == "deezer":
            status_code = check_url("https://api.deezer.com")
            if status_code in (200, 404):
                json_response(True, provider=provider_clean, status="available")
            else:
                json_response(
                    False,
                    error=f"Deezer returned HTTP {status_code}",
                    provider=provider_clean,
                    status="unavailable",
                )

        elif provider_lower == "megalobiz":
            status_code = check_url("https://www.megalobiz.com")
            if status_code in (200, 403):
                json_response(True, provider=provider_clean, status="available")
            else:
                json_response(
                    False,
                    error=f"Megalobiz returned HTTP {status_code}",
                    provider=provider_clean,
                    status="unavailable",
                )

        else:
            # Check syncedlyrics dynamic providers
            try:
                import syncedlyrics
                providers = getattr(syncedlyrics, "providers", None)
                if providers and hasattr(providers, provider_lower.capitalize()):
                    json_response(True, provider=provider_clean, status="available")
                else:
                    json_response(
                        False,
                        error=f"Unknown lyrics provider: {provider_clean}",
                        provider=provider_clean,
                        status="unknown",
                    )
            except Exception:
                json_response(
                    False,
                    error=f"Unknown lyrics provider: {provider_clean}",
                    provider=provider_clean,
                    status="unknown",
                )

    except Exception as e:
        json_response(
            False,
            error=f"Provider test error for {provider_clean}: {e}",
            provider=provider_clean,
            status="unavailable",
        )


def main():
    parser = argparse.ArgumentParser(description="Lyrics Bridge - CLI interface to fetch and test lyrics")
    subparsers = parser.add_subparsers(dest="action", help="Action to perform")

    # fetch
    parser_fetch = subparsers.add_parser("fetch", help="Fetch lyrics for a track")
    parser_fetch.add_argument("track", help="Track name")
    parser_fetch.add_argument("artist", help="Artist name")
    parser_fetch.add_argument("album", nargs="?", default=None, help="Album name (optional)")

    # batch
    parser_batch = subparsers.add_parser("batch", help="Fetch lyrics for multiple tracks from JSON")
    parser_batch.add_argument("json_file", help="Path to JSON file with track info")

    # test
    parser_test = subparsers.add_parser("test", help="Test lyrics provider availability")
    parser_test.add_argument("--provider", required=True, help="Lyrics provider ID to test (e.g. lrclib, musixmatch, genius, apple_music, netease)")

    if len(sys.argv) < 2:
        json_response(False, error="Usage: lyrics_bridge.py <action> [args...]. Valid actions: fetch, batch, test")

    args = parser.parse_args()

    if args.action == "fetch":
        fetch_lyrics(args.track, args.artist, args.album)
    elif args.action == "batch":
        batch_fetch(args.json_file)
    elif args.action == "test":
        test_provider(args.provider)
    else:
        json_response(False, error=f"Unknown action: {args.action}. Valid actions: fetch, batch, test")


if __name__ == "__main__":
    main()

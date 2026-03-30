#!/usr/bin/env python3
"""
Lyrics Bridge - CLI interface to fetch lyrics for tracks.

Usage:
    python lyrics_bridge.py <action> [args...]

Actions:
    fetch <track> <artist> [album]  - Fetch lyrics for a track
    batch <json_file>               - Fetch lyrics for multiple tracks from JSON

Returns JSON:
    {"success": true/false, "data": {...}, "error": "..."}
"""

import json
import sys
import os
from pathlib import Path
from typing import Optional

# Add local services to path (S43: relocated from adjacent_tools/Syncify-test)
SCRIPTS_DIR = Path(__file__).parent
if str(SCRIPTS_DIR) not in sys.path:
    sys.path.insert(0, str(SCRIPTS_DIR))

# Load .env from project root
from dotenv import load_dotenv
load_dotenv(Path(__file__).parent.parent / ".env")


def json_response(success: bool, data=None, error=None):
    """Output JSON response and exit."""
    result = {"success": success}
    if data:
        result["data"] = data
    if error:
        result["error"] = error
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


def main():
    if len(sys.argv) < 2:
        json_response(False, error="Usage: lyrics_bridge.py <action> [args...]")
    
    action = sys.argv[1].lower()
    
    if action == "fetch":
        if len(sys.argv) < 4:
            json_response(False, error="Usage: lyrics_bridge.py fetch <track> <artist> [album]")
        track = sys.argv[2]
        artist = sys.argv[3]
        album = sys.argv[4] if len(sys.argv) > 4 else None
        fetch_lyrics(track, artist, album)
        
    elif action == "batch":
        if len(sys.argv) < 3:
            json_response(False, error="Usage: lyrics_bridge.py batch <json_file>")
        batch_fetch(sys.argv[2])
        
    else:
        json_response(False, error=f"Unknown action: {action}. Valid: fetch, batch")


if __name__ == "__main__":
    main()

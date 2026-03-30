#!/usr/bin/env python3
"""
Playlist Bridge - Sync playlists between streaming services.

Usage:
    python playlist_bridge.py list <service>  # List playlists from service
    python playlist_bridge.py get <service> <playlist_id>  # Get playlist tracks
    python playlist_bridge.py export <service> <playlist_id> [--format json|m3u]
    python playlist_bridge.py match <playlist_file> <target_service>  # Match tracks

Returns JSON:
    {"success": true/false, "data": {...}, "error": "..."}
"""

import json
import sys
import os
from pathlib import Path
from typing import Optional, List, Dict, Any
import asyncio

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
    print(json.dumps(result, ensure_ascii=False, default=str))
    sys.exit(0 if success else 1)


def get_spotify_playlists():
    """Get playlists from Spotify."""
    try:
        from services.spotify_api import get_spotify_connection
        sp = get_spotify_connection()
        
        playlists = []
        results = sp.current_user_playlists(limit=50)
        
        while results:
            for item in results['items']:
                playlists.append({
                    "id": item['id'],
                    "name": item['name'],
                    "description": item.get('description', ''),
                    "track_count": item['tracks']['total'],
                    "owner": item['owner']['display_name'],
                    "public": item.get('public', False),
                    "image_url": item['images'][0]['url'] if item.get('images') else None,
                })
            
            if results.get('next'):
                results = sp.next(results)
            else:
                break
        
        return playlists
    except Exception as e:
        raise Exception(f"Spotify error: {e}")


def get_spotify_playlist_tracks(playlist_id: str):
    """Get tracks from a Spotify playlist."""
    try:
        from services.spotify_api import get_spotify_connection
        sp = get_spotify_connection()
        
        tracks = []
        results = sp.playlist_tracks(playlist_id, limit=100)
        
        while results:
            for item in results['items']:
                track = item.get('track')
                if track:
                    tracks.append({
                        "id": track['id'],
                        "title": track['name'],
                        "artist": ", ".join(a['name'] for a in track.get('artists', [])),
                        "album": track.get('album', {}).get('name'),
                        "duration_ms": track.get('duration_ms'),
                        "isrc": track.get('external_ids', {}).get('isrc'),
                        "added_at": item.get('added_at'),
                    })
            
            if results.get('next'):
                results = sp.next(results)
            else:
                break
        
        return tracks
    except Exception as e:
        raise Exception(f"Spotify error: {e}")


def get_qobuz_playlists():
    """Get playlists from Qobuz (via favorites/albums as pseudo-playlists)."""
    try:
        from services.qobuz_service import QobuzService
        
        app_id = os.getenv("QOBUZ_APP_ID")
        app_secret = os.getenv("QOBUZ_APP_SECRET")
        token = os.getenv("QOBUZ_AUTH_TOKEN")
        
        if not all([app_id, app_secret, token]):
            raise Exception("Qobuz credentials not configured")
        
        service = QobuzService(app_id, app_secret)
        service.auth_token = token
        
        # Get favorite albums as pseudo-playlists
        favorites = asyncio.run(service.get_favorites())
        
        playlists = []
        for album in favorites.get("albums", []):
            playlists.append({
                "id": album.get("id"),
                "name": album.get("title", "Unknown Album"),
                "description": f"By {album.get('artist', {}).get('name', 'Unknown')}",
                "track_count": album.get("tracks_count", 0),
                "owner": "Qobuz",
                "type": "album",
            })
        
        return playlists
    except Exception as e:
        raise Exception(f"Qobuz error: {e}")


def get_tidal_playlists():
    """Get playlists from Tidal."""
    try:
        from services.tidal_service import TidalService
        
        token = os.getenv("TIDAL_ACCESS_TOKEN")
        if not token:
            raise Exception("Tidal not authenticated")
        
        service = TidalService()
        service.access_token = token
        
        playlists_data = asyncio.run(service.get_playlists())
        
        playlists = []
        for item in playlists_data.get("items", []):
            playlists.append({
                "id": item.get("uuid"),
                "name": item.get("title", "Unknown"),
                "description": item.get("description", ""),
                "track_count": item.get("numberOfTracks", 0),
                "owner": item.get("creator", {}).get("name", "Unknown"),
            })
        
        return playlists
    except Exception as e:
        raise Exception(f"Tidal error: {e}")


def list_playlists(service: str):
    """List playlists from a service."""
    service = service.lower()
    
    try:
        if service == "spotify":
            playlists = get_spotify_playlists()
        elif service == "qobuz":
            playlists = get_qobuz_playlists()
        elif service == "tidal":
            playlists = get_tidal_playlists()
        else:
            json_response(False, error=f"Unsupported service: {service}")
            return
        
        json_response(True, {
            "service": service,
            "count": len(playlists),
            "playlists": playlists,
        })
    except Exception as e:
        json_response(False, error=str(e))


def get_playlist(service: str, playlist_id: str):
    """Get playlist tracks from a service."""
    service = service.lower()
    
    try:
        if service == "spotify":
            tracks = get_spotify_playlist_tracks(playlist_id)
        else:
            json_response(False, error=f"Get playlist not implemented for: {service}")
            return
        
        json_response(True, {
            "service": service,
            "playlist_id": playlist_id,
            "track_count": len(tracks),
            "tracks": tracks,
        })
    except Exception as e:
        json_response(False, error=str(e))


def export_playlist(service: str, playlist_id: str, format: str = "json"):
    """Export playlist to a file format."""
    service = service.lower()
    
    try:
        if service == "spotify":
            tracks = get_spotify_playlist_tracks(playlist_id)
        else:
            json_response(False, error=f"Export not implemented for: {service}")
            return
        
        if format == "json":
            output = {
                "service": service,
                "playlist_id": playlist_id,
                "tracks": tracks,
            }
            json_response(True, output)
        elif format == "m3u":
            lines = ["#EXTM3U"]
            for track in tracks:
                duration = track.get("duration_ms", 0) // 1000
                artist = track.get("artist", "Unknown")
                title = track.get("title", "Unknown")
                lines.append(f"#EXTINF:{duration},{artist} - {title}")
                lines.append(f"# ISRC: {track.get('isrc', 'N/A')}")
            
            json_response(True, {
                "format": "m3u",
                "content": "\n".join(lines),
            })
        else:
            json_response(False, error=f"Unknown format: {format}")
    except Exception as e:
        json_response(False, error=str(e))


def match_playlist_tracks(playlist_file: str, target_service: str):
    """Match playlist tracks to another service using ISRC."""
    try:
        with open(playlist_file, 'r') as f:
            playlist_data = json.load(f)
        
        tracks = playlist_data.get("tracks", [])
        
        # For now, just report which tracks have ISRCs (can be matched)
        matchable = []
        unmatchable = []
        
        for track in tracks:
            if track.get("isrc"):
                matchable.append({
                    "title": track.get("title"),
                    "artist": track.get("artist"),
                    "isrc": track.get("isrc"),
                })
            else:
                unmatchable.append({
                    "title": track.get("title"),
                    "artist": track.get("artist"),
                })
        
        json_response(True, {
            "target_service": target_service,
            "total_tracks": len(tracks),
            "matchable": len(matchable),
            "unmatchable": len(unmatchable),
            "matchable_tracks": matchable,
            "unmatchable_tracks": unmatchable,
        })
    except Exception as e:
        json_response(False, error=str(e))


def main():
    import argparse
    
    parser = argparse.ArgumentParser(description="Playlist sync between services")
    subparsers = parser.add_subparsers(dest="command", required=True)
    
    # List command
    list_parser = subparsers.add_parser("list", help="List playlists from service")
    list_parser.add_argument("service", help="Service name (spotify, qobuz, tidal)")
    
    # Get command
    get_parser = subparsers.add_parser("get", help="Get playlist tracks")
    get_parser.add_argument("service", help="Service name")
    get_parser.add_argument("playlist_id", help="Playlist ID")
    
    # Export command
    export_parser = subparsers.add_parser("export", help="Export playlist")
    export_parser.add_argument("service", help="Service name")
    export_parser.add_argument("playlist_id", help="Playlist ID")
    export_parser.add_argument("--format", "-f", default="json",
                               choices=["json", "m3u"], help="Export format")
    
    # Match command
    match_parser = subparsers.add_parser("match", help="Match playlist to target service")
    match_parser.add_argument("playlist_file", help="Path to exported playlist JSON")
    match_parser.add_argument("target_service", help="Target service to match against")
    
    args = parser.parse_args()
    
    if args.command == "list":
        list_playlists(args.service)
    elif args.command == "get":
        get_playlist(args.service, args.playlist_id)
    elif args.command == "export":
        export_playlist(args.service, args.playlist_id, args.format)
    elif args.command == "match":
        match_playlist_tracks(args.playlist_file, args.target_service)


if __name__ == "__main__":
    main()

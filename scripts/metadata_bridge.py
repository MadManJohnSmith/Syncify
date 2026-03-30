#!/usr/bin/env python3
"""
Metadata Bridge - CLI interface to enrich track metadata.

Usage:
    python metadata_bridge.py enrich <track> <artist> [--isrc <isrc>] [--album <album>]
    python metadata_bridge.py match <track> <artist> [--isrc <isrc>]
    python metadata_bridge.py features <isrc>  # Get Spotify audio features

Returns JSON:
    {"success": true/false, "data": {...}, "error": "..."}
"""

import json
import sys
import os
import argparse
from pathlib import Path
from typing import Optional
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
    print(json.dumps(result, ensure_ascii=False))
    sys.exit(0 if success else 1)


def enrich_track(track: str, artist: str, isrc: Optional[str] = None, album: Optional[str] = None):
    """Enrich track metadata using MusicBrainz and Last.fm."""
    from services.metadata_enrichment import enrich_metadata
    
    lastfm_key = os.getenv("LASTFM_API_KEY")
    
    async def _enrich():
        return await enrich_metadata(
            isrc=isrc,
            artist=artist,
            title=track,
            lastfm_api_key=lastfm_key
        )
    
    try:
        result = asyncio.run(_enrich())
        
        # Convert dataclass to dict
        data = {
            "language": result.language,
            "country": result.country,
            "recording_location": result.recording_location,
            "musicbrainz_recording_id": result.musicbrainz_recording_id,
            "musicbrainz_artist_id": result.musicbrainz_artist_id,
            "musicbrainz_release_id": result.musicbrainz_release_id,
            "genres": result.genres,
            "tags": result.tags,
            "bpm": result.bpm,
            "key": result.key,
            "energy": result.energy,
            "danceability": result.danceability,
            "valence": result.valence,
            "acousticness": result.acousticness,
            "instrumentalness": result.instrumentalness,
            "speechiness": result.speechiness,
            "liveness": result.liveness,
            "loudness": result.loudness,
            "spotify_popularity": result.spotify_popularity,
        }
        
        # Filter out None values
        data = {k: v for k, v in data.items() if v is not None}
        
        json_response(True, data)
        
    except Exception as e:
        json_response(False, error=str(e))


def match_track(track: str, artist: str, isrc: Optional[str] = None):
    """Match track to MusicBrainz database."""
    from services.musicbrainz_matcher import MusicBrainzMatcher
    
    matcher = MusicBrainzMatcher(verbose=True)
    
    try:
        result = matcher.enrich_track(
            title=track,
            artist=artist,
            isrc=isrc
        )
        
        if result:
            data = {
                "id": result.id,
                "title": result.title,
                "artist": result.artist,
                "artist_id": result.artist_id,
                "album": result.album,
                "album_id": result.album_id,
                "release_date": result.release_date,
                "genres": result.genres,
                "isrc": result.isrc,
                "duration_ms": result.duration_ms,
                "score": result.score,
            }
            json_response(True, data)
        else:
            json_response(False, error="No match found")
            
    except Exception as e:
        json_response(False, error=str(e))


def get_spotify_features(isrc: str):
    """Get Spotify audio features for a track by ISRC."""
    from services.metadata_enrichment import SpotifyEnricher
    
    client_id = os.getenv("SPOTIFY_CLIENT_ID")
    client_secret = os.getenv("SPOTIFY_CLIENT_SECRET")
    
    if not client_id or not client_secret:
        json_response(False, error="SPOTIFY_CLIENT_ID and SPOTIFY_CLIENT_SECRET required")
        return
    
    async def _get_features():
        async with SpotifyEnricher(client_id, client_secret) as enricher:
            # First search for track by ISRC
            track_id = await enricher.search_by_isrc(isrc)
            if not track_id:
                return None
            
            # Get audio features
            features = await enricher.get_audio_features(track_id)
            track_info = await enricher.get_track_info(track_id)
            
            return {
                "track_id": track_id,
                "features": features,
                "track_info": track_info,
            }
    
    try:
        result = asyncio.run(_get_features())
        
        if result:
            json_response(True, result)
        else:
            json_response(False, error="Track not found on Spotify")
            
    except Exception as e:
        json_response(False, error=str(e))


def main():
    parser = argparse.ArgumentParser(description="Enrich track metadata")
    subparsers = parser.add_subparsers(dest="command", required=True)
    
    # Enrich command
    enrich_parser = subparsers.add_parser("enrich", help="Enrich metadata via MusicBrainz/Last.fm")
    enrich_parser.add_argument("track", help="Track title")
    enrich_parser.add_argument("artist", help="Artist name")
    enrich_parser.add_argument("--isrc", help="ISRC code")
    enrich_parser.add_argument("--album", help="Album name")
    
    # Match command
    match_parser = subparsers.add_parser("match", help="Match to MusicBrainz")
    match_parser.add_argument("track", help="Track title")
    match_parser.add_argument("artist", help="Artist name")
    match_parser.add_argument("--isrc", help="ISRC code")
    
    # Features command
    features_parser = subparsers.add_parser("features", help="Get Spotify audio features")
    features_parser.add_argument("isrc", help="ISRC code")
    
    args = parser.parse_args()
    
    if args.command == "enrich":
        enrich_track(args.track, args.artist, args.isrc, args.album)
    elif args.command == "match":
        match_track(args.track, args.artist, args.isrc)
    elif args.command == "features":
        get_spotify_features(args.isrc)


if __name__ == "__main__":
    main()

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
from typing import Optional, Dict, Any
import asyncio

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


def json_response(success: bool, data=None, error=None):
    """Output JSON response and exit."""
    result = {"success": success}
    if data is not None:
        result["data"] = data
    if error is not None:
        result["error"] = error
    print(json.dumps(result, ensure_ascii=False, default=str))
    sys.exit(0 if success else 1)


def extract_enriched_metadata(result: Any) -> Dict[str, Any]:
    """
    Safely extract metadata dictionary from EnrichedMetadata, arbitrary objects, or dicts.
    Handles attribute name variations and fallbacks (e.g. artist_mbids -> musicbrainz_artist_id,
    genre_tags/style_tags -> genres, lastfm_tags -> tags) without raising AttributeError.
    """
    if result is None:
        return {}

    def _get(attr: str, default: Any = None) -> Any:
        if isinstance(result, dict):
            return result.get(attr, default)
        return getattr(result, attr, default)

    # 1. Resolve musicbrainz_artist_id with fallbacks
    artist_id = _get("musicbrainz_artist_id")
    if not artist_id:
        artist_mbids = _get("artist_mbids")
        if isinstance(artist_mbids, (list, tuple)) and artist_mbids:
            artist_id = artist_mbids[0]
        elif isinstance(artist_mbids, str) and artist_mbids:
            artist_id = artist_mbids
    if not artist_id:
        artist_id = _get("artist_id")

    # 2. Resolve genres with fallbacks
    genres = _get("genres")
    if genres is None:
        genres = _get("genre_tags")
    if genres is None:
        genres = _get("style_tags")

    # 3. Resolve tags with fallbacks
    tags = _get("tags")
    if tags is None:
        tags = _get("lastfm_tags")

    # 4. Resolve IDs
    rec_id = _get("musicbrainz_recording_id") or _get("recording_id")
    rel_id = _get("musicbrainz_release_id") or _get("release_id")

    data = {
        "language": _get("language"),
        "country": _get("country"),
        "recording_location": _get("recording_location"),
        "musicbrainz_recording_id": rec_id,
        "musicbrainz_artist_id": artist_id,
        "artist_mbids": _get("artist_mbids"),
        "musicbrainz_release_id": rel_id,
        "genres": genres,
        "genre_tags": _get("genre_tags"),
        "tags": tags,
        "lastfm_tags": _get("lastfm_tags"),
        "mood_tags": _get("mood_tags"),
        "occasion_tags": _get("occasion_tags"),
        "style_tags": _get("style_tags"),
        "bpm": _get("bpm"),
        "key": _get("key"),
        "musical_key": _get("musical_key"),
        "mode": _get("mode"),
        "time_signature": _get("time_signature"),
        "energy": _get("energy"),
        "danceability": _get("danceability"),
        "valence": _get("valence"),
        "acousticness": _get("acousticness"),
        "instrumentalness": _get("instrumentalness"),
        "speechiness": _get("speechiness"),
        "liveness": _get("liveness"),
        "loudness": _get("loudness"),
        "spotify_popularity": _get("spotify_popularity"),
    }

    # Filter out None values
    return {k: v for k, v in data.items() if v is not None}


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
        data = extract_enriched_metadata(result)
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
                "id": getattr(result, "id", None),
                "title": getattr(result, "title", None),
                "artist": getattr(result, "artist", None),
                "artist_id": getattr(result, "artist_id", None),
                "album": getattr(result, "album", None),
                "album_id": getattr(result, "album_id", None),
                "release_date": getattr(result, "release_date", None),
                "genres": getattr(result, "genres", None),
                "isrc": getattr(result, "isrc", None),
                "duration_ms": getattr(result, "duration_ms", None),
                "score": getattr(result, "score", None),
            }
            data = {k: v for k, v in data.items() if v is not None}
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

#!/usr/bin/env python3
"""
Playlist Bridge - Sync playlists between streaming services and local library.

Usage:
    python playlist_bridge.py list <service>  # List playlists from service (spotify, qobuz, tidal, deezer, soundcloud, local)
    python playlist_bridge.py get <service> <playlist_id>  # Get playlist tracks
    python playlist_bridge.py export <service> <playlist_id> [--format json|m3u|m3u8] [--output <path>]
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

# Ensure scripts directory is in sys.path
SCRIPTS_DIR = Path(__file__).resolve().parent
if str(SCRIPTS_DIR) not in sys.path:
    sys.path.insert(0, str(SCRIPTS_DIR))

# Load .env from project root if python-dotenv is installed
try:
    from dotenv import load_dotenv
    load_dotenv(SCRIPTS_DIR.parent / ".env")
except ImportError:
    pass


def json_response(success: bool, data: Any = None, error: Optional[str] = None):
    """Output JSON response and exit."""
    result = {"success": success}
    if data is not None:
        result["data"] = data
    if error is not None:
        result["error"] = error
    print(json.dumps(result, ensure_ascii=False, default=str))
    sys.exit(0 if success else 1)


# ==============================================================================
# SPOTIFY SERVICE IMPLEMENTATION (Standard spotipy)
# ==============================================================================

def get_spotify_client():
    """Get authenticated spotipy client using env credentials or cached tokens."""
    try:
        import spotipy
        from spotipy.oauth2 import SpotifyOAuth
    except ImportError:
        raise Exception("The 'spotipy' package is not installed. Please install spotipy.")

    access_token = os.getenv("SPOTIFY_ACCESS_TOKEN") or os.getenv("SPOTIPY_ACCESS_TOKEN")
    if access_token:
        return spotipy.Spotify(auth=access_token)

    client_id = os.getenv("SPOTIPY_CLIENT_ID") or os.getenv("SPOTIFY_CLIENT_ID")
    client_secret = os.getenv("SPOTIPY_CLIENT_SECRET") or os.getenv("SPOTIFY_CLIENT_SECRET")
    redirect_uri = (
        os.getenv("SPOTIPY_REDIRECT_URI")
        or os.getenv("SPOTIFY_REDIRECT_URI")
        or "http://localhost:8888/callback"
    )
    cache_path = os.getenv("SPOTIPY_CACHE_PATH") or str(SCRIPTS_DIR.parent / ".spotify_token_cache.json")

    if not client_id or not client_secret:
        raise Exception("Spotify credentials not configured (SPOTIPY_CLIENT_ID / SPOTIPY_CLIENT_SECRET missing)")

    auth_manager = SpotifyOAuth(
        client_id=client_id,
        client_secret=client_secret,
        redirect_uri=redirect_uri,
        scope="playlist-read-private playlist-read-collaborative user-library-read",
        cache_path=cache_path,
        open_browser=False,
    )
    return spotipy.Spotify(auth_manager=auth_manager)


def get_spotify_playlists() -> List[Dict[str, Any]]:
    """Get playlists from Spotify."""
    try:
        sp = get_spotify_client()
        playlists = []
        results = sp.current_user_playlists(limit=50)

        while results:
            for item in results.get("items", []):
                owner_obj = item.get("owner")
                owner_name = owner_obj.get("display_name", "Unknown") if isinstance(owner_obj, dict) else "Unknown"
                tracks_obj = item.get("tracks")
                track_total = tracks_obj.get("total", 0) if isinstance(tracks_obj, dict) else 0
                images = item.get("images") or []
                image_url = images[0].get("url") if images and isinstance(images[0], dict) else None

                playlists.append({
                    "id": item.get("id"),
                    "name": item.get("name"),
                    "description": item.get("description", ""),
                    "track_count": track_total,
                    "owner": owner_name,
                    "public": item.get("public", False),
                    "image_url": image_url,
                })

            if results.get("next"):
                results = sp.next(results)
            else:
                break

        return playlists
    except Exception as e:
        raise Exception(f"Spotify error: {e}")


def get_spotify_playlist_tracks(playlist_id: str) -> List[Dict[str, Any]]:
    """Get tracks from a Spotify playlist."""
    try:
        sp = get_spotify_client()
        tracks = []
        results = sp.playlist_tracks(playlist_id, limit=100)

        while results:
            for item in results.get("items", []):
                track = item.get("track")
                if track:
                    artists = track.get("artists") or []
                    artist_str = ", ".join(a.get("name", "") for a in artists if isinstance(a, dict))
                    album_obj = track.get("album") or {}
                    ext_ids = track.get("external_ids") or {}
                    ext_urls = track.get("external_urls") or {}

                    tracks.append({
                        "id": track.get("id"),
                        "title": track.get("name", "Unknown"),
                        "artist": artist_str or "Unknown",
                        "album": album_obj.get("name", ""),
                        "duration_ms": track.get("duration_ms", 0),
                        "isrc": ext_ids.get("isrc") if isinstance(ext_ids, dict) else None,
                        "uri": track.get("uri"),
                        "url": ext_urls.get("spotify") if isinstance(ext_urls, dict) else None,
                        "file_path": None,
                        "added_at": item.get("added_at"),
                    })

            if results.get("next"):
                results = sp.next(results)
            else:
                break

        return tracks
    except Exception as e:
        raise Exception(f"Spotify error: {e}")


# ==============================================================================
# QOBUZ SERVICE IMPLEMENTATION
# ==============================================================================

def get_qobuz_playlists() -> List[Dict[str, Any]]:
    """Get playlists from Qobuz."""
    try:
        from services.qobuz_service import QobuzService

        app_id = os.getenv("QOBUZ_APP_ID")
        app_secret = os.getenv("QOBUZ_APP_SECRET")
        token = os.getenv("QOBUZ_AUTH_TOKEN")

        if not all([app_id, app_secret, token]):
            raise Exception("Qobuz credentials not configured (QOBUZ_APP_ID, QOBUZ_APP_SECRET, QOBUZ_AUTH_TOKEN)")

        service = QobuzService(app_id, app_secret)
        service.auth_token = token

        try:
            user_playlists = asyncio.run(service.get_user_playlists())
            if user_playlists:
                return user_playlists
        except Exception:
            pass

        # Fallback to favorite albums as pseudo-playlists
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


def get_qobuz_playlist_tracks(playlist_id: str) -> List[Dict[str, Any]]:
    """Get tracks from a Qobuz playlist."""
    try:
        from services.qobuz_service import QobuzService

        app_id = os.getenv("QOBUZ_APP_ID")
        app_secret = os.getenv("QOBUZ_APP_SECRET")
        token = os.getenv("QOBUZ_AUTH_TOKEN")

        if not all([app_id, app_secret, token]):
            raise Exception("Qobuz credentials not configured (QOBUZ_APP_ID, QOBUZ_APP_SECRET, QOBUZ_AUTH_TOKEN)")

        service = QobuzService(app_id, app_secret)
        service.auth_token = token

        tracks_meta = asyncio.run(service.get_playlist_tracks(playlist_id))
        tracks = []
        for t in tracks_meta:
            tracks.append({
                "id": str(t.service_id),
                "title": t.title or "Unknown",
                "artist": ", ".join(t.artists) if t.artists else "Unknown",
                "album": t.album or "",
                "duration_ms": t.duration_ms or 0,
                "isrc": t.isrc,
                "file_path": None,
                "url": getattr(t, "url", None),
            })
        return tracks
    except Exception as e:
        raise Exception(f"Qobuz error: {e}")


# ==============================================================================
# TIDAL SERVICE IMPLEMENTATION
# ==============================================================================

def get_tidal_playlists() -> List[Dict[str, Any]]:
    """Get playlists from Tidal."""
    try:
        from services.tidal_service import TidalService

        token = os.getenv("TIDAL_ACCESS_TOKEN")
        if not token:
            raise Exception("Tidal not authenticated (TIDAL_ACCESS_TOKEN missing)")

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


def get_tidal_playlist_tracks(playlist_id: str) -> List[Dict[str, Any]]:
    """Get tracks from a Tidal playlist."""
    try:
        from services.tidal_service import TidalService

        token = os.getenv("TIDAL_ACCESS_TOKEN")
        if not token:
            raise Exception("Tidal not authenticated (TIDAL_ACCESS_TOKEN missing)")

        service = TidalService()
        service.access_token = token

        tracks_meta = asyncio.run(service.get_playlist_tracks(playlist_id))
        tracks = []
        for t in tracks_meta:
            tracks.append({
                "id": str(t.service_id),
                "title": t.title or "Unknown",
                "artist": ", ".join(t.artists) if t.artists else "Unknown",
                "album": t.album or "",
                "duration_ms": t.duration_ms or 0,
                "isrc": t.isrc,
                "file_path": None,
                "url": getattr(t, "url", None),
            })
        return tracks
    except Exception as e:
        raise Exception(f"Tidal error: {e}")


# ==============================================================================
# DEEZER SERVICE IMPLEMENTATION
# ==============================================================================

def get_deezer_playlists() -> List[Dict[str, Any]]:
    """Get playlists from Deezer."""
    try:
        from services.deezer_service import DeezerService
        service = DeezerService()
        return asyncio.run(service.get_user_playlists())
    except Exception as e:
        raise Exception(f"Deezer error: {e}")


def get_deezer_playlist_tracks(playlist_id: str) -> List[Dict[str, Any]]:
    """Get tracks from a Deezer playlist."""
    try:
        from services.deezer_service import DeezerService
        service = DeezerService()
        tracks_meta = asyncio.run(service.get_playlist_tracks(playlist_id))
        tracks = []
        for t in tracks_meta:
            tracks.append({
                "id": str(t.service_id),
                "title": t.title or "Unknown",
                "artist": ", ".join(t.artists) if t.artists else "Unknown",
                "album": t.album or "",
                "duration_ms": t.duration_ms or 0,
                "isrc": t.isrc,
                "file_path": None,
                "url": getattr(t, "url", None),
            })
        return tracks
    except Exception as e:
        raise Exception(f"Deezer error: {e}")


# ==============================================================================
# SOUNDCLOUD SERVICE IMPLEMENTATION
# ==============================================================================

def get_soundcloud_playlists() -> List[Dict[str, Any]]:
    """Get playlists from SoundCloud."""
    try:
        from services.soundcloud_service import SoundCloudService
        client_id = os.getenv("SOUNDCLOUD_CLIENT_ID")
        auth_token = os.getenv("SOUNDCLOUD_AUTH_TOKEN")
        service = SoundCloudService(client_id=client_id, auth_token=auth_token)
        return asyncio.run(service.get_user_playlists())
    except Exception as e:
        raise Exception(f"SoundCloud error: {e}")


def get_soundcloud_playlist_tracks(playlist_id: str) -> List[Dict[str, Any]]:
    """Get tracks from a SoundCloud playlist."""
    try:
        from services.soundcloud_service import SoundCloudService
        client_id = os.getenv("SOUNDCLOUD_CLIENT_ID")
        auth_token = os.getenv("SOUNDCLOUD_AUTH_TOKEN")
        service = SoundCloudService(client_id=client_id, auth_token=auth_token)
        tracks_meta = asyncio.run(service.get_playlist_tracks(playlist_id))
        tracks = []
        for t in tracks_meta:
            tracks.append({
                "id": str(t.service_id),
                "title": t.title or "Unknown",
                "artist": ", ".join(t.artists) if t.artists else "Unknown",
                "album": t.album or "",
                "duration_ms": t.duration_ms or 0,
                "isrc": t.isrc,
                "file_path": None,
                "url": getattr(t, "url", None),
            })
        return tracks
    except Exception as e:
        raise Exception(f"SoundCloud error: {e}")


# ==============================================================================
# LOCAL PLAYLIST & DATABASE INTEGRATION
# ==============================================================================

def find_syncify_db() -> Optional[Path]:
    """Find syncify.db on the local machine."""
    env_path = os.getenv("SYNCIFY_DB_PATH")
    if env_path and Path(env_path).is_file():
        return Path(env_path)

    candidates = [
        Path.home() / ".local" / "share" / "com.syncify.app" / "syncify.db",
        SCRIPTS_DIR.parent / "src-tauri" / "data" / "syncify.db",
        SCRIPTS_DIR.parent / "data" / "syncify.db",
        SCRIPTS_DIR.parent / "syncify.db",
    ]
    for c in candidates:
        if c.is_file():
            return c
    return None


def get_local_playlists() -> List[Dict[str, Any]]:
    """Get local playlists from SQLite database."""
    db_path = find_syncify_db()
    if not db_path:
        return []

    import sqlite3
    try:
        conn = sqlite3.connect(str(db_path))
        conn.row_factory = sqlite3.Row
        cur = conn.cursor()
        cur.execute("SELECT id, name, description, track_count FROM playlists ORDER BY id ASC")
        rows = cur.fetchall()
        conn.close()
        return [
            {
                "id": str(r["id"]),
                "name": r["name"],
                "description": r["description"] or "",
                "track_count": r["track_count"] or 0,
                "owner": "Local Library",
                "type": "local",
            }
            for r in rows
        ]
    except Exception:
        return []


def parse_m3u_file(path: Path) -> List[Dict[str, Any]]:
    """Parse an existing M3U or M3U8 file into unified track dictionaries."""
    tracks = []
    with open(path, "r", encoding="utf-8", errors="replace") as f:
        lines = [line.strip() for line in f if line.strip()]

    current_title = "Unknown"
    current_artist = "Unknown"
    current_duration_ms = 0
    current_isrc = None

    for line in lines:
        if line.startswith("#EXTINF:"):
            info = line[8:]
            parts = info.split(",", 1)
            try:
                sec = int(parts[0].strip())
                current_duration_ms = max(0, sec * 1000)
            except ValueError:
                current_duration_ms = 0
            if len(parts) > 1:
                artist_title = parts[1].split(" - ", 1)
                if len(artist_title) == 2:
                    current_artist = artist_title[0].strip()
                    current_title = artist_title[1].strip()
                else:
                    current_artist = "Unknown"
                    current_title = parts[1].strip()
        elif line.startswith("# ISRC:"):
            current_isrc = line[7:].strip()
        elif line.startswith("#"):
            continue
        else:
            file_path = line
            if not os.path.isabs(file_path) and not file_path.startswith(("http://", "https://", "file://")):
                file_path = str((path.parent / file_path).resolve())
            tracks.append({
                "id": str(len(tracks) + 1),
                "title": current_title,
                "artist": current_artist,
                "album": "",
                "duration_ms": current_duration_ms,
                "isrc": current_isrc,
                "file_path": file_path,
            })
            current_title = "Unknown"
            current_artist = "Unknown"
            current_duration_ms = 0
            current_isrc = None

    return tracks


def get_local_playlist_tracks(playlist_id: str) -> List[Dict[str, Any]]:
    """Get tracks from a local file (JSON / M3U / M3U8) or from the local SQLite database."""
    p = Path(playlist_id)
    if p.is_file():
        suffix = p.suffix.lower()
        if suffix == ".json":
            with open(p, "r", encoding="utf-8") as f:
                data = json.load(f)
            raw = data if isinstance(data, list) else data.get("tracks", [])
            tracks = []
            for t in raw:
                tracks.append({
                    "id": str(t.get("id", "")),
                    "title": t.get("title") or t.get("name") or "Unknown",
                    "artist": t.get("artist") or t.get("artist_name") or "Unknown",
                    "album": t.get("album") or t.get("album_title") or "",
                    "duration_ms": t.get("duration_ms", 0),
                    "isrc": t.get("isrc"),
                    "file_path": t.get("file_path") or t.get("path"),
                    "uri": t.get("uri"),
                    "url": t.get("url"),
                })
            return tracks

        if suffix in (".m3u", ".m3u8"):
            return parse_m3u_file(p)

    db_path = find_syncify_db()
    if db_path:
        import sqlite3
        try:
            conn = sqlite3.connect(str(db_path))
            conn.row_factory = sqlite3.Row
            cur = conn.cursor()
            query = """
                SELECT 
                    t.id,
                    t.title,
                    t.artist_name,
                    t.album_title,
                    t.duration_ms,
                    t.isrc,
                    d.file_path
                FROM playlist_tracks pt
                INNER JOIN tracks t ON t.id = pt.track_id
                LEFT JOIN downloads d ON d.track_id = t.id
                WHERE pt.playlist_id = ? 
                   OR pt.playlist_id IN (SELECT id FROM playlists WHERE service_playlist_id = ? OR name = ?)
                ORDER BY pt.position ASC, t.id ASC
            """
            cur.execute(query, (playlist_id, playlist_id, playlist_id))
            rows = cur.fetchall()
            conn.close()
            if rows:
                return [
                    {
                        "id": str(r["id"]),
                        "title": r["title"] or "Unknown",
                        "artist": r["artist_name"] or "Unknown",
                        "album": r["album_title"] or "",
                        "duration_ms": r["duration_ms"] or 0,
                        "isrc": r["isrc"],
                        "file_path": r["file_path"],
                    }
                    for r in rows
                ]
        except Exception:
            pass

    if p.suffix.lower() in (".json", ".m3u", ".m3u8") or "/" in playlist_id or "\\" in playlist_id:
        raise FileNotFoundError(f"Local playlist file not found: {playlist_id}")
    raise Exception(f"Local playlist '{playlist_id}' not found in database or filesystem")


# ==============================================================================
# DISPATCHER & EXPORT FUNCTIONS
# ==============================================================================

def get_tracks_for_service(service: str, playlist_id: str) -> List[Dict[str, Any]]:
    """Retrieve tracks for any supported streaming service or local playlist."""
    svc = service.lower()
    if svc == "spotify":
        return get_spotify_playlist_tracks(playlist_id)
    elif svc == "qobuz":
        return get_qobuz_playlist_tracks(playlist_id)
    elif svc == "tidal":
        return get_tidal_playlist_tracks(playlist_id)
    elif svc == "deezer":
        return get_deezer_playlist_tracks(playlist_id)
    elif svc == "soundcloud":
        return get_soundcloud_playlist_tracks(playlist_id)
    elif svc in ("local", "file", "library"):
        return get_local_playlist_tracks(playlist_id)
    else:
        raise ValueError(f"Unsupported service: {service}")


def build_m3u_content(tracks: List[Dict[str, Any]]) -> str:
    """Generate valid M3U / M3U8 formatted playlist text with #EXTM3U and file paths."""
    lines = ["#EXTM3U"]
    for track in tracks:
        duration_ms = track.get("duration_ms")
        try:
            secs = max(0, int(duration_ms) // 1000) if duration_ms is not None else 0
        except (ValueError, TypeError):
            secs = 0

        artist = (track.get("artist") or track.get("artist_name") or "Unknown").strip()
        title = (track.get("title") or track.get("name") or "Unknown").strip()
        lines.append(f"#EXTINF:{secs},{artist} - {title}")

        # Resource line (file path, URI, URL, or fallback)
        file_path = track.get("file_path") or track.get("path")
        if file_path:
            lines.append(str(file_path))
        elif track.get("uri"):
            lines.append(str(track["uri"]))
        elif track.get("url"):
            lines.append(str(track["url"]))
        else:
            lines.append(f"{artist} - {title}.mp3")

    return "\n".join(lines) + "\n"


def export_playlist_data(
    service: str,
    playlist_id: str,
    format_type: str = "json",
    output_path: Optional[str] = None,
) -> Dict[str, Any]:
    """Retrieve tracks and generate export payload in the requested format (json, m3u, m3u8)."""
    service_norm = service.lower()
    fmt_norm = format_type.lower()
    tracks = get_tracks_for_service(service_norm, playlist_id)

    if fmt_norm == "json":
        result = {
            "service": service_norm,
            "playlist_id": playlist_id,
            "track_count": len(tracks),
            "tracks": tracks,
        }
        if output_path:
            out_p = Path(output_path)
            out_p.parent.mkdir(parents=True, exist_ok=True)
            with open(out_p, "w", encoding="utf-8") as f:
                json.dump(result, f, indent=2, ensure_ascii=False)
            result["output_file"] = str(out_p)
        return result

    elif fmt_norm in ("m3u", "m3u8"):
        content = build_m3u_content(tracks)
        result = {
            "service": service_norm,
            "playlist_id": playlist_id,
            "format": fmt_norm,
            "track_count": len(tracks),
            "content": content,
        }
        if output_path:
            out_p = Path(output_path)
            out_p.parent.mkdir(parents=True, exist_ok=True)
            with open(out_p, "w", encoding="utf-8") as f:
                f.write(content)
            result["output_file"] = str(out_p)
        return result

    else:
        raise ValueError(f"Unknown format: {format_type}. Allowed: json, m3u, m3u8")


# ==============================================================================
# CLI COMMAND HANDLERS
# ==============================================================================

def list_playlists(service: str):
    """List playlists from a service."""
    service_norm = service.lower()
    try:
        if service_norm == "spotify":
            playlists = get_spotify_playlists()
        elif service_norm == "qobuz":
            playlists = get_qobuz_playlists()
        elif service_norm == "tidal":
            playlists = get_tidal_playlists()
        elif service_norm == "deezer":
            playlists = get_deezer_playlists()
        elif service_norm == "soundcloud":
            playlists = get_soundcloud_playlists()
        elif service_norm in ("local", "library"):
            playlists = get_local_playlists()
        else:
            json_response(False, error=f"Unsupported service: {service}")
            return

        json_response(True, {
            "service": service_norm,
            "count": len(playlists),
            "playlists": playlists,
        })
    except Exception as e:
        json_response(False, error=str(e))


def get_playlist(service: str, playlist_id: str):
    """Get playlist tracks from a service or local source."""
    try:
        tracks = get_tracks_for_service(service, playlist_id)
        json_response(True, {
            "service": service.lower(),
            "playlist_id": playlist_id,
            "track_count": len(tracks),
            "tracks": tracks,
        })
    except Exception as e:
        json_response(False, error=str(e))


def export_playlist(service: str, playlist_id: str, format_type: str = "json", output_path: Optional[str] = None):
    """Export playlist to a file format (json, m3u, m3u8)."""
    try:
        data = export_playlist_data(service, playlist_id, format_type, output_path)
        json_response(True, data)
    except Exception as e:
        json_response(False, error=str(e))


def match_playlist_tracks(playlist_file: str, target_service: str):
    """Match playlist tracks to another service using ISRC or metadata."""
    try:
        p = Path(playlist_file)
        if not p.exists():
            raise FileNotFoundError(f"Playlist file not found: {playlist_file}")

        if p.suffix.lower() in (".m3u", ".m3u8"):
            tracks = parse_m3u_file(p)
        else:
            with open(p, "r", encoding="utf-8") as f:
                playlist_data = json.load(f)
            tracks = playlist_data.get("tracks", []) if isinstance(playlist_data, dict) else playlist_data

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

    parser = argparse.ArgumentParser(description="Playlist sync between services and local library")
    subparsers = parser.add_subparsers(dest="command", required=True)

    # List command
    list_parser = subparsers.add_parser("list", help="List playlists from service")
    list_parser.add_argument("service", help="Service name (spotify, qobuz, tidal, deezer, soundcloud, local)")

    # Get command
    get_parser = subparsers.add_parser("get", help="Get playlist tracks")
    get_parser.add_argument("service", help="Service name")
    get_parser.add_argument("playlist_id", help="Playlist ID or path to local playlist")

    # Export command
    export_parser = subparsers.add_parser("export", help="Export playlist")
    export_parser.add_argument("service", help="Service name (spotify, qobuz, tidal, deezer, soundcloud, local)")
    export_parser.add_argument("playlist_id", help="Playlist ID or path to local playlist")
    export_parser.add_argument(
        "--format", "-f", default="json",
        choices=["json", "m3u", "m3u8"], help="Export format (json, m3u, m3u8)"
    )
    export_parser.add_argument("--output", "-o", default=None, help="Optional output file destination")

    # Match command
    match_parser = subparsers.add_parser("match", help="Match playlist to target service")
    match_parser.add_argument("playlist_file", help="Path to exported playlist file (JSON, M3U, M3U8)")
    match_parser.add_argument("target_service", help="Target service to match against")

    args = parser.parse_args()

    if args.command == "list":
        list_playlists(args.service)
    elif args.command == "get":
        get_playlist(args.service, args.playlist_id)
    elif args.command == "export":
        export_playlist(args.service, args.playlist_id, args.format, args.output)
    elif args.command == "match":
        match_playlist_tracks(args.playlist_file, args.target_service)


if __name__ == "__main__":
    main()

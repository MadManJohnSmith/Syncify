#!/usr/bin/env python3
"""
Scanner Bridge - Scan local music library and extract metadata.

Usage:
    python scanner_bridge.py scan <directory> [--recursive] [--limit N]
    python scanner_bridge.py metadata <audio_file>

Returns JSON:
    {"success": true/false, "data": {...}, "error": "..."}
"""

import json
import sys
import os
import hashlib
import subprocess
from pathlib import Path
from typing import Optional, List, Dict, Any
from dataclasses import dataclass, asdict

# Load .env from project root
try:
    from dotenv import load_dotenv
    load_dotenv(Path(__file__).parent.parent / ".env")
except ImportError:
    pass

try:
    from mutagen import File as MutagenFile
    from mutagen.easyid3 import EasyID3
    from mutagen.flac import FLAC
    from mutagen.mp4 import MP4
    MUTAGEN_AVAILABLE = True
except ImportError:
    MUTAGEN_AVAILABLE = False


AUDIO_EXTENSIONS = {'.mp3', '.flac', '.m4a', '.wav', '.ogg', '.aac', '.wma', '.opus'}


def json_response(success: bool, data=None, error=None):
    """Output JSON response and exit."""
    result = {"success": success}
    if data:
        result["data"] = data
    if error:
        result["error"] = error
    print(json.dumps(result, ensure_ascii=False, default=str))
    sys.exit(0 if success else 1)


@dataclass
class TrackInfo:
    """Extracted track information."""
    file_path: str
    file_name: str
    file_size: int
    format: str
    title: Optional[str] = None
    artist: Optional[str] = None
    album: Optional[str] = None
    album_artist: Optional[str] = None
    track_number: Optional[int] = None
    disc_number: Optional[int] = None
    year: Optional[int] = None
    genre: Optional[str] = None
    duration_seconds: Optional[float] = None
    bitrate: Optional[int] = None
    sample_rate: Optional[int] = None
    channels: Optional[int] = None
    has_cover_art: bool = False
    isrc: Optional[str] = None
    bit_depth: Optional[int] = None
    file_hash: Optional[str] = None
    audio_source: Optional[str] = None


def compute_sha256(file_path: Path) -> str:
    """Compute SHA256 of a file."""
    h = hashlib.sha256()
    with open(file_path, "rb") as f:
        while chunk := f.read(65536):
            h.update(chunk)
    return h.hexdigest()


def extract_metadata_ffprobe(file_path: Path) -> Optional[TrackInfo]:
    """Extract metadata using ffprobe if mutagen is missing or fails."""
    try:
        cmd = [
            "ffprobe",
            "-v", "quiet",
            "-print_format", "json",
            "-show_format",
            "-show_streams",
            str(file_path),
        ]
        res = subprocess.run(cmd, capture_output=True, text=True, timeout=10)
        if res.returncode != 0:
            return None
        data = json.loads(res.stdout)
        format_info = data.get("format", {})
        raw_tags = format_info.get("tags", {})
        tags = {k.upper(): v for k, v in raw_tags.items()}

        streams = data.get("streams", [])
        audio_stream = next((s for s in streams if s.get("codec_type") == "audio"), None)
        if not audio_stream and streams:
            audio_stream = streams[0]

        info = TrackInfo(
            file_path=str(file_path.absolute()),
            file_name=file_path.name,
            file_size=file_path.stat().st_size,
            format=file_path.suffix[1:].lower(),
            title=tags.get("TITLE"),
            artist=tags.get("ARTIST"),
            album=tags.get("ALBUM"),
            album_artist=tags.get("ALBUMARTIST") or tags.get("ALBUM_ARTIST"),
            genre=tags.get("GENRE"),
            isrc=tags.get("ISRC"),
            audio_source=tags.get("SYNCIFY_AUDIO_SOURCE") or tags.get("SOURCE"),
        )

        if "TRACK" in tags:
            try:
                info.track_number = int(str(tags["TRACK"]).split("/")[0])
            except Exception:
                pass
        if "DISC" in tags:
            try:
                info.disc_number = int(str(tags["DISC"]).split("/")[0])
            except Exception:
                pass
        if "DATE" in tags:
            try:
                info.year = int(str(tags["DATE"])[:4])
            except Exception:
                pass

        if audio_stream:
            if "sample_rate" in audio_stream:
                try:
                    info.sample_rate = int(audio_stream["sample_rate"])
                except Exception:
                    pass
            if "channels" in audio_stream:
                try:
                    info.channels = int(audio_stream["channels"])
                except Exception:
                    pass
            raw_bits = audio_stream.get("bits_per_raw_sample") or tags.get("BITDEPTH")
            if raw_bits:
                try:
                    info.bit_depth = int(raw_bits)
                except Exception:
                    pass
            if "bit_rate" in audio_stream:
                try:
                    info.bitrate = int(audio_stream["bit_rate"])
                except Exception:
                    pass

        if "duration" in format_info:
            try:
                info.duration_seconds = float(format_info["duration"])
            except Exception:
                pass
        if not info.bitrate and "bit_rate" in format_info:
            try:
                info.bitrate = int(format_info["bit_rate"])
            except Exception:
                pass

        return info
    except Exception:
        return None


def extract_metadata(file_path: Path) -> Optional[TrackInfo]:
    """Extract metadata from an audio file."""
    if not MUTAGEN_AVAILABLE:
        ff_info = extract_metadata_ffprobe(file_path)
        if ff_info:
            return ff_info
        return TrackInfo(
            file_path=str(file_path.absolute()),
            file_name=file_path.name,
            file_size=file_path.stat().st_size,
            format=file_path.suffix[1:].lower(),
        )
    
    try:
        audio = MutagenFile(str(file_path))
        if audio is None:
            ff_info = extract_metadata_ffprobe(file_path)
            if ff_info:
                return ff_info
            return None
        
        info = TrackInfo(
            file_path=str(file_path.absolute()),
            file_name=file_path.name,
            file_size=file_path.stat().st_size,
            format=file_path.suffix[1:].lower(),
        )
        
        # Get audio info
        if hasattr(audio, 'info'):
            if hasattr(audio.info, 'length'):
                info.duration_seconds = audio.info.length
            if hasattr(audio.info, 'bitrate'):
                info.bitrate = audio.info.bitrate
            if hasattr(audio.info, 'sample_rate'):
                info.sample_rate = audio.info.sample_rate
            if hasattr(audio.info, 'channels'):
                info.channels = audio.info.channels
            if hasattr(audio.info, 'bits_per_sample'):
                info.bit_depth = audio.info.bits_per_sample
        
        # Extract tags based on file type
        if file_path.suffix.lower() == '.mp3':
            try:
                tags = EasyID3(str(file_path))
                info.title = tags.get('title', [None])[0]
                info.artist = tags.get('artist', [None])[0]
                info.album = tags.get('album', [None])[0]
                info.album_artist = tags.get('albumartist', [None])[0]
                info.genre = tags.get('genre', [None])[0]
                
                track = tags.get('tracknumber', [None])[0]
                if track:
                    info.track_number = int(track.split('/')[0])
                
                disc = tags.get('discnumber', [None])[0]
                if disc:
                    info.disc_number = int(disc.split('/')[0])
                
                date = tags.get('date', [None])[0]
                if date:
                    info.year = int(date[:4])
            except:
                pass
            
            # Check for cover art & ISRC in ID3
            try:
                from mutagen.id3 import ID3
                id3 = ID3(str(file_path))
                info.has_cover_art = any(k.startswith('APIC') for k in id3.keys())
                tsrc = id3.get('TSRC')
                if tsrc and hasattr(tsrc, 'text') and tsrc.text:
                    info.isrc = str(tsrc.text[0])
            except:
                pass
                
        elif file_path.suffix.lower() == '.flac':
            flac = FLAC(str(file_path))
            info.title = flac.get('title', [None])[0]
            info.artist = flac.get('artist', [None])[0]
            info.album = flac.get('album', [None])[0]
            info.album_artist = flac.get('albumartist', [None])[0]
            info.genre = flac.get('genre', [None])[0]
            info.isrc = flac.get('isrc', [None])[0]
            info.audio_source = flac.get('syncify_audio_source', [None])[0] or flac.get('source', [None])[0]
            if hasattr(flac, 'info') and hasattr(flac.info, 'bits_per_sample'):
                info.bit_depth = flac.info.bits_per_sample
            
            track = flac.get('tracknumber', [None])[0]
            if track:
                info.track_number = int(track.split('/')[0])
            
            disc = flac.get('discnumber', [None])[0]
            if disc:
                info.disc_number = int(disc.split('/')[0])
            
            date = flac.get('date', [None])[0]
            if date:
                info.year = int(date[:4])
            
            info.has_cover_art = len(flac.pictures) > 0
            
        elif file_path.suffix.lower() in ('.m4a', '.mp4', '.aac'):
            mp4 = MP4(str(file_path))
            info.title = mp4.tags.get('\xa9nam', [None])[0] if mp4.tags else None
            info.artist = mp4.tags.get('\xa9ART', [None])[0] if mp4.tags else None
            info.album = mp4.tags.get('\xa9alb', [None])[0] if mp4.tags else None
            info.album_artist = mp4.tags.get('aART', [None])[0] if mp4.tags else None
            info.genre = mp4.tags.get('\xa9gen', [None])[0] if mp4.tags else None
            
            if mp4.tags:
                isrc_raw = mp4.tags.get('----:com.apple.iTunes:ISRC', [None])[0]
                if isrc_raw:
                    info.isrc = isrc_raw.decode('utf-8', errors='ignore') if isinstance(isrc_raw, bytes) else str(isrc_raw)
                src_raw = mp4.tags.get('----:com.apple.iTunes:SOURCE', [None])[0]
                if src_raw:
                    info.audio_source = src_raw.decode('utf-8', errors='ignore') if isinstance(src_raw, bytes) else str(src_raw)

                track = mp4.tags.get('trkn', [(None, None)])[0]
                if track and track[0]:
                    info.track_number = track[0]
                
                disc = mp4.tags.get('disk', [(None, None)])[0]
                if disc and disc[0]:
                    info.disc_number = disc[0]
                
                date = mp4.tags.get('\xa9day', [None])[0]
                if date:
                    info.year = int(date[:4])
                
                info.has_cover_art = 'covr' in mp4.tags
        
        return info
        
    except Exception as e:
        print(f"Error extracting metadata from {file_path}: {e}", file=sys.stderr)
        return None


def scan_directory(directory: str, recursive: bool = True, limit: Optional[int] = None):
    """Scan a directory for audio files."""
    dir_path = Path(directory)
    if not dir_path.exists():
        json_response(False, error=f"Directory not found: {directory}")
        return
    
    if not dir_path.is_dir():
        json_response(False, error=f"Not a directory: {directory}")
        return
    
    # Find audio files
    if recursive:
        audio_files = []
        for ext in AUDIO_EXTENSIONS:
            audio_files.extend(dir_path.rglob(f"*{ext}"))
    else:
        audio_files = [f for f in dir_path.iterdir() 
                       if f.is_file() and f.suffix.lower() in AUDIO_EXTENSIONS]
    
    # Apply limit
    if limit:
        audio_files = audio_files[:limit]
    
    # Extract metadata
    tracks = []
    errors = []
    
    for file in audio_files:
        try:
            info = extract_metadata(file)
            if info:
                tracks.append(asdict(info))
        except Exception as e:
            errors.append({"file": str(file), "error": str(e)})
    
    json_response(True, {
        "directory": str(dir_path.absolute()),
        "total_files": len(audio_files),
        "tracks": tracks,
        "errors": errors if errors else None,
    })


def get_file_metadata(audio_path: str):
    """Get metadata for a single audio file."""
    path = Path(audio_path)
    if not path.exists():
        json_response(False, error=f"File not found: {audio_path}")
        return
    
    if path.suffix.lower() not in AUDIO_EXTENSIONS:
        json_response(False, error=f"Not an audio file: {audio_path}")
        return
    
    info = extract_metadata(path)
    if info:
        json_response(True, asdict(info))
    else:
        json_response(False, error="Failed to extract metadata")


def main():
    import argparse
    
    parser = argparse.ArgumentParser(description="Scan local music library")
    subparsers = parser.add_subparsers(dest="command", required=True)
    
    # Scan command
    scan_parser = subparsers.add_parser("scan", help="Scan directory for audio files")
    scan_parser.add_argument("directory", help="Directory to scan")
    scan_parser.add_argument("--recursive", "-r", action="store_true", default=True,
                             help="Scan subdirectories")
    scan_parser.add_argument("--no-recursive", dest="recursive", action="store_false",
                             help="Don't scan subdirectories")
    scan_parser.add_argument("--limit", "-l", type=int, help="Limit number of files")
    
    # Metadata command
    meta_parser = subparsers.add_parser("metadata", help="Get file metadata")
    meta_parser.add_argument("audio_file", help="Path to audio file")
    
    args = parser.parse_args()
    
    if args.command == "scan":
        scan_directory(args.directory, args.recursive, args.limit)
    elif args.command == "metadata":
        get_file_metadata(args.audio_file)


if __name__ == "__main__":
    main()

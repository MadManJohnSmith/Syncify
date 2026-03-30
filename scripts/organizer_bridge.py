#!/usr/bin/env python3
"""
Organizer Bridge - Organize audio files into proper folder structure.

Usage:
    python organizer_bridge.py organize <source_dir> <target_dir> [--pattern <pattern>]
    python organizer_bridge.py rename <audio_file> [--pattern <pattern>]
    python organizer_bridge.py preview <source_dir> [--pattern <pattern>]

Patterns:
    {artist}/{album}/{track:02d} - {title}  (default)
    {album_artist}/{album}/{disc:02d}-{track:02d} {title}
    {genre}/{artist}/{album}/{track:02d} {title}

Returns JSON:
    {"success": true/false, "data": {...}, "error": "..."}
"""

import json
import sys
import os
import re
import shutil
from pathlib import Path
from typing import Optional, List, Dict, Any
from dataclasses import dataclass

# Load .env from project root
from dotenv import load_dotenv
load_dotenv(Path(__file__).parent.parent / ".env")

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


def sanitize_filename(name: str) -> str:
    """Remove invalid characters from filename."""
    if not name:
        return "Unknown"
    # Remove invalid characters
    name = re.sub(r'[<>:"/\\|?*]', '', name)
    # Remove leading/trailing whitespace and dots
    name = name.strip(' .')
    return name or "Unknown"


def extract_tags(file_path: Path) -> Dict[str, Any]:
    """Extract metadata tags from audio file."""
    tags = {
        "title": None,
        "artist": None,
        "album": None,
        "album_artist": None,
        "track": None,
        "disc": None,
        "year": None,
        "genre": None,
    }
    
    if not MUTAGEN_AVAILABLE:
        return tags
    
    try:
        ext = file_path.suffix.lower()
        
        if ext == '.mp3':
            try:
                audio = EasyID3(str(file_path))
                tags["title"] = audio.get('title', [None])[0]
                tags["artist"] = audio.get('artist', [None])[0]
                tags["album"] = audio.get('album', [None])[0]
                tags["album_artist"] = audio.get('albumartist', [None])[0]
                tags["genre"] = audio.get('genre', [None])[0]
                
                track = audio.get('tracknumber', [None])[0]
                if track:
                    tags["track"] = int(track.split('/')[0])
                
                disc = audio.get('discnumber', [None])[0]
                if disc:
                    tags["disc"] = int(disc.split('/')[0])
                
                date = audio.get('date', [None])[0]
                if date:
                    tags["year"] = int(date[:4])
            except:
                pass
                
        elif ext == '.flac':
            flac = FLAC(str(file_path))
            tags["title"] = flac.get('title', [None])[0]
            tags["artist"] = flac.get('artist', [None])[0]
            tags["album"] = flac.get('album', [None])[0]
            tags["album_artist"] = flac.get('albumartist', [None])[0]
            tags["genre"] = flac.get('genre', [None])[0]
            
            track = flac.get('tracknumber', [None])[0]
            if track:
                tags["track"] = int(track.split('/')[0])
            
            disc = flac.get('discnumber', [None])[0]
            if disc:
                tags["disc"] = int(disc.split('/')[0])
            
            date = flac.get('date', [None])[0]
            if date:
                tags["year"] = int(date[:4])
                
        elif ext in ('.m4a', '.mp4', '.aac'):
            mp4 = MP4(str(file_path))
            if mp4.tags:
                tags["title"] = mp4.tags.get('\xa9nam', [None])[0]
                tags["artist"] = mp4.tags.get('\xa9ART', [None])[0]
                tags["album"] = mp4.tags.get('\xa9alb', [None])[0]
                tags["album_artist"] = mp4.tags.get('aART', [None])[0]
                tags["genre"] = mp4.tags.get('\xa9gen', [None])[0]
                
                track = mp4.tags.get('trkn', [(None, None)])[0]
                if track and track[0]:
                    tags["track"] = track[0]
                
                disc = mp4.tags.get('disk', [(None, None)])[0]
                if disc and disc[0]:
                    tags["disc"] = disc[0]
                
                date = mp4.tags.get('\xa9day', [None])[0]
                if date:
                    tags["year"] = int(date[:4])
    except:
        pass
    
    return tags


def format_path(pattern: str, tags: Dict[str, Any], file_path: Path) -> str:
    """Format a path using the pattern and tags."""
    # Defaults
    artist = sanitize_filename(tags.get("album_artist") or tags.get("artist") or "Unknown Artist")
    album = sanitize_filename(tags.get("album") or "Unknown Album")
    title = sanitize_filename(tags.get("title") or file_path.stem)
    genre = sanitize_filename(tags.get("genre") or "Unknown Genre")
    year = tags.get("year") or ""
    track = tags.get("track") or 0
    disc = tags.get("disc") or 1
    
    # Format the pattern
    result = pattern.format(
        artist=artist,
        album_artist=artist,
        album=album,
        title=title,
        genre=genre,
        year=year,
        track=track,
        disc=disc,
    )
    
    # Add extension
    result += file_path.suffix
    
    return result


def preview_organization(source_dir: str, pattern: str):
    """Preview how files would be organized."""
    source = Path(source_dir)
    if not source.exists():
        json_response(False, error=f"Directory not found: {source_dir}")
        return
    
    # Find audio files
    audio_files = []
    for ext in AUDIO_EXTENSIONS:
        audio_files.extend(source.rglob(f"*{ext}"))
    
    if not audio_files:
        json_response(False, error="No audio files found")
        return
    
    previews = []
    for file in audio_files[:50]:  # Limit preview
        tags = extract_tags(file)
        new_path = format_path(pattern, tags, file)
        previews.append({
            "original": str(file.relative_to(source)),
            "new_path": new_path,
            "artist": tags.get("artist"),
            "album": tags.get("album"),
            "title": tags.get("title"),
        })
    
    json_response(True, {
        "total_files": len(audio_files),
        "preview_count": len(previews),
        "previews": previews,
    })


def organize_files(source_dir: str, target_dir: str, pattern: str, copy: bool = False):
    """Organize audio files into folder structure."""
    source = Path(source_dir)
    target = Path(target_dir)
    
    if not source.exists():
        json_response(False, error=f"Source directory not found: {source_dir}")
        return
    
    target.mkdir(parents=True, exist_ok=True)
    
    # Find audio files
    audio_files = []
    for ext in AUDIO_EXTENSIONS:
        audio_files.extend(source.rglob(f"*{ext}"))
    
    if not audio_files:
        json_response(False, error="No audio files found")
        return
    
    organized = []
    errors = []
    
    for file in audio_files:
        try:
            tags = extract_tags(file)
            new_relative_path = format_path(pattern, tags, file)
            new_path = target / new_relative_path
            
            # Create directories
            new_path.parent.mkdir(parents=True, exist_ok=True)
            
            # Copy or move
            if copy:
                shutil.copy2(file, new_path)
            else:
                shutil.move(str(file), str(new_path))
            
            organized.append({
                "original": str(file),
                "new_path": str(new_path),
            })
        except Exception as e:
            errors.append({
                "file": str(file),
                "error": str(e),
            })
    
    json_response(True, {
        "organized": len(organized),
        "errors": len(errors),
        "files": organized[:20],  # Limit output
        "error_list": errors if errors else None,
    })


def main():
    import argparse
    
    parser = argparse.ArgumentParser(description="Organize audio files into folder structure")
    subparsers = parser.add_subparsers(dest="command", required=True)
    
    # Preview command
    preview_parser = subparsers.add_parser("preview", help="Preview organization")
    preview_parser.add_argument("source_dir", help="Source directory")
    preview_parser.add_argument("--pattern", "-p", 
                                default="{artist}/{album}/{track:02d} - {title}",
                                help="Organization pattern")
    
    # Organize command
    org_parser = subparsers.add_parser("organize", help="Organize files")
    org_parser.add_argument("source_dir", help="Source directory")
    org_parser.add_argument("target_dir", help="Target directory")
    org_parser.add_argument("--pattern", "-p",
                            default="{artist}/{album}/{track:02d} - {title}",
                            help="Organization pattern")
    org_parser.add_argument("--copy", "-c", action="store_true",
                            help="Copy instead of move")
    
    args = parser.parse_args()
    
    if args.command == "preview":
        preview_organization(args.source_dir, args.pattern)
    elif args.command == "organize":
        organize_files(args.source_dir, args.target_dir, args.pattern, args.copy)


if __name__ == "__main__":
    main()

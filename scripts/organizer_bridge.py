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
import string
from pathlib import Path
from typing import Optional, List, Dict, Any
from dataclasses import dataclass

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
INVALID_FS_CHARS = re.compile(r'[<>:"/\\|?*\x00-\x1f]')


def json_response(success: bool, data=None, error=None):
    """Output JSON response and exit."""
    result = {"success": success}
    if data:
        result["data"] = data
    if error:
        result["error"] = error
    print(json.dumps(result, ensure_ascii=False, default=str))
    sys.exit(0 if success else 1)


def sanitize_filename(name: str, replacement: str = "_") -> str:
    """Remove or replace invalid characters from filename component."""
    if not name:
        return "Unknown"
    # Replace invalid filesystem characters
    name = INVALID_FS_CHARS.sub(replacement, str(name))
    # Remove leading/trailing whitespace and dots
    name = name.strip(' .')
    return name or "Unknown"


class SafeTemplateFormatter(string.Formatter):
    """
    A robust string.Formatter that safely handles missing keys,
    format specifier mismatches, and positional index errors without throwing exceptions.
    """

    def __init__(self, default: str = ""):
        super().__init__()
        self.default = default

    def get_value(self, key, args, kwargs):
        if isinstance(key, str):
            val = kwargs.get(key, self.default)
            return self.default if val is None else val
        try:
            val = super().get_value(key, args, kwargs)
            return self.default if val is None else val
        except (IndexError, KeyError):
            return self.default

    def format_field(self, value, format_spec):
        try:
            return super().format_field(value, format_spec)
        except Exception:
            if format_spec and isinstance(value, str) and value.isdigit():
                try:
                    return super().format_field(int(value), format_spec)
                except Exception:
                    pass
            return str(value) if value is not None else self.default


def _sanitize_unbalanced_braces(template: str) -> str:
    """
    Strip or normalize stray braces in template strings to prevent
    ValueError on format parse.
    """
    pattern = re.compile(r'(\{\{|\}\}|\{[a-zA-Z0-9_]*(?:![rsa])?(?::[^{}]*)?\})')
    parts = []
    last_idx = 0
    for m in pattern.finditer(template):
        literal = template[last_idx:m.start()]
        literal = literal.replace('{', '').replace('}', '')
        parts.append(literal)
        parts.append(m.group(0))
        last_idx = m.end()
    trailing = template[last_idx:].replace('{', '').replace('}', '')
    parts.append(trailing)
    return ''.join(parts)


def safe_format_template(template: str, tags: Dict[str, Any], fallback_filename: str = "") -> str:
    """
    Safely format a file/folder naming template using a tags dictionary.

    Guarantees:
    - Missing keys do not raise KeyError (substituted with empty string or default).
    - Unbalanced or invalid braces do not raise ValueError (sanitizes or falls back).
    - Illegal filesystem characters (<>:"/\\|?*) are replaced in all path components.
    """
    if not template or not isinstance(template, str):
        return sanitize_filename(fallback_filename) if fallback_filename else "Unknown"

    safe_tags = {}
    for k, v in tags.items():
        if isinstance(v, str):
            safe_tags[k] = INVALID_FS_CHARS.sub("_", v).strip(' .')
        else:
            safe_tags[k] = v

    formatter = SafeTemplateFormatter(default="")
    formatted = None

    try:
        formatted = formatter.format(template, **safe_tags)
    except Exception:
        try:
            repaired = _sanitize_unbalanced_braces(template)
            formatted = formatter.format(repaired, **safe_tags)
        except Exception:
            formatted = fallback_filename or safe_tags.get("title") or safe_tags.get("artist") or "Unknown"

    if not formatted:
        formatted = fallback_filename or "Unknown"

    # Normalize path separators and sanitize each path component
    normalized = formatted.replace("\\", "/")
    raw_components = normalized.split("/")
    clean_components = []
    for comp in raw_components:
        if not comp.strip():
            continue
        cleaned = INVALID_FS_CHARS.sub("_", comp).strip(' .')
        if cleaned:
            clean_components.append(cleaned)

    if not clean_components:
        return sanitize_filename(fallback_filename) if fallback_filename else "Unknown"

    return "/".join(clean_components)


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
    context = dict(tags) if tags else {}
    artist = sanitize_filename(tags.get("album_artist") or tags.get("artist") or "Unknown Artist")
    album = sanitize_filename(tags.get("album") or "Unknown Album")
    title = sanitize_filename(tags.get("title") or file_path.stem)
    genre = sanitize_filename(tags.get("genre") or "Unknown Genre")
    year = tags.get("year") or ""
    track = tags.get("track") if tags.get("track") is not None else 0
    disc = tags.get("disc") if tags.get("disc") is not None else 1

    context["artist"] = artist
    context["album_artist"] = artist
    context["album"] = album
    context["title"] = title
    context["genre"] = genre
    context["year"] = year
    context["track"] = track
    context["disc"] = disc

    result = safe_format_template(pattern, context, fallback_filename=file_path.stem)

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

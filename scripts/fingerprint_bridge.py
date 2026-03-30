#!/usr/bin/env python3
"""
Fingerprint Bridge - CLI interface for audio fingerprinting via AcoustID.

Usage:
    python fingerprint_bridge.py identify <audio_file>
    python fingerprint_bridge.py fingerprint <audio_file>
    python fingerprint_bridge.py duplicates <dir_or_file> [<dir_or_file>...]
    python fingerprint_bridge.py check  # Check if fpcalc is available

Returns JSON:
    {"success": true/false, "data": {...}, "error": "..."}

Requirements:
    - fpcalc binary (from Chromaprint) in PATH or ./bin/
    - pyacoustid package
"""

import json
import sys
import os
import argparse
from pathlib import Path
from typing import Optional, List

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


def check_availability():
    """Check if fpcalc is available."""
    from services.acoustid_matcher import AcoustIDMatcher
    
    matcher = AcoustIDMatcher(verbose=True)
    available = matcher.is_available()
    
    json_response(True, {
        "available": available,
        "fpcalc_path": str(matcher.fpcalc_path) if matcher.fpcalc_path else None,
    })


def get_fingerprint(audio_path: str):
    """Generate fingerprint for an audio file."""
    from services.acoustid_matcher import AcoustIDMatcher
    
    path = Path(audio_path)
    if not path.exists():
        json_response(False, error=f"File not found: {audio_path}")
        return
    
    api_key = os.getenv("ACOUSTID_API_KEY")
    matcher = AcoustIDMatcher(api_key=api_key, verbose=True)
    
    if not matcher.is_available():
        json_response(False, error="fpcalc not found. Install Chromaprint.")
        return
    
    try:
        result = matcher.get_fingerprint(path)
        
        if result:
            duration, fingerprint = result
            json_response(True, {
                "duration": duration,
                "fingerprint": fingerprint,
                "file": str(path.absolute()),
            })
        else:
            json_response(False, error="Failed to generate fingerprint")
            
    except Exception as e:
        json_response(False, error=str(e))


def identify_track(audio_path: str):
    """Identify a track using audio fingerprint."""
    from services.acoustid_matcher import AcoustIDMatcher
    
    path = Path(audio_path)
    if not path.exists():
        json_response(False, error=f"File not found: {audio_path}")
        return
    
    api_key = os.getenv("ACOUSTID_API_KEY")
    matcher = AcoustIDMatcher(api_key=api_key, verbose=True)
    
    if not matcher.is_available():
        json_response(False, error="fpcalc not found. Install Chromaprint.")
        return
    
    try:
        results = matcher.identify(path)
        
        if results:
            matches = []
            for r in results[:5]:  # Top 5 matches
                matches.append({
                    "acoustid": r.acoustid,
                    "score": r.score,
                    "recording_id": r.recording_id,
                    "title": r.title,
                    "artist": r.artist,
                    "album": r.album,
                    "duration": r.duration,
                })
            
            json_response(True, {
                "matches": matches,
                "file": str(path.absolute()),
            })
        else:
            json_response(False, error="No matches found")
            
    except Exception as e:
        json_response(False, error=str(e))


def find_duplicates(paths: List[str]):
    """Find duplicate audio files based on fingerprints."""
    from services.acoustid_matcher import AcoustIDMatcher
    
    # Collect all audio files from paths
    audio_extensions = {'.mp3', '.flac', '.m4a', '.wav', '.ogg', '.aac', '.wma'}
    audio_files = []
    
    for path_str in paths:
        path = Path(path_str)
        if path.is_file():
            if path.suffix.lower() in audio_extensions:
                audio_files.append(path)
        elif path.is_dir():
            for ext in audio_extensions:
                audio_files.extend(path.rglob(f"*{ext}"))
    
    if not audio_files:
        json_response(False, error="No audio files found")
        return
    
    api_key = os.getenv("ACOUSTID_API_KEY")
    matcher = AcoustIDMatcher(api_key=api_key, verbose=True)
    
    if not matcher.is_available():
        json_response(False, error="fpcalc not found. Install Chromaprint.")
        return
    
    try:
        duplicates = matcher.find_duplicates(audio_files)
        
        # Filter to only groups with 2+ files (actual duplicates)
        dup_groups = {
            fp: [str(p) for p in files]
            for fp, files in duplicates.items()
            if len(files) > 1
        }
        
        json_response(True, {
            "total_files_scanned": len(audio_files),
            "duplicate_groups": len(dup_groups),
            "duplicates": dup_groups,
        })
        
    except Exception as e:
        json_response(False, error=str(e))


def main():
    parser = argparse.ArgumentParser(description="Audio fingerprinting via AcoustID")
    subparsers = parser.add_subparsers(dest="command", required=True)
    
    # Check command
    subparsers.add_parser("check", help="Check if fpcalc is available")
    
    # Fingerprint command
    fp_parser = subparsers.add_parser("fingerprint", help="Generate audio fingerprint")
    fp_parser.add_argument("audio_file", help="Path to audio file")
    
    # Identify command
    id_parser = subparsers.add_parser("identify", help="Identify track via AcoustID")
    id_parser.add_argument("audio_file", help="Path to audio file")
    
    # Duplicates command
    dup_parser = subparsers.add_parser("duplicates", help="Find duplicate audio files")
    dup_parser.add_argument("paths", nargs="+", help="Files or directories to scan")
    
    args = parser.parse_args()
    
    if args.command == "check":
        check_availability()
    elif args.command == "fingerprint":
        get_fingerprint(args.audio_file)
    elif args.command == "identify":
        identify_track(args.audio_file)
    elif args.command == "duplicates":
        find_duplicates(args.paths)


if __name__ == "__main__":
    main()

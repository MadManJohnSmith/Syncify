#!/usr/bin/env python3
"""
Conversion Bridge - Audio format conversion via FFmpeg.

Usage:
    python conversion_bridge.py convert <input> <output> [--format mp3|flac|m4a|ogg]
    python conversion_bridge.py info <audio_file>
    python conversion_bridge.py check  # Check if ffmpeg is available

Returns JSON:
    {"success": true/false, "data": {...}, "error": "..."}
"""

import json
import sys
import os
import subprocess
import shutil
from pathlib import Path
from typing import Optional

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


def find_ffmpeg():
    """Find ffmpeg binary."""
    # Check common locations
    locations = [
        shutil.which("ffmpeg"),
        "C:/ffmpeg/bin/ffmpeg.exe",
        "C:/Program Files/ffmpeg/bin/ffmpeg.exe",
        str(Path(__file__).parent.parent / "bin" / "ffmpeg.exe"),
    ]
    
    for loc in locations:
        if loc and Path(loc).exists():
            return loc
    
    return shutil.which("ffmpeg")


def check_availability():
    """Check if ffmpeg is available."""
    ffmpeg_path = find_ffmpeg()
    
    if ffmpeg_path:
        try:
            result = subprocess.run(
                [ffmpeg_path, "-version"],
                capture_output=True,
                text=True
            )
            version_line = result.stdout.split('\n')[0] if result.stdout else None
            json_response(True, {
                "available": True,
                "path": ffmpeg_path,
                "version": version_line,
            })
        except Exception as e:
            json_response(True, {
                "available": False,
                "error": str(e),
            })
    else:
        json_response(True, {
            "available": False,
            "path": None,
        })


def get_audio_info(audio_path: str):
    """Get audio file metadata using ffprobe."""
    path = Path(audio_path)
    if not path.exists():
        json_response(False, error=f"File not found: {audio_path}")
        return
    
    ffmpeg_path = find_ffmpeg()
    if not ffmpeg_path:
        json_response(False, error="ffmpeg not found")
        return
    
    # ffprobe is usually alongside ffmpeg
    ffprobe_path = Path(ffmpeg_path).parent / ("ffprobe.exe" if sys.platform == "win32" else "ffprobe")
    if not ffprobe_path.exists():
        ffprobe_path = shutil.which("ffprobe")
    
    if not ffprobe_path:
        json_response(False, error="ffprobe not found")
        return
    
    try:
        result = subprocess.run(
            [
                str(ffprobe_path),
                "-v", "quiet",
                "-print_format", "json",
                "-show_format",
                "-show_streams",
                str(path)
            ],
            capture_output=True,
            text=True
        )
        
        if result.returncode == 0:
            info = json.loads(result.stdout)
            
            # Extract relevant info
            audio_stream = None
            for stream in info.get("streams", []):
                if stream.get("codec_type") == "audio":
                    audio_stream = stream
                    break
            
            fmt = info.get("format", {})
            
            data = {
                "file": str(path.absolute()),
                "format": fmt.get("format_name"),
                "duration_seconds": float(fmt.get("duration", 0)),
                "size_bytes": int(fmt.get("size", 0)),
                "bitrate": int(fmt.get("bit_rate", 0)) if fmt.get("bit_rate") else None,
            }
            
            if audio_stream:
                data.update({
                    "codec": audio_stream.get("codec_name"),
                    "sample_rate": int(audio_stream.get("sample_rate", 0)),
                    "channels": audio_stream.get("channels"),
                    "bits_per_sample": audio_stream.get("bits_per_raw_sample"),
                })
            
            json_response(True, data)
        else:
            json_response(False, error=result.stderr)
            
    except Exception as e:
        json_response(False, error=str(e))


def convert_audio(input_path: str, output_path: str, format: str = "mp3", quality: str = "high"):
    """Convert audio file to different format."""
    input_file = Path(input_path)
    if not input_file.exists():
        json_response(False, error=f"Input file not found: {input_path}")
        return
    
    ffmpeg_path = find_ffmpeg()
    if not ffmpeg_path:
        json_response(False, error="ffmpeg not found")
        return
    
    output_file = Path(output_path)
    output_file.parent.mkdir(parents=True, exist_ok=True)
    
    # Build ffmpeg command based on format
    cmd = [ffmpeg_path, "-y", "-i", str(input_file)]
    
    if format == "mp3":
        # Quality settings for MP3
        quality_map = {
            "low": ["-q:a", "7"],      # ~100kbps VBR
            "medium": ["-q:a", "4"],   # ~165kbps VBR
            "high": ["-q:a", "0"],     # ~245kbps VBR
            "320": ["-b:a", "320k"],   # 320kbps CBR
        }
        cmd.extend(quality_map.get(quality, quality_map["high"]))
        cmd.append(str(output_file))
        
    elif format == "flac":
        cmd.extend(["-c:a", "flac", "-compression_level", "8"])
        cmd.append(str(output_file))
        
    elif format == "m4a":
        quality_map = {
            "low": ["-b:a", "128k"],
            "medium": ["-b:a", "192k"],
            "high": ["-b:a", "256k"],
        }
        cmd.extend(["-c:a", "aac"])
        cmd.extend(quality_map.get(quality, quality_map["high"]))
        cmd.append(str(output_file))
        
    elif format == "ogg":
        quality_map = {
            "low": ["-q:a", "3"],
            "medium": ["-q:a", "5"],
            "high": ["-q:a", "8"],
        }
        cmd.extend(["-c:a", "libvorbis"])
        cmd.extend(quality_map.get(quality, quality_map["high"]))
        cmd.append(str(output_file))
        
    elif format == "wav":
        cmd.extend(["-c:a", "pcm_s16le"])
        cmd.append(str(output_file))
    
    else:
        json_response(False, error=f"Unsupported format: {format}")
        return
    
    try:
        result = subprocess.run(cmd, capture_output=True, text=True, timeout=300)
        
        if result.returncode == 0 and output_file.exists():
            json_response(True, {
                "input": str(input_file.absolute()),
                "output": str(output_file.absolute()),
                "format": format,
                "size_bytes": output_file.stat().st_size,
            })
        else:
            json_response(False, error=result.stderr or "Conversion failed")
            
    except subprocess.TimeoutExpired:
        json_response(False, error="Conversion timed out")
    except Exception as e:
        json_response(False, error=str(e))


def main():
    import argparse
    
    parser = argparse.ArgumentParser(description="Audio format conversion via FFmpeg")
    subparsers = parser.add_subparsers(dest="command", required=True)
    
    # Check command
    subparsers.add_parser("check", help="Check if ffmpeg is available")
    
    # Info command
    info_parser = subparsers.add_parser("info", help="Get audio file info")
    info_parser.add_argument("audio_file", help="Path to audio file")
    
    # Convert command
    convert_parser = subparsers.add_parser("convert", help="Convert audio format")
    convert_parser.add_argument("input", help="Input audio file")
    convert_parser.add_argument("output", help="Output file path")
    convert_parser.add_argument("--format", "-f", default="mp3",
                                choices=["mp3", "flac", "m4a", "ogg", "wav"],
                                help="Output format")
    convert_parser.add_argument("--quality", "-q", default="high",
                                choices=["low", "medium", "high", "320"],
                                help="Quality level")
    
    args = parser.parse_args()
    
    if args.command == "check":
        check_availability()
    elif args.command == "info":
        get_audio_info(args.audio_file)
    elif args.command == "convert":
        convert_audio(args.input, args.output, args.format, args.quality)


if __name__ == "__main__":
    main()

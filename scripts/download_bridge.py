#!/usr/bin/env python3
"""
Download Bridge - CLI interface to download tracks via Syncify-test services.

Usage:
    python download_bridge.py <service> <track_id> [--output <path>] [--quality <level>]

Services: qobuz, tidal, deezer
Quality: lossless (default), hifi, high, low

Returns JSON:
    {"success": true/false, "data": {...}, "error": "..."}
"""

import json
import sys
import os
import argparse
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


def get_qobuz_service():
    """Initialize Qobuz service with credentials."""
    from services.service_base import ServiceCredentials
    from services.qobuz_service import QobuzService
    
    creds = ServiceCredentials(
        app_id=os.getenv("QOBUZ_APP_ID", ""),
        app_secret=os.getenv("QOBUZ_APP_SECRET", ""),
        username=os.getenv("QOBUZ_USERNAME", ""),
        password=os.getenv("QOBUZ_PASSWORD", ""),
    )
    
    service = QobuzService(creds, verbose=True)
    if not service.authenticate():
        raise Exception("Qobuz authentication failed")
    
    return service


def get_tidal_service():
    """Initialize Tidal service with credentials."""
    from services.service_base import ServiceCredentials
    from services.tidal_service import TidalService
    
    # Tidal uses access token from OAuth
    token = os.getenv("TIDAL_ACCESS_TOKEN", "")
    if not token:
        raise Exception("TIDAL_ACCESS_TOKEN not set")
    
    creds = ServiceCredentials(access_token=token)
    return TidalService(creds, verbose=True)


def get_deezer_service():
    """Initialize Deezer service with credentials."""
    from services.service_base import ServiceCredentials
    from services.deezer_service import DeezerService
    
    arl = os.getenv("DEEZER_ARL", "")
    if not arl:
        raise Exception("DEEZER_ARL not set")
    
    creds = ServiceCredentials(arl=arl)
    return DeezerService(creds, verbose=True)


def download_qobuz(track_id: str, output_path: str, quality: str):
    """Download a track from Qobuz."""
    from services.service_base import DownloadQuality
    
    service = get_qobuz_service()
    
    # Map quality string to enum
    quality_map = {
        "lossless": DownloadQuality.LOSSLESS,
        "hifi": DownloadQuality.HI_RES,
        "high": DownloadQuality.HIGH,
        "low": DownloadQuality.STANDARD,
    }
    
    dl_quality = quality_map.get(quality.lower(), DownloadQuality.LOSSLESS)
    
    result = service.download_track(
        track_id=track_id,
        output_path=output_path,
        quality=dl_quality.value
    )
    
    if result.success:
        json_response(True, {
            "file_path": result.file_path,
            "format": result.format,
            "size_bytes": result.size_bytes,
        })
    else:
        json_response(False, error=result.error_message)


def download_tidal(track_id: str, output_path: str, quality: str):
    """Download a track from Tidal."""
    from services.service_base import DownloadQuality
    
    service = get_tidal_service()
    
    quality_map = {
        "lossless": DownloadQuality.LOSSLESS,
        "hifi": DownloadQuality.HI_RES,
        "high": DownloadQuality.HIGH,
        "low": DownloadQuality.STANDARD,
    }
    
    dl_quality = quality_map.get(quality.lower(), DownloadQuality.LOSSLESS)
    
    result = service.download_track(
        track_id=track_id,
        output_path=output_path,
        quality=dl_quality.value
    )
    
    if result.success:
        json_response(True, {
            "file_path": result.file_path,
            "format": result.format,
            "size_bytes": result.size_bytes,
        })
    else:
        json_response(False, error=result.error_message)


def download_deezer(track_id: str, output_path: str, quality: str):
    """Download a track from Deezer."""
    from services.service_base import DownloadQuality
    
    service = get_deezer_service()
    
    quality_map = {
        "lossless": DownloadQuality.LOSSLESS,
        "high": DownloadQuality.HIGH,
        "low": DownloadQuality.STANDARD,
    }
    
    dl_quality = quality_map.get(quality.lower(), DownloadQuality.LOSSLESS)
    
    result = service.download_track(
        track_id=track_id,
        output_path=output_path,
        quality=dl_quality.value
    )
    
    if result.success:
        json_response(True, {
            "file_path": result.file_path,
            "format": result.format,
            "size_bytes": result.size_bytes,
        })
    else:
        json_response(False, error=result.error_message)


HANDLERS = {
    "qobuz": download_qobuz,
    "tidal": download_tidal,
    "deezer": download_deezer,
}


def main():
    parser = argparse.ArgumentParser(description="Download tracks from streaming services")
    parser.add_argument("service", choices=list(HANDLERS.keys()), help="Service to download from")
    parser.add_argument("track_id", help="Track ID to download")
    parser.add_argument("--output", "-o", default="./downloads", help="Output directory")
    parser.add_argument("--quality", "-q", default="lossless", help="Quality level")
    
    args = parser.parse_args()
    
    try:
        HANDLERS[args.service](args.track_id, args.output, args.quality)
    except Exception as e:
        json_response(False, error=str(e))


if __name__ == "__main__":
    main()

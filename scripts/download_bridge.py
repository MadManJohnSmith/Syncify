#!/usr/bin/env python3
"""
Download Bridge - CLI interface to download tracks via Syncify music services.

CLI contract (mirrored from src-tauri/src/commands/tools.rs :: download_track):
    python download_bridge.py download <service> <track_id> [--output <path>] [--quality <level>]
    python download_bridge.py <service> <track_id> [--output <path>] [--quality <level>]
    python download_bridge.py --service <service> [--track-id <id>] [--output <path>] [--quality <level>]

Services: qobuz, tidal, deezer, soundcloud
Quality: lossless (default), hires, hires_96, standard, high, low

stdout carries EXACTLY ONE JSON document (the whole stdout is parsed by Rust):
    {"success": true,  "data": {"file_path": "...", "format": "...", "size_bytes": 123, ...}}
    {"success": false, "error": "..."}

Exit codes: 0 = success, 1 = download/execution failure, 2 = invalid arguments.
Any incidental library/service output is routed to stderr so it never breaks
the JSON contract.
"""

import argparse
import asyncio
import inspect
import json
import os
import sys
from pathlib import Path
from typing import Any, Dict, Optional, Tuple

# Add local services to path (S43: relocated from adjacent_tools/Syncify-test)
SCRIPTS_DIR = Path(__file__).parent
if str(SCRIPTS_DIR) not in sys.path:
    sys.path.insert(0, str(SCRIPTS_DIR))

# Load .env from project root if available
try:
    from dotenv import load_dotenv
    load_dotenv(Path(__file__).parent.parent / ".env")
except ImportError:
    pass

from services.service_base import (
    DownloadQuality,
    DownloadResult,
    ServiceCredentials,
    ServiceType,
)


QUALITY_MAP = {
    # Lossless CD
    "lossless": DownloadQuality.LOSSLESS_CD,
    "cd": DownloadQuality.LOSSLESS_CD,
    "flac": DownloadQuality.LOSSLESS_CD,
    "lossless_cd": DownloadQuality.LOSSLESS_CD,
    # Hi-Res
    "hires": DownloadQuality.LOSSLESS_HIRES,
    "hi_res": DownloadQuality.LOSSLESS_HIRES,
    "hi-res": DownloadQuality.LOSSLESS_HIRES,
    "hifi": DownloadQuality.LOSSLESS_HIRES,
    "lossless_hires": DownloadQuality.LOSSLESS_HIRES,
    # Hi-Res 96kHz
    "hires_96": DownloadQuality.LOSSLESS_HIRES_96,
    "hi_res_96": DownloadQuality.LOSSLESS_HIRES_96,
    "lossless_hires_96": DownloadQuality.LOSSLESS_HIRES_96,
    # Standard Lossy
    "high": DownloadQuality.LOSSY_STANDARD,
    "standard": DownloadQuality.LOSSY_STANDARD,
    "320": DownloadQuality.LOSSY_STANDARD,
    "lossy_standard": DownloadQuality.LOSSY_STANDARD,
    # Low Lossy
    "low": DownloadQuality.LOSSY_LOW,
    "128": DownloadQuality.LOSSY_LOW,
    "lossy_low": DownloadQuality.LOSSY_LOW,
}


def map_quality(quality: Any) -> DownloadQuality:
    """Map string quality specifier to canonical DownloadQuality enum."""
    if isinstance(quality, DownloadQuality):
        return quality
    key = str(quality).strip().lower()
    return QUALITY_MAP.get(key, DownloadQuality.LOSSLESS_CD)


def resolve_output_path(output_path: str, service: str, track_id: str, quality: DownloadQuality) -> str:
    """
    Ensure output_path is a concrete file path, creating parent directories if needed.
    If a directory path is supplied, generate a filename inside it.
    """
    path = Path(output_path)
    if path.is_dir() or not path.suffix:
        ext = "flac" if quality in (
            DownloadQuality.LOSSLESS_CD,
            DownloadQuality.LOSSLESS_HIRES,
            DownloadQuality.LOSSLESS_HIRES_96,
        ) else "mp3"
        path = path / f"{service}_{track_id}.{ext}"
    path.parent.mkdir(parents=True, exist_ok=True)
    return str(path)


def build_response(success: bool, data: Optional[Dict[str, Any]] = None, error: Optional[str] = None) -> Dict[str, Any]:
    """Build structured response dictionary."""
    payload: Dict[str, Any] = {"success": success}
    if data is not None:
        payload["data"] = data
    if error is not None:
        payload["error"] = str(error)
    return payload


def json_response(
    success: bool,
    data: Optional[Dict[str, Any]] = None,
    error: Optional[str] = None,
    exit_process: bool = True,
) -> Dict[str, Any]:
    """Output JSON response to stdout and optionally exit process."""
    payload = build_response(success, data, error)
    print(json.dumps(payload, ensure_ascii=False))
    if exit_process:
        sys.exit(0 if success else 1)
    return payload


async def close_service(service: Any) -> None:
    """Close a service's aiohttp session, tolerating varied service implementations."""
    try:
        closer = getattr(service, "close", None)
        if callable(closer):
            res = closer()
            if inspect.isawaitable(res):
                await res
            return
        session = getattr(service, "session", None)
        if session is not None and not session.closed:
            await session.close()
    except Exception:
        # Never let teardown failures mask the download result.
        pass


async def get_qobuz_service() -> Any:
    """Initialize and authenticate Qobuz service."""
    app_id = os.getenv("QOBUZ_APP_ID", "")
    app_secret = os.getenv("QOBUZ_APP_SECRET", "")
    username = os.getenv("QOBUZ_USERNAME", "")
    password = os.getenv("QOBUZ_PASSWORD", "")

    if not username or not password:
        raise ValueError("QOBUZ_USERNAME and QOBUZ_PASSWORD environment variables are required")

    from services.qobuz_service import QobuzService

    creds = ServiceCredentials(
        service_type=ServiceType.QOBUZ,
        username=username,
        password=password,
        client_id=app_id or None,
        client_secret=app_secret or None,
        extra={"app_id": app_id, "app_secret": app_secret} if (app_id or app_secret) else None,
    )
    service = QobuzService(creds, verbose=False)
    authenticated = await service.authenticate()
    if not authenticated:
        await close_service(service)
        raise RuntimeError("Qobuz authentication failed")
    return service


async def download_qobuz(track_id: str, output_path: str, quality: str) -> DownloadResult:
    """Download a track from Qobuz."""
    service = await get_qobuz_service()
    try:
        dl_quality = map_quality(quality)
        dest_path = resolve_output_path(output_path, "qobuz", track_id, dl_quality)
        return await service.download_track(
            track_id=track_id,
            output_path=dest_path,
            quality=dl_quality,
        )
    finally:
        await close_service(service)


async def get_tidal_service() -> Any:
    """Initialize and authenticate Tidal service."""
    token = os.getenv("TIDAL_ACCESS_TOKEN", "")
    if not token:
        raise ValueError("TIDAL_ACCESS_TOKEN environment variable is required")
    refresh_token = os.getenv("TIDAL_REFRESH_TOKEN", "")

    from services.tidal_service import TidalService

    creds = ServiceCredentials(
        service_type=ServiceType.TIDAL,
        token=token,
        refresh_token=refresh_token or None,
        extra={"access_token": token},
    )
    service = TidalService(creds, verbose=False)
    authenticated = await service.authenticate()
    if not authenticated:
        await close_service(service)
        raise RuntimeError("Tidal authentication failed")
    return service


async def download_tidal(track_id: str, output_path: str, quality: str) -> DownloadResult:
    """Download a track from Tidal."""
    service = await get_tidal_service()
    try:
        dl_quality = map_quality(quality)
        if dl_quality == DownloadQuality.LOSSLESS_HIRES_96:
            dl_quality = DownloadQuality.LOSSLESS_HIRES
        dest_path = resolve_output_path(output_path, "tidal", track_id, dl_quality)
        return await service.download_track(
            track_id=track_id,
            output_path=dest_path,
            quality=dl_quality,
        )
    finally:
        await close_service(service)


async def get_deezer_service() -> Any:
    """Initialize and authenticate Deezer service."""
    arl = os.getenv("DEEZER_ARL", "")
    if not arl:
        raise ValueError("DEEZER_ARL environment variable is required")

    from services.deezer_service import DeezerService

    creds = ServiceCredentials(
        service_type=ServiceType.DEEZER,
        token=arl,
        extra={"arl": arl},
    )
    service = DeezerService(creds, verbose=False)
    authenticated = await service.authenticate()
    if not authenticated:
        await close_service(service)
        raise RuntimeError("Deezer authentication failed")
    return service


async def download_deezer(track_id: str, output_path: str, quality: str) -> DownloadResult:
    """Download a track from Deezer."""
    service = await get_deezer_service()
    try:
        dl_quality = map_quality(quality)
        if dl_quality == DownloadQuality.LOSSLESS_HIRES_96:
            dl_quality = DownloadQuality.LOSSLESS_HIRES
        dest_path = resolve_output_path(output_path, "deezer", track_id, dl_quality)
        return await service.download_track(
            track_id=track_id,
            output_path=Path(dest_path),
            quality=dl_quality,
        )
    finally:
        await close_service(service)


async def get_soundcloud_service() -> Any:
    """Initialize SoundCloud service."""
    from services.soundcloud_service import SoundCloudService

    token = os.getenv("SOUNDCLOUD_AUTH_TOKEN", "")
    client_id = os.getenv("SOUNDCLOUD_CLIENT_ID", "")

    creds = None
    if token or client_id:
        creds = ServiceCredentials(
            service_type=ServiceType.SOUNDCLOUD,
            token=token or None,
            client_id=client_id or None,
            extra={"client_id": client_id} if client_id else None,
        )
    return SoundCloudService(creds, verbose=False)


async def download_soundcloud(track_id: str, output_path: str, quality: str) -> DownloadResult:
    """Download a track from SoundCloud."""
    service = await get_soundcloud_service()
    try:
        dl_quality = map_quality(quality)
        dest_path = resolve_output_path(output_path, "soundcloud", track_id, dl_quality)
        return await service.download_track(
            track_id=track_id,
            output_path=dest_path,
            quality=dl_quality,
        )
    finally:
        await close_service(service)


HANDLERS = {
    "qobuz": download_qobuz,
    "tidal": download_tidal,
    "deezer": download_deezer,
    "soundcloud": download_soundcloud,
}


async def async_execute_download(
    service_name: str,
    track_id: str,
    output_path: str,
    quality: str,
) -> Dict[str, Any]:
    """Execute download asynchronously for specified service and format response payload."""
    service_key = service_name.lower().strip()
    handler = HANDLERS.get(service_key)
    if not handler:
        return build_response(False, error=f"Unsupported service: {service_name}")

    try:
        res = handler(track_id, output_path, quality)
        if inspect.isawaitable(res):
            result = await res
        else:
            result = res

        if isinstance(result, DownloadResult):
            if result.success:
                filepath = result.filepath or result.file_path
                file_size = result.file_size_bytes or result.size_bytes
                # Legacy results may carry an explicit format attribute
                # (e.g. soundcloud_service.py); otherwise derive from extension.
                fmt = getattr(result, "format", None)
                if isinstance(fmt, str) and fmt.strip():
                    fmt = fmt.lstrip(".").lower()
                else:
                    fmt = None
                if fmt is None and filepath:
                    ext = Path(filepath).suffix.lstrip(".").lower()
                    if ext:
                        fmt = ext

                data = {
                    "file_path": filepath,
                    "filepath": filepath,
                    "format": fmt,
                    "size_bytes": file_size,
                    "file_size_bytes": file_size,
                    "download_duration_seconds": result.download_duration_seconds,
                }
                return build_response(True, data=data)
            else:
                return build_response(False, error=result.error_message or "Download failed")
        elif isinstance(result, dict):
            return result
        else:
            return build_response(False, error=f"Unexpected download result type: {type(result)}")
    except Exception as e:
        return build_response(False, error=str(e))


async def async_main(args_list: Optional[list] = None) -> Tuple[Dict[str, Any], int]:
    """
    Async main routine: parse arguments, dispatch the download and return
    ``(payload, exit_code)``. Printing is deliberately left to ``main()`` so
    that stdout can be reserved exclusively for the final JSON document.
    """
    if args_list is None:
        args_list = sys.argv[1:]

    # Rust CLI compatibility: strip optional leading "download" subcommand
    if args_list and args_list[0] == "download":
        args_list = args_list[1:]

    parser = argparse.ArgumentParser(
        prog="download_bridge.py",
        description="Download tracks from streaming services",
    )
    parser.add_argument("service", nargs="?", default=None, help="Service to download from")
    parser.add_argument("track_id", nargs="?", default=None, help="Track ID to download")
    parser.add_argument("--service", "-s", dest="service_flag", default=None, help="Service (alternative to positional)")
    parser.add_argument("--track-id", "-t", dest="track_id_flag", default=None, help="Track ID (alternative to positional)")
    parser.add_argument("--output", "-o", default="./downloads", help="Output directory")
    parser.add_argument("--quality", "-q", default="lossless", help="Quality level")

    try:
        args = parser.parse_args(args_list)
    except SystemExit as e:
        code = e.code if isinstance(e.code, int) else 1
        payload = build_response(
            False,
            error=(
                "Invalid arguments for download_bridge.py "
                f"(valid services: {', '.join(HANDLERS.keys())}); "
                "usage: download_bridge.py [download] <service> <track_id> "
                "[--service <svc>] [--track-id <id>] [--output <path>] [--quality <level>]"
            ),
        )
        return payload, code

    # Resolve service/track_id from either positional or flag form:
    #   download_bridge.py [download] <service> <track_id> [options]
    #   download_bridge.py --service <svc> <track_id> [options]
    #   download_bridge.py --service <svc> --track-id <id> [options]
    service = args.service_flag
    track_id = args.track_id_flag
    positionals = [p for p in (args.service, args.track_id) if p is not None]
    if service:
        if track_id is None and positionals:
            track_id = positionals[0]
    else:
        if len(positionals) >= 1:
            service = positionals[0]
        if len(positionals) >= 2:
            track_id = positionals[1]

    if not service or not track_id:
        payload = build_response(
            False,
            error=(
                "Both <service> and <track_id> are required "
                "(positionally or via --service/--track-id)"
            ),
        )
        return payload, 2
    if str(service).lower().strip() not in HANDLERS:
        payload = build_response(
            False,
            error=(
                f"Unsupported service: {service} "
                f"(valid services: {', '.join(HANDLERS.keys())})"
            ),
        )
        return payload, 2

    payload = await async_execute_download(service, track_id, args.output, args.quality)
    return payload, (0 if payload.get("success") else 1)


def main() -> None:
    """
    Process entry point.

    stdout hygiene: the Rust host (run_bridge_command) parses the ENTIRE
    stdout as a single JSON value, so all incidental output (library logs,
    service ``print()`` calls, ``_log(..., "error")`` messages) is diverted
    to stderr while the bridge runs, and only the final JSON document is
    written to the real stdout.
    """
    real_stdout = sys.stdout
    payload: Dict[str, Any] = {"success": False, "error": "download_bridge did not produce a result"}
    exit_code = 1
    try:
        try:
            real_stdout.reconfigure(encoding="utf-8")  # type: ignore[union-attr]
        except (AttributeError, ValueError):
            pass
        sys.stdout = sys.stderr
        try:
            payload, exit_code = asyncio.run(async_main())
        except SystemExit as e:
            exit_code = e.code if isinstance(e.code, int) else 1
            payload = build_response(False, error=f"download_bridge exited with code {exit_code}")
        except BaseException as e:  # final safety net: never traceback to stdout
            payload = build_response(False, error=f"{type(e).__name__}: {e}")
            exit_code = 1
    finally:
        sys.stdout = real_stdout

    try:
        real_stdout.write(json.dumps(payload, ensure_ascii=False) + "\n")
        real_stdout.flush()
    except UnicodeEncodeError:
        real_stdout.write(json.dumps(payload, ensure_ascii=True) + "\n")
        real_stdout.flush()
    sys.exit(exit_code)


if __name__ == "__main__":
    main()

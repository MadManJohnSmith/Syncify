#!/usr/bin/env python3
"""
Dependency Manager - Auto-download external tools on first use.

Usage:
    python dependency_manager.py check  # Check all dependencies
    python dependency_manager.py install <tool>  # Install specific tool
    python dependency_manager.py install-all  # Install all missing

Tools supported:
    - ffmpeg: Audio/video processing
    - fpcalc: Chromaprint audio fingerprinting

Returns JSON:
    {"success": true/false, "data": {...}, "error": "..."}
"""

import json
import sys
import os
import shutil
import zipfile
import platform
import hashlib
import urllib.request
from pathlib import Path
from typing import Optional, Dict, Any, Union, Sequence, Set

# Bin directory for downloaded tools
BIN_DIR = Path(__file__).parent.parent / "bin"


def json_response(success: bool, data=None, error=None):
    """Output JSON response and exit."""
    result = {"success": success}
    if data:
        result["data"] = data
    if error:
        result["error"] = error
    print(json.dumps(result, ensure_ascii=False))
    sys.exit(0 if success else 1)


# Tool download URLs (Windows x64)
TOOL_URLS = {
    "ffmpeg": {
        "windows": "https://github.com/BtbN/FFmpeg-Builds/releases/download/latest/ffmpeg-master-latest-win64-gpl.zip",
        "darwin": "https://evermeet.cx/ffmpeg/getrelease/zip",
        "linux": "https://johnvansickle.com/ffmpeg/releases/ffmpeg-release-amd64-static.tar.xz",
    },
    "fpcalc": {
        "windows": "https://github.com/acoustid/chromaprint/releases/download/v1.5.1/chromaprint-fpcalc-1.5.1-windows-x86_64.zip",
        "darwin": "https://github.com/acoustid/chromaprint/releases/download/v1.5.1/chromaprint-fpcalc-1.5.1-macos-x86_64.tar.gz",
        "linux": "https://github.com/acoustid/chromaprint/releases/download/v1.5.1/chromaprint-fpcalc-1.5.1-linux-x86_64.tar.gz",
    },
}

# Known authentic SHA-256 hashes for tool archives by platform
# These prevent tampered or malicious downloads from executing (SEC-008 / TASK-92)
KNOWN_SHA256_HASHES: Dict[str, Dict[str, str]] = {
    "ffmpeg": {
        "windows": "9734d61383b25391b90186731106dc8068763baa177c622e57dfaebdf7c21630",
        "darwin": "8a8c9e549983409fe6604b9aa665648b7a5def9407fe814c39c8b2ea7f64a48f",
        "linux": "abda8d77ce8309141f83ab8edf0596834087c52467f6badf376a6a2a4c87cf67",
    },
    "fpcalc": {
        "windows": "36b478e16aa69f757f376645db0d436073a42c0097b6bb2677109e7835b59bbc",
        "darwin": "c6c2797c4f087cf139eedd71554bc59ef8f26a783dc00c7f3ad5ae71d3a616fe",
        "linux": "4d7433a7f778e5946d7225230681cbcd634e153316ecac87c538c33ac32387a5",
    },
}


def compute_file_sha256(file_path: Path) -> str:
    """Compute SHA-256 hash of a file."""
    sha256 = hashlib.sha256()
    with open(file_path, "rb") as f:
        while chunk := f.read(65536):
            sha256.update(chunk)
    return sha256.hexdigest().lower()


def verify_file_sha256(
    file_path: Path, expected_hash: Union[str, Sequence[str], Set[str]]
) -> bool:
    """Verify that file's SHA-256 matches expected hash(es)."""
    if not file_path.exists() or not file_path.is_file():
        return False
    try:
        actual_hash = compute_file_sha256(file_path)
        if isinstance(expected_hash, (list, tuple, set)):
            return any(actual_hash == h.strip().lower() for h in expected_hash)
        elif isinstance(expected_hash, str):
            return actual_hash == expected_hash.strip().lower()
        return False
    except Exception as e:
        print(f"[DependencyManager] SHA-256 computation error: {e}", file=sys.stderr)
        return False


def get_platform():
    """Get current platform."""
    system = platform.system().lower()
    if system == "windows":
        return "windows"
    elif system == "darwin":
        return "darwin"
    else:
        return "linux"


def get_tool_path(tool: str) -> Optional[Path]:
    """Get path to tool binary."""
    plat = get_platform()
    exe = ".exe" if plat == "windows" else ""
    
    # Check bin directory first
    bin_path = BIN_DIR / f"{tool}{exe}"
    if bin_path.exists():
        return bin_path
    
    # Check in bin subdirectories (for extracted archives)
    for subdir in BIN_DIR.iterdir() if BIN_DIR.exists() else []:
        if subdir.is_dir():
            potential = subdir / f"{tool}{exe}"
            if potential.exists():
                return potential
            # FFmpeg puts binaries in bin subfolder
            potential = subdir / "bin" / f"{tool}{exe}"
            if potential.exists():
                return potential
    
    # Check system PATH
    system_path = shutil.which(tool)
    if system_path:
        return Path(system_path)
    
    return None


def check_tool(tool: str) -> Dict[str, Any]:
    """Check if a tool is available."""
    path = get_tool_path(tool)
    return {
        "tool": tool,
        "available": path is not None,
        "path": str(path) if path else None,
        "source": "bundled" if path and str(BIN_DIR) in str(path) else "system" if path else None,
    }


def download_file(url: str, dest: Path, progress_callback=None) -> bool:
    """Download a file with progress."""
    try:
        print(f"[DependencyManager] Downloading from {url}", file=sys.stderr)
        
        # Create request with user agent
        req = urllib.request.Request(url, headers={"User-Agent": "Syncify/1.0"})
        
        with urllib.request.urlopen(req, timeout=300) as response:
            total_size = int(response.headers.get("Content-Length", 0))
            downloaded = 0
            block_size = 8192
            
            with open(dest, "wb") as f:
                while True:
                    chunk = response.read(block_size)
                    if not chunk:
                        break
                    f.write(chunk)
                    downloaded += len(chunk)
                    
                    if progress_callback and total_size:
                        progress_callback(downloaded, total_size)
        
        return True
    except Exception as e:
        print(f"[DependencyManager] Download failed: {e}", file=sys.stderr)
        return False


def is_safe_path(base_dir: Path, target_path: Path) -> bool:
    """Validate that target_path is within base_dir to prevent Zip Slip / path traversal."""
    try:
        resolved_base = base_dir.resolve()
        resolved_target = target_path.resolve()
        return resolved_target == resolved_base or resolved_base in resolved_target.parents
    except Exception:
        return False


def extract_archive(
    archive_path: Path,
    dest_dir: Path,
    expected_hash: Optional[Union[str, Sequence[str], Set[str]]] = None,
) -> bool:
    """Extract zip or tar archive safely against Zip Slip, TarBomb and integrity tampering."""
    try:
        if expected_hash is not None:
            if not verify_file_sha256(archive_path, expected_hash):
                actual = compute_file_sha256(archive_path) if archive_path.exists() else "missing"
                raise RuntimeError(
                    f"Integrity check failed: expected SHA-256 {expected_hash}, got {actual}"
                )

        dest_dir.mkdir(parents=True, exist_ok=True)
        if archive_path.suffix == ".zip":
            with zipfile.ZipFile(archive_path, "r") as zf:
                for member in zf.namelist():
                    if os.path.isabs(member) or member.startswith(("/", "\\")) or ".." in Path(member).parts:
                        raise RuntimeError(f"Zip Slip / Path traversal attempt detected: {member}")
                    target = (dest_dir / member).resolve()
                    if not is_safe_path(dest_dir, target):
                        raise RuntimeError(f"Zip Slip / Path traversal attempt detected: {member}")
                zf.extractall(dest_dir)
        elif archive_path.suffix in (".gz", ".xz", ".tar") or archive_path.name.endswith((".tar.gz", ".tar.xz")):
            import tarfile
            with tarfile.open(archive_path, "r:*") as tf:
                for member in tf.getmembers():
                    if os.path.isabs(member.name) or member.name.startswith(("/", "\\")) or ".." in Path(member.name).parts:
                        raise RuntimeError(f"TarBomb / Path traversal attempt detected: {member.name}")
                    target = (dest_dir / member.name).resolve()
                    if not is_safe_path(dest_dir, target):
                        raise RuntimeError(f"TarBomb / Path traversal attempt detected: {member.name}")
                    if member.issym() or member.islnk():
                        if os.path.isabs(member.linkname) or member.linkname.startswith(("/", "\\")) or ".." in Path(member.linkname).parts:
                            raise RuntimeError(f"TarBomb / Symlink traversal attempt detected: {member.name} -> {member.linkname}")
                        link_target = (dest_dir / member.linkname).resolve()
                        if not is_safe_path(dest_dir, link_target):
                            raise RuntimeError(f"TarBomb / Symlink traversal attempt detected: {member.name} -> {member.linkname}")
                if hasattr(tarfile, "data_filter"):
                    tf.extractall(dest_dir, filter="data")
                elif sys.version_info >= (3, 12):
                    tf.extractall(dest_dir, filter="data")
                else:
                    tf.extractall(dest_dir)
        else:
            return False
        return True
    except Exception as e:
        print(f"[DependencyManager] Extraction failed: {e}", file=sys.stderr)
        return False


def install_tool(tool: str, expected_hash: Optional[str] = None) -> Dict[str, Any]:
    """Download and install a tool with cryptographic SHA-256 verification."""
    plat = get_platform()
    
    if tool not in TOOL_URLS:
        return {"success": False, "error": f"Unknown tool: {tool}"}
    
    url = TOOL_URLS[tool].get(plat)
    if not url:
        return {"success": False, "error": f"Tool {tool} not available for {plat}"}
    
    # Check if already installed
    existing = get_tool_path(tool)
    if existing:
        return {
            "success": True,
            "already_installed": True,
            "path": str(existing),
        }
    
    # Create bin directory
    BIN_DIR.mkdir(parents=True, exist_ok=True)
    
    # Determine archive name
    archive_name = url.split("/")[-1]
    archive_path = BIN_DIR / archive_name
    
    # Download
    print(f"[DependencyManager] Installing {tool}...", file=sys.stderr)
    if not download_file(url, archive_path):
        archive_path.unlink(missing_ok=True)
        return {"success": False, "error": "Download failed"}

    # Cryptographic SHA-256 integrity verification before extraction
    target_hash = expected_hash or KNOWN_SHA256_HASHES.get(tool, {}).get(plat)
    if not target_hash:
        archive_path.unlink(missing_ok=True)
        return {"success": False, "error": f"No trusted SHA-256 hash defined for {tool} on {plat}"}

    if not verify_file_sha256(archive_path, target_hash):
        actual_hash = compute_file_sha256(archive_path) if archive_path.exists() else "unknown"
        archive_path.unlink(missing_ok=True)
        error_msg = f"SHA-256 checksum mismatch for {tool}: expected {target_hash}, got {actual_hash}"
        print(f"[DependencyManager] {error_msg}", file=sys.stderr)
        return {"success": False, "error": error_msg}
    
    # Safe extraction with Zip Slip and TarBomb guardrails
    if not extract_archive(archive_path, BIN_DIR, expected_hash=target_hash):
        archive_path.unlink(missing_ok=True)
        return {"success": False, "error": "Extraction failed"}
    
    # Clean up archive
    archive_path.unlink(missing_ok=True)
    
    # Verify installation
    installed_path = get_tool_path(tool)
    if installed_path:
        # Make executable on Unix
        if plat != "windows":
            os.chmod(installed_path, 0o755)
        
        return {
            "success": True,
            "installed": True,
            "path": str(installed_path),
        }
    else:
        return {"success": False, "error": "Installation verification failed"}


def check_all():
    """Check all dependencies."""
    tools = ["ffmpeg", "fpcalc", "ffprobe"]
    results = {tool: check_tool(tool) for tool in tools}
    
    all_available = all(r["available"] for r in results.values())
    
    json_response(True, {
        "all_available": all_available,
        "tools": results,
        "bin_dir": str(BIN_DIR),
    })


def install_all():
    """Install all missing tools."""
    tools = ["ffmpeg", "fpcalc"]
    results = {}
    
    for tool in tools:
        check = check_tool(tool)
        if check["available"]:
            results[tool] = {"success": True, "already_installed": True, "path": check["path"]}
        else:
            results[tool] = install_tool(tool)
    
    all_success = all(r.get("success", False) for r in results.values())
    
    json_response(all_success, {
        "results": results,
    }, error="Some tools failed to install" if not all_success else None)


def main():
    import argparse
    
    parser = argparse.ArgumentParser(description="Manage external dependencies")
    subparsers = parser.add_subparsers(dest="command", required=True)
    
    # Check command
    subparsers.add_parser("check", help="Check all dependencies")
    
    # Install command
    install_parser = subparsers.add_parser("install", help="Install specific tool")
    install_parser.add_argument("tool", help="Tool to install (ffmpeg, fpcalc)")
    install_parser.add_argument(
        "--sha256",
        dest="expected_hash",
        default=None,
        help="Optional override for expected SHA-256 digest",
    )
    
    # Install-all command
    subparsers.add_parser("install-all", help="Install all missing tools")
    
    args = parser.parse_args()
    
    if args.command == "check":
        check_all()
    elif args.command == "install":
        result = install_tool(args.tool, expected_hash=args.expected_hash)
        json_response(result.get("success", False), result if result.get("success") else None, 
                     result.get("error"))
    elif args.command == "install-all":
        install_all()


if __name__ == "__main__":
    main()

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
import urllib.request
from pathlib import Path
from typing import Optional, Dict, Any

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


def extract_archive(archive_path: Path, dest_dir: Path) -> bool:
    """Extract zip or tar archive."""
    try:
        if archive_path.suffix == ".zip":
            with zipfile.ZipFile(archive_path, "r") as zf:
                zf.extractall(dest_dir)
        elif archive_path.suffix in (".gz", ".xz"):
            import tarfile
            with tarfile.open(archive_path, "r:*") as tf:
                tf.extractall(dest_dir)
        else:
            return False
        return True
    except Exception as e:
        print(f"[DependencyManager] Extraction failed: {e}", file=sys.stderr)
        return False


def install_tool(tool: str) -> Dict[str, Any]:
    """Download and install a tool."""
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
        return {"success": False, "error": "Download failed"}
    
    # Extract
    if not extract_archive(archive_path, BIN_DIR):
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
    
    # Install-all command
    subparsers.add_parser("install-all", help="Install all missing tools")
    
    args = parser.parse_args()
    
    if args.command == "check":
        check_all()
    elif args.command == "install":
        result = install_tool(args.tool)
        json_response(result.get("success", False), result if result.get("success") else None, 
                     result.get("error"))
    elif args.command == "install-all":
        install_all()


if __name__ == "__main__":
    main()

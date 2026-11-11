#!/usr/bin/env python3
"""
Dependency Manager - Auto-download external tools on first use.

Usage:
    python dependency_manager.py check  # Check all dependencies
    python dependency_manager.py install <tool>  # Install specific tool
    python dependency_manager.py install-all  # Install all missing
    python dependency_manager.py place-external [--triple <triple>]
        # Place ffmpeg/ffprobe/fpcalc (downloading them from the pin table
        # if bin/ lacks them) as src-tauri/binaries/<tool>-<triple>[.exe],
        # the exact names bundle.externalBin demands at build time.

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
import subprocess
import zipfile
import platform
import hashlib
import urllib.request
from pathlib import Path
from typing import Optional, Dict, Any, Union, Sequence, Set

# Bin directory for downloaded tools
BIN_DIR = Path(__file__).parent.parent / "bin"

# Placement directory of the bundled external binaries: the exact names
# `bundle.externalBin` (tauri.conf.json) demands at build time, one file per
# tool per Rust target triple (`<tool>-<triple>[.exe]`). The release builds
# and local production builds both get them through `place-external`.
BINARIES_DIR = Path(__file__).parent.parent / "src-tauri" / "binaries"

# Tools that must exist next to the app executable. ffprobe carries no
# download URL: it is a byproduct of the pinned ffmpeg archive above.
BUNDLED_TOOLS = ("ffmpeg", "ffprobe", "fpcalc")

# Target triples of the published platforms, mapped to the platform keys of
# the pin table. Only these two triples get files placed; darwin is not a
# released platform.
SUPPORTED_TRIPLES = {
    "linux": "x86_64-unknown-linux-gnu",
    "windows": "x86_64-pc-windows-msvc",
}


from services.bridge_protocol import BridgeFailure, run  # noqa: E402


# Tool download URLs.
#
# This table is THE single pin store of the repo (ticket #241): the release
# builds (build-linux.yml / build-windows.yml) consume it through
# `place-external`, so a repin is one commit here and nothing else.
#
# ffmpeg is pinned to a DATED BtbN release on Windows and Linux: "latest" is
# rolling and its hash changes with every build, which would break the SHA-256
# verification. The pins are the ones the release workflows carried when the
# table absorbed them (BtbN autobuild 2026-11-05, N-128433). The Linux entry
# used to be johnvansickle's rolling `ffmpeg-release-amd64-static` tarball with
# a fixed hash — the same trap: the next upstream rebuild would fail the
# integrity check forever. macOS is not a released platform, so its evermeet
# entry stays as-is.
#
# ffprobe has NO row of its own: it travels inside the pinned ffmpeg archive
# as a byproduct and is placed next to ffmpeg by `place_bundled_binaries`.
TOOL_URLS = {
    "ffmpeg": {
        "windows": "https://github.com/BtbN/FFmpeg-Builds/releases/download/autobuild-2026-11-05-12-48/ffmpeg-N-128433-gdb8537d8cc-win64-gpl.zip",
        "darwin": "https://evermeet.cx/ffmpeg/getrelease/zip",
        "linux": "https://github.com/BtbN/FFmpeg-Builds/releases/download/autobuild-2026-11-05-12-48/ffmpeg-N-128433-gdb8537d8cc-linux64-gpl.tar.xz",
    },
    "fpcalc": {
        "windows": "https://github.com/acoustid/chromaprint/releases/download/v1.5.1/chromaprint-fpcalc-1.5.1-windows-x86_64.zip",
        "darwin": "https://github.com/acoustid/chromaprint/releases/download/v1.5.1/chromaprint-fpcalc-1.5.1-macos-x86_64.tar.gz",
        "linux": "https://github.com/acoustid/chromaprint/releases/download/v1.5.1/chromaprint-fpcalc-1.5.1-linux-x86_64.tar.gz",
    },
}

# Known authentic SHA-256 hashes for tool archives by platform
# These prevent tampered or malicious downloads from executing (SEC-008 / TASK-92).
# Same pins the release workflows verified inline before the table absorbed
# them; changing a URL without its hash (or the reverse) breaks the repin-is-
# one-commit guarantee and is caught by test_dependency_manager_pins.py.
KNOWN_SHA256_HASHES: Dict[str, Dict[str, str]] = {
    "ffmpeg": {
        "windows": "6e20a5744f9863883484af1cb5dca91e7e01d9af43b58d97fce7344e34393702",
        "darwin": "8a8c9e549983409fe6604b9aa665648b7a5def9407fe814c39c8b2ea7f64a48f",
        # SHA-256 of the pinned BtbN .tar.xz asset that carries both ffmpeg
        # and ffprobe for the packaged externalBin copies.
        "linux": "759cc1547e23f77f324944e034d27654c383ddbc1117823c08b025db1cf5fa14",
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


def find_bundled_tool(tool: str) -> Optional[Path]:
    """Locate a tool inside the bin/ download tree only, never the system PATH.

    Placement must copy the PINNED builds, so a system ffmpeg on the runner's
    PATH must not shadow the downloaded one (the Ubuntu runner installs ffmpeg
    via apt; copying it would silently downgrade the bundle to a
    dynamically-linked build).
    """
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

    return None


def get_tool_path(tool: str) -> Optional[Path]:
    """Get path to tool binary (bin/ tree first, then system PATH)."""
    bundled = find_bundled_tool(tool)
    if bundled:
        return bundled

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


def install_tool(
    tool: str, expected_hash: Optional[str] = None, use_system_path: bool = True
) -> Dict[str, Any]:
    """Download and install a tool with cryptographic SHA-256 verification.

    ``use_system_path=False`` skips the already-installed shortcut on the
    system PATH: placement uses it so the pinned archive is always downloaded
    and verified even when the runner ships the tool via its package manager.
    """
    plat = get_platform()

    if tool not in TOOL_URLS:
        return {"success": False, "error": f"Unknown tool: {tool}"}

    url = TOOL_URLS[tool].get(plat)
    if not url:
        return {"success": False, "error": f"Tool {tool} not available for {plat}"}

    # Check if already installed
    existing = get_tool_path(tool) if use_system_path else find_bundled_tool(tool)
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
    installed_path = get_tool_path(tool) if use_system_path else find_bundled_tool(tool)
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
    
    return {
        "all_available": all_available,
        "tools": results,
        "bin_dir": str(BIN_DIR),
    }


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
    if all_success:
        return {"results": results}
    raise BridgeFailure("Some tools failed to install")


def resolve_triple(explicit: Optional[str] = None) -> str:
    """Resolve the Rust target triple used for placement.

    Order: explicit argument > TARGET_TRIPLE env > `rustc -vV` host triple >
    the published triple of the running platform. Mirrors what the absorbed
    prepare_bundled_binaries scripts did, so local flows keep working.
    """
    if explicit:
        return explicit.strip()
    env_triple = os.environ.get("TARGET_TRIPLE")
    if env_triple:
        return env_triple.strip()

    rustc = shutil.which("rustc")
    if rustc:
        try:
            proc = subprocess.run(
                [rustc, "-vV"], capture_output=True, text=True, timeout=30
            )
            for line in proc.stdout.splitlines():
                if line.startswith("host:"):
                    return line.split(":", 1)[1].strip()
        except Exception as e:
            print(f"[DependencyManager] rustc -vV failed: {e}", file=sys.stderr)

    return SUPPORTED_TRIPLES[get_platform()]


def bundled_binary_name(tool: str, triple: str) -> str:
    """The exact file name bundle.externalBin expects for tool+triple."""
    ext = ".exe" if "windows" in triple else ""
    return f"{tool}-{triple}{ext}"


def _ensure_bundled_tool(tool: str) -> Dict[str, Any]:
    """Make sure `tool` exists in the bin/ download tree (pin table only).

    Never falls back to the system PATH: placement must ship the pinned
    builds, not whatever the runner's package manager installed.
    """
    found = find_bundled_tool(tool)
    if found:
        return {"success": True, "already_installed": True, "path": str(found)}

    if tool not in TOOL_URLS:
        # ffprobe travels inside the ffmpeg archive: there is nothing to
        # download on its own. Missing while ffmpeg is present means a stale
        # or incomplete bin/ tree.
        return {
            "success": False,
            "error": (
                f"{tool} has no download URL (it is a byproduct of the ffmpeg "
                "archive); reinstall ffmpeg so the archive is extracted again"
            ),
        }

    result = install_tool(tool, use_system_path=False)
    if not result.get("success"):
        return result
    found = find_bundled_tool(tool)
    if not found:
        return {"success": False, "error": f"{tool} not found after installation"}
    return {**result, "path": str(found)}


def place_bundled_binaries(triple: Optional[str] = None) -> Dict[str, Any]:
    """Place the pinned external binaries with the triple-suffixed names.

    Copies ffmpeg/ffprobe/fpcalc from the bin/ download tree into
    src-tauri/binaries/<tool>-<triple>[.exe] — the exact file names
    bundle.externalBin demands at build time. Tools missing from bin/ are
    downloaded and SHA-256-verified against the pin table first. This
    absorbs the download/copy steps the release workflows carried inline and
    the prepare_bundled_binaries.sh/.ps1 scripts.
    """
    resolved = resolve_triple(triple)

    placed = []
    for tool in BUNDLED_TOOLS:
        ensured = _ensure_bundled_tool(tool)
        if not ensured.get("success"):
            return {
                "success": False,
                "error": f"{tool}: {ensured.get('error', 'unavailable')}",
                "triple": resolved,
            }
        source = Path(ensured["path"])
        BINARIES_DIR.mkdir(parents=True, exist_ok=True)
        dest = BINARIES_DIR / bundled_binary_name(tool, resolved)
        shutil.copyfile(source, dest)
        if get_platform() != "windows":
            os.chmod(dest, 0o755)
        placed.append(
            {
                "tool": tool,
                "source": str(source),
                "dest": str(dest),
            }
        )

    return {
        "success": True,
        "triple": resolved,
        "binaries_dir": str(BINARIES_DIR),
        "placed": placed,
    }


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

    # Place-external command (bundled binaries for bundle.externalBin)
    place_parser = subparsers.add_parser(
        "place-external",
        help=(
            "Place ffmpeg/ffprobe/fpcalc in src-tauri/binaries/ with the "
            "<tool>-<triple>[.exe] names bundle.externalBin demands"
        ),
    )
    place_parser.add_argument(
        "--triple",
        dest="triple",
        default=None,
        help=(
            "Rust target triple (default: TARGET_TRIPLE env, rustc host "
            "triple, or the published triple of the running platform)"
        ),
    )

    return run(_handle, parse=parser.parse_args)


def _handle(args):
    if args.command == "check":
        return check_all()
    if args.command == "install":
        result = install_tool(args.tool, expected_hash=args.expected_hash)
        if result.get("success"):
            return {"success": True, "data": result}
        if result.get("error"):
            raise BridgeFailure(result["error"])
        return {"success": False}
    if args.command == "install-all":
        return install_all()
    if args.command == "place-external":
        result = place_bundled_binaries(triple=args.triple)
        if result.get("success"):
            return {"success": True, "data": result}
        raise BridgeFailure(result.get("error", "external binary placement failed"))
    raise BridgeFailure("Unknown command: %s" % args.command)


if __name__ == "__main__":
    sys.exit(main())

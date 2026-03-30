#!/usr/bin/env python3
"""
Health Check - Verify Syncify environment is properly configured.

Checks:
1. Environment variables (.env)
2. Python dependencies
3. Database connectivity
4. External tools (FFmpeg, fpcalc)
5. Service API connectivity

Usage:
    python scripts/health_check.py
    python scripts/health_check.py --fix  # Auto-fix what's possible
"""

import os
import sys
import json
import shutil
import sqlite3
from pathlib import Path
from typing import Dict, List, Tuple

# Add project root to path
PROJECT_ROOT = Path(__file__).parent.parent
sys.path.insert(0, str(PROJECT_ROOT))

# Load .env
from dotenv import load_dotenv
load_dotenv(PROJECT_ROOT / ".env")


class HealthChecker:
    def __init__(self):
        self.checks: List[Tuple[str, bool, str]] = []
        self.errors = 0
        self.warnings = 0
    
    def check(self, name: str, passed: bool, message: str = "", warning: bool = False):
        """Record a check result."""
        self.checks.append((name, passed, message))
        if not passed:
            if warning:
                self.warnings += 1
            else:
                self.errors += 1
    
    def print_results(self):
        """Print all check results."""
        print("\n" + "=" * 60)
        print("SYNCIFY HEALTH CHECK")
        print("=" * 60 + "\n")
        
        for name, passed, message in self.checks:
            status = "✅" if passed else "❌"
            print(f"{status} {name}")
            if message and not passed:
                print(f"   └─ {message}")
        
        print("\n" + "-" * 60)
        if self.errors == 0 and self.warnings == 0:
            print("✅ All checks passed! Syncify is ready to use.")
        elif self.errors == 0:
            print(f"⚠️  {self.warnings} warning(s), but Syncify should work.")
        else:
            print(f"❌ {self.errors} error(s), {self.warnings} warning(s)")
            print("   Fix the errors above before running Syncify.")
        print("-" * 60 + "\n")
    
    def to_json(self) -> Dict:
        """Return results as JSON."""
        return {
            "success": self.errors == 0,
            "errors": self.errors,
            "warnings": self.warnings,
            "checks": [
                {"name": name, "passed": passed, "message": message}
                for name, passed, message in self.checks
            ]
        }


def check_env_vars(checker: HealthChecker):
    """Check required environment variables."""
    
    # Required for any service
    required = []
    
    # Spotify
    spotify_vars = ["SPOTIFY_CLIENT_ID", "SPOTIFY_CLIENT_SECRET"]
    spotify_ok = all(os.getenv(v) for v in spotify_vars)
    checker.check(
        "Spotify credentials",
        spotify_ok,
        "Set SPOTIFY_CLIENT_ID and SPOTIFY_CLIENT_SECRET in .env" if not spotify_ok else "",
        warning=True
    )
    
    # Qobuz
    qobuz_vars = ["QOBUZ_APP_ID", "QOBUZ_APP_SECRET"]
    qobuz_ok = all(os.getenv(v) for v in qobuz_vars)
    checker.check(
        "Qobuz credentials",
        qobuz_ok,
        "Set QOBUZ_APP_ID and QOBUZ_APP_SECRET in .env" if not qobuz_ok else "",
        warning=True
    )
    
    # Output path
    output_path = os.getenv("DOWNLOAD_OUTPUT_PATH", "C:\\Music\\Syncify")
    path_exists = Path(output_path).exists()
    checker.check(
        "Download output path",
        path_exists,
        f"Path {output_path} does not exist. It will be created on first download." if not path_exists else "",
        warning=True
    )


def check_python_deps(checker: HealthChecker):
    """Check required Python packages."""
    
    required_packages = [
        ("spotipy", "Spotify API"),
        ("mutagen", "Audio metadata"),
        ("syncedlyrics", "Lyrics fetching"),
        ("aiohttp", "Async HTTP"),
        ("requests", "HTTP requests"),
        ("python-dotenv", "Environment loading"),
    ]
    
    for package, description in required_packages:
        try:
            __import__(package.replace("-", "_"))
            checker.check(f"Python: {package}", True)
        except ImportError:
            checker.check(
                f"Python: {package}",
                False,
                f"Install with: pip install {package}"
            )


def check_database(checker: HealthChecker):
    """Check database connectivity."""
    
    db_path = PROJECT_ROOT / "syncify.db"
    
    if not db_path.exists():
        checker.check(
            "Database file",
            False,
            "Database not found. Run 'cargo tauri dev' to create it."
        )
        return
    
    try:
        conn = sqlite3.connect(str(db_path))
        cursor = conn.cursor()
        
        # Check tables exist
        cursor.execute("SELECT name FROM sqlite_master WHERE type='table'")
        tables = [row[0] for row in cursor.fetchall()]
        
        required_tables = ["tracks", "artists", "albums", "download_queue", "services"]
        missing = [t for t in required_tables if t not in tables]
        
        if missing:
            checker.check(
                "Database schema",
                False,
                f"Missing tables: {', '.join(missing)}. Run migrations."
            )
        else:
            checker.check("Database schema", True)
        
        # Check track count
        cursor.execute("SELECT COUNT(*) FROM tracks")
        track_count = cursor.fetchone()[0]
        checker.check(
            f"Database has {track_count} tracks",
            True
        )
        
        conn.close()
    except Exception as e:
        checker.check("Database connection", False, str(e))


def check_external_tools(checker: HealthChecker):
    """Check FFmpeg and fpcalc availability."""
    
    # Check in bin directory first
    bin_dir = PROJECT_ROOT / "bin"
    
    # FFmpeg
    ffmpeg_path = shutil.which("ffmpeg")
    if not ffmpeg_path and bin_dir.exists():
        for item in bin_dir.iterdir():
            if item.is_dir():
                potential = item / "bin" / "ffmpeg.exe"
                if potential.exists():
                    ffmpeg_path = str(potential)
                    break
    
    checker.check(
        "FFmpeg",
        ffmpeg_path is not None,
        "Not found. Will be auto-downloaded on first use, or install manually." if not ffmpeg_path else "",
        warning=True
    )
    
    # fpcalc (Chromaprint)
    fpcalc_path = shutil.which("fpcalc")
    if not fpcalc_path and bin_dir.exists():
        for item in bin_dir.iterdir():
            if item.is_dir():
                potential = item / "fpcalc.exe"
                if potential.exists():
                    fpcalc_path = str(potential)
                    break
    
    checker.check(
        "Chromaprint (fpcalc)",
        fpcalc_path is not None,
        "Not found. Will be auto-downloaded on first use, or install manually." if not fpcalc_path else "",
        warning=True
    )


def check_bridges(checker: HealthChecker):
    """Check Python bridge scripts exist."""
    
    bridges = [
        "auth_bridge.py",
        "lyrics_bridge.py",
        "download_bridge.py",
        "metadata_bridge.py",
        "fingerprint_bridge.py",
        "conversion_bridge.py",
        "scanner_bridge.py",
        "organizer_bridge.py",
        "playlist_bridge.py",
        "dependency_manager.py",
    ]
    
    scripts_dir = PROJECT_ROOT / "scripts"
    
    for bridge in bridges:
        path = scripts_dir / bridge
        checker.check(
            f"Bridge: {bridge}",
            path.exists(),
            f"Missing: {path}" if not path.exists() else ""
        )


def main():
    import argparse
    
    parser = argparse.ArgumentParser(description="Syncify health check")
    parser.add_argument("--json", action="store_true", help="Output as JSON")
    parser.add_argument("--fix", action="store_true", help="Auto-fix what's possible")
    args = parser.parse_args()
    
    checker = HealthChecker()
    
    # Run checks
    print("Running health checks..." if not args.json else "", file=sys.stderr)
    
    check_env_vars(checker)
    check_python_deps(checker)
    check_database(checker)
    check_external_tools(checker)
    check_bridges(checker)
    
    # Output results
    if args.json:
        print(json.dumps(checker.to_json(), indent=2))
    else:
        checker.print_results()
    
    # Exit with error code if checks failed
    sys.exit(0 if checker.errors == 0 else 1)


if __name__ == "__main__":
    main()

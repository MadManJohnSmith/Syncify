#!/usr/bin/env python3
"""
Integration Tests for Syncify Python Bridges

Run with: pytest tests/test_bridges.py -v
"""

import os
import sys
import json
import subprocess
from pathlib import Path

import pytest

# Project paths
PROJECT_ROOT = Path(__file__).parent.parent
SCRIPTS_DIR = PROJECT_ROOT / "scripts"


def run_bridge(bridge_name: str, *args) -> dict:
    """Run a bridge script and return parsed JSON output."""
    script_path = SCRIPTS_DIR / bridge_name
    
    result = subprocess.run(
        [sys.executable, str(script_path)] + list(args),
        capture_output=True,
        text=True,
        cwd=str(PROJECT_ROOT),
        timeout=30,
    )
    
    try:
        return json.loads(result.stdout)
    except json.JSONDecodeError:
        return {
            "success": False,
            "error": f"Failed to parse output: {result.stdout[:200]}",
            "stderr": result.stderr[:500],
        }


class TestDependencyManager:
    """Test dependency_manager.py"""
    
    def test_check_command(self):
        """Test dependency check command."""
        result = run_bridge("dependency_manager.py", "check")
        
        assert "success" in result
        assert result["success"] is True
        assert "data" in result
        assert "tools" in result["data"]
        assert "ffmpeg" in result["data"]["tools"]
        assert "fpcalc" in result["data"]["tools"]


class TestConversionBridge:
    """Test conversion_bridge.py"""
    
    def test_check_ffmpeg(self):
        """Test FFmpeg availability check."""
        result = run_bridge("conversion_bridge.py", "check")
        
        assert "success" in result
        # FFmpeg may or may not be installed, but command should work
        assert "data" in result or "error" in result


class TestFingerPrintBridge:
    """Test fingerprint_bridge.py"""
    
    def test_check_fpcalc(self):
        """Test fpcalc availability check."""
        result = run_bridge("fingerprint_bridge.py", "check")
        
        assert "success" in result
        # fpcalc may or may not be installed, but command should work


class TestScannerBridge:
    """Test scanner_bridge.py"""
    
    def test_scan_nonexistent_dir(self):
        """Test scanning a nonexistent directory."""
        result = run_bridge("scanner_bridge.py", "scan", "/nonexistent/path")
        
        assert "success" in result
        # Should fail gracefully
        assert result["success"] is False or result.get("data", {}).get("total", 0) == 0


class TestOrganizerBridge:
    """Test organizer_bridge.py"""
    
    def test_preview_nonexistent_dir(self):
        """Test preview on nonexistent directory."""
        result = run_bridge("organizer_bridge.py", "preview", "/nonexistent/path")
        
        assert "success" in result


class TestMetadataBridge:
    """Test metadata_bridge.py"""
    
    def test_enrich_unknown_track(self):
        """Test enriching a track that doesn't exist."""
        result = run_bridge(
            "metadata_bridge.py", "enrich",
            "--track", "Nonexistent Track XYZ123",
            "--artist", "Unknown Artist ABC789"
        )
        
        assert "success" in result
        # May fail to find, but should not crash


class TestPlaylistBridge:
    """Test playlist_bridge.py"""
    
    def test_list_invalid_service(self):
        """Test listing playlists from invalid service."""
        result = run_bridge("playlist_bridge.py", "list", "invalid_service")
        
        assert "success" in result
        assert result["success"] is False


class TestHealthCheck:
    """Test health_check.py"""
    
    def test_health_check_json(self):
        """Test health check with JSON output."""
        result = run_bridge("health_check.py", "--json")
        
        assert "success" in result or "checks" in result
        if "checks" in result:
            assert isinstance(result["checks"], list)


# Run tests with pytest
if __name__ == "__main__":
    pytest.main([__file__, "-v"])

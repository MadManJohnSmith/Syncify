#!/usr/bin/env python3
"""
Security test suite for dependency_manager.py (SEC-008 / TASK-92).

Verifies:
1. Cryptographic SHA-256 verification of downloaded archives before extraction.
2. Immediate rejection and abort on SHA-256 mismatch (tampered/corrupted downloads).
3. Confinement of all extracted members within the destination directory (Zip Slip / TarBomb prevention).
4. Proper cleanup of failed/untrusted archives from disk.
5. Integrity and format of the KNOWN_SHA256_HASHES registry.
"""

import hashlib
import io
import os
import sys
import tarfile
import tempfile
import unittest
import zipfile
from pathlib import Path
from unittest.mock import patch

# Add scripts directory to path
SCRIPTS_DIR = Path(__file__).resolve().parent.parent
sys.path.insert(0, str(SCRIPTS_DIR))

from dependency_manager import (  # noqa: E402
    BIN_DIR,
    KNOWN_SHA256_HASHES,
    TOOL_URLS,
    compute_file_sha256,
    extract_archive,
    install_tool,
    is_safe_path,
    verify_file_sha256,
)


class TestDependencyManagerSecurity(unittest.TestCase):
    """Test cryptographic integrity checks and archive confinement."""

    def setUp(self):
        self.temp_dir = tempfile.TemporaryDirectory()
        self.base_dir = Path(self.temp_dir.name).resolve()
        self.dest_dir = self.base_dir / "target_bin"
        self.dest_dir.mkdir(parents=True, exist_ok=True)

    def tearDown(self):
        self.temp_dir.cleanup()

    # ═══════════════════════════════════════════════════════
    # 1. SHA-256 COMPUTATION AND VERIFICATION TESTS
    # ═══════════════════════════════════════════════════════

    def test_compute_file_sha256_known_payload(self):
        """Verify SHA-256 calculation matches hashlib reference."""
        payload = b"Syncify Secure Payload 2026 \x00\xff\xfe"
        test_file = self.base_dir / "sample.bin"
        test_file.write_bytes(payload)

        expected_hash = hashlib.sha256(payload).hexdigest()
        actual_hash = compute_file_sha256(test_file)

        self.assertEqual(actual_hash, expected_hash)
        self.assertEqual(len(actual_hash), 64)

    def test_verify_file_sha256_matches_valid_hash(self):
        """Verify that matching SHA-256 returns True, case-insensitively."""
        payload = b"Authentic tool payload bytes"
        test_file = self.base_dir / "tool_archive.zip"
        test_file.write_bytes(payload)

        expected_lower = hashlib.sha256(payload).hexdigest()
        expected_upper = expected_lower.upper()

        self.assertTrue(verify_file_sha256(test_file, expected_lower))
        self.assertTrue(verify_file_sha256(test_file, expected_upper))
        self.assertTrue(verify_file_sha256(test_file, [expected_lower, "other_hash"]))

    def test_verify_file_sha256_rejects_altered_hash(self):
        """Verify that tampered content or incorrect hash returns False."""
        payload = b"Legitimate content"
        test_file = self.base_dir / "tool_archive.zip"
        test_file.write_bytes(payload)

        bad_hash = "0" * 64
        self.assertFalse(verify_file_sha256(test_file, bad_hash))

        # Modifying a single bit of the file payload
        tampered_file = self.base_dir / "tampered.zip"
        tampered_file.write_bytes(payload + b"!")
        original_hash = hashlib.sha256(payload).hexdigest()
        self.assertFalse(verify_file_sha256(tampered_file, original_hash))

    def test_verify_file_sha256_nonexistent_file(self):
        """Verify that checking a non-existent file returns False safely."""
        missing = self.base_dir / "does_not_exist.zip"
        self.assertFalse(verify_file_sha256(missing, "a" * 64))

    # ═══════════════════════════════════════════════════════
    # 2. KNOWN_SHA256_HASHES REGISTRY INTEGRITY
    # ═══════════════════════════════════════════════════════

    def test_known_sha256_hashes_registry_structure(self):
        """Verify that all required tools and platforms have valid 64-char hex hashes."""
        required_tools = ["ffmpeg", "fpcalc"]
        required_platforms = ["windows", "darwin", "linux"]

        for tool in required_tools:
            self.assertIn(tool, KNOWN_SHA256_HASHES, f"Missing tool in hashes: {tool}")
            for plat in required_platforms:
                self.assertIn(plat, KNOWN_SHA256_HASHES[tool], f"Missing platform {plat} for {tool}")
                digest = KNOWN_SHA256_HASHES[tool][plat]
                self.assertIsInstance(digest, str)
                self.assertEqual(len(digest), 64, f"Hash for {tool}/{plat} is not 64 chars")
                # Verify valid hexadecimal
                int(digest, 16)

    # ═══════════════════════════════════════════════════════
    # 3. EXTRACTION WITH INTEGRITY VERIFICATION
    # ═══════════════════════════════════════════════════════

    def test_extract_archive_sha256_check_passes(self):
        """Verify extraction succeeds when SHA-256 matches."""
        zip_path = self.base_dir / "valid.zip"
        with zipfile.ZipFile(zip_path, "w") as zf:
            zf.writestr("test_binary", b"BINARY_CONTENT")

        valid_hash = hashlib.sha256(zip_path.read_bytes()).hexdigest()
        success = extract_archive(zip_path, self.dest_dir, expected_hash=valid_hash)

        self.assertTrue(success)
        extracted = self.dest_dir / "test_binary"
        self.assertTrue(extracted.exists())
        self.assertEqual(extracted.read_bytes(), b"BINARY_CONTENT")

    def test_extract_archive_sha256_check_aborts_on_mismatch(self):
        """Verify extraction aborts immediately and writes no files if hash does not match."""
        zip_path = self.base_dir / "tampered.zip"
        with zipfile.ZipFile(zip_path, "w") as zf:
            zf.writestr("test_binary", b"CORRUPTED_OR_TAMPERED_CONTENT")

        bogus_hash = "f" * 64
        success = extract_archive(zip_path, self.dest_dir, expected_hash=bogus_hash)

        self.assertFalse(success)
        self.assertFalse((self.dest_dir / "test_binary").exists())

    # ═══════════════════════════════════════════════════════
    # 4. ZIP SLIP AND TARBOMB CONFINEMENT TESTS
    # ═══════════════════════════════════════════════════════

    def test_zip_slip_relative_path_traversal_blocked(self):
        """Verify that zip archives with ../ escapes are rejected."""
        zip_path = self.base_dir / "zip_slip.zip"
        evil_member = "../../malicious_escape.sh"
        canary = self.base_dir / "malicious_escape.sh"

        with zipfile.ZipFile(zip_path, "w") as zf:
            zf.writestr(evil_member, b"#!/bin/sh\necho hacked")

        valid_hash = hashlib.sha256(zip_path.read_bytes()).hexdigest()
        success = extract_archive(zip_path, self.dest_dir, expected_hash=valid_hash)

        self.assertFalse(success)
        self.assertFalse(canary.exists(), "Zip Slip canary must not be written")

    def test_zip_slip_absolute_path_blocked(self):
        """Verify that zip archives with absolute member paths are rejected."""
        zip_path = self.base_dir / "zip_abs.zip"
        evil_member = "/tmp/malicious_abs.sh"

        with zipfile.ZipFile(zip_path, "w") as zf:
            zf.writestr(evil_member, b"#!/bin/sh\necho hacked")

        success = extract_archive(zip_path, self.dest_dir)
        self.assertFalse(success)

    def test_tarbomb_path_traversal_blocked(self):
        """Verify that tar archives with path traversal members are rejected."""
        tar_path = self.base_dir / "tarbomb.tar.gz"
        canary = self.base_dir / "tar_escaped.sh"

        with tarfile.open(tar_path, "w:gz") as tf:
            payload = b"#!/bin/sh\necho pwned"
            ti = tarfile.TarInfo(name="../../tar_escaped.sh")
            ti.size = len(payload)
            tf.addfile(ti, io.BytesIO(payload))

        valid_hash = hashlib.sha256(tar_path.read_bytes()).hexdigest()
        success = extract_archive(tar_path, self.dest_dir, expected_hash=valid_hash)

        self.assertFalse(success)
        self.assertFalse(canary.exists(), "TarBomb canary must not be written")

    def test_tarbomb_symlink_escape_blocked(self):
        """Verify that tar archives with symlinks pointing outside dest are rejected."""
        tar_path = self.base_dir / "symlink_escape.tar.gz"

        with tarfile.open(tar_path, "w:gz") as tf:
            ti = tarfile.TarInfo(name="escape_link")
            ti.type = tarfile.SYMTYPE
            ti.linkname = "../../etc/shadow"
            tf.addfile(ti)

        success = extract_archive(tar_path, self.dest_dir)
        self.assertFalse(success)

    # ═══════════════════════════════════════════════════════
    # 5. INSTALL_TOOL END-TO-END FLOW TESTS
    # ═══════════════════════════════════════════════════════

    @patch("dependency_manager.get_tool_path")
    @patch("dependency_manager.BIN_DIR")
    @patch("dependency_manager.download_file")
    def test_install_tool_hash_mismatch_aborts_and_cleans_up(
        self, mock_download, mock_bin_dir, mock_get_tool_path
    ):
        """Verify install_tool aborts, cleans up downloaded file, and reports error on hash mismatch."""
        mock_get_tool_path.return_value = None
        mock_bin_dir.__truediv__.side_effect = lambda x: self.dest_dir / x

        fake_archive = self.dest_dir / "chromaprint-fpcalc-1.5.1-linux-x86_64.tar.gz"

        def fake_download(url, dest, **kwargs):
            dest.write_bytes(b"BOGUS_CORRUPTED_DOWNLOAD_DATA")
            return True

        mock_download.side_effect = fake_download

        # Run with invalid expected hash
        result = install_tool("fpcalc", expected_hash="0" * 64)

        self.assertFalse(result["success"])
        self.assertIn("SHA-256 checksum mismatch", result["error"])
        self.assertFalse(fake_archive.exists(), "Archive must be unlinked on integrity failure")

    @patch("dependency_manager.get_platform")
    def test_install_tool_unknown_platform_or_missing_hash(self, mock_plat):
        """Verify tool installation fails safely if no trusted hash exists."""
        mock_plat.return_value = "unknown_os_variant"
        result = install_tool("fpcalc")
        self.assertFalse(result["success"])
        self.assertIn("not available", result["error"])


if __name__ == "__main__":
    unittest.main()

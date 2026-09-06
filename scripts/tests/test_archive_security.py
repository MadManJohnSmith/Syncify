#!/usr/bin/env python3
"""
Regression tests for CVE-2007-4559 (Path Traversal / Zip Slip / TarBomb) in dependency_manager.py.

Verifies that malicious zip and tar archives containing relative paths ('../')
or absolute paths cannot escape the designated destination directory.
"""

import os
import sys
import io
import tarfile
import zipfile
import tempfile
import unittest
from pathlib import Path

# Add scripts directory to path
SCRIPTS_DIR = Path(__file__).resolve().parent.parent
sys.path.insert(0, str(SCRIPTS_DIR))

from dependency_manager import is_safe_path, extract_archive  # noqa: E402


class TestArchiveSecurity(unittest.TestCase):
    """Test suite verifying protection against Zip Slip and TarBomb vulnerabilities."""

    def setUp(self):
        self.temp_dir = tempfile.TemporaryDirectory()
        self.base_dir = Path(self.temp_dir.name).resolve()
        self.dest_dir = self.base_dir / "target"
        self.dest_dir.mkdir(parents=True, exist_ok=True)

    def tearDown(self):
        self.temp_dir.cleanup()

    def test_is_safe_path_valid_paths(self):
        """Verify safe paths inside base directory return True."""
        safe_child = self.dest_dir / "tool.exe"
        safe_nested = self.dest_dir / "sub" / "bin" / "tool"
        self.assertTrue(is_safe_path(self.dest_dir, safe_child))
        self.assertTrue(is_safe_path(self.dest_dir, safe_nested))
        self.assertTrue(is_safe_path(self.dest_dir, self.dest_dir))

    def test_is_safe_path_traversal_attempts(self):
        """Verify traversal attempts return False."""
        escaped_parent = (self.dest_dir / ".." / "evil.txt").resolve()
        escaped_root = Path("/etc/passwd").resolve()
        deep_traversal = (self.dest_dir / "nested" / ".." / ".." / "evil.txt").resolve()

        self.assertFalse(is_safe_path(self.dest_dir, escaped_parent))
        self.assertFalse(is_safe_path(self.dest_dir, escaped_root))
        self.assertFalse(is_safe_path(self.dest_dir, deep_traversal))

    def test_extract_archive_blocks_zip_slip(self):
        """Verify that zip archives with path traversal members are blocked."""
        zip_path = self.base_dir / "malicious.zip"
        evil_name = "../evil_zip.txt"
        canary_target = self.base_dir / "evil_zip.txt"

        with zipfile.ZipFile(zip_path, "w") as zf:
            zf.writestr(evil_name, "MALICIOUS_PAYLOAD")

        # Attempt extraction
        success = extract_archive(zip_path, self.dest_dir)

        # Must fail and not write the canary file
        self.assertFalse(success, "Malicious zip extraction should return False")
        self.assertFalse(canary_target.exists(), "Zip Slip canary file should not be created")

    def test_extract_archive_blocks_tar_traversal(self):
        """Verify that tar archives with path traversal members are blocked."""
        tar_path = self.base_dir / "malicious.tar.gz"
        canary_target = self.base_dir / "evil_tar.txt"

        with tarfile.open(tar_path, "w:gz") as tf:
            payload = b"MALICIOUS_PAYLOAD"
            ti = tarfile.TarInfo(name="../evil_tar.txt")
            ti.size = len(payload)
            tf.addfile(ti, io.BytesIO(payload))

        # Attempt extraction
        success = extract_archive(tar_path, self.dest_dir)

        # Must fail and not write the canary file
        self.assertFalse(success, "Malicious tar extraction should return False")
        self.assertFalse(canary_target.exists(), "TarBomb canary file should not be created")

    def test_extract_archive_blocks_symlink_traversal_in_tar(self):
        """Verify that tar archives with escaping symlinks are blocked."""
        tar_path = self.base_dir / "malicious_symlink.tar.gz"

        with tarfile.open(tar_path, "w:gz") as tf:
            ti = tarfile.TarInfo(name="evil_link")
            ti.type = tarfile.SYMTYPE
            ti.linkname = "../../etc/passwd"
            tf.addfile(ti)

        success = extract_archive(tar_path, self.dest_dir)
        self.assertFalse(success, "Escaping symlink in tar should be blocked")

    def test_extract_archive_allows_safe_zip(self):
        """Verify legitimate zip archives extract without errors."""
        zip_path = self.base_dir / "safe.zip"
        with zipfile.ZipFile(zip_path, "w") as zf:
            zf.writestr("safe.txt", "BENIGN_CONTENT")
            zf.writestr("subdir/safe2.txt", "BENIGN_SUBDIR")

        success = extract_archive(zip_path, self.dest_dir)
        self.assertTrue(success, "Legitimate zip extraction should succeed")
        self.assertTrue((self.dest_dir / "safe.txt").exists())
        self.assertTrue((self.dest_dir / "subdir" / "safe2.txt").exists())
        self.assertEqual((self.dest_dir / "safe.txt").read_text(), "BENIGN_CONTENT")

    def test_extract_archive_allows_safe_tar(self):
        """Verify legitimate tar archives extract without errors."""
        tar_path = self.base_dir / "safe.tar.gz"
        payload = b"BENIGN_TAR_CONTENT"
        with tarfile.open(tar_path, "w:gz") as tf:
            ti = tarfile.TarInfo(name="safe_tar.txt")
            ti.size = len(payload)
            tf.addfile(ti, io.BytesIO(payload))

        success = extract_archive(tar_path, self.dest_dir)
        self.assertTrue(success, "Legitimate tar extraction should succeed")
        self.assertTrue((self.dest_dir / "safe_tar.txt").exists())
        self.assertEqual((self.dest_dir / "safe_tar.txt").read_bytes(), payload)


if __name__ == "__main__":
    unittest.main()

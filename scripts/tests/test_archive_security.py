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
from unittest import mock
from pathlib import Path

# Add scripts directory to path
SCRIPTS_DIR = Path(__file__).resolve().parent.parent
sys.path.insert(0, str(SCRIPTS_DIR))

import dependency_manager  # noqa: E402
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

    def test_failed_extraction_leaves_destination_unchanged(self):
        """A member written before an extraction error must not persist."""
        zip_path = self.base_dir / "partial.zip"
        with zipfile.ZipFile(zip_path, "w"):
            pass
        existing = self.dest_dir / "existing.txt"
        existing.write_text("ORIGINAL")

        def fail_after_write(target):
            (Path(target) / "partial.txt").write_text("INCOMPLETE")
            raise OSError("simulated extraction failure")

        with mock.patch.object(zipfile.ZipFile, "__enter__", autospec=True) as enter:
            archive = enter.return_value
            archive.namelist.return_value = ["partial.txt"]
            archive.extractall.side_effect = fail_after_write
            success = extract_archive(zip_path, self.dest_dir)

        self.assertFalse(success)
        self.assertEqual(existing.read_text(), "ORIGINAL")
        self.assertFalse((self.dest_dir / "partial.txt").exists())

    def test_publication_failure_restores_destination_and_cleans_residues(self):
        """A failed staging publish restores the previous destination."""
        zip_path = self.base_dir / "publish-failure.zip"
        with zipfile.ZipFile(zip_path, "w") as zf:
            zf.writestr("new.txt", "NEW")
        existing = self.dest_dir / "existing.txt"
        existing.write_text("ORIGINAL")
        real_replace = Path.replace

        def fail_staging_publish(path, target):
            if path.name.startswith(f".{self.dest_dir.name}-") and not path.name.startswith(
                f".{self.dest_dir.name}-old-"
            ) and Path(target) == self.dest_dir:
                raise OSError("simulated publish failure")
            return real_replace(path, target)

        with mock.patch.object(Path, "replace", autospec=True, side_effect=fail_staging_publish):
            success = extract_archive(zip_path, self.dest_dir)

        self.assertFalse(success)
        self.assertEqual(existing.read_text(), "ORIGINAL")
        self.assertFalse((self.dest_dir / "new.txt").exists())
        self.assertEqual(list(self.base_dir.glob(".target-*")), [])

    def test_restore_rename_failure_uses_copy_and_cleans_residues(self):
        """A failed restoring rename falls back to copying the prior tree."""
        zip_path = self.base_dir / "restore-failure.zip"
        with zipfile.ZipFile(zip_path, "w") as zf:
            zf.writestr("new.txt", "NEW")
        existing = self.dest_dir / "existing.txt"
        existing.write_text("ORIGINAL")
        real_replace = Path.replace

        def fail_publish_and_restore(path, target):
            if Path(target) == self.dest_dir and path.name.startswith(f".{self.dest_dir.name}-"):
                raise OSError("simulated publish/restore rename failure")
            return real_replace(path, target)

        with mock.patch.object(Path, "replace", autospec=True, side_effect=fail_publish_and_restore):
            success = extract_archive(zip_path, self.dest_dir)

        self.assertFalse(success)
        self.assertEqual(existing.read_text(), "ORIGINAL")
        self.assertFalse((self.dest_dir / "new.txt").exists())
        self.assertEqual(list(self.base_dir.glob(".target-*")), [])

    def test_successful_publication_cleans_residues(self):
        """Successful publication removes staging and backup directories."""
        zip_path = self.base_dir / "success.zip"
        with zipfile.ZipFile(zip_path, "w") as zf:
            zf.writestr("new.txt", "NEW")
        (self.dest_dir / "existing.txt").write_text("ORIGINAL")

        self.assertTrue(extract_archive(zip_path, self.dest_dir))
        self.assertEqual((self.dest_dir / "new.txt").read_text(), "NEW")
        self.assertEqual(list(self.base_dir.glob(".target-*")), [])

    def test_get_tool_path_requires_regular_executable_file(self):
        """Directories and non-executable files must not count as installed tools."""
        bin_dir = self.base_dir / "bin"
        bin_dir.mkdir()
        tool = bin_dir / "test-tool"

        with mock.patch.object(dependency_manager, "BIN_DIR", bin_dir), \
                mock.patch.object(dependency_manager, "get_platform", return_value="linux"), \
                mock.patch.object(dependency_manager.shutil, "which", return_value=None):
            tool.mkdir()
            self.assertIsNone(dependency_manager.get_tool_path("test-tool"))
            tool.rmdir()
            tool.write_text("#!/bin/sh\n")
            tool.chmod(0o644)
            self.assertIsNone(dependency_manager.get_tool_path("test-tool"))
            tool.chmod(0o755)
            self.assertEqual(dependency_manager.get_tool_path("test-tool"), tool)

    def test_install_tool_rejects_untrusted_hash_override(self):
        """A caller-supplied digest cannot replace the configured trusted digest."""
        bin_dir = self.base_dir / "bin"
        trusted_hash = dependency_manager.KNOWN_SHA256_HASHES["ffmpeg"]["linux"]

        def fake_download(_url, destination):
            destination.parent.mkdir(parents=True, exist_ok=True)
            destination.write_bytes(b"attacker-controlled archive")
            return True

        with mock.patch.object(dependency_manager, "BIN_DIR", bin_dir), \
                mock.patch.object(dependency_manager, "get_platform", return_value="linux"), \
                mock.patch.object(dependency_manager, "get_tool_path", return_value=None), \
                mock.patch.object(dependency_manager, "download_file", side_effect=fake_download), \
                mock.patch.object(dependency_manager, "extract_archive") as extract:
            result = dependency_manager.install_tool("ffmpeg", expected_hash="0" * 64)

        self.assertFalse(result["success"])
        self.assertIn("does not match", result["error"])
        self.assertNotEqual("0" * 64, trusted_hash)
        extract.assert_not_called()

    def test_main_rejects_sha256_abbreviation(self):
        """Argparse must not treat --sha as an abbreviation for --sha256."""
        with mock.patch.object(sys, "argv", ["dependency_manager.py", "install", "ffmpeg", "--sha", "abc"]), \
                self.assertRaises(SystemExit) as raised:
            dependency_manager.main()

        self.assertEqual(raised.exception.code, 2)

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

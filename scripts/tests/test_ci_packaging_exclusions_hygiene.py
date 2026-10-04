#!/usr/bin/env python3
"""
CI/CD Packaging Exclusions Hygiene & Download Integrity Test Suite (TASK-95 / SEC-011).

Verifies:
1. Strict exclusion rules in .github/workflows/build-windows.yml preventing secrets,
   credentials, tokens, session caches, logs, databases, and temporary files from
   being packaged into portable releases.
2. Simulated pattern matching ensuring all known sensitive file patterns are excluded
   while legitimate production scripts are retained.
3. Cryptographic SHA-256 integrity verification for all binary, archive, and script
   downloads in packaging workflows prior to execution or decompression.
4. Enforcement of known static SHA-256 hashes for pinned dependencies (fpcalc, python-embed).
5. Abort/cleanup mechanisms on integrity check failures to prevent executing untrusted binaries.
"""

import fnmatch
import re
import unittest
from pathlib import Path
import yaml


class TestCiPackagingExclusionsHygiene(unittest.TestCase):
    """Test suite for packaging exclusions and download integrity in CI/CD workflows."""

    @classmethod
    def setUpClass(cls):
        cls.repo_root = Path(__file__).resolve().parent.parent.parent
        cls.workflows_dir = cls.repo_root / ".github" / "workflows"
        cls.build_windows_path = cls.workflows_dir / "build-windows.yml"

        if not cls.build_windows_path.exists():
            raise FileNotFoundError(f"Workflow file missing: {cls.build_windows_path}")

        cls.build_windows_content = cls.build_windows_path.read_text(encoding="utf-8")

        # Required strict exclusion patterns (TASK-95 / SEC-011)
        cls.required_exclusion_patterns = [
            ".*",
            "*.json",
            "*.cache",
            "*.pyc",
            "__pycache__",
            "*.log",
            "*.tmp",
            "*.db",
            "*.key",
            "credentials*",
            ".env*",
        ]

    # ═══════════════════════════════════════════════════════
    # 1. EXCLUSIONS HYGIENE TESTS
    # ═══════════════════════════════════════════════════════

    def test_workflow_file_exists(self):
        """Ensure .github/workflows/build-windows.yml exists and is valid YAML."""
        self.assertTrue(self.build_windows_path.is_file())
        with open(self.build_windows_path, "r", encoding="utf-8") as f:
            data = yaml.safe_load(f)
        self.assertIsInstance(data, dict)
        self.assertIn("jobs", data)

    def test_all_required_exclusion_patterns_present(self):
        """Verify that every required exclusion pattern is explicitly declared in build-windows.yml."""
        for pattern in self.required_exclusion_patterns:
            self.assertIn(
                pattern,
                self.build_windows_content,
                f"Required exclusion pattern '{pattern}' missing in {self.build_windows_path.name}"
            )

    def test_sensitive_artifacts_are_excluded_by_patterns(self):
        """Verify that known sensitive files match at least one exclusion pattern."""
        sensitive_samples = [
            ".gui_credentials_cache.json",
            ".gui_settings.json",
            ".spotify_token_cache.json",
            ".env",
            ".env.local",
            ".env.production",
            "credentials.json",
            "credentials.txt",
            "credentials_backup",
            "syncify.db",
            "test_library.sqlite.db",
            "app.log",
            "error_debug.log",
            "temporary_worker.tmp",
            "private_signing.key",
            "auth_token.cache",
            "bridge.pyc",
            "__pycache__",
            ".browser_profile_default",
            ".gitignore",
            ".git",
        ]

        for sample in sensitive_samples:
            matched = any(
                fnmatch.fnmatch(sample, pat) for pat in self.required_exclusion_patterns
            )
            self.assertTrue(
                matched,
                f"Sensitive sample '{sample}' was NOT matched by any packaging exclusion pattern!"
            )

    def test_legitimate_production_scripts_not_excluded(self):
        """Verify that legitimate script files are NOT inadvertently matched by file exclusion rules."""
        legitimate_files = [
            "auth_bridge.py",
            "conversion_bridge.py",
            "fingerprint_bridge.py",
            "lyrics_bridge.py",
            "metadata_bridge.py",
            "organizer_bridge.py",
            "playlist_bridge.py",
            "scanner_bridge.py",
            "requirements.txt",
            "services/__init__.py",
            "services/qobuz_service.py",
            "services/tidal_service.py",
            "services/deezer_service.py",
            "services/soundcloud_service.py",
            "services/acoustid_matcher.py",
            "services/metadata_enrichment.py",
        ]

        # For legitimate non-hidden files, only check against file patterns (excluding '.*')
        file_exclusion_patterns = [
            pat for pat in self.required_exclusion_patterns if pat != ".*"
        ]

        for script in legitimate_files:
            file_name = Path(script).name
            matched = any(
                fnmatch.fnmatch(file_name, pat) for pat in file_exclusion_patterns
            )
            self.assertFalse(
                matched,
                f"Legitimate script '{script}' falsely matched exclusion pattern!"
            )

    def test_post_copy_recursive_hygiene_purge_is_present(self):
        """Verify that a recursive post-copy hygiene sweep exists to prevent nested leaks."""
        self.assertIn(
            "Remove-Item",
            self.build_windows_content,
            "Workflow should contain post-copy hygiene/cleanup with Remove-Item"
        )
        # Check that the scripts packaging step has recursive filtering
        self.assertRegex(
            self.build_windows_content,
            r'Get-ChildItem\s+-Path\s+["\']dist/portable/Syncify-Portable/scripts["\']\s+-Recurse',
            "Workflow must contain a recursive sweep over portable scripts directory"
        )

    # ═══════════════════════════════════════════════════════
    # 2. DOWNLOAD INTEGRITY & SHA-256 VERIFICATION TESTS
    # ═══════════════════════════════════════════════════════

    def test_all_invoke_webrequests_have_sha256_verification(self):
        """Verify that all external downloads in build-windows.yml perform SHA-256 verification."""
        # Find all Invoke-WebRequest calls with -OutFile
        download_matches = re.findall(
            r'Invoke-WebRequest[^\n]+-OutFile\s+["\']?([^"\'\n]+)["\']?',
            self.build_windows_content
        )
        self.assertGreater(len(download_matches), 0, "No downloads found in build-windows.yml")

        # Every downloaded file should be checked with Get-FileHash ... -Algorithm SHA256
        hash_check_matches = re.findall(
            r'Get-FileHash[^\n]+-Algorithm\s+SHA256',
            self.build_windows_content,
            re.IGNORECASE
        )
        self.assertEqual(
            len(download_matches),
            len(hash_check_matches),
            f"Mismatch between downloads ({len(download_matches)}: {download_matches}) "
            f"and SHA256 checks ({len(hash_check_matches)})"
        )

    def test_pinned_dependencies_have_known_sha256_digests(self):
        """Verify that pinned tools have their authentic SHA-256 digests in build-windows.yml."""
        known_hashes = {
            "fpcalc (Chromaprint 1.5.1 Windows)": "36b478e16aa69f757f376645db0d436073a42c0097b6bb2677109e7835b59bbc",
            "python-embed (Python 3.11.9 amd64)": "009d6bf7e3b2ddca3d784fa09f90fe54336d5b60f0e0f305c37f400bf83cfd3b",
        }

        for tool_name, digest in known_hashes.items():
            self.assertIn(
                digest,
                self.build_windows_content.lower(),
                f"Known authentic SHA-256 digest for {tool_name} missing in build-windows.yml"
            )

    def test_archive_expansion_and_execution_guarded_by_hash_check(self):
        """Verify that Expand-Archive and script executions occur strictly AFTER hash verification."""
        # Split workflow by steps
        steps = self.build_windows_content.split("- name:")

        for step in steps:
            if "Expand-Archive" in step and "Invoke-WebRequest" in step:
                download_idx = step.find("Invoke-WebRequest")
                hash_idx = step.find("Get-FileHash")
                expand_idx = step.find("Expand-Archive")

                self.assertNotEqual(hash_idx, -1, f"Step with download & Expand-Archive missing Get-FileHash:\n{step[:200]}")
                self.assertLess(download_idx, hash_idx, "Download must occur before Get-FileHash")
                self.assertLess(hash_idx, expand_idx, "Get-FileHash must occur before Expand-Archive")

            if "get-pip.py" in step and "Invoke-WebRequest" in step:
                download_idx = step.find("Invoke-WebRequest")
                hash_idx = step.find("Get-FileHash")
                exec_idx = step.find("& \"python/python.exe\" \"python/get-pip.py\"")

                self.assertNotEqual(hash_idx, -1, "get-pip.py missing Get-FileHash check")
                self.assertLess(download_idx, hash_idx, "get-pip download must occur before Get-FileHash")
                self.assertLess(hash_idx, exec_idx, "Get-FileHash must occur before executing get-pip.py")

    def test_integrity_check_failure_aborts_or_purges(self):
        """Verify that checksum mismatch causes an abort (throw) and purges the unverified file."""
        # Checks should contain throw statements on failure
        throw_matches = re.findall(r'throw\s+["\'].*?SHA256.*?["\']', self.build_windows_content, re.IGNORECASE)
        self.assertGreaterEqual(
            len(throw_matches),
            3,
            "Workflow must contain throw statements on SHA256 integrity verification failures"
        )

    def test_no_insecure_downloads_in_any_workflow(self):
        """Scan all GitHub Actions workflows to ensure no untrusted curl/wget/Invoke-WebRequest exists without hash check."""
        for yml_file in self.workflows_dir.glob("*.yml"):
            content = yml_file.read_text(encoding="utf-8")
            # If a workflow downloads archives or executables, it must verify hashes
            has_archive_download = bool(re.search(r'Invoke-WebRequest[^\n]+\.(zip|tar\.gz|exe|py)', content))
            if has_archive_download:
                self.assertIn(
                    "Get-FileHash",
                    content,
                    f"Workflow {yml_file.name} downloads archives/executables without Get-FileHash verification"
                )


class TestLinuxBundledPythonPackaging(unittest.TestCase):
    """Congela el empaquetado de Python en build-linux.yml (AppImage/DEB/tarball).

    Cada artefacto Linux debe llevar un CPython autónomo con las dependencias
    de scripts/requirements.txt ya instaladas, para que la app funcione tras
    instalar sin pedirle Python, pip ni debugging al usuario (mirror del
    python-embed de Windows). Quitar el bundle, el hash pineado o el smoke
    test rompe esa garantía y debe fallar la CI aquí primero.
    """

    @classmethod
    def setUpClass(cls):
        cls.build_linux_path = Path(__file__).resolve().parent.parent.parent / (
            ".github/workflows/build-linux.yml"
        )
        if not cls.build_linux_path.exists():
            raise FileNotFoundError(f"Workflow file missing: {cls.build_linux_path}")
        cls.content = cls.build_linux_path.read_text(encoding="utf-8")

    def test_workflow_is_valid_yaml(self):
        with open(self.build_linux_path, "r", encoding="utf-8") as f:
            docs = yaml.safe_load(f)
        self.assertIsInstance(docs, dict)
        self.assertIn("jobs", docs)

    def test_python_bundle_step_downloads_pinned_standalone_cpython(self):
        self.assertIn("python-build-standalone", self.content)
        self.assertIn(
            "cpython-3.11.9",
            self.content,
            "el bundle debe pinear la misma 3.11.x que el python-embed de Windows",
        )
        # Hash verificado contra el sidecar .sha256 oficial de la release.
        self.assertIn(
            "78b1c16a9fd032997ba92a60f46a64f795cd18ff335659dfdf6096df277b24d5",
            self.content.lower(),
            "el tarball del CPython autónomo debe verificarse con su SHA-256",
        )

    def test_python_bundle_installs_requirements_and_verifies_imports(self):
        self.assertIn(
            "requirements.txt", self.content,
            "las dependencias de los puentes deben instalarse en el bundle",
        )
        self.assertRegex(
            self.content,
            r"import spotipy, acoustid[^\n]*mutagen[^\n]*yarl",
            "el bundle debe verificar los módulos críticos antes de empaquetarse",
        )

    def test_appimage_injection_places_python_and_scripts_in_resources(self):
        self.assertRegex(
            self.content,
            r"resources/scripts",
            "el AppImage debe llevar resources/scripts (get_project_root lo usa de marcador)",
        )
        self.assertRegex(
            self.content,
            r"resources/python",
            "el AppImage debe llevar resources/python (candidate 0 de get_python_executable)",
        )
        self.assertIn(
            "appimagetool", self.content,
            "la inyección debe reempaquetar con appimagetool conservando AppRun",
        )

    def test_deb_injection_places_python_beside_up_scripts(self):
        self.assertIn("dpkg-deb -R", self.content)
        self.assertIn("dpkg-deb --root-owner-group -b", self.content)
        # El resolutor (cmd_utils::packaged_python_candidates) busca un
        # python/ hermano del _up_/scripts que Tauri empaqueta.
        self.assertRegex(
            self.content,
            r"\*_up_/scripts/dependency_manager\.py",
            "la inyección en el DEB debe localizar _up_/scripts por el marcador",
        )
        self.assertIn(
            "md5sums", self.content,
            "los md5sums del DEB deben regenerarse tras la inyección",
        )

    def test_final_smoke_test_covers_all_three_artifacts(self):
        self.assertIn("OK tarball", self.content)
        self.assertIn("OK appimage", self.content)
        self.assertIn("OK deb", self.content)
        # El smoke del AppImage replica el entorno de AppRun: si un
        # LD_LIBRARY_PATH del AppDir sombreara el libpython del bundle,
        # este es el sitio donde se detecta.
        self.assertIn("LD_LIBRARY_PATH", self.content)

    def test_smoke_env_clears_python_env_vars(self):
        # AppRun exporta PYTHONHOME (ver main.rs); el smoke test debe correr
        # sin él para reproducir lo que ve el binario tras el clear de main.rs.
        self.assertIn(
            "-u PYTHONHOME", self.content,
            "el smoke test debe limpiar PYTHONHOME/PYTHONPATH como hace la app",
        )

    def test_tauri_resources_include_services_package(self):
        conf = (self.build_linux_path.parent.parent.parent / "src-tauri/tauri.conf.json"
                ).read_text(encoding="utf-8")
        self.assertIn(
            "../scripts/services/*.py", conf,
            "el glob de recursos debe incluir scripts/services/ o los puentes "
            "empaquetados fallan con \"No module named 'services'\"",
        )


if __name__ == "__main__":
    unittest.main()

#!/usr/bin/env python3
"""
CI/CD Packaging Exclusions Hygiene & Download Integrity Test Suite (TASK-95 / SEC-011).

Since #247 the Windows portable is assembled by
``scripts/ci/assemble_portable.ps1``, which consumes the single exclusion
source ``scripts/ci/exclusions.txt`` (the same file the Linux raw tarball
applies). The workflow only delegates to the script, so this suite follows the
behaviour: the pattern contract is read from the source file, the recursive
purge and the WebView2 digest stay in the script, and the workflow must not
carry the old inline copy again.

Verifies:
1. The single exclusion source still declares every required SEC-011 pattern,
   and the Windows workflow delegates the portable assembly to the script
   instead of carrying a twin list inline.
2. Simulated pattern matching ensuring all known sensitive file patterns are
   excluded while legitimate production scripts are retained.
3. Cryptographic SHA-256 integrity verification for the remaining download
   (the optional WebView2 bootstrapper), which lives in the portable script.
4. Since #241/#242, ALL pinned dependencies (ffmpeg/fpcalc/ffprobe, CPython,
   get-pip.py) live in the provider's single pin table
   (scripts/dependency_manager.py) and are proven by
   test_dependency_manager_pins.py / test_dependency_manager_runtime.py; the
   workflows carry no digests of their own anymore.
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

        cls.exclusions_path = cls.repo_root / "scripts" / "ci" / "exclusions.txt"
        cls.portable_script_path = cls.repo_root / "scripts" / "ci" / "assemble_portable.ps1"
        if not cls.exclusions_path.is_file():
            raise FileNotFoundError(f"Exclusions source missing: {cls.exclusions_path}")
        if not cls.portable_script_path.is_file():
            raise FileNotFoundError(f"Portable assembler missing: {cls.portable_script_path}")
        cls.portable_content = cls.portable_script_path.read_text(encoding="utf-8")
        cls.exclusion_patterns = [
            line.strip()
            for line in cls.exclusions_path.read_text(encoding="utf-8").splitlines()
            if line.strip() and not line.strip().startswith("#")
        ]

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
        """Every SEC-011 pattern is declared in the single exclusion source."""
        for pattern in self.required_exclusion_patterns:
            self.assertIn(
                pattern,
                self.exclusion_patterns,
                f"Required exclusion pattern '{pattern}' missing in {self.exclusions_path.name}"
            )

    def test_workflow_delegates_portable_assembly_and_drops_the_inline_copy(self):
        """The job calls the script; the inline assembly (and its twin list) is gone."""
        self.assertIn(
            "assemble_portable.ps1",
            self.build_windows_content,
            "build-windows.yml must delegate the portable assembly to scripts/ci/assemble_portable.ps1",
        )
        for inline_marker in (
            "$scriptExclusions",
            "readmeLines",
            'Copy-Item -Path "scripts" -Destination "dist/portable/Syncify-Portable/scripts"',
            "instalar_webview2_si_falla.exe",
            "Invoke-WebRequest",
        ):
            self.assertNotIn(
                inline_marker,
                self.build_windows_content,
                f"build-windows.yml still carries the inline portable assembly: {inline_marker}",
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
                fnmatch.fnmatch(sample, pat) for pat in self.exclusion_patterns
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
            pat for pat in self.exclusion_patterns if pat != ".*"
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
            self.portable_content,
            "The portable assembler must contain post-copy hygiene/cleanup with Remove-Item"
        )
        self.assertRegex(
            self.portable_content,
            r"Get-ChildItem[^\n]*-Recurse",
            "The portable assembler must contain a recursive sweep over the portable scripts tree"
        )
        self.assertIn(
            "scripts/ci/exclusions.txt",
            self.portable_content,
            "The portable assembler must consume the single exclusion source",
        )

    # ═══════════════════════════════════════════════════════
    # 2. DOWNLOAD INTEGRITY & SHA-256 VERIFICATION TESTS
    # ═══════════════════════════════════════════════════════

    def test_all_invoke_webrequests_have_sha256_verification(self):
        """Verify that every download left inline in the packaging scripts performs SHA-256 verification.

        The pinned runtime/tool downloads moved into the provider module
        (#241/#242), where the hash-before-extract rule is enforced and proven
        by the module suites. What stays inline is the OPTIONAL WebView2
        bootstrapper, which moved with the portable assembly to
        ``scripts/ci/assemble_portable.ps1`` in #247, and it must keep its
        digest check. No download may drift back into the workflow.
        """
        self.assertNotIn(
            "Invoke-WebRequest",
            self.build_windows_content,
            "downloads belong to the packaging scripts, not to build-windows.yml",
        )

        # Find all Invoke-WebRequest calls with -OutFile
        download_matches = re.findall(
            r'Invoke-WebRequest[^\n]+-OutFile\s+["\']?([^"\'\n]+)["\']?',
            self.portable_content
        )
        self.assertGreater(len(download_matches), 0, "No downloads found in assemble_portable.ps1")

        # Every downloaded file should be checked with Get-FileHash ... -Algorithm SHA256
        hash_check_matches = re.findall(
            r'Get-FileHash[^\n]+-Algorithm\s+SHA256',
            self.portable_content,
            re.IGNORECASE
        )
        self.assertEqual(
            len(download_matches),
            len(hash_check_matches),
            f"Mismatch between downloads ({len(download_matches)}: {download_matches}) "
            f"and SHA256 checks ({len(hash_check_matches)})"
        )

    def test_webview2_bootstrapper_keeps_fwlink_allowlist_and_tolerance(self):
        """The optional WebView2 bootstrapper keeps the inline contract exactly.

        Its download is network-only, so the cross-adapter behaviour suite runs
        the assembler with -SkipWebView2; this text gate keeps the security
        properties that moved with it: the fwlink, the SHA-256 allowlist, the
        purge on mismatch and the release-engineering tolerance (warning, not a
        failed release).
        """
        self.assertIn("https://go.microsoft.com/fwlink/p/?LinkId=2124703", self.portable_content)
        self.assertIn(
            "17debf797a6c737959bc588236e897936ffac1af5f7e515e674ab32f9edfe719",
            self.portable_content,
        )
        self.assertIn("$knownWvHashes -notcontains $wvHash", self.portable_content)
        self.assertIn("Remove-Item -Force $webview2Path", self.portable_content)
        self.assertIn(
            "No se pudo descargar o validar el instalador opcional de WebView2",
            self.portable_content,
        )

    def test_integrity_check_failure_aborts_or_purges(self):
        """Verify that checksum mismatch causes an abort (throw) and purges the unverified file.

        One inline download remains (the optional WebView2 installer, now in
        the portable script) and it must keep its throw-on-mismatch. The pinned
        downloads abort inside the provider module — mismatch means no
        extraction — and the workflow throws on its nonzero exit code; that
        behavior is frozen by test_dependency_manager_security.py and the
        runtime suite, not here.
        """
        # Checks should contain throw statements on failure
        throw_matches = re.findall(r'throw\s+["\'].*?SHA256.*?["\']', self.portable_content, re.IGNORECASE)
        self.assertGreaterEqual(
            len(throw_matches),
            1,
            "assemble_portable.ps1 must contain throw statements on SHA256 integrity verification failures"
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
    """Lo que build-linux.yml sigue siendo dueño de declarar (AppImage/DEB/tarball).

    Cada artefacto Linux debe llevar un CPython autónomo con las dependencias
    de scripts/requirements.txt ya instaladas, para que la app funcione tras
    instalar sin pedirle Python, pip ni debugging al usuario (mirror del
    runtime de Windows). Desde #242 el MONTAJE del runtime vive en el
    proveedor (install-runtime en scripts/dependency_manager.py): la descarga
    pineada, los requirements y el smoke canónico se prueban en las suites del
    módulo, no congelando aquí los pasos que ya no existen. Desde #245 los
    SMOKES de los tres artefactos finales (imports canónicos con el env limpio
    + arranque del AppRun bajo xvfb con la guarda EGL) viven en
    scripts/ci/smoke_artifacts.sh, probados contra fixtures en
    test_ci_smoke_artifacts.py: las aserciones congeladas de
    "OK tarball/appimage/deb" murieron aquí. Ambas inyecciones dejaron de ser
    inline: la cadena del AppImage vive desde #243 en
    scripts/ci/package_appimage.sh (comportamiento en test_package_appimage.py)
    y la del DEB desde #244 en scripts/ci/package_deb.sh (comportamiento en
    test_ci_package_deb.py); sus aserciones congeladas murieron aquí. Esta
    clase conserva solo lo que el workflow sigue declarando: YAML válido y el
    glob de recursos compartido con los puentes.
    """

    @classmethod
    def setUpClass(cls):
        cls.build_linux_path = Path(__file__).resolve().parent.parent.parent / (
            ".github/workflows/build-linux.yml"
        )
        if not cls.build_linux_path.exists():
            raise FileNotFoundError(f"Workflow file missing: {cls.build_linux_path}")

    def test_workflow_is_valid_yaml(self):
        with open(self.build_linux_path, "r", encoding="utf-8") as f:
            docs = yaml.safe_load(f)
        self.assertIsInstance(docs, dict)
        self.assertIn("jobs", docs)

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

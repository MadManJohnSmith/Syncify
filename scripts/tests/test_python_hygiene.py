#!/usr/bin/env python3
"""
Python Module Hygiene & Anti-Orphan Regression Test Suite (TASK-130).

Validates:
1. Purged modules and legacy archive directory are completely absent from production paths.
2. Every remaining module in scripts/services/ has at least one active consumer.
3. All production bridge scripts compile cleanly and can resolve core imports.
4. Production modules carry no test harness and no unresolved TODO/FIXME markers
   (scripts/**/*.py is bundled into the app by src-tauri/tauri.conf.json, so a
   harness with a hardcoded fixture there ships to users).
"""

import ast
import re
import sys
import unittest
from pathlib import Path


class TestPythonModuleHygiene(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.repo_root = Path(__file__).resolve().parent.parent.parent
        cls.scripts_dir = cls.repo_root / "scripts"
        cls.services_dir = cls.scripts_dir / "services"

        cls.purged_production_paths = [
            cls.services_dir / "audio_converter.py",
            cls.services_dir / "soundcloud_api.py",
            cls.services_dir / "local_file_scanner.py",
            cls.services_dir / "spotify_api.py",
            cls.services_dir / "settings_manager.py",
            cls.scripts_dir / "health_check.py",
            cls.scripts_dir / "archive" / "replace_folders.py",
            cls.scripts_dir / "archive" / "replace_sync.py",
            cls.scripts_dir / "archive" / "parse_ndjson.py",
            cls.scripts_dir / "archive" / "test_bridges.py",
            cls.scripts_dir / "archive",
            cls.repo_root / "src-tauri" / "get_token.py",
            # IN-7: one-off repair scripts purged; each task was re-implemented
            # natively (Rust fn/command or SQL migration) and no consumer was left.
            cls.scripts_dir / "backfill_featured_artists.py",       # TASK-106 -> enrichment.rs (clean_title_and_extract_featured)
            cls.scripts_dir / "backfill_social_metadata.py",        # TASK-113 -> services/enrichment.rs::backfill_social_metadata
            cls.scripts_dir / "calculate_replaygain.py",            # TASK-73  -> services/enrichment.rs::calculate_replaygain + migration 0083
            cls.scripts_dir / "dedup_playlists.py",                 # TASK-107 -> commands/playlists.rs::sanitize_all_playlists
            cls.scripts_dir / "normalize_disk_layout.py",           # TASK-110 -> services/operation_recovery.rs::reconcile_canonical_download_records
            cls.scripts_dir / "purge_carriage_return_credits.py",   # TASK-133 -> migrations/0080_purge_carriage_return_artists_and_harden.sql
            cls.scripts_dir / "purge_catalog_placeholders.py",      # TASK-103 -> migrations/0078_album_stubs_and_placeholder_purge.sql
            cls.scripts_dir / "purge_technical_role_artists.py",    # TASK-68  -> migrations/0082_purge_technical_role_artists.sql
            cls.scripts_dir / "recalculate_audio_quality.py",       # TASK-72  -> commands/library.rs::perform_reconcile_library_physical_state
            cls.scripts_dir / "recompact_playlist_positions.py",    # TASK-79  -> commands/playlists.rs::recompact_playlist_positions + migration 0077
            cls.scripts_dir / "reconcile_downloads_storage.py",     #          -> commands/integrity.rs::reconcile_downloads_from_storage
            cls.scripts_dir / "repopulate_downloads_lyrics.py",     # TASK-111 -> commands/lyrics.rs sidecar completeness
            cls.scripts_dir / "rhythm_key_analyzer.py",             # TASK-74  -> commands/tempo.rs (pure Rust BPM analysis)
            cls.scripts_dir / "unify_artists_nocase.py",            # TASK-105 -> migrations/0079_canonical_artists_unification.sql
            # Residual bytecode of sources that no longer exist (PY-10).
            cls.scripts_dir / "tests" / "__pycache__" / "test_qobuz_service.cpython-314.pyc",
        ]

        # IN-7: real operation/maintenance tools were relocated here and must stay
        # documented in their README.
        cls.maintenance_dir = cls.scripts_dir / "maintenance"
        cls.maintenance_tools = (
            "detect_fake_e2e_tests.py",
            "spotify_free_tier_probe.py",
            "verify_migration_ledger.py",
        )


    def test_purged_modules_do_not_exist_in_production(self):
        """Ensure purged orphaned modules and obsolete archive dirs are removed from tree."""
        for path in self.purged_production_paths:
            self.assertFalse(
                path.exists(),
                f"Orphaned or legacy path still exists in production: {path}"
            )

    def test_all_services_have_active_consumers(self):
        """Ensure every service module in scripts/services/ is consumed by at least one file."""
        service_files = [
            f.name for f in self.services_dir.glob("*.py")
            if f.name != "__init__.py"
        ]

        # Scan all python files in scripts/ (root, services, tests) for imports
        python_files = list(self.scripts_dir.glob("*.py")) + \
                       list(self.services_dir.glob("*.py")) + \
                       list((self.scripts_dir / "tests").glob("*.py"))

        imported_modules = set()

        for py_file in python_files:
            try:
                content = py_file.read_text(encoding="utf-8")
                tree = ast.parse(content, filename=str(py_file))
            except Exception:
                continue

            for node in ast.walk(tree):
                if isinstance(node, ast.Import):
                    for alias in node.names:
                        parts = alias.name.split(".")
                        if parts[0] == "services" and len(parts) > 1:
                            imported_modules.add(parts[1] + ".py")
                        else:
                            imported_modules.add(parts[0] + ".py")
                elif isinstance(node, ast.ImportFrom):
                    if node.module:
                        parts = node.module.split(".")
                        if parts[0] == "services" and len(parts) > 1:
                            imported_modules.add(parts[1] + ".py")
                        elif parts[0] == "services":
                            for alias in node.names:
                                imported_modules.add(alias.name + ".py")
                        else:
                            imported_modules.add(parts[0] + ".py")
                    elif node.level > 0:
                        # Relative import: from .service_base import ... or from . import ...
                        for alias in node.names:
                            imported_modules.add(alias.name + ".py")

        for service in service_files:
            self.assertIn(
                service,
                imported_modules,
                f"Service module '{service}' appears to have no active importers in scripts/"
            )

    def test_maintenance_tools_live_in_maintenance_and_are_documented(self):
        """IN-7: every maintenance tool lives in scripts/maintenance/ and is
        documented in scripts/maintenance/README.md (no orphan tools elsewhere)."""
        readme = self.maintenance_dir / "README.md"
        self.assertTrue(readme.exists(), "scripts/maintenance/README.md is missing")
        readme_text = readme.read_text(encoding="utf-8")
        for tool in self.maintenance_tools:
            self.assertTrue(
                (self.maintenance_dir / tool).exists(),
                f"Maintenance tool missing from scripts/maintenance/: {tool}"
            )
            self.assertIn(
                tool,
                readme_text,
                f"Maintenance tool '{tool}' is not documented in scripts/maintenance/README.md"
            )

    def test_maintenance_tools_resolve_repo_root_after_relocation(self):
        """The relocated tools must still resolve the actual repo root (not scripts/)."""
        repo_root = str(self.repo_root.resolve())
        checks = {
            "detect_fake_e2e_tests.py": "REPO_ROOT",
            "spotify_free_tier_probe.py": "raiz_repo",
        }
        import importlib.util
        for tool, func_name in checks.items():
            path = self.maintenance_dir / tool
            spec = importlib.util.spec_from_file_location(tool[:-3], path)
            mod = importlib.util.module_from_spec(spec)
            spec.loader.exec_module(mod)
            resolved = mod.REPO_ROOT.resolve() if hasattr(mod, "REPO_ROOT") else Path(getattr(mod, func_name)())
            self.assertEqual(
                str(resolved), repo_root,
                f"{tool} no longer resolves the repo root after relocation"
            )

    def test_maintenance_tools_are_self_contained(self):
        """Maintenance tools must not import repo modules (stdlib only), so they
        keep working from their relocated location."""
        import ast
        allowed_roots = {"__future__"} | set(sys.stdlib_module_names)
        for tool in self.maintenance_tools:
            tree = ast.parse((self.maintenance_dir / tool).read_text(encoding="utf-8"))
            for node in ast.walk(tree):
                roots = []
                if isinstance(node, ast.Import):
                    roots = [alias.name.split(".")[0] for alias in node.names]
                elif isinstance(node, ast.ImportFrom) and node.level == 0 and node.module:
                    roots = [node.module.split(".")[0]]
                for root in roots:
                    self.assertIn(
                        root, allowed_roots,
                        f"{tool} imports non-stdlib module '{root}'; maintenance tools must be self-contained"
                    )

    def test_production_bridges_syntax(self):
        """Verify all production bridge scripts compile cleanly to AST."""
        bridges = list(self.scripts_dir.glob("*_bridge.py"))
        self.assertGreater(len(bridges), 0, "No bridge scripts found in scripts/")
        for bridge in bridges:
            with open(bridge, "r", encoding="utf-8") as f:
                content = f.read()
            # ast.parse ensures valid Python syntax without runtime side-effects
            try:
                tree = ast.parse(content, filename=str(bridge))
                self.assertIsNotNone(tree)
            except SyntaxError as e:
                self.fail(f"Syntax error in bridge {bridge.name}: {e}")

    def test_production_services_carry_no_test_harness(self):
        """Service modules are bundled with the app and expose no CLI: no harness allowed."""
        offenders = []
        for path in self.services_dir.glob("*.py"):
            try:
                tree = ast.parse(path.read_text(encoding="utf-8"), filename=str(path))
            except SyntaxError as exc:
                self.fail(f"Syntax error in {path}: {exc}")
            for node in tree.body:
                if isinstance(node, (ast.FunctionDef, ast.AsyncFunctionDef)) and node.name.startswith("test"):
                    offenders.append(f"{path.name}:{node.lineno} {node.name}()")

        self.assertEqual(
            offenders, [],
            f"Test harnesses must live in scripts/tests/, not in bundled service modules: {offenders}",
        )

    def test_no_production_module_runs_a_test_harness_as_main(self):
        """A `__main__` block that executes a `test_*` function is a harness shipped in the bundle."""
        production_files = list(self.scripts_dir.glob("*.py")) + \
            list(self.services_dir.glob("*.py"))

        offenders = []
        for path in production_files:
            tree = ast.parse(path.read_text(encoding="utf-8"), filename=str(path))
            for node in tree.body:
                if not isinstance(node, ast.If) or not self._is_dunder_main_guard(node):
                    continue
                for inner in ast.walk(node):
                    if isinstance(inner, ast.Call) and isinstance(inner.func, ast.Name) \
                            and inner.func.id.startswith("test"):
                        offenders.append(f"{path.name}:{inner.lineno} {inner.func.id}()")

        self.assertEqual(
            offenders, [],
            f"Executable test harnesses must live in scripts/tests/, not in bundled modules: {offenders}",
        )

    @staticmethod
    def _is_dunder_main_guard(node):
        """True when the `if` statement tests `__name__ == "__main__"`."""
        test = node.test
        return (
            isinstance(test, ast.Compare)
            and isinstance(test.left, ast.Name)
            and test.left.id == "__name__"
            and any(
                isinstance(op, ast.Eq)
                and isinstance(comparator, ast.Constant)
                and comparator.value == "__main__"
                for op, comparator in zip(test.ops, test.comparators)
            )
        )

    def test_production_modules_carry_no_unresolved_markers(self):
        """TODO/FIXME in bundled Python means unfinished work shipped to users."""
        marker = re.compile(r"#\s*(TODO|FIXME|XXX)\b")
        production_files = list(self.scripts_dir.glob("*.py")) + \
            list(self.services_dir.glob("*.py"))

        offenders = []
        for path in production_files:
            for lineno, line in enumerate(path.read_text(encoding="utf-8").splitlines(), start=1):
                if marker.search(line):
                    offenders.append(f"{path.name}:{lineno}")

        self.assertEqual(
            offenders, [],
            f"Unresolved markers left in bundled production Python: {offenders}",
        )

    def test_download_bridge_handlers(self):
        """Verify download_bridge.py registers expected production streaming services."""
        import importlib.util
        spec = importlib.util.spec_from_file_location("download_bridge", self.scripts_dir / "download_bridge.py")
        mod = importlib.util.module_from_spec(spec)
        spec.loader.exec_module(mod)
        self.assertIn("qobuz", mod.HANDLERS)
        self.assertIn("tidal", mod.HANDLERS)
        self.assertIn("deezer", mod.HANDLERS)
        self.assertIn("soundcloud", mod.HANDLERS)


if __name__ == "__main__":
    unittest.main()

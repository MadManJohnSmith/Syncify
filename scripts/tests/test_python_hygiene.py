#!/usr/bin/env python3
"""
Python Module Hygiene & Anti-Orphan Regression Test Suite (TASK-130).

Validates:
1. Purged modules and legacy archive directory are completely absent from production paths.
2. Purged modules are properly archived in workspace/audit_archive/scripts/orphaned_python_modules/.
3. Every remaining module in scripts/services/ has at least one active consumer.
4. All production bridge scripts compile cleanly and can resolve core imports.
"""

import ast
import os
import unittest
from pathlib import Path


class TestPythonModuleHygiene(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.repo_root = Path(__file__).resolve().parent.parent.parent
        cls.scripts_dir = cls.repo_root / "scripts"
        cls.services_dir = cls.scripts_dir / "services"
        cls.archive_dir = cls.repo_root / "workspace" / "audit_archive" / "scripts" / "orphaned_python_modules"

        cls.purged_production_paths = [
            cls.services_dir / "audio_converter.py",
            cls.services_dir / "soundcloud_api.py",
            cls.services_dir / "local_file_scanner.py",
            cls.services_dir / "settings_manager.py",
            cls.scripts_dir / "health_check.py",
            cls.scripts_dir / "archive" / "replace_folders.py",
            cls.scripts_dir / "archive" / "replace_sync.py",
            cls.scripts_dir / "archive" / "parse_ndjson.py",
            cls.scripts_dir / "archive" / "test_bridges.py",
            cls.scripts_dir / "archive",
            cls.repo_root / "src-tauri" / "get_token.py",
        ]

        cls.expected_archived_files = [
            "audio_converter.py",
            "soundcloud_api.py",
            "local_file_scanner.py",
            "settings_manager.py",
            "health_check.py",
            "replace_folders.py",
            "replace_sync.py",
            "parse_ndjson.py",
            "test_bridges.py",
            "README.md",
        ]

    def test_purged_modules_do_not_exist_in_production(self):
        """Ensure purged orphaned modules and obsolete archive dirs are removed from tree."""
        for path in self.purged_production_paths:
            self.assertFalse(
                path.exists(),
                f"Orphaned or legacy path still exists in production: {path}"
            )

    def test_archived_modules_and_readme_present(self):
        """Ensure all purged files are properly preserved in the audit archive with a README."""
        self.assertTrue(
            self.archive_dir.is_dir(),
            f"Archive directory missing: {self.archive_dir}"
        )
        for filename in self.expected_archived_files:
            file_path = self.archive_dir / filename
            self.assertTrue(
                file_path.exists(),
                f"Expected archived artifact missing: {file_path}"
            )
            self.assertGreater(
                file_path.stat().st_size,
                0,
                f"Archived artifact is empty: {file_path}"
            )

        readme_text = (self.archive_dir / "README.md").read_text(encoding="utf-8")
        for filename in self.expected_archived_files:
            if filename == "README.md":
                continue
            self.assertIn(
                filename,
                readme_text,
                f"README.md in archive must document archived module: {filename}"
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

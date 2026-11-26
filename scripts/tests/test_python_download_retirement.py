#!/usr/bin/env python3
"""D14 (#126): la descarga no vuelve a cruzar a Python.

La app descarga en Rust (ADR-0009). Este guardián fija las dos mitades de esa
decisión sin red:

1. Ningún fuente de producción en `src-tauri/src/` lanza
   `download_bridge.py` ni construye un comando Python dentro de
   `src-tauri/src/download/`.
2. `scripts/services/` no vuelve a exponer `download_track` ni sus helpers de
   resolución/volcado, y `scripts/download_bridge.py` sigue siendo un
   tombstone que falla con un envelope JSON y exit 1.
"""

import ast
import importlib.util
import io
import json
import unittest
from contextlib import redirect_stdout
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parent.parent.parent
SCRIPTS_DIR = REPO_ROOT / "scripts"
RUST_SRC = REPO_ROOT / "src-tauri" / "src"

RETIRED_SERVICE_METHODS = {
    "download_track",
    "_get_download_url",
    "_get_stream_url",
    "_download_hls_stream",
    "_download_original_file",
}


class TestPythonDownloadRetirement(unittest.TestCase):
    def test_rust_production_never_names_the_python_bridge(self):
        offenders = []
        for path in RUST_SRC.rglob("*.rs"):
            if "download_bridge" in path.read_text(encoding="utf-8"):
                offenders.append(str(path.relative_to(REPO_ROOT)))
        self.assertEqual(offenders, [], f"Rust vuelve a invocar el puente Python: {offenders}")

    def test_rust_download_module_does_not_spawn_python(self):
        download_dir = RUST_SRC / "download"
        offenders = []
        for path in download_dir.rglob("*.rs"):
            content = path.read_text(encoding="utf-8")
            for marker in ("BridgeClient", "create_python_tokio_command", "create_python_std_command"):
                if marker in content:
                    offenders.append(f"{path.relative_to(REPO_ROOT)} contiene {marker}")
        self.assertEqual(offenders, [], f"El camino nativo de descarga lanza Python: {offenders}")

    def test_services_no_longer_define_download_entry_points(self):
        offenders = []
        for path in (SCRIPTS_DIR / "services").glob("*.py"):
            tree = ast.parse(path.read_text(encoding="utf-8"), filename=str(path))
            for node in ast.walk(tree):
                if isinstance(node, (ast.FunctionDef, ast.AsyncFunctionDef)) and node.name in RETIRED_SERVICE_METHODS:
                    offenders.append(f"{path.name}:{node.lineno} {node.name}()")
                if isinstance(node, ast.ImportFrom) and node.module and node.module.endswith("service_base"):
                    for alias in node.names:
                        if alias.name in {"DownloadResult", "DownloadStatus"}:
                            offenders.append(f"{path.name}:{node.lineno} importa {alias.name}")
        self.assertEqual(offenders, [], f"scripts/services/ vuelve a exponer descarga: {offenders}")

    def test_download_bridge_is_only_a_retired_tombstone(self):
        path = SCRIPTS_DIR / "download_bridge.py"
        spec = importlib.util.spec_from_file_location("download_bridge_tombstone", path)
        module = importlib.util.module_from_spec(spec)
        spec.loader.exec_module(module)

        self.assertFalse(hasattr(module, "HANDLERS"))
        content = path.read_text(encoding="utf-8")
        self.assertNotIn("from services", content)
        self.assertNotIn("import services", content)
        stdout = io.StringIO()
        with redirect_stdout(stdout):
            exit_code = module.main()
        payload = json.loads(stdout.getvalue())
        self.assertEqual(exit_code, 1)
        self.assertFalse(payload["success"])
        self.assertIn("retirado", payload["error"].lower())


if __name__ == "__main__":
    unittest.main()

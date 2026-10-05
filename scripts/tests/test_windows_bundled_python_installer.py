#!/usr/bin/env python3
"""
Windows Bundled Python Installer Test Suite (auditoría 1).

Congela el empaquetado del runtime de Python en el instalador de Windows:

1. ``tauri.windows.conf.json`` declara el runtime embebido como recurso, de modo
   que el instalador NSIS lo copia en lugar de dejar que la app caiga al Python
   del sistema en una instalación limpia.
2. El orden del workflow mantiene el runtime preparado ANTES de ``tauri build``
   (que es cuando el NSIS empaqueta los recursos).
3. Existe una verificación posterior al build que mira DENTRO del instalador y
   falla si el runtime no está: es el único sitio donde se caza el fallo
   silencioso (un glob que no casa con nada se traduce en un instalador sin
   Python y la app arranca igual, connecting con el Python del sistema).

El resource mapping de Tauri (``resource_relpath``) convierte cada ``..`` del
glob en el directorio ``_up_``, así que ``../python/**/*`` se instala como
``_up_/python/python.exe``. La verificación tiene que mirar exactamente ahí.
"""

import json
import unittest
from pathlib import Path

import yaml

REPO_ROOT = Path(__file__).resolve().parent.parent.parent
WINDOWS_WORKFLOW = REPO_ROOT / ".github" / "workflows" / "build-windows.yml"
WINDOWS_TAURI_CONFIG = REPO_ROOT / "src-tauri" / "tauri.windows.conf.json"

PYTHON_PREP_STEP = "Preparar entorno Python embebido autónomo"
TAURI_BUILD_STEP = "Compilar con Tauri v2 (Instalador NSIS y Binarios)"
PYTHON_GUARD_STEP = "Verificar que el instalador NSIS incluye el Python embebido"


def resource_relpath(relative: str) -> str:
    """Replica el mapeo de Tauri: cada `..` del glob se vuelve `_up_`."""
    parts = []
    for segment in relative.split("/"):
        if segment in ("", "."):
            continue
        parts.append("_up_" if segment == ".." else segment)
    return "\\".join(parts)


class TestWindowsBundledPythonInstaller(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        if not WINDOWS_WORKFLOW.is_file():
            raise FileNotFoundError(f"Workflow file missing: {WINDOWS_WORKFLOW}")
        cls.workflow = yaml.safe_load(WINDOWS_WORKFLOW.read_text(encoding="utf-8"))
        cls.steps = cls.workflow["jobs"]["build-windows"]["steps"]
        cls.step_names = [step.get("name", "") for step in cls.steps]

    def step_index(self, name: str) -> int:
        self.assertIn(name, self.step_names, f"Falta el paso '{name}' en build-windows.yml")
        return self.step_names.index(name)

    def step_script(self, name: str) -> str:
        return self.steps[self.step_index(name)].get("run", "")

    # ═══════════════════════════════════════════════════════
    # 1. DECLARACIÓN DEL RECURSO
    # ═══════════════════════════════════════════════════════

    def test_windows_resources_include_the_embedded_runtime(self):
        self.assertTrue(
            WINDOWS_TAURI_CONFIG.is_file(),
            "Falta src-tauri/tauri.windows.conf.json: sin el, el NSIS instala sin Python",
        )
        config = json.loads(WINDOWS_TAURI_CONFIG.read_text(encoding="utf-8"))
        resources = config["bundle"]["resources"]
        self.assertTrue(
            any(entry.startswith("../python/") for entry in resources),
            f"bundle.resources debe incluir el runtime embebido: {resources}",
        )

    def test_shared_resources_are_kept_alongside_the_runtime(self):
        # La CLI fusiona la config de plataforma con json_patch::merge, que
        # sustituye arrays: repetir solo el glob de Python dejaría fuera los
        # scripts y la app se quedaría sin puentes.
        shared = json.loads(
            (REPO_ROOT / "src-tauri" / "tauri.conf.json").read_text(encoding="utf-8")
        )["bundle"]["resources"]
        windows = json.loads(WINDOWS_TAURI_CONFIG.read_text(encoding="utf-8"))["bundle"]["resources"]
        for entry in shared:
            self.assertIn(entry, windows, f"tauri.windows.conf.json pierde el recurso {entry}")

    # ═══════════════════════════════════════════════════════
    # 2. ORDEN DEL WORKFLOW
    # ═══════════════════════════════════════════════════════

    def test_runtime_is_prepared_before_tauri_builds_the_installer(self):
        prep = self.step_index(PYTHON_PREP_STEP)
        build = self.step_index(TAURI_BUILD_STEP)
        self.assertLess(
            prep,
            build,
            "El runtime debe existir en el árbol ANTES de `tauri build`: el NSIS "
            "empaqueta los recursos en ese momento",
        )

    def test_installer_content_is_verified_before_publishing(self):
        build = self.step_index(TAURI_BUILD_STEP)
        guard = self.step_index(PYTHON_GUARD_STEP)
        publish = self.step_index("Subir Instalador NSIS como Artefacto")
        self.assertLess(build, guard, "La verificación va después del build")
        self.assertLess(guard, publish, "La verificación va antes de publicar el instalador")

    # ═══════════════════════════════════════════════════════
    # 3. LA VERIFICACIÓN MIRA DÓNDE VA A PARAR EL RECURSO
    # ═══════════════════════════════════════════════════════

    def test_guard_lists_the_installer_and_fails_when_the_runtime_is_missing(self):
        script = self.step_script(PYTHON_GUARD_STEP)
        self.assertIn("7z l", script, "La verificación debe listar el instalador NSIS")
        self.assertIn(
            "python/python.exe", script, "Debe comprobar que el runtime estaba en el árbol"
        )
        self.assertIn(
            "_up_", script, "Debe buscar el runtime en la ruta donde Tauri lo instala"
        )
        self.assertIn(
            "throw", script, "Sin throw el fallo silencioso pasa: el instalador sale sin Python"
        )

    def test_guard_looks_where_the_declared_glob_lands(self):
        resources = json.loads(WINDOWS_TAURI_CONFIG.read_text(encoding="utf-8"))["bundle"][
            "resources"
        ]
        python_globs = [entry for entry in resources if entry.startswith("../python/")]
        self.assertTrue(python_globs, "No hay glob de Python declarado")

        script = self.step_script(PYTHON_GUARD_STEP)
        for glob in python_globs:
            # `../python/**/*` -> `_up_\python\...`
            self.assertTrue(
                resource_relpath(glob).startswith("_up_\\python"),
                f"El glob {glob} no instala bajo _up_\\python: la verificación miraría otra ruta",
            )
        self.assertIn(
            r"_up_.+python\.exe",
            script,
            "El patrón de búsqueda debe cubrir la ruta real del runtime dentro del instalador",
        )


if __name__ == "__main__":
    unittest.main(verbosity=2)
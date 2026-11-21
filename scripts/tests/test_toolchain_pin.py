#!/usr/bin/env python3
"""Coherencia del pin de Rust entre rust-toolchain.toml y los workflows (#249).

El canal vive en un unico sitio versionado, ``rust-toolchain.toml``: rustup lo
aplica a cualquier cargo/rustc que corra dentro del repo (override de
directorio), sin que la accion de toolchain lo lea ni exporte
``RUSTUP_TOOLCHAIN``. Cada workflow que compila Rust instala ademas ese mismo
canal con ``dtolnay/rust-toolchain@<canal>``, de modo que local, CI y releases
compilan con el mismo compilador.

La puerta es hermética: solo lee el arbol, no habla con la red ni con GitHub
Actions. Rechaza alias flotantes (``stable``, ``beta``, ``nightly``) en ambos
lados: el pin solo se mueve con un commit explicito.
"""

import re
import tomllib
import unittest
from pathlib import Path

import yaml

REPO_ROOT = Path(__file__).resolve().parent.parent.parent
TOOLCHAIN_FILE = REPO_ROOT / "rust-toolchain.toml"
WORKFLOWS_DIR = REPO_ROOT / ".github" / "workflows"
TOOLCHAIN_ACTION = "dtolnay/rust-toolchain"

# Los tres workflows que compilan Rust. El barrido mantiene la lista honesta:
# si aparece otro que compile sin pin, o uno de estos deja de compilar, el test
# lo reporta en vez de dejar la lista desactualizada.
RUST_WORKFLOWS = {"build-linux.yml", "build-windows.yml", "ci.yml"}

# Un canal pineado es una version exacta, no un alias que se mueve solo.
CONCRETE_CHANNEL = re.compile(r"^\d+\.\d+\.\d+$")

# "Compila Rust" se detecta por las herramientas que invocan el compilador:
# cargo/rustc directos y `tauri build`, que compila el core a traves de cargo.
COMPILES_RUST = re.compile(r"\b(?:cargo|rustc)\b|\btauri\s+build\b")


def _toolchain_section():
    return tomllib.loads(TOOLCHAIN_FILE.read_text(encoding="utf-8")).get("toolchain", {})


def _steps(workflow):
    for job in (workflow.get("jobs") or {}).values():
        for step in job.get("steps") or []:
            if isinstance(step, dict):
                yield step


def _toolchain_refs(workflow):
    """Ramas ``@<ref>`` de la accion de toolchain usadas por un workflow."""
    refs = []
    for step in _steps(workflow):
        uses = step.get("uses")
        if isinstance(uses, str) and uses.strip().startswith(f"{TOOLCHAIN_ACTION}@"):
            refs.append(uses.strip().split("@", 1)[1])
    return refs


def _compiles_rust(workflow):
    return any(
        COMPILES_RUST.search(step["run"])
        for step in _steps(workflow)
        if isinstance(step.get("run"), str)
    )


def _load_workflow(name):
    return yaml.safe_load((WORKFLOWS_DIR / name).read_text(encoding="utf-8"))


class RustToolchainPinTest(unittest.TestCase):
    """El canal del fichero y el de cada workflow que compila Rust es el mismo."""

    def test_toolchain_file_declares_only_a_concrete_channel(self):
        self.assertTrue(
            TOOLCHAIN_FILE.is_file(),
            f"Falta {TOOLCHAIN_FILE.relative_to(REPO_ROOT)}: es la fuente unica del canal de Rust",
        )
        toolchain = _toolchain_section()
        channel = toolchain.get("channel")
        self.assertIsInstance(channel, str, "rust-toolchain.toml debe declarar [toolchain] channel")
        self.assertRegex(
            channel,
            CONCRETE_CHANNEL,
            f"El canal {channel!r} no es una version exacta: un alias flotante mueve fmt y clippy solo",
        )
        self.assertEqual(
            set(toolchain),
            {"channel"},
            "rust-toolchain.toml declara solo el canal: componentes y targets son inputs por workflow",
        )

    def test_every_rust_workflow_uses_the_pinned_channel(self):
        channel = _toolchain_section()["channel"]
        detected = set()
        for path in sorted(WORKFLOWS_DIR.glob("*.yml")):
            workflow = yaml.safe_load(path.read_text(encoding="utf-8"))
            if _compiles_rust(workflow):
                detected.add(path.name)
            for ref in _toolchain_refs(workflow):
                self.assertEqual(
                    ref,
                    channel,
                    f"{path.name}: la accion instala @{ref} pero rust-toolchain.toml pinea {channel}",
                )

        self.assertEqual(
            detected,
            RUST_WORKFLOWS,
            "La lista de workflows que compilan Rust cambio: actualiza este test y pina el nuevo",
        )
        for name in sorted(RUST_WORKFLOWS):
            self.assertTrue(
                _toolchain_refs(_load_workflow(name)),
                f"{name} compila Rust y no invoca {TOOLCHAIN_ACTION}@{channel}",
            )


if __name__ == "__main__":
    unittest.main()

#!/usr/bin/env python3
"""Recuentos del AGENTS.md: una sola medida y una sola reescritura.

La puerta `scripts/tests/test_agents_claims.py` compara cinco cifras del
AGENTS.md contra el arbol y `tools/update-agents-claims.sh` las reescribe con
lo medido. Antes de #368 esa resta la hacia cada carril a mano: dos carriles
que anadian un objetivo de test cada uno subian la misma cifra en 1, el merge
tenia +2 y el segundo PR fallaba en docs aunque su rama fuera correcta.

Aqui viven el patron de cada cifra, la medida y la reescritura; la puerta y el
helper importan este modulo, de modo que no hay dos implementaciones que se
separen (el patron que reescribe es el que verifica).

Contrato con el helper:

  python scripts/agents_claims.py [--repo-root DIR] [--check]
      Sin --check reescribe AGENTS.md y solo las lineas medidas.
      Con --check no escribe: sale 1 si alguna cifra esta obsoleta.
      Sale 2 (sin escribir nada) si no puede medir una cifra.

Notas de la medida, heredadas de la puerta:

  - Los objetivos de test se derivan de `cargo metadata` mas los atributos de
    test en `src/`, que es la derivacion que reproduce el recuento original.
  - Los tests de Python los cuenta la discovery del interprete que ejecuta
    esto: fuera de `.venv` pierde modulos y la cifra falla por el entorno, no
    por el arbol. El helper prefiere `.venv/bin/python` por eso.
  - La cifra de `let _ = sqlx::query` no se mide por grep: su fuente de verdad
    es el suelo del ratchet `scripts/tests/test_silent_write_ratchet.py`, y la
    puerta exige que el AGENTS.md publique ese suelo. Reescribir un grep aqui
    romperia la unica fuente de verdad; el helper copia el suelo.
"""

from __future__ import annotations

import argparse
import importlib.util
import json
import re
import subprocess
import sys
import unittest
from dataclasses import dataclass
from pathlib import Path
from typing import Callable, Mapping, Sequence

DEFAULT_REPO_ROOT = Path(__file__).resolve().parents[1]
AGENTS_MD = "AGENTS.md"
IGNORE_GLOBS = ("src-tauri/tests/**/*.rs", "crates/*/tests/**/*.rs")
RATCHET_PATH = Path("scripts") / "tests" / "test_silent_write_ratchet.py"
RUST_SOURCE_ROOT = Path("src-tauri") / "src"

# Patrones con un unico grupo: la cifra tal como la publica el AGENTS.md.
# Son los mismos que usa la puerta para leer y el helper para reescribir.
UNIT_TEST_ATTRIBUTE = re.compile(r"#\[(?:tokio::test|test|sqlx::test)\b")


class MeasurementError(RuntimeError):
    """Una cifra no se puede medir o no se puede reescribir con seguridad."""


class CargoUnavailable(MeasurementError):
    """`cargo` no esta en PATH: la cifra de objetivos no se puede medir aqui."""


@dataclass(frozen=True)
class Claim:
    key: str
    pattern: str
    label: str


@dataclass(frozen=True)
class Change:
    key: str
    label: str
    before: int
    after: int


CLAIMS: tuple[Claim, ...] = (
    Claim("cargo_targets", r"(\d+) objetivos de test", "los objetivos de cargo"),
    Claim("python_tests", r"(\d+) tests \(\d+ skipped\)", "la suite Python"),
    Claim("ui_files", r"(\d+) ficheros de test UI", "los ficheros de la UI"),
    Claim("ignore_tests", r"los (\d+) tests `#\[ignore\]`", "los tests ignorados"),
    Claim("silent_writes", r"~(\d+) `let _ = sqlx::query", "las escrituras silenciosas"),
)
SPEC_BY_KEY = {spec.key: spec for spec in CLAIMS}


def measure_cargo_targets(repo_root: Path) -> int:
    """Objetivos de test del workspace: `cargo metadata` mas unit tests.

    Cada paquete aporta sus targets de integracion (kind "test"); lib y bins
    aportan un objetivo unitario solo si algun `.rs` bajo `src/` lleva
    atributo de test.
    """
    try:
        completed = subprocess.run(
            ["cargo", "metadata", "--locked", "--no-deps", "--format-version", "1"],
            capture_output=True,
            text=True,
            cwd=repo_root,
        )
    except FileNotFoundError as error:
        raise CargoUnavailable("cargo no esta en PATH: no se pueden medir los objetivos") from error
    if completed.returncode != 0:
        raise MeasurementError(f"cargo metadata fallo: {completed.stderr.strip()}")
    total = 0
    for package in json.loads(completed.stdout)["packages"]:
        kinds = [target.get("kind") or [] for target in package["targets"]]
        total += sum(1 for kind in kinds if "test" in kind)
        package_dir = Path(package["manifest_path"]).parent
        if any(
            UNIT_TEST_ATTRIBUTE.search(path.read_text(encoding="utf-8", errors="ignore"))
            for path in (package_dir / "src").rglob("*.rs")
        ):
            total += sum(1 for kind in kinds if "lib" in kind or "bin" in kind)
    return total


def measure_python_tests(repo_root: Path) -> int:
    """Casos que carga la discovery sobre `scripts/tests`."""
    tests_dir = repo_root / "scripts" / "tests"
    if not tests_dir.is_dir():
        raise MeasurementError(f"no existe {tests_dir}: no se puede contar la suite Python")
    return unittest.defaultTestLoader.discover(str(tests_dir)).countTestCases()


def measure_ui_files(repo_root: Path) -> int:
    ui_src = repo_root / "ui" / "src"
    if not ui_src.is_dir():
        raise MeasurementError(f"no existe {ui_src}: no se pueden contar los ficheros de la UI")
    return len(list(ui_src.rglob("*.spec.ts"))) + len(list(ui_src.rglob("*.test.ts")))


def measure_ignore_sites(repo_root: Path) -> int:
    sites = sum(
        path.read_text(encoding="utf-8").count("#[ignore")
        for pattern in IGNORE_GLOBS
        for path in repo_root.glob(pattern)
    )
    if sites == 0:
        raise MeasurementError("no hay sitios #[ignore]: el conteo seria vacuo")
    return sites


def measure_silent_writes(repo_root: Path) -> int:
    """Suelo del ratchet de escrituras silenciosas, no un grep del arbol."""
    ratchet = repo_root / RATCHET_PATH
    if not ratchet.is_file():
        raise MeasurementError(f"no existe {ratchet}: no hay suelo que publicar")
    spec = importlib.util.spec_from_file_location("agents_claims_silent_write_ratchet", ratchet)
    if spec is None or spec.loader is None:
        raise MeasurementError(f"no se puede importar {ratchet}")
    module = importlib.util.module_from_spec(spec)
    # El modulo debe estar en sys.modules antes de ejecutarse: en 3.14,
    # dataclasses y las anotaciones resuelven su modulo por nombre.
    sys.modules[spec.name] = module
    try:
        spec.loader.exec_module(module)
    except Exception as error:  # noqa: BLE001 - el modulo es del repo, no una API
        raise MeasurementError(f"no se puede leer BASELINE de {ratchet}: {error}") from error
    baseline = getattr(module, "BASELINE", None)
    if not isinstance(baseline, int):
        raise MeasurementError(f"{ratchet} no declara un BASELINE entero")
    return baseline


MEASURERS: dict[str, Callable[[Path], int]] = {
    "cargo_targets": measure_cargo_targets,
    "python_tests": measure_python_tests,
    "ui_files": measure_ui_files,
    "ignore_tests": measure_ignore_sites,
    "silent_writes": measure_silent_writes,
}


def measure(repo_root: Path) -> dict[str, int]:
    """Las cinco cifras del AGENTS.md medidas sobre `repo_root`.

    Mide todo antes de devolver nada: el helper escribe solo cuando las cinco
    medidas existen, para no dejar el AGENTS.md a medias.
    """
    return {spec.key: MEASURERS[spec.key](repo_root) for spec in CLAIMS}


def claimed(text: str, spec: Claim) -> int | None:
    """La cifra que el AGENTS.md publica para `spec`, o None si no la cifra."""
    match = re.search(spec.pattern, text)
    return None if match is None else int(match.group(1))


def _with_group(match: re.Match[str], value: int) -> str:
    """Sustituye solo el grupo 1 del match, sin tocar el resto de la linea."""
    whole = match.group(0)
    start = match.start(1) - match.start(0)
    end = match.end(1) - match.start(0)
    return f"{whole[:start]}{value}{whole[end:]}"


def rewrite(text: str, measurements: Mapping[str, int]) -> tuple[str, list[Change]]:
    """Devuelve el texto con las cifras medidas y la lista de cambios.

    Falla, sin devolver texto, si una cifra no tiene medida o si su patron dejo
    de aparecer exactamente una vez: reescribir a ciegas tocaria lineas ajenas.
    """
    missing = [spec.key for spec in CLAIMS if spec.key not in measurements]
    if missing:
        raise MeasurementError(f"faltan medidas para {', '.join(missing)}")

    updated = text
    changes: list[Change] = []
    for spec in CLAIMS:
        pattern = re.compile(spec.pattern)
        found = pattern.findall(updated)
        if not found:
            raise MeasurementError(
                f"el AGENTS.md ya no cifra {spec.label}: arregla el patron de la puerta "
                "y el helper antes de reescribir"
            )
        if len(found) > 1:
            raise MeasurementError(
                f"el AGENTS.md cifra {spec.label} {len(found)} veces: el patron dejo de "
                "ser unico y reescribirlo tocaria lineas ajenas"
            )
        before = int(found[0])
        measured = measurements[spec.key]
        if before != measured:
            updated = pattern.sub(lambda match: _with_group(match, measured), updated, count=1)
            changes.append(Change(spec.key, spec.label, before, measured))
    return updated, changes


def main(argv: Sequence[str] | None = None) -> int:
    parser = argparse.ArgumentParser(
        description="Deja las cifras del AGENTS.md en lo que mide el arbol."
    )
    parser.add_argument(
        "--repo-root",
        type=Path,
        default=DEFAULT_REPO_ROOT,
        help="checkout a medir (por defecto, el de este fichero)",
    )
    parser.add_argument(
        "--check",
        action="store_true",
        help="no escribe: sale 1 si alguna cifra esta obsoleta",
    )
    args = parser.parse_args(argv)

    repo_root = args.repo_root.resolve()
    agents_path = repo_root / AGENTS_MD
    if not agents_path.is_file():
        print(f"update-agents-claims: no existe {agents_path}", file=sys.stderr)
        return 2

    text = agents_path.read_text(encoding="utf-8")
    try:
        measurements = measure(repo_root)
        updated, changes = rewrite(text, measurements)
    except MeasurementError as error:
        print(f"update-agents-claims: {error}", file=sys.stderr)
        return 2

    for spec in CLAIMS:
        print(f"{spec.key}: {measurements[spec.key]}")
    if not changes:
        print("AGENTS.md ya estaba al dia; no se escribio nada.")
        return 0
    for change in changes:
        print(f"{change.label}: {change.before} -> {change.after}")
    if args.check:
        print(
            "AGENTS.md tiene cifras obsoletas: corre tools/update-agents-claims.sh",
            file=sys.stderr,
        )
        return 1

    agents_path.write_text(updated, encoding="utf-8", newline="\n")
    return 0


if __name__ == "__main__":
    sys.exit(main())

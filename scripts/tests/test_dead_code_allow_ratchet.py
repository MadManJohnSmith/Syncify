#!/usr/bin/env python3
"""Ratchet de `#![allow(dead_code)]` en modulos de produccion.

Un `#![allow(dead_code)]` a nivel de modulo apaga el lint para todo el
fichero: es la forma de fusionar un modulo grande sin llamador y cerrar el
issue como si el comportamiento ya existiera. La retro del 2026-11-19 (R6)
midio exactamente un caso de produccion: `commands/account_refresh.rs`, cuya
adopcion quedo pendiente en el issue #342. La adopcion aterrizo (A03-ter,
issue #342): los llamadores consumen la puerta y el allow se retiro, asi que
el suelo es 0. Los `#![allow(dead_code)]` de helpers de tests
(`crates/*/tests`, `src-tauri/tests`) no cuentan aqui.

El suelo es 0 y solo puede bajar... y ya no puede bajar mas: uno nuevo obliga
a justificarlo en revision, abrir un issue de adopcion (el precedente es el
propio #342) y citarlo en el doc del modulo. El test congela el recuento para
que la deuda no vuelva por la puerta de atras: si reaparece el allow del caso
conocido o aparece otro, la primera prueba falla con el detalle.
"""

import re
import unittest
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parents[2]
PRODUCTION_ROOTS = (REPO_ROOT / "src-tauri" / "src",)
CRATE_SOURCE_GLOB = "crates/*/src"
BASELINE = 0
ADOPTION_ISSUE = "#342"
KNOWN_SITE = "src-tauri/src/commands/account_refresh.rs"
PATTERN = re.compile(r"#!\s*\[\s*allow\s*\(\s*dead_code\s*\)\s*\]")


def _production_files():
    files = []
    for root in PRODUCTION_ROOTS:
        files.extend(root.rglob("*.rs"))
    for crate_src in sorted(REPO_ROOT.glob(CRATE_SOURCE_GLOB)):
        files.extend(crate_src.rglob("*.rs"))
    return files


def _allow_sites():
    """(ruta relativa, numero de allows) por fichero de produccion."""
    sites = []
    for path in _production_files():
        count = len(PATTERN.findall(path.read_text(encoding="utf-8", errors="ignore")))
        if count:
            sites.append((path.relative_to(REPO_ROOT).as_posix(), count))
    return sites


class DeadCodeAllowRatchetTests(unittest.TestCase):
    def test_module_level_dead_code_allows_do_not_grow(self) -> None:
        sites = _allow_sites()
        total = sum(count for _, count in sites)
        detail = ", ".join(f"{path} ({count})" for path, count in sites) or "ninguno"
        self.assertLessEqual(
            total,
            BASELINE,
            f"hay {total} `#![allow(dead_code)]` de modulo en produccion ({detail}) "
            f"y el suelo del ratchet es {BASELINE}. Un allow nuevo exige un issue "
            f"de adopcion abierto citado en el doc del modulo (precedente: "
            f"account_refresh, follow-up {ADOPTION_ISSUE}). Si la adopcion ya "
            "aterrizo, quita el allow y baja BASELINE en "
            "scripts/tests/test_dead_code_allow_ratchet.py; el suelo solo puede bajar.",
        )

    def test_the_baseline_site_is_the_known_adoption_pending_module(self) -> None:
        sites = _allow_sites()
        if not sites:
            return  # adopcion aterrizada: nada que atar
        self.assertEqual(
            sites,
            [(KNOWN_SITE, 1)],
            "el unico allow de produccion ya no es el caso conocido "
            f"{KNOWN_SITE}: si es un modulo nuevo, abre su issue de adopcion "
            f"(como {ADOPTION_ISSUE}) y actualiza esta prueba; no amplies el baseline.",
        )


if __name__ == "__main__":
    unittest.main()

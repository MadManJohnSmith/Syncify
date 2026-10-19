"""Ratchet de escrituras SQLx silenciosas.

`let _ = sqlx::query(...)` descarta el error de una escritura: si falla, ni se
registra ni se propaga. La auditoria de pilares (2026-10-19) midio 243 sitios en
`src-tauri/src`. Este test congela ese numero para que la deuda no crezca
mientras se migran a la frontera de escritura (D-06 en
`docs/Deuda_Tecnica_y_UX.md`).

Bajar el numero es siempre bienvenido; subirlo obliga a justificarlo en la
revision. El conteo usa la misma expresion que la medicion original, sobre todos
los `.rs` de `src-tauri/src`.
"""

import re
import unittest
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parents[2]
SOURCE_ROOT = REPO_ROOT / "src-tauri" / "src"
BASELINE = 180
PATTERN = re.compile(r"let _ = sqlx::query")


class SilentWriteRatchetTests(unittest.TestCase):
    def test_silent_write_sites_do_not_grow(self) -> None:
        hits = sum(
            len(PATTERN.findall(path.read_text(encoding="utf-8")))
            for path in SOURCE_ROOT.rglob("*.rs")
        )
        self.assertLessEqual(
            hits,
            BASELINE,
            f"las escrituras silenciosas crecieron a {hits} (baseline {BASELINE}): "
            "usa la frontera de escritura en vez de descartar el error con `let _ =`",
        )

    def test_the_pattern_is_still_present_somewhere(self) -> None:
        """Si el patron desaparece, el ratchet dejo de vigilar nada."""
        hits = sum(
            len(PATTERN.findall(path.read_text(encoding="utf-8")))
            for path in SOURCE_ROOT.rglob("*.rs")
        )
        self.assertGreater(hits, 0, "el patron ya no aparece: revisa si el ratchet sigue siendo valido")


if __name__ == "__main__":
    unittest.main()

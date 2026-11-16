"""Ratchet de escrituras SQLx silenciosas.

`let _ = sqlx::query(...)` descarta el error de una escritura: si falla, ni se
registra ni se propaga. La auditoria de pilares (2026-10-19) midio 243 sitios en
`src-tauri/src`; al migrarlos se comprobo que 18 de ellos terminan en
`.await.map_err(..)?`, es decir propagan y no eran silenciosos. Los lotes
D-06a (#37) y D-06b (#38) migraron el resto a la frontera `write_or_log` y el
suelo quedo en 19 (medido 2026-11-14 sobre el arbol con ambos lotes fusionados:
18 que ya propagan y 1 en un bloque comentado de `main.rs`). Este test congela
ese suelo para que la deuda no vuelva por la puerta de atras; el registro
canonico del recuento y del suelo esta en D-06 de `docs/Deuda_Tecnica_y_UX.md`.

El suelo solo puede bajar: migrar un sitio y bajar BASELINE (con su fecha de
medicion nueva en D-06) es siempre bienvenido; subirlo obliga a justificarlo en
la revision. Desde #39 el patron tolera la query en la linea siguiente
(`let _ =` con salto de linea antes de `sqlx::query`), el agujero que D-06a
destapo con el caso multilinea de Spotify; con este arbol el recuento no cambia.
"""

import re
import unittest
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parents[2]
SOURCE_ROOT = REPO_ROOT / "src-tauri" / "src"
# Suelo del ratchet, registrado en docs/Deuda_Tecnica_y_UX.md (D-06,
# "Cierra") con fecha de medicion 2026-11-14. Solo puede bajar; si bajas este
# numero, actualiza tambien la cifra y la fecha en D-06.
BASELINE = 19
# \s tambien casa el salto de linea: cubre `let _ =` con la query en la
# linea siguiente, que la metrica de una sola linea dejaba fuera (D-06a).
PATTERN = re.compile(r"let _ =\s*sqlx::query")

D06_DOC = REPO_ROOT / "docs" / "Deuda_Tecnica_y_UX.md"
D06_HEADING = "### D-06 —"
MEASUREMENT_DATE = re.compile(r"\d{4}-\d{2}-\d{2}")


def _count_silent_writes() -> int:
    return sum(
        len(PATTERN.findall(path.read_text(encoding="utf-8")))
        for path in SOURCE_ROOT.rglob("*.rs")
    )


def _d06_section() -> str:
    text = D06_DOC.read_text(encoding="utf-8")
    start = text.index(D06_HEADING)
    next_heading = text.find("\n### ", start + len(D06_HEADING))
    return text[start:] if next_heading < 0 else text[start:next_heading]


class SilentWriteRatchetTests(unittest.TestCase):
    def test_silent_write_sites_do_not_grow(self) -> None:
        hits = _count_silent_writes()
        self.assertLessEqual(
            hits,
            BASELINE,
            f"hay {hits} escrituras silenciosas (`let _ = sqlx::query`) y el suelo "
            f"del ratchet es {BASELINE}. Para bajar el recuento: migra cada sitio a "
            "`crate::db::write_or_log(contexto, futuro)` (o propaga el error con "
            "`map_err(..)?`) y despues baja BASELINE en "
            "scripts/tests/test_silent_write_ratchet.py, actualizando en D-06 de "
            "docs/Deuda_Tecnica_y_UX.md la cifra y su fecha de medicion. El suelo "
            "solo puede bajar; para subirlo primero justifica el caso en la "
            "revision y registra el nuevo suelo en D-06.",
        )

    def test_the_floor_is_recorded_in_d06_with_its_measurement_date(self) -> None:
        """El suelo del test y la cifra de D-06 no pueden divergir."""
        section = _d06_section()
        # El parrafo puede partir la frase en varias lineas: se normaliza el
        # espacio antes de buscar la cifra junto a cada mencion del suelo.
        flat = " ".join(section.split())
        floor_windows = [
            flat[match.start() : match.start() + 80]
            for match in re.finditer(r"[Ss]uelo", flat)
        ]
        self.assertTrue(
            any(str(BASELINE) in window for window in floor_windows),
            f"D-06 de docs/Deuda_Tecnica_y_UX.md ya no cita el suelo {BASELINE} del "
            "ratchet: actualiza la cifra y la fecha de medicion en D-06 para que "
            "coincida con BASELINE en scripts/tests/test_silent_write_ratchet.py",
        )
        self.assertTrue(
            MEASUREMENT_DATE.search(section),
            "D-06 no tiene fecha de medicion para el suelo del ratchet: "
            "anota cuando se midio junto a la cifra del suelo",
        )

    def test_the_pattern_is_still_present_somewhere(self) -> None:
        """Si el patron desaparece, el ratchet dejo de vigilar nada."""
        hits = _count_silent_writes()
        self.assertGreater(hits, 0, "el patron ya no aparece: revisa si el ratchet sigue siendo valido")


if __name__ == "__main__":
    unittest.main()

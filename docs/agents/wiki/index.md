# Wiki del agente — índice

Memoria operativa entre sesiones del jefe de gabinete. Convención de esta
wiki (desde su creación el 2026-11-13):

- **Historia, no presente**: cada claim lleva fecha (y commit cuando toca
  código). Nada de «actualmente»: lo vigente se re-deriva del árbol y del
  tracker; el estado de sesiones vive en `docs/historico/HANDOFF-*.md`.
- **Cita, no copia**: las fuentes (issues, PRs, handoffs, retros) se citan,
  nunca se copian dentro de una página.
- La puerta `scripts/tests/test_docs_consistency.py` cubre estas páginas
  desde el 2026-11-13: toda ruta citada entre tiques debe resolver.

## Cómo se trabaja

- [`modo-ejecucion.md`](modo-ejecucion.md) — reglas vigentes de ejecución
  (bloque fechado 2026-11-19: modo por issue con subagentes, cupo real,
  target por checkout, contrato de entrega, revisión con artefacto y CI de
  docs) y puntero al histórico. Las decisiones y el progreso fechado del
  2026-11-13 al 2026-11-19 están completos en
  [`docs/historico/wiki-modo-ejecucion.md`](../../historico/wiki-modo-ejecucion.md).
- [`protocolo-pr.md`](protocolo-pr.md) — protocolo de integración: PR con
  base `dev`, squash-merge, cierre manual de issues y qué viaja directo.

## Entorno y herramientas

- [`entorno.md`](entorno.md) — gotchas medidos en esta máquina:
  `PKG_CONFIG_PATH`, `CARGO_TARGET_DIR` compartido, symlinks de entorno y
  procesos huérfanos sobre el lock del target.

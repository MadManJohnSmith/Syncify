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

- [`modo-ejecucion.md`](modo-ejecucion.md) — decisiones del propietario sobre
  ejecución: por issue con un subagente por issue (2026-11-14, vigente),
  antes inline preferente (2026-11-13, supersado); contrato de entrega
  endurecido, riesgos del modo delegado y progreso fechado de carriles.
- [`protocolo-pr.md`](protocolo-pr.md) — protocolo de integración: PR con
  base `dev`, squash-merge, cierre manual de issues y qué viaja directo.

## Entorno y herramientas

- [`entorno.md`](entorno.md) — gotchas medidos en esta máquina:
  `PKG_CONFIG_PATH`, `CARGO_TARGET_DIR` compartido, symlinks de entorno y
  procesos huérfanos sobre el lock del target.

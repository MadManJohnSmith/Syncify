# Wiki del agente — índice

Memoria operativa entre sesiones del jefe de gabinete. Convención de esta
wiki (desde su creación el 2026-11-13):

- **Historia, no presente**: cada claim lleva fecha (y commit cuando toca
  código). Nada de «actualmente»: lo vigente se re-deriva del árbol y del
  tracker; el estado de sesiones vive en `docs/historico/HANDOFF-*.md`.
  Desde el 2026-11-22, `docs/HANDOFF.md` es solo un puntero sin estado
  (cómo re-derivarlo) y ninguna página cita `~/.local/state` como fuente.
- **Superado, no reescrito**: cuando una decisión posterior contradice una
  anterior, la vieja se marca «superado el <fecha>» en su sitio.
- **Cita, no copia**: las fuentes (issues, PRs, handoffs, retros) se citan,
  nunca se copian dentro de una página.
- La puerta `scripts/tests/test_docs_consistency.py` cubre estas páginas
  desde el 2026-11-13: toda ruta citada entre tiques debe resolver.

## Cómo se trabaja

- [`modo-ejecucion.md`](modo-ejecucion.md) — reglas de ejecución en
  bloques fechados; prevalece el más reciente. 2026-11-19: modo por issue
  con subagentes, revisión con artefacto y CI de docs. 2026-11-21: Ola 0 por
  oleadas. 2026-11-22: ≤3 carriles por PC (PC-A/PC-B, 6 carriles `lane:*`
  con afinidad de máquina), locks `lock:*` exclusivos por ola, prioridad
  `prio:P0…P3` y protocolo de despacho (reclamo `in-progress`, target por
  carril, consolidado a `main` con 4–6 PRs). Lo superado queda marcado. Las decisiones y el progreso fechado del
  2026-11-13 al 2026-11-19 están completos en
  [`docs/historico/wiki-modo-ejecucion.md`](../../historico/wiki-modo-ejecucion.md).
- [`protocolo-pr.md`](protocolo-pr.md) — protocolo de integración. Desde
  la Ola 0 (medido 2026-11-21, decidido 2026-11-22): PR de carril →
  `wave/ola-N-batch-M` → consolidado a `main` con merge commit →
  fast-forward de `dev`; docs directos a `dev`; protección de `main` (6
  contextos con matriz, strict). Abiertas para el dueño: regla de cierre de
  issues y retorno a `main` de los docs directos a `dev`. El flujo
  2026-11-13 (PR con base `dev`, squash) queda como historia.

## Entorno y herramientas

- [`entorno.md`](entorno.md) — gotchas medidos en esta máquina:
  `PKG_CONFIG_PATH`, `CARGO_TARGET_DIR` compartido, symlinks de entorno,
  procesos huérfanos sobre el lock del target, máquinas enlazadas y dónde
  van los handoffs (borrador en la carpeta de estado, registro versionado
  en `docs/historico/`, 2026-11-22).

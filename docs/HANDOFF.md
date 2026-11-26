# Handoff — puntero

Este fichero no guarda estado: solo dice dónde re-derivarlo. Desde el
2026-11-22 los relevos de sesión son registros fechados en `docs/historico/`
y nunca se editan para ponerlos al día.

No conviertas este fichero en un relevo: sin listas de issues ni de próximos
pasos. Un puntero solo envejece si deja de apuntar a lo que existe; la puerta
`scripts/tests/test_docs_consistency.py` comprueba que siga así.

## Cómo re-derivar el estado vigente

1. **Tracker** (GitHub Issues vía `gh`, ver `docs/agents/issue-tracker.md`):
   filtra por las etiquetas de carril `lane:*`, de lock exclusivo `lock:*`,
   de prioridad `prio:*` y por `in-progress` (trabajo ya reclamado). Las
   etiquetas de triage están en `docs/agents/triage-labels.md`.
2. **Qué hay en `dev` y aún no en `main`**:
   `git fetch && git log --oneline origin/main..origin/dev`.
3. **Lotes abiertos**: las ramas `origin/wave/*` y los PRs abiertos contra
   ellas (`git branch -r --list 'origin/wave/*'`); un lote termina con su PR
   consolidado a `main`.
4. **Estado por ítem de ajuste**: `docs/auditoria/matriz-ajustes.md`.

## Dónde está lo demás

- **Último relevo**: el `docs/historico/HANDOFF-*.md` de fecha más reciente
  (`ls docs/historico/HANDOFF-*.md | tail -1`). Es evidencia de cómo quedó
  una sesión, no estado; revalida sus punteros antes de actuar.
- **Reglas de ejecución** (carriles, locks, prioridad, despacho):
  `docs/agents/wiki/modo-ejecucion.md`.
- **Integración** (PR de carril, lote, consolidado a `main`):
  `docs/agents/wiki/protocolo-pr.md`.

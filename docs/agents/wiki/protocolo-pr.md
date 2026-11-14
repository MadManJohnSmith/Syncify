# Protocolo de PRs e integración

Cómo viaja el trabajo a `dev`. El mapa canónico de puertas y comandos es
`AGENTS.md` §«Comandos deterministas» y §«Puertas que no se relajan»; aquí
quedan los hechos fechados que lo complementan. Relacionada:
[`modo-ejecucion.md`](modo-ejecucion.md).

## Flujo verificado el 2026-11-13

- PR con base `dev`; squash-merge; el merge **no** autocierra issues: el
  cierre es manual citando el SHA del squash (así se cerró el trabajo de
  PRs #283/#284). Unwatch de PRs ya fusionados. Fuente:
  `docs/historico/HANDOFF-2026-11-13.md` §Restricciones operativas
  §Protocolo.
- `batch_50_audit_test` siempre con `--test-threads=1`; suite Python con
  `.venv/bin/python`. Misma fuente.

## Qué viaja directo a `dev` y qué vía PR

- **Directo** (precedente del 2026-11-13: `ca6bf67`, `db8d0be`, `dff789a`,
  `883e9f5`): registros fechados internos — handoffs y retros en
  `docs/historico/`, y desde el 2026-11-13 esta wiki.
- **Vía PR**: portada (`README.md` / `README.es.md`), código, cambios de
  puertas (precedente: PRs #26/#27 para el README; #283/#284 para código).

## Docs y CI

- **2026-11-13, verificado en `.github/workflows/ci.yml`**: CI ignora
  `**.md` y `docs/**` (CONTRIBUTING.md está obligado a revelarlo por
  `scripts/tests/test_readme_claims.py`). Consecuencia: los cambios de docs
  no corren CI; la suite Python local (`.venv/bin/python -m unittest
  discover -s scripts/tests`) es la única red antes de empujar docs.
- La puerta `scripts/tests/test_docs_consistency.py` cubre `docs/agents/`
  recursivamente desde el 2026-11-13 (incluida esta wiki): toda ruta citada
  entre tiques debe resolver.

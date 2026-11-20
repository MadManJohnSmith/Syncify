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

## Revisión independiente con artefacto

- **2026-11-19, regla**: la revisión independiente de un PR publica su
  veredicto con `gh pr review --comment` (hallazgos, fecha y veredicto) antes
  del merge. El comentario queda junto al diff y es comprobable; sin ese
  artefacto, el claim «revisión independiente» no cuenta. El bot de
  CodeRabbit (`.coderabbit.yaml`, auto-review sobre `dev`) es una segunda
  lectura, no sustituye el artefacto. Fuente: retro 2026-11-19 §R2.

## Módulos sin llamador

- **2026-11-19, regla**: un issue que añade un módulo sin llamador no se
  cierra hasta que la adopción aterrice en producción o exista un follow-up
  enlazado. El ratchet
  `scripts/tests/test_dead_code_allow_ratchet.py` congela el único
  `#![allow(dead_code)]` de producción (`commands/account_refresh.rs`, cuya
  adopción sigue en #342). Fuente: retro 2026-11-19 §R6.

## Docs y CI

- **2026-11-13, verificado en `.github/workflows/ci.yml`**: CI ignora
  `**.md` y `docs/**` (CONTRIBUTING.md está obligado a revelarlo por
  `scripts/tests/test_readme_claims.py`). Consecuencia: los cambios de docs
  no corren CI; la suite Python local (`.venv/bin/python -m unittest
  discover -s scripts/tests`) es la única red antes de empujar docs.
- **2026-11-19, corregido para `dev`**: un push a `dev` que solo toca
  `**.md`, `docs/**`, `AGENTS.md` o `GLOSSARY.md` dispara
  `.github/workflows/docs-check.yml` con las tres puertas de
  `scripts/tests/`; un PR de solo docs ejecuta el job `docs` de `ci.yml` sin
  la matriz Rust/frontend. Los pushes a `main` siguen ignorando esos paths.
  Fuente: retro 2026-11-19 §R1.
- La puerta `scripts/tests/test_docs_consistency.py` cubre `docs/agents/`
  recursivamente desde el 2026-11-13 (incluida esta wiki): toda ruta citada
  entre tiques debe resolver.

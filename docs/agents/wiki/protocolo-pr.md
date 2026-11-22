# Protocolo de PRs e integración

Cómo viaja el trabajo a `dev` y, desde la Ola 0 (2026-11-21), por lotes a
`main` (ver §«Flujo por lotes»). El mapa canónico de puertas y comandos es
`AGENTS.md` §«Comandos deterministas» y §«Puertas que no se relajan»; aquí
quedan los hechos fechados que lo complementan. Relacionada:
[`modo-ejecucion.md`](modo-ejecucion.md).

## Flujo verificado el 2026-11-13

- PR con base `dev`; squash-merge; el merge **no** autocierra issues: el
  cierre es manual citando el SHA del squash (así se cerró el trabajo de
  PRs #283/#284). Unwatch de PRs ya fusionados. Fuente:
  `docs/historico/HANDOFF-2026-11-13.md` §Restricciones operativas
  §Protocolo. *(Superado para el código el 2026-11-21/22: el PR de carril
  va contra la rama del lote, no contra `dev`; ver §«Flujo por lotes». La
  regla de cierre de issues está abierta: ver allí.)*
- `batch_50_audit_test` siempre con `--test-threads=1`; suite Python con
  `.venv/bin/python`. Misma fuente.

## Flujo por lotes (Ola 0)

Medido el 2026-11-21 sobre git y GitHub; decidido por el dueño el
2026-11-22. Reglas de despacho (carriles, locks, prioridad): bloque
2026-11-22 de [`modo-ejecucion.md`](modo-ejecucion.md).

- **Cadena.** PR de carril → rama del lote `wave/ola-N-batch-M` →
  PR consolidado del lote a `main` → fast-forward de `dev` a `main`.
  - Lote 1 (medido): los PRs de carril entraron en `wave/ola-0-batch-1`
    con merge commit (`2ced041`, `f1c43d8`, `8c20a86`, `f97ff19`) o squash
    (`623aec8`); el consolidado #354 entró en `main` con **merge commit**
    `d09f481`, y `dev` se avanzó después por fast-forward. Fuente:
    `git log --first-parent` y `docs/historico/HANDOFF-2026-11-21.md`.
  - Lote 2 (medido el 2026-11-22): los PRs de carril entran en
    `wave/ola-0-batch-2` por squash (`5e42af3`, `0ca49a9`, `74d6735`,
    `c742093`).
  - Decidido el 2026-11-22: con 4–6 PRs verdes en el lote (o un hito
    clave) se abre el consolidado a `main` y se arranca el lote siguiente
    sin esperar al CI de Windows del consolidado.
- **Docs durante una wave.** Wiki, handoffs, retros y registros fechados
  siguen viajando directos a `dev` (precedente: `1dd31cc`, `b773aba` el
  2026-11-21). Medido el 2026-11-22: eso deja `dev` por delante de `main`
  y fuera de la rama del lote (`git log origin/main..origin/dev`), así que
  el fast-forward de `dev` tras el siguiente consolidado ya no es posible
  sin reconciliar antes. **Pregunta abierta para el dueño**: cómo vuelven
  esos commits a `main` (¿se incluyen en el consolidado, se fusiona `main`
  en `dev`, o los docs van también por la wave?).
- **Protección de `main`** (cambiada el 2026-11-21, comprobada el
  2026-11-22 con
  `gh api repos/MadManJohnSmith/Syncify/branches/main/protection/required_status_checks`):
  exige los **6 contextos con sufijo de matriz** (Rust, Frontend y Python ×
  `ubuntu-latest`/`windows-latest`) con `strict=true` (rama al día con
  `main` antes de fusionar). Los 3 contextos anteriores no llevaban sufijo
  de matriz y ningún workflow los producía: ningún PR a `main` era
  fusionable. Fuente: `docs/historico/HANDOFF-2026-11-21.md`.
- **Pregunta abierta para el dueño: cuándo se cierra el issue.** Hoy es
  inconsistente (tracker, 2026-11-22): en el Lote 2, #69, #167, #181 y
  #338 se cerraron el 2026-11-21 al fusionar en `wave/ola-0-batch-2`, con
  su código aún fuera de `main` y de `dev`; en el Lote 1, #179 y #342
  seguían abiertos al auditar (2026-11-22) aunque su código (`f5bcd3d`, `623aec8`) ya es ancestro
  de `origin/main`. Opciones: cerrar al entrar en la wave, o al llegar a
  `main` citando el merge del consolidado.

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
- **Superado el 2026-11-21** (comprobado el 2026-11-22): la adopción de
  `account_refresh` aterrizó con #342 (squash `623aec8`, en el Lote 1, ya
  ancestro de `origin/main` vía `d09f481`); el ratchet bajó a
  `BASELINE = 0` en `scripts/tests/test_dead_code_allow_ratchet.py` y
  `AGENTS.md` lo recoge. El issue #342 seguía abierto en el tracker el
  2026-11-22 (ver la pregunta de cierre en §«Flujo por lotes»).

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

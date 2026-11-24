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
  - Lote 2 (completado el 2026-11-22): los 6 PRs de carril entraron en
    `wave/ola-0-batch-2` por squash (`5e42af3`, `0ca49a9`, `74d6735`,
    `c742093`, `e88c356`, `5e84538`); el consolidado #369 entró en `main`
    con merge commit `e43ea3a`, y `dev` avanzó por fast-forward después.
  - Lote 3 (completado el 2026-11-23 UTC): los 5 PRs de carril entraron en
    `wave/ola-0-batch-3` por squash (`66a0921`, `3a929c7`, `67fb67c`,
    `0820d3e`, `7180fcf`); el consolidado #379 entró en `main` con merge
    commit `98d83e1` y `dev` avanzó por fast-forward. La pata Windows de los
    carriles quedó en cola; la matriz completa se corrió en el consolidado
    (**6/6**).
  - Decidido el 2026-11-22: con 4–6 PRs verdes en el lote (o un hito
    clave) se abre el consolidado a `main` y se arranca el lote siguiente
    sin esperar al CI de Windows del consolidado.
- **Docs durante una wave.** Wiki, handoffs, retros y registros fechados
  siguen viajando directos a `dev` (precedente: `1dd31cc`, `b773aba` el
  2026-11-21). Medido el 2026-11-22: eso deja `dev` por delante de `main`
  y fuera de la rama del lote (`git log origin/main..origin/dev`).
  *(Resuelto el 2026-11-22 en el Lote 2: se mergeó `dev` en la wave antes
  del consolidado (`193d329`), el consolidado `e43ea3a` llevó los docs a
  `main` y `dev` avanzó por fast-forward. Se adopta ese patrón: merge de
  `dev` en la wave antes del consolidado.)*
- **Coste de CI en wave vs main** (2026-11-23): la pata `windows-latest` del
  job Rust solo corre en push a `main` y en PRs con base `main`; los PRs a
  `wave/*` pagan solo Ubuntu. Los jobs de Docs y Python miden el **head** del
  PR, no el merge, para que los recuentos de `AGENTS.md` no choquen entre
  carriles. `main` conserva los 6 contextos con `strict=true`. Fuente: #368,
  PR #380; helper de recuentos de #368.
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
  `main` citando el merge del consolidado. *(Aplicado el 2026-11-22 en el
  cierre del Lote 2: #155 y #184 se cerraron al llegar a `main` citando
  `e43ea3a`; #69, #338, #167 y #181 siguen cerradas desde la wave. En el
  Lote 3, #362/#364/#128/#186 se cerraron al llegar a `main` citando
  `98d83e1`, y #73 sigue abierto por #374. La política no se decide aquí:
  los dos lotes dejan evidencia de las dos prácticas.)*

## Cifras del AGENTS.md y alcance de CI (#368)

Medido el 2026-11-22 sobre el Lote 2: #361 y #360 añadieron un objetivo de
test cada uno; el job Docs de #360 sobre `41135bb` falló con `360 != 361`
(run [45068016557](https://github.com/MadManJohnSmith/Syncify/actions/runs/45068016557))
aunque su rama era correcta, porque la puerta medía el merge con la base. El
arreglo no es contar mejor: es medir el árbol del head y dejar la cifra en un
comando. Detalle y criterios en el issue #368.

- **Antes del push final de un carril**, y solo después de
  `git merge origin/wave/ola-0-batch-N`: corre
  `tools/update-agents-claims.sh`. Reescribe únicamente las líneas medidas
  (objetivos de cargo, tests Python, ficheros de test UI, `#[ignore]` y el
  suelo de escrituras silenciosas), mide las cinco antes de escribir y sale 2
  sin tocar el fichero si no puede medir. `--check` no escribe y sale 1 si
  alguna cifra está obsoleta. La lógica es `scripts/agents_claims.py`, la
  misma que importa `scripts/tests/test_agents_claims.py`; no se editan esas
  líneas a mano.
- **Qué corre en un PR de carril a `wave/*`.** Rust solo en Ubuntu;
  Frontend y Python mantienen sus dos patas; Docs corre sus tres puertas
  sobre el árbol del head del PR (`github.event.pull_request.head.sha`). En
  `push` se sigue midiendo el commit empujado.
- **Qué corre en `main`.** El PR consolidado tiene base `main`: produce la
  pata `windows-latest` de Rust además de la de Ubuntu, más Frontend y Python
  × dos patas. La protección de `main` no se tocó: sigue exigiendo los **6
  contextos con sufijo de matriz** y `strict=true` (comprobado el 2026-11-22
  con `gh api repos/MadManJohnSmith/Syncify/branches/main/protection/required_status_checks`).
- **Cómo se mide el cambio de matriz.** No se puede ejercitar el caso «PR a
  `main`» desde el PR del carril: GitHub usa el workflow del merge del propio
  PR, así que la pata Windows reaparece en el siguiente consolidado. Medición
  prevista: (1) el PR del carril no debe listar el contexto de Rust con
  sufijo `windows-latest`; (2) el consolidado a `main` sí debe producirlo, y
  los 6 contextos deben quedar verdes antes del merge; (3) la regla local se
  comprueba con `scripts/tests/test_rust_matrix.py`, que simula
  `GITHUB_EVENT_NAME` y `GITHUB_BASE_REF` sin red.

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

# Handoff de AUDITORÍA pre-reset — workspace Syncify — `/home/alan/Documents/Syncify-improvement`

## LÉEME PRIMERO — actualización 2026-11-08 (agente de desarrollo; el formateo AÚN no se ejecutó)

Este workspace ya está preparado para el formateo. De esta máquina solo hay que
salvar el repositorio (todo lo alcanzable está en GitHub) y el respaldo local
verificado de abajo. El formateo sigue pendiente: este respaldo ES el estado previo.

### Restaurar (máquina nueva)

1. `git clone https://github.com/MadManJohnSmith/Syncify` — `dev` = `74c5a03` y
   `p7/track-browser` = `7202103`, ambas 100% empujadas (verificado:
   `origin/dev..dev` y `origin/p7/track-browser..HEAD` vacíos).
2. Los 21 commits colgantes (D03 `2d6d4cb` incluido) están **DURADEROS en origin**:
   tras clonar, `git fetch origin 'refs/rescate/*:refs/rescate/*'` (verificado con
   `git ls-remote`: 21 refs, 08-nov). El bundle local `syncify-all.bundle` también
   los contiene y quedó verificado (`bundle-verify.txt`: `git bundle verify` OK con
   «records a complete history», 21/21 fetches de prueba, sha256 en
   `bundle-sha256.txt`), pero muere con la máquina: no se subió a Drive porque
   tras el push 100% de su contenido está en el repo remoto (decisión del dueño,
   08-nov).
3. Recrear el entorno: `npm ci` (raíz y `ui/`), `.venv`, target de cargo fuera del
   repo (`CARGO_TARGET_DIR=/tmp/syncify-target`, `CARGO_BUILD_JOBS=4` — el host va
   justo de memoria), `ui/dist/index.html` dummy para cargo. Puertas y
   convenciones: `AGENTS.md` del repo.

### Respaldo (reemplaza al del 07-nov, por decisión del dueño del 08-nov)

`handoffs/syncify-respaldo-2026-11-08/` (45 MiB):

- `syncify-all.bundle` — todas las refs (`--all`) + **21 colgantes** como
  `refs/rescate/<sha>`: entre ellos `2d6d4cb` (el D03 del otro agente rescatado el
  07-nov; sobrevivía solo vía reflog, sin ninguna rama) y dos WIP de stash de
  `p7/track-browser` (`3b47459`, `97ee856`). El resto son duplicados supersedidos
  (WIP de P4, dups de BridgeClient/dispatch, stash de rebases) conservados por
  costo cero. La carpeta `syncify-respaldo-2026-11-07` se ELIMINÓ tras verificar
  este: su contenido está aquí o en GitHub (`mb-analysis-docs/` y
  `perf-probe-scripts/` quedaron obsoletos porque ya viven en el repo:
  `74c5a03` y `4968171`).
- `uncommitted-docs-HANDOFF.patch` — el único cambio sin commit del repo
  (`docs/HANDOFF.md`, trabajo del otro agente; preservado como patch, sin
  commitear: no es nuestro).
- `zcode-config.json`, docs de proceso de P7 (inventario, briefs A–C, cuerpo del
  PR #224), handoffs históricos (`HANDOFF-2026-10-11.md`, `HANDOFF-2026-11-06-P5.md`),
  informe de arquitectura HTML y harness/tools de `syncify-repro`.
- **Subida a Drive (08-nov, decisión del dueño: solo lo no-versionado):**
  `syncify-respaldo-2026-11-08/` SIN el bundle, más los 3 handoffs actualizados,
  en `drive:respaldo-cachyos-20261107/`, verificado con `rclone check` (0
  diferencias). Sin BD (nunca estuvo en el respaldo). La copia anterior de Drive
  (`20-auditoria.tar.zst`, del 07-nov) queda obsoleta para este workspace.

### Estado del repo (verificado 2026-11-08)

- P1–P6 mergeadas (PRs #150/#205/#209/#210/#211/#221). **P7 · TrackBrowser
  IMPLEMENTADO** en `p7/track-browser` (7 commits, `caa3244..7202103`, ADR-0025):
  kernel `useVirtualWindow` con paridad estricta, composable `useTrackBrowser`
  (paginación + selección por ids), `quality_bucket` alineado en las tres fuentes
  de biblioteca, higiene IPC (11 invokes migrados), docs/cifras.
  Verificación local: suite 2987 passed / 0 failed / 13 ignored, batch_50 2/2,
  UI 129 ficheros / 1130 tests, Python 311 OK (2 skipped).
- **PR #224** (base `dev`) ABIERTO y MERGEABLE. Pendiente: pasada visual humana de
  las tres vistas + merge. CI del PR: Frontend y Python en verde; Rust FALLÓ por
  un test FLAKY ajeno al diff:
  `url_import_queue_test::test_concurrent_same_url_import_reuses_one_queue_row`
  (`url_import_queue_test.rs:668`, «(code: 5) database is locked», corrida
  42518449475). El diff del PR no toca esa área; localmente la suite entera pasó.
- Sin empujar: nada. Cambios sin commit: solo `docs/HANDOFF.md` (patch en el
  respaldo).

### Qué muere con el formateo (reglas del dueño, con fechas)

- Datos de la app `~/.local/share/com.syncify.app` (~5,0 GiB) y logs
  `~/.local/state/syncify-manual-test` (41 MiB): **MUEREN** (decisión del dueño,
  07-nov).
- `/tmp` entero: worktree `syncify-p7` (limpio y empujado — regenerable),
  target de cargo (9,1 GiB, regenerable), fixtures de test.
- 15 binarios ELF scratch sin trackear en la raíz del repo (nombres tipo tests de
  rustc: `box_assign`, `builtin_ctor_turbofish`, `hello`, `rpit_binding_same_type`,
  ...): regenerables, **MUEREN**. `.lust-daemon-67d01375.sock.lock`: muere.
  `.zcode/config.json`: copiado al respaldo.

### Siguiente paso concreto (máquina nueva)

1. Restaurar, levantar el entorno y correr las puertas de `AGENTS.md` para
   confirmar el estado.
2. Pasada visual de PR #224 con el dueño (biblioteca: filtros/orden/paginación/
   agrupación/selección masiva/deep-link; metadatos; cola de descargas) — criterio:
   que no haya cambiado nada visualmente. Re-lanzar el CI de Rust si hace falta
   (el fallo actual es flaky).
3. Merge de #224 → cierra P1–P7 (plan completo). Deudas vivas después: issue #222
   (el runtime se detiene ~83 s con la cola en tormenta), el test flaky de
   `url_import_queue`, caché/reintentos de apt en `ci.yml`, y las deudas del
   ADR-0025 (`metadata_score` de FTS divergente, homogenización de ventana
   virtual, toast local de MetadataView).

### Resolución de las tareas del registro de auditoría (2026-11-07 → 08-nov)

1. Cambios sin commit de `Syncify/`: resueltos en el 07-nov; hoy queda solo
   `docs/HANDOFF.md` (patch preservado en el respaldo).
2. Handoff de desarrollo preservado: `syncify.md` (actualizado hoy con el estado
   P7 y el respaldo nuevo).
3. `perf-probe`: los scripts `bench_*.py` YA están en el repo (`scripts/bench/`,
   `4968171`); las 7 bases .db mueren (regla regenerable).
4. `mb-analysis`: `ANALISIS_MUSICBRAINZ.md` + SHA256SUMS YA en el repo
   (`docs/mb-analysis/`, `74c5a03`); los tarballs (~7,6 GiB) mueren.
5. `syncify-repro` (~5,2 GiB): el peso MUERE (decisión del dueño, 07-nov);
   harness y tools quedan respaldados en `syncify-respaldo-2026-11-08/`.
6. Datos de la app: **MUEREN** (decisión del dueño, 07-nov).
7. Cluster RE (`/home/alan/tools`, `squashfs-root`, `.owprobe`): fuera del alcance
   de este handoff; ver la auditoría general y sus handoffs.
8. Regenerables confirmados: `.cache/syncify` y `/tmp/syncify-*` mueren.

---

## Registro de la auditoría (2026-11-07) — preservado tal cual

> NOTA: el archivo `syncify.md` de esta carpeta contiene el handoff de SESIÓN DE DESARROLLO (P5/P6/P7 de MadManJohnSmith/Syncify) y se preserva intacto. Este documento, regenerado el 2026-11-07 tras ese solapamiento, es el handoff de la auditoría de almacenamiento.

**Corrección del dueño (2026-11-07):** el workspace `Syncify-improvement` como tal NO es útil (regenerable, fue una prueba). Lo que SÍ se necesita es la SUBCARPETA `/home/alan/Documents/Syncify-improvement/Syncify/`.

**Dato clave verificado (2026-11-07):** esa subcarpeta ES el checkout de `https://github.com/MadManJohnSmith/Syncify.git`, rama `dev`, 0 commits sin empujar, pero **18 archivos con cambios sin commit** (incluye trabajo del otro agente: errors.rs, docs — ver riesgos en `syncify.md`). Por tanto el rescate NO es "crear repo": es commit+push de lo pendiente ANTES del reset. Referirse a `syncify.md` (handoff de desarrollo) para el estado P5/P6/P7, riesgos y worktrees; ese documento debe PRESERVARSE (copia al repo o Drive privado junto con el rescate).

Tareas del agente de auditoría:
1. Con el dueño: revisar los 18 cambios sin commit de `Syncify/` (rama dev) y decidir commit+push o descarte. Cuidado: hay trabajo ajeno sin commitear mencionado en `syncify.md`; no descartar a ciegas.
2. Preservar el handoff de desarrollo (`.../handoffs/syncify.md`): copiarlo a Drive privado junto al resto de la auditoría (ya está dentro de `20-auditoria.tar.zst` en `drive:respaldo-cachyos-20261107/`).
3. `/home/alan/perf-probe`: benchmark de Syncify verificado por esquema (mb_dump_*, albums, download_queue) y por README; resultados YA documentados en `Syncify/docs/historico/perf-probe-2026-10-12.md`. Regla del dueño (regenerable → solo fuentes): las 7 bases .db mueren; valorar empujar los scripts bench_*.py al repo si aportan.
4. `/home/alan/mb-analysis`: `ANALISIS_MUSICBRAINZ.md` documenta fuente oficial (ftp.musicbrainz.org fullexport 20261011) + SHA256SUMS. Los tarballs (~7,6 GiB) mueren; empujar el .md + SHA256SUMS al repo.
5. `/home/alan/syncify-repro` (~5,2 GiB: AppImages dl/fixed, contract-check): preguntar al dueño si la evidencia del parche "fixed" ya quedó documentada en el repo; si sí, muere entero.
6. Datos de la app `/home/alan/.local/share/com.syncify.app` (db+WAL ~5 GiB): pregunta abierta para el dueño — sin decisión, mueren con el reset.
7. Cluster RE candidato a este workspace (confirmar): `/home/alan/tools` (ghidra/il2cpp/unity/paf scripts), `/home/alan/squashfs-root` (open-whispr), `/home/alan/.owprobe`.
8. Regenerables confirmados del dominio (no rescatar): `/home/alan/.cache/syncify` (targets cargo ~16 GiB totales, regenerables; solo borrar tras confirmar con el dueño que no se reutilizan, ver `syncify.md` §Worktrees), `/tmp/syncify-*` (mueren solos).
Skills: `research`, `grill-with-docs`.

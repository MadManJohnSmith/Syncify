# Brief — P7 Fase A: alineación de backend + higiene IPC

Contrato: **ADR-0025** en `docs/adr/0025-navegador-de-pistas.md` del worktree
(decisiones 5 y 6). Hechos verificados: **/tmp/p7-inventory.md** (§9, §11, §12).
Fase mecánica y de riesgo bajo, ANTES de tocar composables.

## Dónde

- Worktree: `/tmp/syncify-p7`, rama `p7/track-browser`, base `caa3244` (ADR+glosario
  rebasado sobre el dev actual).
- Target Rust: `CARGO_TARGET_DIR=/tmp/syncify-target` (compartido; usa
  `CARGO_BUILD_JOBS=4` y nunca dos comandos pesados a la vez — el host va justo de
  memoria y ya hubo un OOM).
- Si faltan `node_modules` (raíz y `ui/`): `npm ci` y `npm --prefix ui ci` contra
  los lockfiles. `ui/dist` ya tiene el index.html dummy para cargo.
- El worktree se recreó tras un reinicio: NO asumas node_modules ni cachés previos.
  La compilación de Rust será fría (el target /tmp/syncify-target está vacío).

## Parte 1 — Backend: quality_bucket en las tres fuentes (decisión 5)

- Añadir `quality_bucket` al SELECT de `search_tracks` y de
  `get_favorite_tracks`/`query_favorite_tracks` con la expresión CASE ya existente
  en `get_library` (`library.rs:440-445` según el inventario §11; re-verifica la
  línea). Sin cambiar el contrato de `SearchResult`/`LibraryPage` más allá del
  campo nuevo en las filas.
- Tests de paridad al estilo de `test_get_library_quality_bucket_matches_dashboard`
  (inventario §12; re-verifica la línea): uno para `search_tracks` y otro para
  favoritos, verificando que el bucket coincide con la clasificación del dashboard.
- NO tocar el `metadata_score` de FTS (deuda declarada en el ADR).

## Parte 2 — UI: higiene IPC (decisión 6)

- Wrappers nuevos en `ui/src/api/` siguiendo las convenciones del módulo
  (mira `metadata.ts` getMetadataStats para camelCase/snake_case y el estilo de
  wrapper fino de `library.ts`): `readTrackTags`, `writeTrackTags`,
  `enrichGenreLastfm`, `enrichMetadata`, `enrichMetadataMusicbrainz` (colócalos en
  el módulo que corresponda por dominio: tags/enrichment van en `metadata.ts`).
- Migrar los 6 invokes crudos de MetadataView (read_track_tags ×2, write_track_tags,
  enrich_genre_lastfm, enrich_metadata, enrich_metadata_musicbrainz; posiciones en
  el inventario §9) y el de LibraryView (enrich_metadata_musicbrainz) a los
  wrappers. Actualizar los mocks de los specs afectados.
- Migrar los 2 bypasses de DownloadsView: `invokeCommand('reorder_queue')` →
  wrapper `reorderQueue` existente en `library.ts`;
  `invokeCommand('show_in_folder')` → `libraryApi.showInFolder` (existente).
- Extender/actualizar los specs de API (`metadata.spec.ts`, `library.spec.ts`,
  `queue.spec.ts` según toque) al estilo existente.

## Puertas

Backend (tras parte 1):
- `CARGO_TARGET_DIR=/tmp/syncify-target CARGO_BUILD_JOBS=4 cargo test --bins -- library` (targeted)
- `CARGO_TARGET_DIR=/tmp/syncify-target CARGO_BUILD_JOBS=4 cargo test --locked --no-fail-fast`
- `cargo fmt --all -- --check`
- `CARGO_TARGET_DIR=/tmp/syncify-target CARGO_BUILD_JOBS=4 cargo clippy --workspace --all-targets --locked -- -D warnings`

UI (tras parte 2):
- `npm --prefix ui run build && npm --prefix ui run test:run`

## Commits

- Uno para la parte 1 (`feat(library): quality_bucket en FTS y favoritos (ADR-0025, P7)`)
  y otro para la parte 2 (`refactor(ui): wrappers de tags/enrichment y fin de los
  invokes crudos (ADR-0025, P7)`). Mensajes en español, estilo del repo. Cada commit
  con sus puertas en verde.

## Restricciones

- Solo este worktree y esta rama. No pushees. No toques el worktree principal ni
  otros worktrees ni `/tmp/syncify-target` salvo como target de compilación.
- No toques composables ni ventanas virtuales (eso es Fase B/C).
- No des-ignores tests `#[ignore]`; no relajes aserciones; sin `#[allow(dead_code)]`
  en comandos.

## Informe

1. Qué cambió por parte (ficheros y puntos), hashes de commits.
2. Resultados de cada puerta (cifras).
3. Rarezas encontradas al migrar (p. ej. qué spec necesitó cambios de mock).

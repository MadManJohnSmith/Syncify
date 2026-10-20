# Deuda técnica y de UX

Registro vivo de la deuda conocida del proyecto. Es el documento al que apuntan
`docs/PLAN_UNIFICACION_IMPORTACION.md` (Fase 4, entregable 3) y el núcleo
`sync_soundcloud_likes_with_engine` de `src-tauri/src/commands/service.rs`.

> Hasta `b9559da` este archivo se citaba desde el código y desde la documentación
> pero **no existía en el árbol**: `workspace/` está en `.gitignore` y ningún
> archivo bajo esa ruta llega a un checkout ni a CI (misma causa raíz que
> SYNC-AUD-058 y SYNC-AUD-065). Esta es la versión versionada.

## Cómo se registra una entrada

Cada entrada dice **qué se observó**, **dónde** y **qué la cerraría**. Una
entrada sin observación reproducible no entra aquí. Los riesgos aceptados se
declaran como tales y no se reopened por reflejo.

Última revisión: auditoría de pilares y reparación posterior (2026-10-19) —
se abren D-05 a D-08 (subsistema de reproducción retirado, escrituras silenciosas,
contrato de identidad por edición y funciones monstruo) y se cierran C-12 a C-18.
Antes: auditoría 73 ítems, entrada A-01 reescrita contra el árbol (2026-10-05) — el par `app_id`/`app_secret` está versionado en Python y en
Rust, no solo el `app_id`. Antes de eso: ítem 9.1 del plan post-auditoría
(2026-10-02), que cierra D-02 y redefine D-01 y D-04 a la luz de las fases 4 y
8. Y antes: `b9559da` + reparaciones de SYNC-AUD-062 a SYNC-AUD-071.

## Riesgos aceptados

### A-01 — El par `app_id`/`app_secret` público de Qobuz está en el árbol

- **Observado**: el bundle de identificadores de aplicación que usa el cliente
  open-source de Qobuz está versionado **en los dos lados**: en Python,
  `scripts/services/qobuz_service.py` (líneas 56 y 63) y en el núcleo Rust,
  `src-tauri/src/services/qobuz.rs` (líneas 19-20,
  `QOBUZ_APP_ID_FALLBACK` y `QOBUZ_APP_SECRET_FALLBACK`). El `app_id` público
  es `798273057`; el `app_secret` emparejado no se reproduce aquí para no
  darle una segunda copia.
- **Decisión**: exposición **aceptada y documentada** (SYNC-AUD-071). Ambos
  valores son identificadores de la aplicación pública de Qobuz, no
  credenciales personales de ningún usuario: sin ellos la API responde 400
  «Invalid or missing app_id». Lo que este riesgo **no** cubre son las
  credenciales personales del usuario (token/login), que viajan por el
  cifrado de la app.
- **Prioridad de resolución**: las variables de entorno `QOBUZ_APP_ID` y
  `QOBUZ_APP_SECRET` ganan sobre los literales de respaldo
  (`src-tauri/src/services/qobuz.rs`, líneas 30-38;
  `scripts/services/qobuz_service.py`, líneas 95-96), de modo que un operador
  puede rotar el par sin tocar el árbol.
- **Lo que no se acepta**: credenciales personales de usuario versionadas, o
  una petición firmada con el secreto vacío cuando no hay ninguno
  (SYNC-AUD-062).
- **Vigilancia**: `scripts/tests/test_docs_consistency.py`
  (`AcceptedQobuzAppIdRiskTests`) exige que esta entrada siga nombrando el
  `app_id` que el árbol lleva y que los dos lados compartan el mismo valor
  público; `src-tauri/tests/secrets_isolation_security_test.rs` y
  `src-tauri/tests/qobuz_credentials_leak_test.rs` tratan la exposición
  como material vigilado.
- **Cierra**: decisión del propietario, no trabajo pendiente. Se revisita solo si
  Qobuz rota el identificador.

## Deuda abierta

### D-01 — Fase 3 implementada; falta verificarla contra las APIs reales

- **Observado**: la Fase 3 del plan de unificación ya no está bloqueada en código.
  El brazo `"soundcloud"` de `perform_sync_service_with_emitter` pasa cada like por
  `sync_soundcloud_likes_with_engine` → `EnrichmentEngine`, con identidad canónica,
  artista de `publisher_metadata`, álbum, portada y `added_at` del like
  (`tests/soundcloud_unified_engine_test.rs`); el brazo `"apple_music"` importa
  canciones, álbumes y playlists con storefront desde credenciales y cursor `next`
  (`tests/apple_music_storefront_pagination_and_migration_test.rs`). Ambos están
  cubiertos contra un servidor mock, no contra la API real.
- **Lo que sigue sin poder verificarse** es el comportamiento de esos dos brazos
  contra los servicios reales, porque no hay con qué llamar: ni la OAuth app de
  SoundCloud ni el developer token JWT de Apple Music están en el árbol, y no pueden
  estarlo.
- **Cierra**: con las credenciales del propietario — la OAuth app de SoundCloud y el
  developer token de Apple Music—. Es una comprobación, no una implementación.

### D-04 — 4 de los 6 documentos de `docs/features/` siguen re-derivados solo por rutas

- **Observado**: esta revisión re-derivó del código la narrativa de
  `docs/features/metadata-enrichment.md` y `docs/features/playlist-system.md`, que
  describían como vigente un flujo de Spotify Audio Features retirado con S68 y
  afirmaban que solo Spotify importaba playlists en Rust cuando el motor unificado
  las importa para cinco servicios. Los otros cuatro —`auth-flow.md`,
  `download-pipeline.md`, `library-import.md` y `settings-system.md`— conservan el
  encabezado que declara su narrativa como referencia de diseño sin verificar.
- **Cierra**: re-derivar los cuatro restantes cuando toque modificar ese subsistema.
  No depende de una entrada externa, pero tampoco de una entrega con fecha: es tiempo
  de lectura que se ejecuta junto al próximo cambio de cada subsistema, y hasta
  entonces se declara con su condición en vez de quedar como pendiente suelto.

## Alcance condicionado: lo que este registro NO declara como pendiente

Las dos entradas anteriores (D-01 y D-04) están **registradas, con su condición de
cierre, y ninguna de las dos bloquea una entrega**. No son defectos: son alcance
condicionado. Un cierre correcto las declara como tales y no como trabajo pendiente,
porque confundirlas convierte un bloqueo conocido en una promesa que el proyecto no
puede cumplir.

| ID | Condiciona | Entrada externa que la resuelve |
|---|---|---|
| D-01 | Verificación end-to-end de los brazos SoundCloud y Apple Music contra sus APIs reales | Credenciales del propietario (OAuth app y developer token JWT) |
| D-04 | Narrativa de los 4 documentos de `docs/features/` aún no re-derivada | Tiempo de lectura por subsistema, no una dependencia externa |

Lo que este registro **no** afirma en ningún caso: que la funcionalidad del producto
esté completa. La verificación end-to-end de SoundCloud y Apple Music sigue sin estar
hecha, y `docs/MATRIZ_PARIDAD_IMPORTACION.md` lo dice en cada fila afectada (⚠️ y ⛔).

- **Cierra**: la declaración misma. Cada entrada se reabre cuando llegue su entrada
  externa, y se convierte en deuda abierta solo si aparece trabajo que el equipo sí
  pueda hacer.

## Deuda abierta accionable (auditoría de pilares, 2026-10-19)

Las entradas de esta sección **no** están condicionadas a una entrada externa: son
trabajo que el equipo puede hacer ya. Por eso no aparecen en la tabla de alcance
condicionado, cuyo contrato es declarar solo lo que espera a un tercero.

### D-06 — Escrituras SQLx silenciosas

- **Observado**: 243 `let _ = sqlx::query(...)` en producción (125 UPDATE, 82 INSERT,
  11 DELETE). Una escritura puede fallar y la transacción confirmar sin rastro. Reparto:
  `src-tauri/src/services/enrichment.rs` 45, `src-tauri/src/commands/service.rs` 28,
  `src-tauri/src/services/qobuz.rs` 21.
- **Corrección medida al migrar (2026-10-19)**: de esos 243, **18 terminan en
  `.await.map_err(..)?`**, o sea que propagan el error y nunca fueron silencios (10 en
  `commands/favorites.rs`, 8 en `services/tidal.rs`). Los silenciosos reales eran **225**.
- **Frontera creada**: `crate::db::write_or_log(contexto, futuro)` en `src-tauri/src/db.rs`
  concentra la política (registra con `tracing::warn!` y devuelve `Option`), con sus tests.
  Las escrituras que deban abortar tendrán su variante cuando exista el primer llamador.
- **Progreso**: 130 de los 225 silenciosos migrados a `crate::db::write_or_log` —
  `commands/library.rs` 18, `services/enrichment.rs` 45 (los caminos calientes
  `apply_to_database` 25 y `enrich_and_persist_sync_track` 19), `commands/favorites.rs` 9,
  `services/tidal.rs` 9, `services/qobuz.rs` 21 y `commands/service.rs` 28. Los cinco
  ficheros de mayor riesgo del plan quedan cubiertos; el ratchet baja de 243 a 113 (patrón
  bruto; 95 silenciosos + 18 que propagan) y solo puede bajar.
- **Lo que la frontera destapó (2026-10-19)**: en la primera prueba manual,
  `context="enrich_and_persist_sync_track"` registró 51 fallos que antes eran
  invisibles: `UNIQUE constraint failed: tracks.isrc`. Corregido en `c7325a7`
  (ver D-07). Es el argumento de la frontera: no cambia el comportamiento, pero
  convierte un fallo mudo en una línea de log accionable.
- **Cierra**: migrar el resto por lotes (enrichment 45, service 28, qobuz 21, favorites 19,
  tidal 17), un fichero por commit con su suite verde, y seguir bajando el baseline.

### D-08 — Dos funciones monstruo

- **Observado**: `perform_sync_service_with_emitter_inner`
  (`src-tauri/src/commands/service.rs:3285`) mide 4 556 líneas y
  `resolve_token_for_account` (`src-tauri/src/download/qobuz.rs:417`) 2 860; el censo del
  hub da mediana 23 líneas y máximo 4 559.
- **Medido de nuevo (2026-10-19, tras migrar D-06)**: la función ocupa las líneas
  3291-7836 de `src-tauri/src/commands/service.rs`, **4 546 líneas**, con 214 `if`, 88
  `match`, 40 `for` y 122 `.await`; los brazos se distinguen por servicio (qobuz 16
  comparaciones, tidal 13, spotify 10, deezer 9, soundcloud 4).
- **Primer brazo a extraer**: soundcloud, el más pequeño y el que menos comparte con los
  demás. Antes de mover una línea: tests de caracterización del orquestador (secuencia de
  progreso emitida, recuento por servicio, mapeo de errores y qué escribe cada brazo).
- **Hecho (2026-10-19)**: tests de caracterización del orquestador (3 contratos sin red:
  `requires_auth` terminal, emisor opcional y determinismo) y el brazo SoundCloud extraído
  entero a `sync_soundcloud_arm` (249 líneas) con un `ArmOutcome` de 19 acumuladores:
  el `match` pasa a ser una llamada y un volcado explícito. La precondición de credenciales
  vive en `soundcloud_credentials`, función pura con 5 tests, y el brazo tiene su contrato
  (sin fases activas no emite ni toca nada). `perform_sync_service_with_emitter_inner` baja
  de **4 546 a 4 330 líneas**.
- **Hecho (2026-10-19, segundo brazo)**: Deezer extraído a `sync_deezer_arm` (591 líneas).
  Aquí el patrón se generalizó: `ArmOutcome` crece a 26 contadores y el orquestador siembra
  el struct entero y lo vuelca por asignación, sin sumas parciales; los `Vec` de errores y
  avisos se mueven y se devuelven (el brazo corre en exclusiva, nadie más los toca mientras
  tanto). `album_expansion_metrics` entra tal cual porque el brazo lo incrementa por campo.
  El orquestador baja a **3 809 líneas** (desde 4 546: un 16 % menos) y el límite de 8
  argumentos se marca con `allow(clippy::too_many_arguments)` en el brazo, no en el
  orquestador.
- **Límite conocido**: ningún test de integración recorre los brazos con red; la equivalencia
  se apoya en el compilador, en el contrato de cada brazo (autenticación sin red, sin fases
  no se toca nada) y en la suite completa, no en una reproducción con proveedor real.
- **Hecho (2026-10-19, tercer brazo)**: Spotify extraído a `sync_spotify_arm` (841 líneas).
  El struct crece con `reported_favorites`/`reported_albums`/`reported_playlists`
  (`Option<i64>`, los recuentos que declara el proveedor) y el call site de Deezer se alineó
  para sembrarlos también: un struct con campos nuevos no puede dejar inicializadores
  desactualizados. `service_name` volvió a aparecer en una firma sin usarse y se retiró igual
  que en Deezer: el aviso del compilador es el que manda, no mi inventario. El módulo de
  tests pasa a `sync_arm_contracts` porque ya cubre tres brazos (8 contratos).
  El orquestador baja a **3 057 líneas** (desde 4 546: −33 %).
- **Hecho (2026-10-19, cuarto brazo)**: TIDAL extraído a `sync_tidal_arm` (1 098 líneas), el
  brazo con más fases. `ArmOutcome` llega a 33 campos con `albums_unavailable`,
  `tracks_unavailable`, `tracks_expansion_deferred` y `tracks_expansion_failed`. Antes de
  escribirlo hubo un casi-accidente que merece quedar: la línea `availability_checked,`
  aparece también en el literal de `ServiceSyncResult` del final del orquestador, así que
  insertar «detrás de la primera coincidencia» habría corrompido ese struct; el `assert` del
  script lo paró antes de escribir y la inserción quedó restringida a literales
  `ArmOutcome {` sin `..Default::default()`. `service_name` volvió a sobrar y se retiró.
  El orquestador baja a **2 054 líneas** (desde 4 546: −55 %).
- **Hecho (2026-10-19, quinto y último brazo)**: Qobuz extraído a `sync_qobuz_arm`
  (1 237 líneas), el más grande. `ArmOutcome` queda en 34 campos con `purchases_total`.
  `perform_sync_service_with_emitter_inner` baja a **958 líneas**: de 4 546 a 958 (−79 %),
  sin cambiar el contrato del orquestador (los 3 tests de caracterización y los 10 contratos
  de brazo siguen verdes).
- **Validado en la app real (2026-10-19)**: un sync completo de los cinco servicios
  recorrió los cinco brazos sin panic ni cuelgue, con los tiempos que registra el
  orquestador: Tidal 670 s (enriquecimiento 348 s, persistencia 124 s), Qobuz 81 s,
  Apple Music 29 s, Deezer 4 s, SoundCloud 1,4 s. Tidal importó 16 087 de 17 492
  favoritos y 27 832 de 27 832 pistas de playlist.
- **Estado de D-08**: los cinco brazos extraídos (soundcloud 249, deezer 591, spotify 841,
  tidal 1 098, qobuz 1 237). Lo que queda **no** es mover líneas: el `match` ya es una
  llamada y un volcado por brazo, y el siguiente trabajo real es decidir si `ArmOutcome` se
  convierte en el contador único del orquestador (hoy se siembra y se vuelca por brazo) y
  escribir la caracterización con red que hoy falta (secuencia de progreso completa y
  recuento por servicio contra un proveedor simulado).
- **Cierra**: un brazo por commit, cada uno con su test de caracterización verde antes y
  después, y el orquestador reducido a un bucle corto sobre los brazos.

## Cerrado en esta revisión

| ID | Qué se cerró | Evidencia |
|---|---|---|
| C-01 | Los tests de `batch_50_audit_test` se colgaban por el pool SQLite en memoria compartida, no por el paralelismo | SYNC-AUD-060, `b9559da` |
| C-02 | Los tests de `batch_50_audit_test` escribían fuera de su `TempDir` | SYNC-AUD-061, `b9559da` |
| C-03 | La cabecera de `.github/workflows/ci.yml` describía una Deuda A12 ya cerrada | SYNC-AUD-063 |
| C-04 | El README apuntaba a la rama `syncify-graphical` (29 commits por detrás) | SYNC-AUD-064 |
| C-05 | El README citaba un archivo bajo `workspace/`, ruta no versionada | SYNC-AUD-065 |
| C-06 | El puente Python de Qobuz firmaba con secreto vacío | SYNC-AUD-062 |
| C-07 | `sanitize_flac_pictures` descartaba portada sin reportarlo | SYNC-AUD-066 |
| C-08 | Este registro no existía pese a estar citado desde el código y los docs | SYNC-AUD-067 |
| C-09 | La matriz publicaba gates de suite obsoletos | SYNC-AUD-068 |
| C-10 | El plan declaraba "pendiente de ejecución" con F0–F2 y F4 ya ejecutadas | SYNC-AUD-069 |
| C-11 | Los docs de `docs/features/` no declaraban su estado de revalidación | SYNC-AUD-070 |
| C-12 | `sanitize_flac_pictures` no se ejecutaba en la app y el bloque PICTURE irreparable se perdía sin recuperación (D-03) | Ítem 5.3 del plan post-auditoría: `sanitize_flac_pictures_with_recovery` + `salvage_cover_to_jpeg` en `crates/syncify-flac-writer`, `services::flac_cover_sanitizer` con las dos rutas de D-03 (re-encode en el host y sidecar externo `<track>.unrecovered-cover.<sha8>.<ext>`) y registro en `repair_history`; wired en las cinco rutas de escritura FLAC de producción (post-promoción del pipeline Tidal, finalización de descarga Qobuz, re-tag de enriquecimiento, editor manual de tags y el fallback sin BD del descargador Tidal), con `tests/flac_cover_sanitizer_recovery_test.rs` como guardia |
| C-13 | `sync_favorites` solo cubría 3 de los 6 servicios (D-02) | Fase 8 del plan post-auditoría, ítem 8.3: `perform_sync_favorites` tiene los seis brazos (`tidal`, `qobuz`, `spotify`, `deezer`, `soundcloud`, `apple_music`), cada uno paginando completo y persistiendo en `favorites` y en el catálogo vía `persist_favorite_track_via_engine`; `tests/favorites_service_arms_test.rs` los cubre contra servidor mock |
| C-14 | La narrativa de `docs/features/` afirmaba capacidades que el código ya no tiene (DO-4, DO-5) | Ítem 9.1: `metadata-enrichment.md` re-derivado sin Spotify Audio Features y con el worker real (MusicBrainz), `playlist-system.md` re-derivado con la importación real por servicio, README sin «apariciones» ni la cifra de migraciones, `LYRICS_16_PROVIDER_MATRIX.md` sin la ruta `legacy/syncify-cli` |
| C-12 | `cargo fmt --all -- --check` estaba en rojo (9 ficheros) | `ff11167`; equivalencia probada normalizando whitespace y comas |
| C-13 | `cargo clippy -D warnings` en rojo: 2 lints, no 1 (clippy para por crate) | `3174642`; el segundo apareció al arreglar el primero |
| C-14 | El selector de hilos de la biblioteca invocaba un comando inexistente y el error se tragaba | `f5e2317`; vitest 1 failed/44 → 45 passed |
| C-15 | La puerta IPC no vigilaba comandos que la UI invoca y no existen, ni veía `logs_history::*` | `f5e2317`; test nuevo + parser generalizado |
| C-16 | Dos objetivos de test rojos por fixtures que usaban `account_id: 1` sin crear la cuenta | `41b2f08`; 4/4 y 5/5 verdes |
| C-17 | El README afirmaba 9 tests ignorados y el árbol tiene 13 | `1e35108`; puerta que compara la cifra en ambos idiomas |
| C-18 | No había mapa de entrada para un agente | `b4a9588`; `AGENTS.md` escaneado por `scripts/tests/test_docs_consistency.py` |
| C-19 | El subsistema de reproducción retirado seguía registrado (puerta de higiene en rojo) | `resolve_playback_source`, el protocolo y sus tests borrados; `playback.rs` 628 → 194 líneas; puerta 9/9, suite 1 rojo menos |
| C-20 | Misma grabación en otra edición pasaba sin revisión (ED-04 inalcanzable en ese camino) | Guarda antes de escribir: álbum canónico ≠ entrante sin evidencia de release → `needs_review`; suite 2740/0/13. **Corregida el 2026-10-19**: la comparación solo normalizaba espacios y mayúsculas y rechazaba ediciones legítimas; `misma_familia_de_album` las unifica (`46e0a57`) |
| C-21 | El alta de pista con un ISRC ya presente se perdía en silencio (`UNIQUE constraint failed: tracks.isrc`: 51 avisos el 2026-10-19 y 65 el 2026-10-20, sobre 736 descargas) | `INSERT_PISTA_SQL` con `ON CONFLICT DO NOTHING` (sin objetivo: cubre el UNIQUE BINARY de la columna y el índice NOCASE parcial de 0064:186) y resolución por `isrc COLLATE NOCASE`; completa lo que falte sin mover la pista de álbum; test rojo→verde con el 2067 real (variante de caja) |

### D-07 — Cerrada, con corrección de rumbo tras la prueba manual (2026-10-19)

La guarda de ED-04 se validó primero en la app real, no solo en la suite: un sync
completo de los cinco servicios (guion `syncify-prueba-limpia.sh`, logs en
`~/.local/state/syncify-manual-test`) dejó **16 766 errores de ítem** en seis
pasadas (Tidal 7 644, Qobuz 5 098, Spotify 4 023, SoundCloud 1) y los favoritos de
Tidal se quedaron en **16 087 de 17 492** (92 %). Los motivos dominantes eran la
guarda nueva, con pares que son la misma grabación en dos ediciones:

    «Nothing But Thieves»          <- «Nothing But Thieves (Deluxe)»
    «Moral Panic»                  <- «Moral Panic (The Complete Edition)»
    «1»                            <- «1 (Remastered 2015)»
    «Hybrid Theory (DMD Album + 3 Bonus Tracks)» <- «Hybrid Theory (Bonus Edition)»

La comparación normalizaba solo espacios y mayúsculas, así que cualquier sufijo de
edición contaba como álbum distinto y, como `album_provider_track_id` no llega en
estas rutas, el resultado era `needs_review` para todo. `misma_familia_de_album`
quita ahora los sufijos de edición (Deluxe, Remastered, Anniversary, Expanded,
Complete, Bonus, Reissue, Collector, Legacy/Definitive y sus equivalentes en
español; entre paréntesis, corchetes o tras « - ») y solo entonces compara. Siguen
yendo a revisión los lanzamientos distintos de verdad («Jazz Dance» vs «60s Stars»,
recopilatorios «The Essentials» vs «60s Essential Hits»). Dos tests fijan ambos
lados con los pares reales del log, en ambas direcciones (`46e0a57`).

La misma prueba destapó un fallo preexistente que la frontera de escritura de D-06
hizo visible: 51 altas de pista perdidas por `tracks.isrc` UNIQUE. Se corrigió con
`ON CONFLICT(isrc)` que reutiliza la fila y no la mueve de álbum (`c7325a7`).

**Segunda prueba manual (2026-10-20).** Repetido el sync con el arreglo de ED-04
aplicado: los errores de ítem bajan de 16 766 a **7 864** (Tidal 3 834, Spotify 1 773,
Qobuz 2 257) y el motivo dominante deja de ser la guarda ED-04 para pasar a ser la
**colisión de identidad del ítem** (620 + 497 + 597), que sigue yendo a `needs_review`
a propósito. Los ED-04 que quedan son lanzamientos realmente distintos (recopilatorios,
bandas sonoras, «B-Sides», reediciones) o variantes de formato que el guardián no debe
unificar a ciegas.

La misma prueba destapó que el primer arreglo del ISRC era incompleto: la tabla tiene
**dos** restricciones sobre la columna (`tracks.isrc` UNIQUE BINARY y
`idx_tracks_isrc_unique` NOCASE parcial, 0064:186) y el `ON CONFLICT(isrc)` solo veía la
primera, así que un ISRC enriquecido que solo cambiaba de caja seguía perdiendo la fila:
65 de 736 descargas. Corregido con `ON CONFLICT DO NOTHING` sin objetivo más resolución
por `isrc COLLATE NOCASE`, con un test que reproduce la variante de caja y el mismo
código 2067.

### D-02 — Cerrada, y con qué alcance exacto

La entrada D-02 pedía que `sync_favorites` cubriera los 6 servicios, no que dejara de
funcionar para 3. Está cerrada porque los seis brazos existen y están cubiertos:

| Servicio | Canciones | Álbumes | Artistas |
|---|---|---|---|
| Tidal | paginado (offset/limit 100) | una página (0, 100) | una página (0, 100) |
| Qobuz | paginado | una página (0, 100) | una página (0, 100) |
| Spotify | paginado (50) | una página (0, 50) | una página: `get_followed_artists(None, 50)`, sin iterar cursor |
| Deezer | paginado | paginado | paginado |
| SoundCloud | likes paginados por `next_href` | — (la API no lo expone) | — (la API no lo expone) |
| Apple Music | biblioteca paginada por `next` | biblioteca paginada por `next` | — (sin rama de artistas) |

Las diferencias de la tabla son del código, no del documento: la fase 8 dio a
`sync_favorites` un brazo por servicio, no una paginación uniforme por colección.

Los seis escriben la fila en `favorites` **y** la persisten en el catálogo por
`persist_favorite_track_via_engine`, que es el mismo camino que usa el motor
unificado; SoundCloud y Apple Music además conservan el atributo que solo ellos
traen (`publisher_metadata` y `date_added`). El brazo inexistente devuelve
`Unsupported service for favorites sync` en vez de fingir una importación vacía.

**Lo que la verificación de esta entrada NO cubre**: el comportamiento de los brazos
`deezer`, `soundcloud` y `apple_music` contra sus APIs reales. Los tests de
`tests/favorites_service_arms_test.rs` usan un servidor mock en proceso. Esa
verificación end-to-end es la parte de D-01, y depende de credenciales del
propietario.

### D-03 — Cerrada, y con qué límite exacto

La entrada D-03 pedía una ruta de recuperación antes de declarar cerrado el descarte
del bloque PICTURE. Se implementaron **las dos** opciones que la dejaban como decisión
de producto, no una:

1. **Re-encode en el host** — `salvage_cover_to_jpeg` decodifica el payload dañado con
   `-err_detect ignore_err -fflags +discardcorrupt` sobre un archivo temporal con la
   extensión que corresponde a su contenedor, y el fotograma reconstruido vuelve al
   bloque PICTURE (`FlacPictureSanitizeReport::recovered_blocks`).
2. **Sidecar externo** — si ni la ruta estricta ni la tolerante producen un fotograma,
   los bytes originales se conservan junto al FLAC como
   `<track>.unrecovered-cover.<sha256[..8]>.<ext>` (direccionado por contenido, así que
   una segunda pasada no duplica nada y dos portadas perdidas del mismo track no se
   pisan). Los sidecars canónicos de Symfonium no se tocan.

Lo que **no** se promete: que un payload que ningún decodificador del host puede leer
vuelva a ser una imagen. El límite real es explícito y queda registrado, nunca mudo: si
ambas rutas fallan, el bloque sale del contenedor, la causa (incluida la ruta del
sidecar) viaja en `FlacPictureSanitizeReport::dropped_unrepairable_blocks`, en las
`actions` y en el `details_json` de la fila de `repair_history`, y el usuario la ve en
el modal de historial de reparaciones.


# Matriz viva de paridad de importación

> **Documento vivo** (Fase 4-3 del plan). Actualizar en cada sprint que toque
> un brazo del motor. Última actualización: 2026-10-02 (ítem 9.1 del plan
> post-auditoría: `sync_favorites` a 6 servicios con la fase 8, migración entre
> servicios activa con la fase 4, y estado real del brazo de Apple Music). Antes:
> 2026-09-29 (SYNC-AUD-068: los gates se remedieron en vez de publicarse como
> cifras heredadas).
>
> «Motor unificado» = `EnrichmentEngine::enrich_and_persist_sync_track` vía
> `enrich_persist_with_locked_retry` (identidad A→B→C, enriquecimiento,
> transacción con retry). «Crudo» = INSERT directo sin identidad canónica.
> «⚠️ importador propio» = la fase llega pero por un importador específico del
> servicio, con identidad por ISRC/`track_sources` y **sin** pasar por
> `EnrichmentEngine`.

## Sincronización completa (`perform_sync_service_with_emitter`)

| Fase | Tidal | Qobuz | Spotify | Deezer | SoundCloud | Apple Music |
|---|---|---|---|---|---|---|
| Favoritos (tracks) | ✅ motor, paginado | ✅ motor, paginado (S187/S198) | ✅ motor, paginado | ✅ motor, paginado (**F1**) | ✅ motor, paginado (**F3-1**) | ⚠️ importador propio, ISRC + `track_sources`, sin `EnrichmentEngine` (**F3-2**) |
| Álbumes favoritos | ✅ motor + marcado | ✅ motor + marcado + `qobuz_id` (S198) | ✅ motor + marcado (S198) | ✅ expansión + marcado sin columna dedicada (**F1**, decisión F4-2) | 📋 sin endpoint: nacen del `publisher_metadata` de cada like (**F3-1**) | ⚠️ álbumes de biblioteca, no "favoritos" (**F3-2**) |
| Artistas favoritos | ✅ | ✅ | ✅ cursor completo (**F2**) | ✅ real (**F1**) | 📋 sin endpoint: nacen de la atribución de cada like (**F3-1**) | 📋 sin endpoint: nacen de la atribución de cada canción (**F3-2**) |
| Playlists | ✅ paginado | ✅ paginado (S198) | ✅ paginado (S198) | ✅ upsert + expansión paginada (**F1**) | 📋 capacidad no expuesta por la API pública (**F3-1**) | ✅ paginado con `next` + expansión si la relación viene truncada (**F3-2**) |
| Historial | ✅ | ✅ | ✅ | 📋 capacidad no expuesta por API pública — warning honesto (**F1**) | 📋 capacidad no expuesta por la API pública (**F3-1**) | ⛔ la biblioteca importada es de favoritos; no hay historial |
| Auth parity (RequiresAuth + invalidación) | ✅ | ✅ (S186) | ✅ | ✅ init/user_id (**F1**) | ✅ 401 → RequiresAuth + invalidación (**F3-1**) | parcial: credencial ausente → `RequiresAuth`, sin invalidación |
| Contadores canónicos de resultado | ✅ | ✅ referencia | ✅ | ✅ alineado (**F2-6**) | ✅ alineado (**F3-1**) | ⚠️ contadores tomados del resumen del importador (**F3-2**) |

### Apple Music: qué está implementado y qué no

- **Storefront**: se resuelve desde la credencial `storefront` del login, con
  `APPLE_MUSIC_STOREFRONT` y `us` como respaldo — ya no está fijado a ciegas
  (`normalize_storefront`/`resolve_storefront` en `services/apple_music.rs`).
- **Paginación**: canciones con `next` (`next_apple_music_offset`), playlists con
  `fetch_all_playlist_tracks` cuando la relación embebida está truncada.
- **Identidad**: persiste ISRC y `track_sources` (AAC), pero las pistas entran con
  `INSERT OR IGNORE (title, duration_ms, isrc)` y `get_or_create_artist`: no hay
  enriquecimiento ni la resolución A→B→C del motor unificado. Esa diferencia es la
  razón de ⚠️ y no de ✅.
- **Verificación**: todo está cubierto contra servidor mock en
  `tests/apple_music_storefront_pagination_and_migration_test.rs`; la verificación
  end-to-end contra la API real sigue necesitando el developer token del propietario
  (`docs/Deuda_Tecnica_y_UX.md`, D-01).

## Comandos auxiliares

| Comando | Estado |
|---|---|
| `sync_favorites` (catálogo + biblioteca) | ✅ **los 6 servicios** desde la fase 8 del plan post-auditoría (ítem 8.3): `perform_sync_favorites` tiene brazo `tidal`, `qobuz`, `spotify`, `deezer`, `soundcloud` y `apple_music` (`src-tauri/src/commands/favorites.rs`), cada uno persistiendo en `favorites` **y** en el catálogo vía `persist_favorite_track_via_engine`. Canciones paginadas en los seis; en Deezer también álbumes y artistas. Las colecciones de álbumes/artistas de Tidal, Qobuz y Spotify se leen en una sola página, y SoundCloud y Apple Music no tienen rama de artistas. Cubierto por `tests/favorites_service_arms_test.rs` contra servidor mock |
| `sync_playlists` | lectura agregada REAL por servicio (**F2-5**, `2043672`) |
| Rate limiting | GLOBAL_RATE_LIMITER con perfiles por servicio; `get_playlists` spotify incorporado (**F2-3**, `2d43e5e`) |
| Verificación de credenciales | `perform_get_service_auth_status` presenta el token guardado a la API antes de declarar `connected_valid`; un 401 revocado ya no se ve como sano (fase 8, ítem 8.3) |
| Criterio de reintentos | un único `ErrorTaxonomy` (`services/http_retry.rs`) en vez de tres criterios escritos a mano; SoundCloud y Apple Music ya pasan por el rate limiter compartido |
| Código muerto retirado | `SpotifyClient::import_library`, `refresh_from_sp_dc`, pipeline audio-features S68, `upsert_canonical_favorite_track` (**F4-1**, `aa806b8`) |

## Migración entre servicios (fase 4 del plan post-auditoría)

| Aspecto | Estado |
|---|---|
| Modo servicio → servicio | ✅ activo. `preview_migration` cuenta **matches reales** contra el catálogo del destino y no escribe nada (job, items); sin cuenta de destino se niega en vez de devolver la estimación `× 0.85` retirada |
| Destinos | ✅ los 6. `MIGRATION_DESTINATION_SERVICES` (`src-tauri/src/commands/migration.rs`) es la lista única, y `get_migration_destinations` la sirve a la UI: añadir un destino es backend, no cambio de interfaz |
| Sin cliente de destino | `start_migration` no declara transferencia de ninguna pista — tampoco de las emparejadas a mano. Nada fabrica un éxito |
| Emparejamientos manuales | `find_manual_match` recupera el último matching manual de la misma ruta origen → destino, así que lo revisado en una pasada sirve en la siguiente |
| Alcance del trabajo | ✅ respeta las playlists seleccionadas (BD-6) y contabiliza por fila de `migration_items` (BD-7), con `source_playlist_id`, `source_playlist_name` y `error_message` |
| `library_items` | ✅ poblada en continuo por los triggers de `migrations/0086_library_items_continuous_sync.sql`, ya desde `migrations/0085_alignment_audit.sql` (BD-4): los tres comandos que la leen ven datos |
| Cobertura | `tests/migration_service_mode_test.rs`, `tests/migration_job_scope_and_accounting_test.rs`, `tests/library_items_continuous_sync_test.rs` |

## Identidad

Ver `docs/DECISION_IDENTIDAD_TRACKS.md`: clave maestra
`track_sources(service_id, service_track_id)`; sin columnas nuevas por proveedor.

## Deuda conocida de paridad (orden del plan)

> Las filas ⛔ de Apple Music son **alcance condicionado**, no defectos ni trabajo
> pendiente: están registradas con su condición de cierre en
> `docs/Deuda_Tecnica_y_UX.md` (D-01) y dependen de credenciales del
> propietario. Las filas 📋 de SoundCloud y Deezer son constantes de capacidad del
> servicio, declaradas como tales. Las filas ⚠️ de Apple Music son alcance
> implementado por un importador propio que no pasa por `EnrichmentEngine`.
> La matriz no afirma que la funcionalidad esté completa; afirma que lo que falta
> está declarado (SYNC-AUD-077).

1. **Fase 3 implementada; lo que queda es la verificación contra la API real**:
   ambos brazos están escritos y cubiertos contra servidor mock
   (`tests/soundcloud_unified_engine_test.rs`,
   `tests/apple_music_storefront_pagination_and_migration_test.rs`). Lo que sigue
   pendiente es end-to-end contra las APIs reales, que necesita la OAuth app de
   SoundCloud y el developer token JWT de Apple Music del propietario
   (`docs/Deuda_Tecnica_y_UX.md`, D-01).
2. `sync_favorites` para los 6 servicios: **cerrado** en la fase 8 (ítem 8.3); la
   verificación end-to-end de sus brazos sc/am comparte la condición de D-01.
3. `service_sync_settings.qobuz.sync_albums=0` obsoleto en producción — dato
   del propietario, no código.

### SoundCloud: qué es límite del servicio y qué era integración

- **Identidad**: el ISRC solo existe cuando el track lo entrega un sello
  (`publisher_metadata.isrc`). Los subidas directamente (remixes, bootlegs, sets)
  no lo traen, así que su identidad se resuelve por Check A, el id de SoundCloud
  en `track_sources`. Es una constante de capacidad, no un hueco pendiente.
- **Fases**: la API pública no expone álbumes, artistas, playlists ni historial
  de usuario; esas fases emiten un warning honesto y los rows de álbum/artista
  nacen igual de cada like cuando su `publisher_metadata` los aporta.

## Gates que avalan esta matriz

Cifras **medidas el 2026-09-29 sobre `bc2006c`**. Cada fila lleva su comando, la
revisión que la respalda y si la cifra está medida o no, para que cualquiera la repita
en lugar de heredarla de un sprint anterior (SYNC-AUD-068 y SYNC-AUD-072). La revisión
del ítem 9.1 (2026-10-02) actualiza el estado que estas cifras respaldan, pero **no
re-mide ninguna de ellas**: las cifras son las de `bc2006c` y cada fila lo declara.

Los comandos se escriben desde la raíz del repositorio con `--manifest-path`. La forma
`cargo --manifest-path <ruta> test` no la acepta el cargo 1.98.1 de este proyecto
(`unexpected argument '--manifest-path' found`), así que publicarla sería publicar un
comando que no se puede copiar y pegar. CI los ejecuta desde `src-tauri/` sin
`--manifest-path`, que es la misma ejecución.

| Gate | Comando (desde la raíz) | Revisión | Resultado medido |
|---|---|---|---|
| Suite Rust, pase general | `cargo test --manifest-path src-tauri/Cargo.toml --locked -- --skip test_batch_50_pipeline_e2e_concurrency_and_forensic_audit --skip test_batch_health_check_detects_anomalies_and_staging_orphans` | `bc2006c` | **1589 passed / 0 failed / 10 ignored**, 243 binarios de test |
| Suite Rust, `batch_50` serializado | `cargo test --manifest-path src-tauri/Cargo.toml --locked --test batch_50_audit_test -- --test-threads=1` | `bc2006c` | **2 passed / 0 failed** → **1591** en el pase general que ejecuta CI |
| Tipos | `cargo check --manifest-path src-tauri/Cargo.toml --all-targets --locked` | `bc2006c` | 0 errores |
| Lint | `cargo clippy --manifest-path src-tauri/Cargo.toml --workspace --all-targets --locked -- -D warnings` | `bc2006c` | exit 0, 0 avisos |
| Formato | `cargo fmt --manifest-path src-tauri/Cargo.toml --all -- --check` | `bc2006c` | limpio |
| Puentes Python | `python3 -m unittest discover -s scripts/tests -p 'test_*.py'` | `bc2006c` | **152 tests, 0 fallos** (incluye los gates de SYNC-AUD-070 y SYNC-AUD-071, los 4 diagnósticos de Qobuz de SYNC-AUD-076, los 5 de higiene de artefactos de SYNC-AUD-074, los 4 de alcance condicionado de SYNC-AUD-077 y los 2 de trazabilidad de gates de SYNC-AUD-072) |
| Frontend (vitest) | `npm --prefix ui run test:run` y `npx --prefix ui vue-tsc --noEmit` | — | **sin medir**: el árbol no trae `ui/node_modules` y el modo de reparación prohíbe instalar dependencias de red. La instalación sin red tampoco es posible: la caché local de npm no está completa (`npm ci --offline` falla con `ENOTCACHED` en `xmlchars@2.2.0`). No se publica aquí una cifra que nadie ha reproducido en esta revisión |

La cifra de Python cambió respecto a la publicada antes (137 → 152) por los 15 tests
nuevos de esta revisión, no por una regresión. La de Rust también cambia respecto a la
publicada para `b9559da` (1756 → 1589 sobre 243 binarios): son revisiones distintas
del árbol, y la cifra de esta tabla es la medida en `bc2006c`, no la heredada.

Commits ancla: `f84219f` (S198+F0) · `413da34`/`c27fc0c`/`ba5ba37` (F1/F2 deezer)
· `2043672` (F2-5) · `6ee9ce4` (F2-4) · `aa806b8` (F4-1) · `2d43e5e` (F2-3).

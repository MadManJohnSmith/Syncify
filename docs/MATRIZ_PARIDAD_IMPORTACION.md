# Matriz viva de paridad de importación

> **Documento vivo** (Fase 4-3 del plan). Actualizar en cada sprint que toque
> un brazo del motor. Última actualización: 2026-09-29 (SYNC-AUD-068: los gates se
> remedieron en vez de publicarse como cifras heredadas).
>
> «Motor unificado» = `EnrichmentEngine::enrich_and_persist_sync_track` vía
> `enrich_persist_with_locked_retry` (identidad A→B→C, enriquecimiento,
> transacción con retry). «Crudo» = INSERT directo sin identidad canónica.

## Sincronización completa (`perform_sync_service_with_emitter`)

| Fase | Tidal | Qobuz | Spotify | Deezer | SoundCloud | Apple Music |
|---|---|---|---|---|---|---|
| Favoritos (tracks) | ✅ motor, paginado | ✅ motor, paginado (S187/S198) | ✅ motor, paginado | ✅ motor, paginado (**F1**) | ⛔ crudo, incompleto | ⛔ crudo, incompleto |
| Álbumes favoritos | ✅ motor + marcado | ✅ motor + marcado + `qobuz_id` (S198) | ✅ motor + marcado (S198) | ✅ expansión + marcado sin columna dedicada (**F1**, decisión F4-2) | ⛔ | ⛔ |
| Artistas favoritos | ✅ | ✅ | ✅ cursor completo (**F2**) | ✅ real (**F1**) | ⛔ | ⛔ |
| Playlists | ✅ paginado | ✅ paginado (S198) | ✅ paginado (S198) | ✅ upsert + expansión paginada (**F1**) | ⛔ | ⛔ (campo `next` ignorado) |
| Historial | ✅ | ✅ | ✅ | 📋 capacidad no expuesta por API pública — warning honesto (**F1**) | ⛔ | ⛔ |
| Auth parity (RequiresAuth + invalidación) | ✅ | ✅ (S186) | ✅ | ✅ init/user_id (**F1**) | parcial | parcial |
| Contadores canónicos de resultado | ✅ | ✅ referencia | ✅ | ✅ alineado (**F2-6**) | ⛔ | ⛔ |

## Comandos auxiliares

| Comando | Estado |
|---|---|
| `sync_favorites` (catálogo + biblioteca) | tracks tidal/qobuz/spotify → motor + paginación completa (**F2-4**); álbumes/artistas siguen siendo catálogo propio; 6 servicios: pendiente sc/am (F3) |
| `sync_playlists` | lectura agregada REAL por servicio (**F2-5**, `2043672`) |
| Rate limiting | GLOBAL_RATE_LIMITER con perfiles por servicio; `get_playlists` spotify incorporado (**F2-3**, `2d43e5e`) |
| Código muerto retirado | `SpotifyClient::import_library`, `refresh_from_sp_dc`, pipeline audio-features S68, `upsert_canonical_favorite_track` (**F4-1**, `aa806b8`) |

## Identidad

Ver `docs/DECISION_IDENTIDAD_TRACKS.md`: clave maestra
`track_sources(service_id, service_track_id)`; sin columnas nuevas por proveedor.

## Deuda conocida de paridad (orden del plan)

> Las filas ⛔ de SoundCloud y Apple Music son **alcance condicionado**, no defectos
> ni trabajo pendiente: están registradas con su condición de cierre en
> `docs/Deuda_Tecnica_y_UX.md` (D-01 y D-02) y dependen de credenciales del
> propietario. La matriz no afirma que la funcionalidad esté completa; afirma que lo
> que falta está declarado (SYNC-AUD-077).

1. **Fase 3 — BLOQUEADA en credenciales reales del propietario**: SoundCloud
   (OAuth app) y Apple Music (developer token JWT). Los arreglos a ciegas de
   pagination/storefront/publisher-metadata se harán junto a esa verificación.
2. `sync_favorites` para los 6 servicios (hoy 3) tras Fase 3.
3. `service_sync_settings.qobuz.sync_albums=0` obsoleto en producción — dato
   del propietario, no código.

## Gates que avalan esta matriz

Cifras **medidas el 2026-09-29 sobre `bc2006c`**, la revisión que las reparaciones de
SYNC-AUD-072 a SYNC-AUD-077 dejan en la candidata. Cada fila lleva su comando, la
revisión que la respalda y si la cifra está medida o no, para que cualquiera la repita
en lugar de heredarla de un sprint anterior (SYNC-AUD-068 y SYNC-AUD-072).

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

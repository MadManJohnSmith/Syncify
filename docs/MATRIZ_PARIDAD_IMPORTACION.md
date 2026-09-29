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

1. **Fase 3 — BLOQUEADA en credenciales reales del propietario**: SoundCloud
   (OAuth app) y Apple Music (developer token JWT). Los arreglos a ciegas de
   pagination/storefront/publisher-metadata se harán junto a esa verificación.
2. `sync_favorites` para los 6 servicios (hoy 3) tras Fase 3.
3. `service_sync_settings.qobuz.sync_albums=0` obsoleto en producción — dato
   del propietario, no código.

## Gates que avalan esta matriz

Cifras **medidas el 2026-09-29** sobre `b9559da` + las reparaciones de
SYNC-AUD-062 a SYNC-AUD-071. Cada una lleva su comando, para que cualquiera la
repita en lugar de heredarla de un sprint anterior (SYNC-AUD-068).

| Gate | Comando | Resultado medido |
|---|---|---|
| Suite Rust, pase general | `cargo test --locked -- --skip test_batch_50_pipeline_e2e_concurrency_and_forensic_audit --skip test_batch_health_check_detects_anomalies_and_staging_orphans` | **1756 passed / 0 failed / 10 ignored**, 252 binarios de test (7 unit + 245 integración) |
| Suite Rust, `batch_50` serializado | `cargo test --locked --test batch_50_audit_test -- --test-threads=1` | **2 passed / 0 failed** → **1758** en el pase general que ejecuta CI |
| Tipos | `cargo check --all-targets --locked` | 0 errores |
| Lint | `cargo clippy --workspace --all-targets --locked -- -D warnings` | 0 avisos |
| Formato | `cargo fmt --all -- --check` | limpio |
| Puentes Python | `python3 -m unittest discover -s scripts/tests -p 'test_*.py'` | **137 passed / 0 failed** (incluye el gate de coherencia documental de SYNC-AUD-070 y el de riesgo aceptado de SYNC-AUD-071) |
| Frontend (vitest) | `npm --prefix ui run test:run` | **sin medir**: la reparación se ejecuta sin red y sin `ui/node_modules`. No se publica aquí una cifra que nadie ha reproducido en esta revisión |

Commits ancla: `f84219f` (S198+F0) · `413da34`/`c27fc0c`/`ba5ba37` (F1/F2 deezer)
· `2043672` (F2-5) · `6ee9ce4` (F2-4) · `aa806b8` (F4-1) · `2d43e5e` (F2-3).

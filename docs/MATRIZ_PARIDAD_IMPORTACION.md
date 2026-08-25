# Matriz viva de paridad de importación

> **Documento vivo** (Fase 4-3 del plan). Actualizar en cada sprint que toque
> un brazo del motor. Última actualización: 2026-08-25.
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

Suite completa **1019+ passed / 0 failed / 10 ignored** (lib 175 · bin 177 ·
integración 320+347 chunked), `cargo check --all-targets` 0/0, vitest 283/283.
Commits ancla: `f84219f` (S198+F0) · `413da34`/`c27fc0c`/`ba5ba37` (F1/F2 deezer)
· `2043672` (F2-5) · `6ee9ce4` (F2-4) · `aa806b8` (F4-1) · `2d43e5e` (F2-3).

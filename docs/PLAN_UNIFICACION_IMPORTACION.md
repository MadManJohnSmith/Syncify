# Plan: Unificación del pipeline de importación entre servicios

> Estado: propuesta aprobada (pendiente de ejecución)
> Origen: auditoría comparativa de los métodos de importación de todos los servicios
> Fecha: 2025-08-25

## Objetivo

Que los 6 servicios fuente (Qobuz, Tidal, Spotify, Deezer, SoundCloud, Apple Music) pasen por el **mismo contrato de importación**: mismo flujo de fases, misma persistencia canónica, misma gestión de errores/auth/throttle y mismos eventos de progreso. Last.fm y MusicBrainz quedan fuera (son capa de enriquecimiento, no importadores).

## Definición de "parejos" (matriz objetivo)

| Capacidad | Hoy | Objetivo |
|---|---|---|
| Motor unificado S128B | qobuz/tidal/spotify/deezer(stub) | Los 6 |
| Persistencia vía `EnrichmentEngine` | qobuz/tidal/spotify | Los 6 |
| Fases: favoritos tracks/álbumes/artistas | qobuz/tidal/spotify completos; deezer solo tracks; sc/am incompletos | Todos los que la API permita |
| Playlists importadas con paginación completa | Parcial (spotify/qobuz leen 1 página en el motor) | Paginación completa siempre |
| Paginación robusta (semántica S187) | Solo tidal | Helper común usado por todos |
| Throttle global + reintentos | Solo spotify/tidal/qobuz parciales | Todos invocan `rate_limiter` + política compartida |
| 401 → `RequiresAuth` + invalidar credenciales | tidal/qobuz | Todos |
| Eventos `SyncProgressEvent` + outcome (`success/partial_failure/failed`) | Motor unificado solo | Todos |
| Toggle favoritos desde UI | qobuz/tidal/spotify | + deezer, sc, am |
| Destino de migración (matching ISRC→metadata) | qobuz/tidal/deezer/soundcloud | + apple_music (activar el matching que ya existe) |

Asimetrías **intencionales que se conservan**: compras (solo Qobuz lo expone), metadatos hi-res (solo servicios lossless), historial (solo Qobuz).

---

## Fase 0 — Infraestructura compartida (base, ~2-3 días)

1. **Helper de paginación común** (`services/import_pagination.rs`): `paginate_offset()` y `paginate_cursor()` con la semántica S187 generalizada (página corta ≠ fin si hay total; avanzar por longitud real; warn-and-continue con registro del hueco). Tidal ya lo tiene inline → se refactoriza para usarlo.
2. **Taxonomía de errores compartida**: extraer `is_transient_page_error` de `tidal.rs:163` y fusionarla con `http_retry.rs` en un solo criterio transitorio/terminal/auth usado por los 6 clientes.
3. **Throttle real para los rezagados**: invocar `GLOBAL_RATE_LIMITER.acquire()` + `penalize_service()` en 429 dentro de Deezer, SoundCloud y Apple Music (la configuración ya existe en `rate_limiter.rs:53`, nadie la llama).
4. **Contrato único de resultado**: `ServiceSyncResult` como única respuesta; definir `skipped = ya presente` formalmente; los comandos heredados se convierten en wrappers.

## Fase 1 — Deezer al motor unificado (mayor brecha visible, ~3-4 días)

1. Ampliar `services/deezer.rs` con la API pública (estable, sin depender de gw-light): `/user/{id}/albums`, `/user/{id}/artists`, `/user/{id}/playlists`, `/playlist/{id}/tracks`.
2. Reescribir la rama `"deezer"` de `perform_sync_service_with_emitter` (`service.rs:4108`): hoy álbumes/artistas/playlists/historial **solo emiten eventos falsos** → fases reales con `SyncTrackInput` → `EnrichmentEngine`.
3. Auth en paridad: `init()` fallido o checkForm ausente ⇒ `mark_account_credentials_invalid` + evento `RequiresAuth` (hoy tolera continuar con user_id cacheado).
4. Unificar tamaño de página (100 en cliente vs 50 en comando hoy).
5. Toggle de favoritos desde UI (`favorites.rs`): `favorite_song.add/delete` por gw-light.

## Fase 2 — Completar el motor unificado (~2-3 días)

1. **Paginación completa de tracks por playlist**: loop en rama spotify (`service.rs:3969`, hoy solo `0,100`) y qobuz (hoy una llamada de 200).
2. **Followed artists de Spotify**: iterar el cursor `after` completo (hoy 1 página de 50).
3. Añadir rate-limiter/retry internos a `SpotifyClient.get_playlists` (es el único lector grande sin ellos).
4. **`sync_favorites` pasa a delegar en el motor unificado** (elimina la variante de una sola página y la duplicación de upserts de `favorites.rs:814`); extender a los 6 servicios.
5. **`sync_playlists`**: convertirlo en lectura agregada real de la tabla `playlists` multi-servicio (hoy es un stub que cuenta, `playlists.rs:213`) o retirarlo de la UI.

## Fase 3 — SoundCloud y Apple Music a paridad (~3-4 días)

1. Ramas `"soundcloud"`/`"apple_music"` en el motor unificado con `EnrichmentEngine` (hoy insertan crudo, sin enriquecimiento ni identidad canónica A→B→C).
2. **Apple Music**: paginar con el campo `next` (hoy se ignora), storefront configurable desde credenciales (hoy `"us"` hardcodeado), activarlo como destino de migración (su matching ya está escrito pero sin llamador).
3. **SoundCloud**: artista desde `publisher_metadata.artist` cuando exista (hoy usa el username del uploader), unificar la dedup duplicada entre cliente y comando, dejar el "sin ISRC" documentado como constante de capacidad.
4. Integrar ambos en `get_service_auth_status` y en el pre-check de autenticación del motor.

## Fase 4 — Limpieza de deuda (~1-2 días)

1. Eliminar código muerto: `SpotifyClient::import_library` (sin llamadores), `refresh_from_sp_dc` (letras usan su propio camino), pipeline audio-features legacy S68 (API retirada), `#![allow(dead_code)]` de sc/am donde aplique.
2. Documentar decisión de identidad: `track_sources(service_id, service_track_id)` es la clave maestra para servicios sin columna dedicada (no se crean columnas `tidal_id`/`deezer_id` — Check A ya cubre).
3. Matriz de paridad viva en `docs/` + actualización de `Deuda_Tecnica_y_UX.md`.

## Fase 5 — Tests (en paralelo con cada fase)

1. Tests de regresión contra mock server por servicio (patrón existente `s187_tests`): paginación de cada fase nueva.
2. Test de idempotencia global: importar dos veces ⇒ 0 nuevos en los 6 servicios.
3. Test de `RequiresAuth`: 401 simulado ⇒ credenciales invalidadas + evento emitido.

---

## Riesgos y mitigaciones

- **API privada de Deezer (gw-light)** inestable → todo lo nuevo usa la API pública; gw-light queda solo para escritura de favoritos.
- **Cambio de atribución de artista en SoundCloud** podría desalinear matches históricos → hacerlo flag-gated y aprovechar `repair_guardrail`/`catalog_identity_repair` ya existentes.
- **Apple Music exige developer_token vigente** → mensajes `RequiresAuth` accionables, sin crash del import.
- **Cambios aditivos**: `EnrichmentEngine` ya es la vía de las descargas; no se toca su contrato.

## Orden de ejecución recomendado

Fase 0 → Fase 1 → Fase 2 → Fase 3 → Fase 4, con los tests de la Fase 5 escritos junto a cada fase (no al final).

## Hallazgos que motivan el plan (resumen de la auditoría)

- **Auth por servicio** (todas cifradas en `accounts.credentials_json`): Spotify OAuth Authorization Code (refresh preventivo buffer 300 s; vía sp_dc con Chromium headless existe pero sin llamadores de importación); Tidal OAuth Device Code Flow vía script Python (~4 h, scope `r_usr+w_usr+w_sub`); Qobuz app_id/secret embebidos + firma MD5 por request + auto-login user/pass persistible (S186); Deezer cookie ARL + token efímero gw-light; SoundCloud OAuth token + user_id; Apple Music doble JWT MusicKit (developer + music_user_token); Last.fm API key pública; MusicBrainz anónimo.
- **Alcance actual**: Qobuz 5–6 fases (incluye compras e historial); Tidal 4 fases; Spotify 4 fases + único importador con fetch paralelo (4×50); Deezer solo tracks favoritos (resto stubs que emiten eventos vacíos); SoundCloud solo likes; Apple Music solo `/me/library/songs`.
- **Persistencia**: motor moderno = transacción por track con identidad canónica A) `track_sources(service_id, service_track_id)` → B) ISRC → C) columna dedicada (**solo existen `qobuz_id`/`spotify_id`**), preservando metadatos manuales. Caminos heredados = dedup cruda distinta por servicio (Deezer ISRC-first, SoundCloud title+duration, Apple Music title+isrc; Tidal heredado deja duplicar tracks sin ISRC porque UNIQUE ignora NULL).
- **Throttle/reintentos desiguales**: config global existe (spotify 30/s, tidal 20/s, qobuz 10/s+100 ms, deezer 50/5 s, soundcloud 15/s, apple_music 30/s, lastfm 4/s, musicbrainz 1/s) pero deezer/soundcloud/apple_music no la invocan; Last.fm/MusicBrainz throttle propio local; solo las escrituras `add_to_favorites` tienen retry backoff exponencial común.
- **Inconsistencias menores**: expansión de playlist-tracks incompleta en el motor (spotify `0,100` una página; qobuz una llamada de 200); followed artists spotify sin iterar cursor; Deezer límite 100 vs 50 según punto de entrada; `sync_favorites` solo primera página y solo 3 servicios; `sync_playlists` stub; `ImportResult.skipped` con semántica distinta por ruta; audio features de Spotify retirado por la API (S68).

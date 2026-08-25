# Decisión de identidad de tracks — `track_sources` como clave maestra

**Estado:** DECIDIDO (Fase 4-2 del plan de unificación) · **Fecha:** 2026-08-25 · **Autoriza:** plan aprobado por el propietario (`docs/PLAN_UNIFICACION_IMPORTACION.md`)

## Decisión

La identidad canónica de una pista en Syncify es el par
**`track_sources(service_id, service_track_id)`**, gestionado exclusivamente por
el motor unificado (`EnrichmentEngine::enrich_and_persist_sync_track`).

**No se crearán columnas dedicadas por proveedor** (`tidal_id`, `deezer_id`,
`soundcloud_id`, …) en `tracks`. Las únicas columnas de identidad externa que
existen son históricas y concretas:

| Columna | Motivo de existencia | Política |
|---|---|---|
| `albums.qobuz_id` | matching de álbumes favoritos S198 (índice único parcial `WHERE qobuz_id IS NOT NULL`) | última aceptada; no extender sin necesidad demostrada |
| `tracks.isrc` | llave de dedup entre servicios cuando el proveedor la expone | fuente de verdad cross-servicio |
| `artists.name` (NOCASE) + `albums.title NOCASE + artist_id` | identidad de respaldo cuando no hay ID externo | contratos del motor documentados en código |

## Por qué

1. **Un servicio sin columna dedicada ya tiene clave maestra**: cualquier
   proveedor nuevo (o uno al que solo se le conozca su ID interno) se registra
   como fila en `track_sources`; el grafo A→B→C del motor resuelve equivalentes
   vía ISRC y coincidencia canónica título+artista.
2. **Elimina la explosión combinatoria** de columnas/migraciones por proveedor.
3. **Un solo punto de escritura**: toda mutación de identidad pasa por el motor,
   con transacción, verificación y reintentos ante SQLITE_BUSY. Los caminos
   crudos duplicados fueron retirados (F2-4 eliminó `upsert_canonical_favorite_track`;
   F4-1 eliminó `SpotifyClient::import_library`).

## Reglas operativas derivadas

- Código nuevo NUNCA inserta directamente en `tracks`/`track_sources`;
  construye un `SyncTrackInput` y llama a
  `enrich_persist_with_locked_retry`.
- Los brazos de sincronización por servicio solo traducen DTOs del proveedor a
  `SyncTrackInput` y contabilizan el `SyncTrackResult`.
- La dedup dentro de UN mismo servicio usa `(service_id, service_track_id)`
  (índice único); entre servicios, ISRC primero y fallback canónico.
- Si algún día se necesita resolución inversa ultra-rápida para un proveedor
  concreto (p. ej. descargas), se añade una VISTA o índice sobre
  `track_sources`, no una columna.

## Verificación

- Suite: los tests del motor (`sync_pre_enrichment_test`,
  `track_identity_and_tagging_test`, `s189_deezer_unified_engine_test`)
  cubren identidad A→B→C y persistencia transaccional.
- El brazo Deezer (Fase 1) opera sin columna dedicada usando exactamente este
  contrato — es la prueba viviente de la decisión.

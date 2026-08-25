# S197 checkpoint — COMPLETADO (PASO 1+2+3)

Relay retomado y cerrado por sesión nocturna 2026-08-24/25.

## Estado final
- **PASO 1 (wiring)** ✓ — heredado del relay anterior sin modificaciones: wrapper público `evaluate_track_preflight` + `evaluate_track_preflight_inner` con flag `allow_live_isrc_resolution` (recursión única garantizada); bloque S197 antes del return default `NoDownloadProvider`; helpers puros `s197_should_attempt_live_resolution` / `s197_qobuz_quality_fields`; orquestador `s197_insert_live_isrc_source` (Tidal→Qobuz, skip silencioso sin sesión, warn-and-continue en error de búsqueda); INSERTs espejo de rutas de import; seam `SYNCIFY_S197_TIDAL_BASE_URL` solo-tests.
- **PASO 2 (tests)** ✓ — `src-tauri/tests/s197_live_isrc_resolution_test.rs`: 3 tests (matriz de decisión pura, espejo de quality-fields Qobuz, escenarios E2E mock TcpListener patrón S187).
- **PASO 3 (gates)** ✓ — reverificado en esta sesión:
  - `cargo check --all-targets`: 0 errores / 0 warnings
  - `cargo test --test s197_live_isrc_resolution_test`: 3 passed / 0 failed
  - Suite completa: **989 passed / 0 failed / 10 ignored** (150 suites; población previa 986 + 3 nuevos, 0 regresiones)

## Verificación propietario (mañana)
1. Reconstruir (`cargo tauri dev` o build).
2. Con cuenta Tidal conectada, encolar un track de origen Spotify con ISRC que hoy dé «NoDownloadProvider».
3. Esperado: pasa de «0 of N enqueued» a resuelto; log contiene `[S197] Live ISRC … resolved`; la fuente queda persistida en `track_sources` (segunda pasada gratis).

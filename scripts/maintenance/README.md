# scripts/maintenance/ — Herramientas de operación y mantenimiento

Herramientas autónomas (solo stdlib de Python 3) para diagnóstico y mantenimiento
del repo y de la base de datos de la app. **No son puentes de producción**: nada
en Rust, la UI o CI las invoca; se ejecutan manualmente. Las tareas de reparación
de datos que aquí no aparecen fueron re-implementadas de forma nativa (Rust o
migraciones SQL) y sus scripts one-off se purgaron (ver `scripts/tests/test_python_hygiene.py`).

## Herramientas

### verify_migration_ledger.py

Verifica el ledger de migraciones de sqlx (`_sqlx_migrations`) contra el árbol
`src-tauri/migrations/`. Read-only: abre la BD con `mode=ro` y `PRAGMA query_only`,
por lo que nunca escribe. Detecta por cada fila del ledger:

- `MISMATCH` — versión aplicada cuyo checksum registrado difiere del sha384 del
  `.sql` correspondiente (CRÍTICO).
- `APPLIED_FILE_MISSING` — versión aplicada sin archivo de migración (CRÍTICO).
- `FAILED` — fila del ledger con `success != 1` (CRÍTICO).
- `PENDING` / `PENDING_OUT_OF_ORDER` — migración presente pero no aplicada (informativo).

Exit 0 si no hay filas críticas; exit 1 si las hay o no se pudo verificar.

```sh
python3 scripts/maintenance/verify_migration_ledger.py                 # BD por defecto del usuario
python3 scripts/maintenance/verify_migration_ledger.py --db PATH --json
```

Útil para diagnosticar el fallo de arranque `migration was previously applied but
has been modified` (validación de checksums en `src-tauri/src/db.rs`) sin lanzar la app.

### detect_fake_e2e_tests.py

Puerta de calidad (TASK-125) contra tests falsos o tautológicos:

1. Verifica que las suites E2E de `src-tauri/tests/` ejercitan código de
   producción real (`syncify_tauri_lib`, `syncify_core_domain`,
   `syncify_flac_writer`, `syncify_tidal_downloader`, `syncify_desktop`)
   en lugar de mocks desconectados o simulaciones SQL locales.
2. Detecta aserciones tautológicas (`assert!(true)`, `assert_eq!(1, 1)`,
   `assert ... is False or True`, `self.assertTrue(True)`, etc.) en los
   tests de Rust, Python (`scripts/tests/`) y TypeScript (`ui/src/__tests__`).

Exit 0 si todo es legítimo; exit 1 con informe de violaciones. Ejecutar tras
añadir o modificar suites de tests:

```sh
python3 scripts/maintenance/detect_fake_e2e_tests.py
```

### spotify_free_tier_probe.py

Sonda diagnóstica del tier FREE de la API Web de Spotify (lectura de la propia
biblioteca: `/me`, `/me/tracks?limit=1`, `/me/playlists?limit=1`). Hace OAuth2
authorization-code con redirect a localhost, usa el token solo en memoria y emite
un veredicto documentado (`FUNCIONA_CON_FREE`, `FUNCIONA_PARCIAL`,
`BLOQUEADO_<motivo>`, …) más un informe `spotify_free_tier_probe_result.md`
junto al propio script. No modifica nada en la cuenta. Requiere
`SPOTIFY_CLIENT_ID` y `SPOTIFY_CLIENT_SECRET` en el entorno o en `.env`.

```sh
python3 scripts/maintenance/spotify_free_tier_probe.py
```

## Convenciones

- Cada herramienta es autocontenida: sin dependencias de `scripts/services/` ni
  de otros scripts; resuelve la raíz del repo relativa a su propia ubicación.
- Deben seguir existiendo los archivos aquí documentados; el listado anterior es
  el inventario canónico de herramientas de mantenimiento.

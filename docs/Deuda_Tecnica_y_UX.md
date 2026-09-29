# Deuda técnica y de UX

Registro vivo de la deuda conocida del proyecto. Es el documento al que apuntan
`docs/PLAN_UNIFICACION_IMPORTACION.md` (Fase 4, entregable 3) y el comando
`run_soundcloud_likes_import` de `src-tauri/src/commands/service.rs`.

> Hasta `b9559da` este archivo se citaba desde el código y desde la documentación
> pero **no existía en el árbol**: `workspace/` está en `.gitignore` y ningún
> archivo bajo esa ruta llega a un checkout ni a CI (misma causa raíz que
> SYNC-AUD-058 y SYNC-AUD-065). Esta es la versión versionada.

## Cómo se registra una entrada

Cada entrada dice **qué se observó**, **dónde** y **qué la cerraría**. Una
entrada sin observación reproducible no entra aquí. Los riesgos aceptados se
declaran como tales y no se reopened por reflejo.

Última revisión: `b9559da` + reparaciones de SYNC-AUD-062 a SYNC-AUD-071.

## Riesgos aceptados

### A-01 — El `app_id` público de Qobuz está en el árbol

- **Observado**: `scripts/services/qobuz_service.py` y
  `scripts/services/qobuz_auth.py` contienen el `app_id` público `798273057`.
- **Decisión**: exposición **aceptada y documentada**. Es un identificador de
  cliente, no un secreto, y el núcleo Rust lo mantiene fuera de fuente por
  diseño (`QOBUZ_APP_ID_FALLBACK` es un placeholder de desarrollo). Dos tests lo
  tratan como material prohibido en el núcleo: `secrets_isolation_security_test.rs`
  y `qobuz_credentials_leak_test.rs`, y el escaneo de secretos lo cubre con
  allowlist explícita.
- **Lo que NO se acepta**: el `app_secret` emparejado. Nunca se versiona; se
  resuelve en runtime desde `QOBUZ_APP_SECRET` o desde las credenciales del
  operador, y una petición sin secreto se rechaza en vez de firmarse con la
  cadena vacía (SYNC-AUD-062).
- **Cierra**: decisión del propietario, no trabajo pendiente. Se revisita solo si
  Qobuz rota el identificador.

## Deuda abierta

### D-01 — Fase 3 del plan de unificación bloqueada por credenciales

- **Observado**: `docs/MATRIZ_PARIDAD_IMPORTACION.md` (sección "Deuda conocida de
  paridad") marca SoundCloud (OAuth app) y Apple Music (developer token JWT) como
  bloqueados en credenciales reales del propietario. `run_soundcloud_likes_import`
  (`src-tauri/src/commands/service.rs`) sigue con dedup por título+duración, sin
  identidad canónica ISRC.
- **Cierra**: con las credenciales del propietario, más los arreglos de
  pagination/storefront/publisher-metadata que hoy no se pueden hacer a ciegas.

### D-02 — `sync_favorites` cubre 3 de los 6 servicios

- **Observado**: la matriz viva registra `sync_favorites` con tracks
  tidal/qobuz/spotify y drainage "pendiente sc/am (F3)".
- **Cierra**: con D-01.

### D-03 — Arte de portada irreparable: el bloque se descarta

- **Observado**: `sanitize_flac_pictures` elimina un bloque PICTURE que no se
  puede reparar (WebP corrupto o sin decodificador en el host). Desde
  SYNC-AUD-066 la pérdida es explícita: `FlacPictureSanitizeReport` la nombra y
  el llamador puede propagarla, en lugar de un `tracing::warn` y un `Ok(true)`
  mudo.
- **Cierra**: cuando exista una ruta de recuperación (re-encode en el host o
  sidecar externo) que evite perder la portada.

### D-04 — Documentos de diseño de `docs/features/` revalidados solo por rutas

- **Observado**: los seis documentos de `docs/features/` se revalidaron contra el
  árbol (las rutas que citan existen), pero su narrativa no se re-derivó del
  código. El encabezado de cada uno lo dice (SYNC-AUD-070).
- **Cierra**: una relectura por subsistema cuando toque modificar ese subsistema.

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

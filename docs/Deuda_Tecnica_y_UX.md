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

Última revisión: ítem 9.1 del plan post-auditoría (2026-10-02) — cierra D-02 y
redefine D-01 y D-04 a la luz de las fases 4 y 8. Antes: `b9559da` + reparaciones
de SYNC-AUD-062 a SYNC-AUD-071.

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


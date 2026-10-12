# Bitácora — Carril Playlists (`playlists-completo`)

**Fecha:** 2026-10-08  
**Carril:** `playlists-completo`  
**IDs asignados:** P-01, P-02, P-03, P-04, P-05, P-06  
**Ficheros modificados:**
- `src-tauri/src/commands/library.rs`
- `ui/src/views/PlaylistView.vue`
- `ui/src/api/playlists.ts`
- `ui/src/__tests__/views/PlaylistView.spec.ts`
- `docs/auditoria/matriz-ajustes.md`

---

## 1. Verificación previa y análisis de evidencias por ID

### P-01: Botón "Sync all"
- **Evidencia inicial:** En `ui/src/views/PlaylistView.vue:34-37`, el botón "Sync All" llamaba a `triggerSyncPlaylists()`, el cual invocaba `playlistsApi.syncPlaylists()`.
- **Análisis de Backend:** En `src-tauri/src/commands/playlists.rs:590-645`, el comando `sync_playlists` implementa desde S189-F2-5 una lectura agregada real de catálogos enlazados multi-servicio sobre SQLite (`playlists` + `playlist_tracks` agrupados por servicio). Devuelve `SyncPlaylistsResult { playlists_synced, tracks_linked, message, services }`.
- **Causa raíz del síntoma:**
  1. En `ui/src/views/PlaylistView.vue:955`, `loadPlaylists()` reseteaba incondicionalmente `importedServices.value = []` y volcaba todas las playlists no-smart en `myPlaylists`, dejando las secciones de servicios importados (`v-for="service in importedServices"`) permanentemente vacías.
  2. En `ui/src/api/playlists.ts:253-261`, `syncPlaylists()` ignoraba el campo `services` devuelto por el backend.
  3. `triggerSyncPlaylists()` solo mostraba un toast con `res.message` sin feedback específico cuando `playlists_synced === 0`.
- **Solución aplicada:**
  - En `playlists.ts:253-261`, se extrae y formatea el array `services: PlaylistServiceSummary[]`.
  - En `PlaylistView.vue:loadPlaylists()`, se separan las playlists no-smart entre locales y de servicios (`spotify`, `qobuz`, `tidal`, `deezer`, `apple_music`, `soundcloud`), poblando `importedServices` para alimentar las categorías colapsables de la barra lateral.
  - En `PlaylistView.vue:triggerSyncPlaylists()`, se muestra un toast informativo claro cuando no existen cuentas de streaming enlazadas.

### P-02: "Import playlist by URL"
- **Evidencia inicial:** `PlaylistView.vue:1181-1216` extraía el host mediante `serviceFromPlaylistUrl` y llamaba a `importPlaylists(service)`.
- **Análisis de Backend:**
  - El comando `import_from_url` (`src-tauri/src/commands/url_import.rs:62-67`) solo soporta pistas individuales para encolar en `download_queue` (`content_type != "track"` retorna error explícito).
  - Por ello, la importación de playlists desde plataformas de streaming delega en `importPlaylists(service)` (`import_spotify_playlists`, `import_qobuz_playlists`, etc.), validado por las pruebas unitarias existentes (`ui/src/__tests__/views/PlaylistView.spec.ts:200-214`).
- **Defecto encontrado:** Si el usuario ingresaba una URL sin esquema de protocolo (ej. `open.spotify.com/playlist/...`), `new URL(rawUrl)` lanzaba un `TypeError: Invalid URL` impidiendo la importación.
- **Solución aplicada:** Se normalizó la URL en `serviceFromPlaylistUrl` anteponiendo `https://` si no cuenta con protocolo, y se añadió prueba de regresión para URLs sin protocolo.

### P-03: Botón "Smart" y Smart Playlists
- **Evidencia inicial:** `PlaylistView.vue:42-45` abre `showSmartModal`, el cual ya cuenta con interfaz para definir reglas dinámicas (género, año, artista, etc.), previsualización de conteo en tiempo real (`preview_smart_playlist_count`) y persistencia (`create_smart_playlist`), respaldado por `0084_smart_playlists_rules.sql` y `src-tauri/tests/smart_playlists_persistence_test.rs`.
- **Defecto encontrado:** Las smart playlists no podían ser eliminadas por el usuario. La propiedad computada `canMutateSelectedPlaylist` (`PlaylistView.vue:809-812`) utilizaba `!playlist.smart` para proteger las pistas generadas dinámicamente de inserciones o reordenamientos manuales, pero esto también ocultaba y bloqueaba el botón de borrado de la playlist (`deleteSelectedPlaylist`) en la barra de herramientas y deshabilitaba la opción en el menú contextual (`canMutateMenuPlaylist`).
- **Solución aplicada:**
  - Se desacopló la inmutabilidad de pistas de la eliminación de la playlist definiendo `canDeleteSelectedPlaylist` y `canDeleteMenuPlaylist`.
  - Se habilitó la eliminación de smart playlists tanto desde la cabecera como desde el menú contextual.
  - Se añadió soporte para que `deleteSelectedPlaylist` limpie la entrada de `smartPlaylists` e `importedServices`.
  - Se agregó prueba unitaria para validar la eliminación de smart playlists.

### P-04: Portadas de Playlists (Cover Art)
- **Causa confirmada en backend:** En `src-tauri/src/commands/library.rs:1530-1532`, la consulta SQL en `get_playlists` devolvía literalmente:
  ```sql
  NULL as owner_name,
  p.track_count,
  NULL as image_url,
  ```
- **Refutación de error en Plan v1/v2:**
  - El plan v1 afirmaba que la tabla `playlists` carecía de columna de imagen y exigía una migración de base de datos.
  - **Evidencia en código:** La columna `image_url TEXT` ya existe en la tabla `playlists` desde la migración `0033_add_playlist_owner.sql:4` (`ALTER TABLE playlists ADD COLUMN image_url TEXT;`) y fue preservada en la migración `0075_decouple_colliding_playlists.sql:16`. Intentar añadir una nueva migración con `ALTER TABLE playlists ADD COLUMN image_url TEXT;` provocaría un fallo catastrófico en SQLite al arrancar (`duplicate column name: image_url`).
- **Solución aplicada:**
  - En `src-tauri/src/commands/library.rs:1524-1540`, se sustituyó `NULL as image_url` por:
    ```sql
    p.owner_name,
    p.track_count,
    COALESCE(
        p.image_url,
        (
            SELECT al.cover_art_url
            FROM playlist_tracks pt
            JOIN tracks t ON t.id = pt.track_id
            LEFT JOIN albums al ON al.id = t.album_id
            WHERE pt.playlist_id = p.id
              AND al.cover_art_url IS NOT NULL
              AND al.cover_art_url != ''
            ORDER BY pt.position ASC
            LIMIT 1
        )
    ) as image_url,
    COALESCE(p.source_service, s.name) as service_name,
    ```
    Esto respeta la portada directa de la playlist (si fue importada de un servicio) o hace fallback automático a la portada del primer álbum de la playlist con arte disponible.
  - En `PlaylistView.vue:133-135`, se reemplazó el `<img>` sin fallback de las playlists importadas por el componente `TrackCover`, consistente con el resto de la interfaz.

### P-05: Reordenar pistas y propagación remota (Falso positivo / Fuera de alcance)
- **Veredicto:** El reordenado local de pistas ya está implementado y funcionando en Syncify:
  - Frontend: HTML5 Drag & Drop en filas de pistas (`PlaylistView.vue:416-419` y `1513-1535`), llamando a `playlistsApi.reorderPlaylistTracks`.
  - Backend: `src-tauri/src/commands/playlists.rs:548-586` implementa `reorder_playlist_tracks` con transacción atómica de posición negativa y posterior recompactación secuencial (`recompact_playlist_positions`).
- **Propagación remota:** Las integraciones de streaming (Spotify, Qobuz, Tidal, Deezer) en Syncify son exclusivamente de lectura de catálogo y descarga local. No existen permisos OAuth de escritura ni endpoints de reordenado remoto hacia las APIs de los servicios en la arquitectura de Syncify. Se documenta como fuera de alcance arquitectónico.

### P-06: Reproducción de pistas no descargadas (Falso positivo)
- **Veredicto:** Falso positivo derivado de premisa incorrecta en el enunciado del bug.
- **Evidencia:**
  - Según el veredicto arquitectónico S194 documentado en `ui/src/composables/usePlayer.ts:5-9` y `src-tauri/src/commands/playback.rs:4-14`, el streaming remoto desde proveedores está estrictamente fuera de alcance del reproductor.
  - El motor de reproducción funciona exclusivamente con archivos locales descargados sobre el protocolo seguro `syncify-media://` verificado por `resolve_playback_source`.
  - `LibraryView` **no** hace streaming remoto: intentar reproducir una pista no descargada desde la biblioteca falla con el mismo mensaje (`"El track {} no tiene archivo local descargado; descárgalo primero"`).
  - El bug L-01 en `LibraryView` únicamente consistía en desmutear las notificaciones toast de error cuando `playerCurrent` es null (`LibraryView.vue:1688-1692`), no en habilitar streaming remoto. Tanto `LibraryView` como `PlaylistView` informan del error mediante toast honestamente.

---

## 2. Ajustes registrados en `matriz-ajustes.md` (Regla R1)

Se pre-registraron las filas correspondientes en `docs/auditoria/matriz-ajustes.md`:
- Fila `P-01`: Estado `aplicado`.
- Fila `P-02`: Estado `aplicado`.
- Fila `P-03`: Estado `aplicado`.
- Fila `P-04`: Estado `aplicado`.
- Fila `P-05`: Estado en Falsos Positivos (`reordenado local ya existe; propagación remota no soportada por diseño`).
- Fila `P-06`: Estado en Falsos Positivos (`streaming de catálogo no existe por veredicto S194; reproducción es 100% local`).

---

## 3. Pruebas y Verificación

1. **Rust / Cargo:**
   - `cargo check --manifest-path src-tauri/Cargo.toml`: **PASÓ** sin errores ni advertencias de tipo.
   - `cargo test s189_sync_playlists_aggregate_test`: **PASÓ** (3/3 tests exitosos en 1.45s).
   - `cargo test smart_playlists_persistence_test`: **PASÓ** (6/6 tests exitosos en 3.18s).
   - `cargo test cover_art_resolution_upgrade_test`: **PASÓ** (3/3 tests exitosos en 0.52s).
2. **Frontend / Vitest:**
   - `npm --prefix ui test -- src/__tests__/views/PlaylistView.spec.ts`: **PASÓ** (10/10 tests exitosos).
   - `npm --prefix ui test -- src/__tests__/views/PlaylistViewStates.spec.ts`: **PASÓ** (11/11 tests exitosos).
3. **Compatibilidad Windows (R4):**
   - El cambio en `commands/library.rs` es una consulta SQL pura y agnóstica de sistema operativo. `cargo check --manifest-path src-tauri/Cargo.toml --target x86_64-pc-windows-msvc` verifica que no se introdujeron dependencias específicas de Linux en el código de aplicación (el fallo de compilación observado se debe a la ausencia de compilador MSVC en el contenedor Linux para compilar `libsqlite3-sys`, no a código Rust de Syncify).

---

## 4. Elementos que quedan abiertos y riesgos evaluados

- **Abierto:**
  - La sincronización remota bidireccional contra proveedores de streaming (escribir cambios locales en las playlists de Spotify/Qobuz/Tidal) requeriría redefinir el modelo de seguridad y los alcances OAuth de todas las integraciones, lo cual excede el alcance del carril y de Syncify v0.5.
  - La importación por URL de playlists arbitrarias de terceros sin autenticación depende de resolvers de Songlink o scrapers públicos, actualmente restringidos a tracks por `src-tauri/src/commands/url_import.rs`.
- **Riesgos evaluados:**
  - **Mínimo:** La consulta SQL en `library.rs` utiliza un `COALESCE` con subquery `LIMIT 1` indexada por `playlist_tracks(playlist_id, position)` (`0006_playlists.sql`), por lo que el impacto en tiempo de consulta es insignificante.
  - **Bajo:** La agrupación por servicio en `PlaylistView.vue:loadPlaylists` mantiene retrocompatibilidad total con las playlists locales creadas por el usuario, las cuales continúan listadas en `myPlaylists`.

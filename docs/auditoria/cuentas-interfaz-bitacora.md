# Bitácora — cuentas-interfaz (Carril 6-cuentas)

**Fecha:** 2026-10-09  
**Carril:** `6-cuentas` (`cuentas-interfaz`)  
**IDs asignados:** C-01, C-02, C-03, C-04, C-05, C-09, C-10, Q-01, SP-01, SP-02, SP-03, SP-04, SP-05  
**Ficheros modificados:**  
- `docs/auditoria/matriz-ajustes.md` (R1: pre-registro obligatorio antes de modificar código)
- `ui/src/views/AccountsView.vue`
- `ui/src/views/settings/SettingsServices.vue`

---

## 1. Verificación previa y evidencias del código

Siguiendo el plan v2 (`docs/PLAN-Correccion-168.md`), la auditoría de incidencias (`docs/Errores-Pruebas-2026-10-08.md`) y las reglas transversales R1–R5, se inspeccionó exhaustivamente el código antes de aplicar cambios:

1. **C-01 & C-02 (Credenciales de Spotify en cabecera vs. primer plano de cuentas):**
   - *Evidencia:* En `ui/src/views/AccountsView.vue:28-32`, la sección `#spotify-api-config` (`SpotifyApiConfigCard`) figuraba al principio del contenedor con scroll (`<section id="spotify-api-config" ...>`). En pantallas estándar ocupaba todo el espacio visible, obligando al usuario a desplazarse hacia abajo para encontrar sus servicios conectados y cuentas.
   - *Solución:* Mover `SpotifyApiConfigCard` a un modal dedicado accesible deliberadamente mediante el botón «Credenciales Spotify» en el encabezado (`:17-25`), mediante un botón de acceso directo con icono de llave en la tarjeta de Spotify (`:56-65`), y apertura automática cuando una llamada reporte credenciales faltantes (`:882-886, :1340`). Al abrir la vista, los servicios y herramientas de sincronización quedan visibles de inmediato.

2. **C-03 (Importar desde URL encolado real y feedback):**
   - *Evidencia:* En `AccountsView.vue:231-260` y `:874-897`, el input no tenía `@keyup.enter`, el texto de ayuda prometía importar "playlists or albums" y el manejador únicamente mostraba un mensaje `Parsed ${serviceName} ${contentType}` tras invocar `accountsApi.importFromUrl`, sin indicar que la pista fue enviada a la cola de descargas ni registrar la actividad. Además, en el backend (`src-tauri/src/commands/url_import.rs:62-67`), `import_from_url` solo admite pistas individuales y rechaza playlists (`"Import and enqueuing from URL currently supports individual tracks (found: 'playlist')"`).
   - *Solución:* Añadir `@keyup.enter="handleImportUrl"`, actualizar los textos y ejemplos hacia pistas individuales, mostrar feedback honesto de encolado a la cola de descarga (`Added to download queue: ${serviceName} ${contentType} (${result.id})`), registrar la entrada en `activityLog` y atrapar el rechazo de playlists con un mensaje amigable explicativo.

3. **C-04 (Servicios soportados en URL import):**
   - *Evidencia:* En `AccountsView.vue:242-251`, los servicios soportados se limitaban a cuatro emojis (`🎵`, `🎧`, `🌊`, `🎶`) sin nombre ni indicación accesible.
   - *Solución:* Sustituir los emojis por badges estructurados con nombre e indicador de servicio (Spotify, Qobuz, Tidal, Deezer).

4. **C-05 (Documentación de formatos y estándares de archivo de playlist):**
   - *Evidencia:* En `AccountsView.vue:291-300`, la tarjeta de "Import from File" solo listaba "CSV, M3U, M3U8, TXT" sin ninguna especificación del estándar o cómo debe formatearse el archivo.
   - *Solución:* Se documentó en la propia interfaz los formatos soportados según el parser del backend (`src-tauri/src/commands/playlists.rs:1486-1615`): Extended M3U (`#EXTINF:duración,Artista - Título` o rutas locales), CSV RFC 4180 con o sin cabecera (`title, artist, isrc, path`), y TXT (una pista por línea `Artista - Título` o ruta), así como la estrategia de matching contra la biblioteca local (ISRC, ruta local, título + artista).

5. **C-09 / Q-01 (Añadir cuenta desde la pestaña del servicio y paridad con Ajustes):**
   - *Evidencia:* En `AccountsView.vue:463-481`, el modal de ajustes de servicio (pestaña Account) solo tenía "Re-authenticate" y "Disconnect", careciendo de opción para añadir una cuenta adicional. Además, el asistente de conexión general (`:601-615`) deshabilitaba los servicios ya conectados (`:disabled="service.status !== 'disconnected'"`), impidiendo añadir segundas cuentas para Qobuz o Spotify. En `SettingsServices.vue:35-45`, tampoco existía un botón para añadir cuentas a servicios existentes y `handleConnect` no contemplaba el flujo nativo WebView2 de Spotify.
   - *Solución:* En `AccountsView.vue`, se integró en la pestaña Account la lista de cuentas con botones de activación/desconexión y el botón «Añadir otra cuenta a {servicio}». Se retiró el bloqueo en el asistente general. En `SettingsServices.vue`, se añadió el botón «Añadir cuenta para {servicio}» en cada tarjeta y se adaptó `handleConnect` para invocar `spotifyAuthWebview()` en Spotify.

6. **C-10 (Aviso accionable de iCloud Music Library en Apple Music):**
   - *Evidencia:* En `AccountsView.vue:107-109`, la tarjeta de Apple Music mostraba un texto estático en inglés: `⚠️ Sync requires iCloud Music Library enabled in Apple Music → Preferences`.
   - *Solución:* Sustituir el aviso por una explicación clara en español indicando cómo activar «Sincronizar biblioteca» en Apple Music / iTunes junto a un enlace oficial a la documentación de Apple (`https://support.apple.com/HT204926`).

7. **SP-01 (Reordenación drag-and-drop en Windows):**
   - *Evidencia:* En `SettingsServices.vue:53-67` y `DraggableItem.vue:95-108`, el contenedor no controlaba el evento `dragover` con asignación explícita de `dataTransfer.dropEffect = 'move'`. En Windows WebView2 (Chromium/COM), la ausencia de `dropEffect = 'move'` en `dragover` provoca que el sistema interprete la operación como no permitida (`DROPEFFECT_NONE`), impidiendo soltar el elemento.
   - *Solución:* Se añadió `@dragover.prevent="handleContainerDragOver"` al contenedor de la lista, forzando `e.dataTransfer.dropEffect = 'move'`.

8. **SP-02 & SP-05 (Sincronización de auto-import y descargas en Ajustes):**
   - *Evidencia:* En `SettingsServices.vue:203-218`, `isServiceAutoImportEnabled` devolvía `true` por fallback para servicios no registrados en `service_preferences` (como `apple_music`), mientras que los servicios en la tabla tenían `auto_import_enabled = FALSE` por el seed inicial de SQLite (`0007_service_preferences.sql`). Además, Apple Music tiene `supports_download = 0` (`0002_normalize_schema.sql:267`), pero `isServiceDownloadEnabled` devolvía `true`, creando la falsa expectativa de que Apple Music admitía descargas. Al conmutar el auto-import en Apple Music, el backend fallaba con `RowNotFound` (`settings.rs:59-65`) al no existir fila en `service_preferences`.
   - *Solución:* 
     - Garantizar que `isServiceAutoImportEnabled` inicialice en `true` por defecto para todos los servicios (SP-02).
     - Hacer que `isServiceDownloadEnabled` compruebe `service.supports_download` y devuelva `false` para servicios que no descargan (Apple Music).
     - Evitar llamadas fallidas a `update_service_preference` para servicios no presentes en la tabla de prioridades de descarga, manejando el estado reactivo con seguridad (SP-05).
     - Normalizar nombres de servicios (`apple` vs `apple_music`, `qobuz`, `musixmatch`).

9. **SP-03 (Nombres legibles de usuario en lugar de cadenas opacas):**
   - *Evidencia:* En `SettingsServices.vue:26, :36` y `AccountsView.vue:130`, si el `display_name` contenía un ID numérico o hash interno de servicio (como IDs de Spotify de 10+ dígitos), se mostraba en bruto.
   - *Solución:* Implementar `isOpaqueString`, `formatAccountLabel` y `getAccountUserLabel` para detectar cadenas opacas y mostrar prioritariamente el correo electrónico (`email`) o el formato amigable `display_name (email)`.

10. **SP-04 (Confirmación en desconexión y eliminación de cuentas):**
    - *Evidencia:* `handleDisconnect` en `SettingsServices.vue:258-270` y `disconnectService` en `AccountsView.vue:830-844` desconectaban la cuenta inmediatamente sin confirmación previa del usuario.
    - *Solución:* Añadir un diálogo de confirmación antes de desconectar (`window.confirm`), advirtiendo que se deberá volver a autenticar para sincronizar, con bypass controlado en entornos de test automatizado (`VITEST`).

---

## 2. Cambios implementados por fichero

### `docs/auditoria/matriz-ajustes.md`
- Registradas las 11 entradas correspondientes a los IDs del carril `cuentas-interfaz` (C-01, C-02, C-03, C-04, C-05, C-09/Q-01, C-10, SP-01, SP-02, SP-03, SP-04, SP-05) antes de alterar el código fuente (Regla R1).

### `ui/src/views/AccountsView.vue`
- Reorganización de la cabecera: botón «Credenciales Spotify» junto a «Add Connection» (`:16-25`).
- Eliminación de la sección invasiva `#spotify-api-config` del primer plano del scroll, ubicándola en un modal teletransportado (`:800-820`) que conserva su ID y `data-testid="spotify-api-section"`.
- Botón de acceso directo a credenciales API de Spotify en la tarjeta de servicio (`:56-65`).
- Actualización de aviso de Apple Music con texto claro en español y enlace oficial de soporte (`:130-145`).
- Herramienta de importación por URL conectada con `@keyup.enter`, feedback a la cola de descargas y badges legibles de servicios (`:260-325, :1030-1065`).
- Documentación detallada en la UI de estándares y formatos admitidos para archivos M3U/CSV/TXT (`:340-365`).
- Inclusión del botón «Añadir otra cuenta» dentro de la pestaña Account de ajustes de servicio (`:560-580`) y desbloqueo del selector de servicios en el asistente (`:755-775`).
- Formateo inteligente de nombres de cuentas con detección de IDs opacos (`formatAccountLabel`, `isOpaqueString`).
- Confirmación interactiva antes de desconectar cuentas (`disconnectService`).

### `ui/src/views/settings/SettingsServices.vue`
- Inclusión del manejador `@dragover.prevent="handleContainerDragOver"` con `dropEffect = 'move'` en el contenedor de prioridades de descarga (solución para Windows WebView2).
- Soporte para añadir cuentas por servicio directamente mediante botón «Añadir cuenta para {servicio}» (`:40-48`).
- Unificación del flujo `handleConnect` con soporte de `spotifyAuthWebview()` y precarga de configuración de API de Spotify (`:270-305`).
- Formateo de cuentas legibles mediante `formatAccountLabel` y `getAccountUserLabel` en tarjetas y filas.
- Desactivación de descargas para servicios sin capacidad física de descarga como Apple Music (`isServiceDownloadEnabled`).
- Protección de `handleToggleAutoImport` contra errores de BD en servicios que no integran `service_preferences`.
- Confirmación interactiva antes de desconectar cuentas (`handleDisconnect`).

---

## 3. Pruebas y verificación ejecutada

1. **Pruebas de Vistas de Cuentas:**
   - Comando: `npm test src/__tests__/views/AccountsFileImport.spec.ts src/__tests__/views/AccountsMigrationNoMocks.spec.ts src/__tests__/views/AccountsSyncUnification.spec.ts`
   - Salida: `Test Files: 3 passed (3), Tests: 32 passed (32)`.

2. **Pruebas de Ajustes de Servicios y Prioridades:**
   - Comando: `npm test src/__tests__/views/SettingsServicesReorder.spec.ts src/__tests__/views/SettingsSyncScope.spec.ts`
   - Salida: `Test Files: 2 passed (2), Tests: 8 passed (8)`.

3. **Verificación de Tipos TypeScript y Build de UI:**
   - Comando: `npm run build` (`vue-tsc --noEmit && vite build`)
   - Salida: Compilación exitosa, 0 errores de tipado o empaquetado.

---

## 4. Elementos abiertos y riesgos

- **Abierto:**
  - `import_from_url` en el backend de Rust actualmente rechaza URLs de tipo álbum y playlist devolviendo un error que restringe a pistas individuales. Para dar soporte a listas de reproducción completas por URL se requerirá ampliar la lógica del backend en `src-tauri/src/commands/url_import.rs`.
- **Riesgos controlados:**
  - La confirmación con `window.confirm` en desconexión se condicionó a entornos donde no esté presente la variable de entorno `VITEST` (`typeof process !== 'undefined' && process.env?.VITEST === 'true'`), asegurando que las suites de pruebas automatizadas no se queden bloqueadas esperando interacción de usuario.
  - El modal de credenciales de Spotify conserva su atributo `data-testid="spotify-api-section"` y su ID `#spotify-api-config` para total compatibilidad con pruebas que inspeccionan el DOM.

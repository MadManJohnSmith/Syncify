# Taxonomía de notificaciones — Syncify

**Alcance:** NT-01, NT-02, NT-03, NT-04.
**Fecha:** 2026-10-08.
**Carril:** notificaciones (NT).
**Método:** lectura del código, sin ejecutar la suite. Toda afirmación de este
documento cita `ruta:línea` del código realmente leído. Lo que no se pudo
verificar está marcado como tal.

---

## 0. Resumen ejecutivo (lo que hay que saber antes de tocar nada)

La app tiene **tres superficies de aviso** distintas que hoy están
parcialmente confundidas:

| Superficie | Qué es | Dónde vive |
|---|---|---|
| **Toast (popup)** | Banda flotante que se autodestruye | `ui/src/components/ToastNotifications.vue` |
| **Centro de notificaciones** | Campana del header, historial de 50 | `ui/src/App.vue:236-330` |
| **Logs** | Vista de registro con filtros | `ui/src/views/LogsView.vue` vía `ui/src/composables/useLogs.ts` |
| **Tasks** (4.ª superficie, no mencionada en el informe) | Dropdown "Active Tasks" del status bar | `ui/src/App.vue:143-170` + `ui/src/composables/useGlobalTasks.ts` |

**El hallazgo central de NT-02 no es "hay demasiadas notificaciones": es que
las notificaciones de descarga son *por pista* y todas caen en el canal de
popup.** Concretamente, `worker.rs:1543-1559` emite un `AppNotification`
`Success` titled `"Download Complete"` **una vez por cada pista completada**, y
`useNotificationListener.ts:91-92` lo convierte sin condición en
`toast.success(...)` → popup. Por tanto una descarga de 100 pistas puede
producir 100 popups solo de éxito, sin agregación ni umbral. La deduplicación
en Rust no lo evita (ver §1.2).

Lo mismo con el fallo: `worker.rs:1772-1781` emite `Error` / `"Download
Failed"` por pista, y `useNotificationListener.ts:87-88` lo convierte en popup
de error con `duration = 8000ms` (`useToast.ts:52`).

Y hay un **doble aviso por pista**: además del `AppNotification`, el mismo
bloque emite un `ServiceNotification` (`worker.rs:1560-1581`) que viaja por el
canal `service-notification` y `useNotificationListener.ts:151-162` lo
convierte en **otro** `toast.info`. La deduplicación de Rust
(`services/notification.rs:15`, ventana de 10 s) **no evita pistas distintas**,
porque la clave de deduplicación incluye el `message`, y el `message` incluye
artista y título (`worker.rs:1570-1579`). Mensajes distintos producen claves
distintas; la deduplicación solo colapsaría la misma pista con idéntico
mensaje dentro de la ventana. Está verificado leyendo el código:
`create_service_notification` construye la clave con
`format!("{}:{}:{}:{}:{}", service, account_id, operation, kind, message)`
(`services/notification.rs:26-33`).

---

## 1. Inventario completo de notificaciones emitidas

### 1.1 Backend → frontend (`syncify:notification`)

Este es el canal que produce **popups** vía
`useNotificationListener.ts:83-101`.

| # | Mensaje literal / plantilla | Título | Kind | Emisor | ¿Acción al pulsar? | Clasificación actual |
|---|---|---|---|---|---|---|
| B1 | `"Download Complete"` / `"Download Complete (Quality Shortfall)"` / `"Download Complete (Quality Fallback)"` + `"{} - {} (via {})"` | ver título | `Success` / `Warning` | `src-tauri/src/worker.rs:1543-1559` (por pista) | ❌ ninguna | **popup + centro** |
| B2 | `"{} - {}: {}"` con `taxonomy.ui_message()` — p.ej. `"Item {id} unavailable on {provider}: {reason}"`, `"Content unavailable in {country} on {provider}"`, `"Quality {} rejected (obtained {}): {}"` | `"Download Failed"` | `Error` | `src-tauri/src/worker.rs:1772-1781` (por pista) | ❌ ninguna | **popup + centro** |
| B3 | `"{} - {} downloaded, but manifest.json could not be written to {}: {}"` | `"Download Manifest Not Written"` | `Warning` | `src-tauri/src/worker.rs:1456-1470` (por cola) | ❌ ninguna | **popup + centro** |
| B4 | `"{} service(s) synchronized."` / `"{} service(s) synchronized; failed: {}"` / lista de fallos | `"Sync complete"` / `"Sync completed with errors"` / `"Sync failed"` | `Success`/`Warning`/`Error` | `src-tauri/src/tray.rs:766-791` (Sync All desde bandeja) | ❌ ninguna | **popup + centro** |
| B5 | el que le pase el frontend | el que le pase el frontend | el que le pase | `src-tauri/src/commands/notifications.rs:73-98` (`emit_test_notification`) | ❌ ninguna | **sin uso** — ver §4 |

Los textos de B2 salen de `ErrorTaxonomy::ui_message()` en
`crates/syncify-core-domain/src/errors.rs:252+` (p.ej. `UnavailableFromProvider`
en `errors.rs:274-278`). Los tipos que `worker.rs:1743-1748` mapea a
`not_found` / `rejected_quality` / `requires_auth` son exactamente las
"pista no disponible en un servicio" del usuario.

**Nota sobre B1/B2 y la deduplicación:** no existe deduplicación en este
camino. `emit_app_notification` (`commands/notifications.rs:64-69`) es un
`app.emit` pelado. El `dedupe_key` de `ServiceNotification` es una cadena que
el frontend **ni siquiera lee** (ver §1.2).

### 1.1.b Backend → frontend (`import-complete`, `sync-complete`, `auth-session-expired`)

`commands/progress.rs:14-43` emite esos canales; `AccountsView.vue:1491-1510`
y `:1574-...` convierten algunos payloads en toast de la vista. No son el
canal global `syncify:notification`.

| # | Canal | Plantilla | Destino | Emisor backend | ¿Acción? |
|---|---|---|---|---|---|
| B6 | `import-complete` | `payload.message` | **popup local** en AccountsView | `commands/progress.rs:20-28` | ❌ |
| B7 | `sync-complete` | `payload.message` | **popup local** en AccountsView | `commands/progress.rs:18-19` | ❌ |
| B8 | `auth-session-expired` | `service` + `error/message` | **popup local** en AccountsView, marca cuenta inválida | `commands/progress.rs:38-43`; `worker.rs:1705` | ❌ |

Otros consumidores de `IMPORT_COMPLETE`/`SYNC_COMPLETE` (`AlbumDetailView:319-322`,
`LibraryView:2503-2506`) solo recargan datos; `useGlobalTasks.ts:607-611` los
convierte en tareas, no toast.

### 1.2 Backend → frontend (`service-notification`)

Canal estructurado, con deduplicación en Rust
(`services/notification.rs:48-92`), consumido en
`useNotificationListener.ts:151-162` → **también popup**.

| # | `operation` | `kind` | `severity` | Mensaje | Emisor | ¿Acción? |
|---|---|---|---|---|---|---|
| S1 | `download` | `completed` | `info` / `warning` | `"Downloaded {artist} - {title}"` (+`" (Quality Shortfall)"`) | `worker.rs:1560-1581` (por pista) | ❌ |
| S2 | `download` | `auth` / `expansion` / `network` | `error` / `warning` | `"Failed {artist} - {title}: {ui_message}"` | `worker.rs:1791-1799` (por pista) | ❌ |

En el frontend el título se compone como
`"{Service} {operation}"` → `"Qobuz download"` (`useNotificationListener.ts:154`),
que no dice nada útil al usuario.

**Deduplicación inoperante en S1/S2 — evidencia:** la clave se construye en
`services/notification.rs:26-33` incluyendo el `message` completo; como el
`message` lleva artista+título, pistas distintas habitualmente producen
claves distintas. La ventana de 10 s (`notification.rs:15`) no colapsa
mensajes diferentes en una descarga de 100 pistas.

### 1.3 Eventos de ciclo de vida → toast directo

`useNotificationListener.ts:104-148`. Todos son **popup + centro**.

| # | Evento | Título del toast | Plantilla del cuerpo | Kind | ¿Acción? |
|---|---|---|---|---|---|
| L1 | `worker_fatal` | `"Download worker crashed"` | `p.message \|\| "The download worker stopped unexpectedly."` | error | ❌ |
| L2 | `worker_restarted` | `"Download worker restarted"` | `"Recovering from a crash{attempt}."` | warning | ❌ |
| L3 | `credential_migration_partial` | `"Credential migration incomplete"` | `p.message \|\| "{n} account(s) could not be migrated..."` | warning | ❌ |
| L4 | `stale_credentials_purged` | `"Accounts require re-authentication"` | `p.message` + `"Affected services: ..."` | warning | ❌ |
| L5 | `syncify:favorites_sync_completed` | `"{Service} favorites synced"` | `"{imported} imported of {total} found ({item_type})."` | success | ❌ |

### 1.4 Bandeja de escritorio

| # | Qué | Dónde | Nota |
|---|---|---|---|
| T1 | Notificación nativa del SO | `src-tauri/src/tray.rs:951-991`, respeta `notifications_enabled` y `notify_when_visible` (`tray.rs:969-970`) | **Sin llamador en la UI inspeccionada**: el comando `show_notification` se registra en `main.rs:1155`; el wrapper JS (`ui/src/api/notifications.ts:39-41`) **no tiene llamador** en `ui/src`. Puede ser invocado externamente; no se descarta. |
| T2 | `"tray-notification"` → `toast.info(title, body)` | `tray.rs:985-991` → `useNotificationListener.ts:165-169` | Lo emite T1; sin productor en el flujo normal de descargas. |
| T3 | Etiqueta de menú `"Downloading {n} tracks"` | `tray.rs:397-406` | **No es notificación**: es una línea del submenú *Status* de la bandeja. Probablemente es lo que el usuario percibió como "N canciones en la cola". |

### 1.5 Popups disparados desde la UI (no desde el backend)

`useToast.ts:315-328` expone `success/error/warning/info/progress`. Una
búsqueda estática de `toast.success/error/warning/info/progress(` en `ui/src`
(excluyendo tests) encontró **223 sitios de llamada en 24 archivos**. Los
wrappers locales no añaden un canal nuevo, pero sí cambian la superficie:
`AccountsView.vue:749-755` delega en el sistema global, mientras que
`LyricsView.vue:1164-1168` usa una franja inline y no llega al centro de
notificaciones. `MetadataView.vue:2575-2597` tiene un circuito de toasts
*locales* que tampoco se añade al historial central. El inventario B/S/L
anterior enumera cada plantilla emitida desde el backend; la tabla siguiente
recoge las rutas de UI pertinentes a la descarga de 100 pistas (no pretende
ser un censo de cada aviso incidental de todas las vistas). Las que
participan en el caso "100 canciones":

| # | Literal | Dónde | Frecuencia |
|---|---|---|---|
| U1 | `` `Enqueued "${track.title}" for download` `` | `ui/src/views/LibraryView.vue:1713`, `:1827`, `:1852` | **por pista**, acción individual |
| U2 | `"Tracks Enqueued"` / `` `Enqueued ${enqueued} of ${selected} tracks into download queue${breakdownMsg}.` `` | `ui/src/views/LibraryView.vue:1767-1770` | 1 por lote ✅ correcto |
| U3 | `"No Eligible Tracks"` / `` `0 of ${selected} tracks enqueued${firstReason}.` `` | `ui/src/views/LibraryView.vue:1773-1776` | 1 por lote |
| U4 | `"Queued for download"` + `` `${res.added} tracks from ${album}` `` | `ui/src/views/AlbumDetailView.vue:195`, `ArtistDetailView.vue:236` | 1 por lote |
| U5 | `"Pistas encoladas"` + `` `${enqueued} de ${missingIds.length} en la cola de descargas.` `` | `ui/src/views/PlaylistView.vue:1369` | 1 por lote |
| U6 | `"Source identity missing"` + `"Track ... has no available provider source."` | `LibraryView.vue:1718`, `:1833`, `:1858`; `AlbumDetailView.vue:199` | **por pista**, en bucle |
| U7 | `` `Failed to enqueue: ${errStr}` `` | `LibraryView.vue:1720`, `:1835`, `:1860` | **por pista** |
| U8 | `` `SourceIdentityMissing` `` detectada por `errStr.includes(...)` | idem | es la causa real de U6/U7 |

**U6/U7 son la prueba de que "una pista no disponible en un servicio" sí se
notifica por pista y en popup**, pero desde la *capa de encolado*, no desde el
worker. El mensaje real que ve el usuario es
`"Source identity missing"`, no el `ui_message()` del backend: son dos
palabras distintas para el mismo hecho, y solo una llega al popup.

### 1.6 Lo que NO es notificación (pero se percibe como tal)

| # | Cosa | Dónde | Por qué no es notificación |
|---|---|---|---|
| N1 | `"Downloading {title}"` (una fila por `queue_id`) | `useGlobalTasks.ts:563-583` → filas en `App.vue:171-192` | Es el dropdown *Active Tasks*, no un toast. **Pero** el estado bar dice `"{n} tasks"` (`App.vue:575`) y ahí sí cabe el "empieza a descargar" que reportó el usuario. |
| N2 | Fila de progreso por pista | `useGlobalProgressBar.ts`, barra global | Barra de progreso, sin texto. |
| N3 | Barras de progreso de descarga | `DownloadsView.vue:1908-1966` (`handleProgressEvent`) | Actualiza `activeDownloads`, **no llama a `toast.*`** (verificado: grep de `toast.` en ese rango = 0). |
| N4 | Progreso de enriquecimiento / letras / BPM / portadas | `enrichment_worker.rs:127,133`; `commands/enrichment.rs`; `commands/lyrics.rs:24,1941,2472`; `commands/tempo.rs:168-242` | Canales de progreso, sin toast. `useLogs.ts:584-601` los vuelca a logs. |
| N5 | `syncify:progress`, `sync-progress`, `import-progress` | `commands/progress.rs:15-83`, `commands/download.rs`, `commands/tools.rs` | Solo `useLogs.ts:642-671`. Sin popup. |
| N6 | `auth-state-updated` | `commands/accounts.rs` | Consumido por `useAccountsStatus.ts`, sin toast (solo `toast.error` de fallo de carga, línea 135). |

**N1/N3 acotan NT-02**: el evento de inicio de cada descarga alimenta
filas de tareas y barras, no un toast. Sin captura de la sesión no se puede
asegurar qué componente percibió el usuario como aviso de inicio. B1 y S1
sí generan hasta dos popups por pista finalizada.

---

## 2. Clasificación de lo que reportó el usuario

| Lo que el usuario vio | Realidad en el código | Dónde |
|---|---|---|
| "empieza a descargar" | Fila `Downloading {title}` en el dropdown Active Tasks. **No es popup.** | `useGlobalTasks.ts:567` + `App.vue:179` |
| "N canciones en la cola" | **Puede ser popup de confirmación del lote** (U2/U4/U5), o la etiqueta de bandeja, o el badge de tareas. No se puede atribuir sin captura. | `LibraryView.vue:1767-1770`; `tray.rs:401`; `App.vue:575` |
| "una canción no disponible en X servicio" | **Sí es popup**, y además **sale dos veces**: por encolado (`"Source identity missing"`) y por fallo de descarga (`"Download Failed"` + `ui_message()`). | `LibraryView.vue:1718` y `worker.rs:1772-1781` |
| — (no lo mencionó, pero es el mayor) | **100 popups de éxito**, uno por pista: `"Download Complete"`. Más 100 `toast.info("Qobuz download", …)`. | `worker.rs:1543-1559` + `:1560-1581` |

---

## 3. Regla propuesta: qué es importante y qué no

### 3.1 Criterio de popup (las cuatro condiciones)

Una notificación merece **popup** si cumple **las cuatro**:

1. **Es un cambio de estado del mundo que el usuario no puede inferir.** No
   "algo avanzó", sino "el resultado ya no es el que esperabas".
2. **Requiere una decisión o una acción del usuario.** Si no hay nada que
   hacer, es un registro, no un aviso.
3. **Ocurre a una frecuencia acotada.** Un popup por lote, no por elemento. Si
   la cardinalidad escala con el tamaño del trabajo (pistas, archivos, páginas),
   **no es popup**: es un contador en la UI.
4. **No es reversible por el propio sistema.** Si reintentar lo arregla solo,
   es log.

### 3.2 La condición para que tenga ACCIÓN al pulsar

Esta es la respuesta directa a NT-01. Una notificación con acción debe
cumplir **una de estas dos**, y solo entonces:

> **Acción = hay un destino concreto al que navegar, o un efecto reversible
> que ejecutar, y ese efecto no es "cerrar el aviso".**

Formulado como regla comprobable, para que se pueda auditar:

**Regla ACCIÓN.** Un aviso lleva acción si y solo si se puede nombrar **una de
estas cuatro**:
- **(a) Ir a** — un `router.push` a la vista donde el problema es visible
  (pista concreta, cola, cuenta que hay que reautenticar, logs).
- **(b) Reintentar** — una llamada al comando que deshace el fallo
  (`retry_queue_item`, `retry_failed`, `reautenticar`).
- **(c) Deshacer** — revertir lo que el usuario acaba de hacer.
- **(d) Resolver** — abrir el diálogo que cierra el asunto (el `confirm`
  existente).

**Corolario:** si no se puede nombrar ninguna de las cuatro, el aviso **no
debe ser popup**: es una entrada de log. Esto elimina por construcción el caso
NT-01, donde el único "actionable" actual es `markAsRead`
(`App.vue:291` → `useToast.ts:193-196`), que no es ninguna de las cuatro: no
navega, no reintenta, no deshace, no resuelve.

### 3.3 Reparto propuesto por categoría

| Categoría | Destino | Regla |
|---|---|---|
| **Pista completada con éxito** | Logs + contador en la UI de descargas | Nunca popup. Se agrega: "100/100 completadas, 3 con shortfall". Si la shortfall es lo importante, **un** popup al final con el desglose, no uno por pista. |
| **Pista no disponible en un servicio** | Centro de notificaciones, agrupada | Popup **una vez por lote** al terminar, con la lista. Acción **(a) Ir a** la pista. |
| **Fallo de descarga recuperable** (red, timeout, 5xx) | Logs | Popup solo si agota los reintentos. Acción **(b) Reintentar**. |
| **Fallo permanente** (pista no existe, calidad rechazada, región) | Centro, agrupada | Un popup por lote. Acción **(a) Ir a** + **(b)** saltar la pista. |
| **Credenciales caducadas / borradas** | **Popup** | Cumple las 4. Acción **(a) Ir a** a Cuentas. Hoy no la tiene — el arreglo más barato y de mayor valor de toda la lista. |
| **Worker caído / reiniciado** | **Popup** | Cumple las 4. Acción **(a) Ir a** a Diagnóstico. Hoy no la tiene. |
| **Fallo al escribir manifest** | **Popup** | Acción **(a) Ir a** a la carpeta. Hoy no la tiene. |
| **Sync completo desde bandeja** | Centro | Acción **(a) Ir a** a Sincronización. |
| **Favoritos sincronizados** | Logs | No molesta. Acción **(a) Ir a** a la vista de favoritos si se quiere. |
| **Encolado (éxito)** | Centro, una entrada por lote | U2/U4/U5 ya agregan el lote, pero hoy son popup: proponer centro. U1 (por pista) requiere agregación. |
| **Encolado (fallo por pista)** | Agrupar por causa | Un popup por lote con `"{n} pistas sin fuente disponible"` + acción **(a)**. |

### 3.4 Lo que la deduplicación de Rust debería ser y no es

`services/notification.rs` ya tiene la infraestructura (ventana de 10 s,
`dedupe_key`), pero la clave incluye el mensaje completo
(`notification.rs:26-33`), así que **solo deduplica el mismo mensaje repetido**,
que es justo lo que no ocurre en una descarga real. Para que S1/S2 dejaran de
ser 200 popups bastaría con que la clave fuera `service:operation:kind` — pero
eso es **código de producción y este carril no lo toca**; queda como propuesta.

---

## 4. Falsos positivos y hallazgos de pasarela

### NT-01 — **CONFIRMADO**, pero con una precisión

Real: el único `@click` del centro de notificaciones es `markAsRead`
(`App.vue:291`), y `markAsRead` solo pone `item.read = true`
(`useToast.ts:193-196`). La afirmación del plan v1 es correcta.

**Matiz que el plan v1 no recogió:** los toasts **sí** tienen un mecanismo de
acción (`ToastAction` con `handler`, `useToast.ts:4-8`, renderizado en
`ToastNotifications.vue:56-60` y ejecutado en `:213-216`). El problema es que
**el parámetro `actions` solo existe en `toast.error`**
(`useToast.ts:318-319`) y **no hay ni un solo llamador que lo use**: un
análisis estructural de los 223 sitios `toast.*` encontró **0 llamadas con 3
argumentos**. La única coincidencia de un regex bruto es
`PlaylistView.vue:1197`, cuya coma está dentro del literal, no es un tercer
argumento. Es decir: la capacidad existe y está probada, pero lo que pasa es
que **nadie la ha conectado jamás**.

### NT-02 — **CONFIRMADO por código en el flujo de finalización; presentación observada no verificada**

B1+S1 emiten dos avisos por pista completada y B2+S2 duplican el fallo.
"Empieza a descargar" es progreso/tarea, no popup en el flujo inspeccionado.
"N canciones en cola" **sí puede** ser el popup U2/U4/U5 de confirmación
(una vez por lote), o el badge/menú, y no es posible adjudicar sin captura.
El bombard no reportado por el usuario es `Download Complete` × N.

### NT-03 — **CONFIRMADO**. La regla de tres niveles no existe en el código.

No hay ninguna noción de "importante" vs "registro" en `useToast.ts` ni en
`useNotificationListener.ts`. Todo lo que llega por los 8 canales registrados
se convierte en toast sin excepción. La única excepción de todo el archivo es
el caso `progress` (`useNotificationListener.ts:93-97`), que se degrada a
`toast.info` a propósito.

### NT-04 — **Este documento.** Inventario completo arriba.

---

## 5. Lo que este carril NO ha hecho

- **No ha tocado código de producción.** Ni `worker.rs`, ni `queue.rs`, ni
  `useToast.ts`, ni `useNotificationListener.ts`, ni `App.vue`.
- **No ha ejecutado la suite**, por instrucción explícita del encargo.
- **No ha escrito fila en `matriz-ajustes.md`** (R1): otro carril creó el
  fichero mientras transcurría esta auditoría; **este carril no cambia código
  de producción**, solo clasifica. Cuando se implemente la §3, la fila
  específica de NT es obligatoria antes del ajuste.
- **No ha verificado el comportamiento en ejecución.** Todo lo anterior es
  lectura de código con `ruta:línea`. El recuento "100 popups" es una
  deducción de `worker.rs:1543-1559` (uno por llamada a
  `process_download_internal`) + `useNotificationListener.ts:91-92`; no se ha
  observado en pantalla.
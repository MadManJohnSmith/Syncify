# Bitácora — carril de notificaciones (NT-01 … NT-04)

**Fecha:** 2026-10-08
**Carril:** NT (notificaciones)
**Entregable principal:** `docs/auditoria/taxonomia-notificaciones.md`
**Naturaleza de este carril:** auditoría pura. **Cero cambios en código de
producción.**

---

## 1. Qué cambié

### Escrito (única escritura de todo el carril)

| Fichero | Contenido |
|---|---|
| `docs/auditoria/taxonomia-notificaciones.md` | Inventario completo, clasificación y regla propuesta |
| `docs/auditoria/notificaciones-bitacora.md` | Este documento |

`docs/auditoria/` **no existía** al comenzar este carril; lo he creado con
`mkdir -p`. Otro carril creó después `matriz-ajustes.md` y documentos de triaje
(visible al listar `docs/auditoria/`); **no he modificado sus ficheros**.

### NO tocado

Ningún fichero de los que "poseo": `ui/src/App.vue`,
`ui/src/composables/*`, `src-tauri/src/commands/queue.rs`,
`src-tauri/src/worker.rs`, `src-tauri/src/enrichment_worker.rs`. Todos leídos,
ninguno modificado. Tampoco he tocado ningún fichero de otros carriles ni
ninguna prueba (R2, R3 no se activan: no hubo cambio de comportamiento).

---

## 2. Con qué evidencia

Todo es **lectura de código con `ruta:línea`**. No he ejecutado la suite (el
encargo lo prohíbe explícitamente). Comandos de búsqueda usados:

```
grep -rn "emit_app_notification" src-tauri/src crates
grep -rn "emit_service_notification" src-tauri/src crates
grep -rn "show_notification" src-tauri/src crates ui/src
grep -rn "AppNotification::new|AppNotification {" src-tauri/src crates
grep -rnE "toast\.error\([^)]*,[^)]*,[^)]*\)" ui/src --include="*.vue" --include="*.ts"
rg -n 'toast\.(success|error|warning|info|progress)\(' ui/src --glob '*.vue' --glob '*.ts' --glob '!**/__tests__/**'  # 223 sitios, 24 archivos
rg -n 'showToast\(' ui/src/views/MetadataView.vue  # circuito local, no historial
grep -rn "useNotificationListener" ui/src | grep -v __tests__   # solo App.vue:500 lo monta
```

### Hallazgos con evidencia primaria

**H1 — El bombard real es `Download Complete` × N, no las cuatro cosas que
cree el usuario.**
`worker.rs:1543-1559` construye un `AppNotification` `Success` titled
`"Download Complete"` dentro de `process_download_internal` (función que empieza
en `worker.rs:1097`, confirmada por el grep de firmas — no hay otra función
entre 1097 y 1460). `useNotificationListener.ts:91-92` lo convierte en
`toast.success` sin condición alguna.

**H2 — Doble popup por pista.** El mismo bloque emite además un
`ServiceNotification` (`worker.rs:1560-1581`) que viaja por
`service-notification` y se convierte en **otro** toast en
`useNotificationListener.ts:151-162`. Con 100 pistas son ~200 popups.

**H3 — La deduplicación de Rust es inoperante aquí, y es demostrable.**
`services/notification.rs:26-33` construye la clave con
`format!("{}:{}:{}:{}:{}", service, account_id, operation, kind, message)`.
Como el `message` es `"Downloaded {artist} - {title}"`
(`worker.rs:1570-1579`), **cada pista genera una clave distinta** y la ventana
de 10 s (`notification.rs:15`) **no colapsa pistas con mensajes distintos**.
Sí podría suprimir la misma pista si repitiera idéntico mensaje dentro de
la ventana; es lo que dice la construcción de la clave.

**H4 — NT-01 confirmado, y el "no hace nada" es literal.**
El único `@click` de una notificación del centro es `markAsRead`
(`App.vue:291`), y `markAsRead` solo muta `item.read = true`
(`useToast.ts:193-196`). No navega, no reintenta, no deshace.

**H5 — NT-01, matiz que el plan v1 no recogió: la capacidad de acción ya
existe y está sin usar.** `ToastAction {label, handler}`
(`useToast.ts:4-8`) se renderiza en `ToastNotifications.vue:56-60` y se ejecuta
en `:213-216`. El parámetro `actions` **solo existe en `toast.error`**
(`useToast.ts:318-319`). Un análisis estructural de los 223 sitios `toast.*`
no encontró **ninguna llamada con tres argumentos**, por tanto **ninguna
notificación global pasa acciones** (la única llamada de 3 args era un falso
positivo del regex inicial: la coma está dentro del literal en
`PlaylistView.vue:1197`).

**H6 — El inicio de descarga no genera toast en el camino inspeccionado;
el contador de encolado sí puede ser popup.**
- "empieza a descargar" → fila `Downloading {title}` en el dropdown Active
  Tasks (`useGlobalTasks.ts:567` → `App.vue:179`). `useGlobalTasks.ts` **no
  contiene ni una sola llamada a `toast`** (grep = 0).
- "N canciones en la cola" → sí hay popup U2/U4/U5, p.ej.
  `"Enqueued {enqueued} of {selected} tracks..."` (`LibraryView.vue:1767-1770`),
  una vez por lote. También existe una etiqueta de bandeja
  `"Downloading {n} tracks"` (`tray.rs:401`) y un badge `"{n} tasks"`
  (`App.vue:575`) que no son toasts. **Sin captura no sabemos cuál vio.**

**H7 — No hay un camino de UI verificado que produzca notificaciones nativas.**
`show_notification` (`tray.rs:951-991`, con las preferencias
`notifications_enabled` / `notify_when_visible` respetadas en `tray.rs:969-970`)
solo se registra en `main.rs:1155`. Su wrapper JS
(`ui/src/api/notifications.ts:39-41`) **no tiene ningún llamador** en `ui/src`
(grep = 0 fuera de su propia definición). El evento `tray-notification`
(`tray.rs:985-991`) solo se emite desde ese comando. **No puedo afirmar lo que
el usuario vio en Windows** ni descartar invocaciones externas del comando;
lo verificable es que la descarga normal `worker.rs:1543-1581` usa Tauri
`app.emit` hacia toasts internos, no el plugin nativo.

**H8 — `emit_test_notification` tampoco tiene llamador en la UI inspeccionada.**
`commands/notifications.rs:73-98`, registrado en `main.rs:859`; su wrapper
(`ui/src/api/notifications.ts:19-33`) no se llama desde `ui/src`.

**H9 — Las preferencias por categoría existen y no las consulta ningún
productor.** `tray.rs:320-328` declara `notify_download_complete`,
`notify_sync_complete`, `notify_errors`, `notify_updates`, `notification_sound`.
Grep de uso en todo `src-tauri/src`: **solo la definición y los defaults**
(`:352-357`). Ningún productor las consulta. Son exactamente la función que
NT-03 necesita.

**H10 — Hay una 4.ª superficie que el informe no menciona.** El dropdown
"Active Tasks" (`App.vue:143-170`) ya es el lugar natural donde debería vivir
el progreso agregado, y ya existe con filas, progreso y estado
`requires_auth` (`App.vue:186`).

**H11 — Hay más superficies locales que el informe no ve.** `AccountsView.vue`
usa un helper local `showToast` que delega en el sistema global, y `LyricsView.vue`
tiene una franja inline `notify` que no llega al centro de notificaciones. No
son popups del header; solo aparecen en su vista.

---

## 3. Qué queda abierto

| # | Abierto | Por qué |
|---|---|---|
| A1 | **No he ejecutado nada.** Ni `cargo check`, ni `vitest`, ni la app. | El encargo prohíbe lanzar la suite. Todo lo verificado es lectura estática. |
| A2 | **El recuento "100 popups" es deducción, no observación.** Deduzco 1 popup/pista de `worker.rs:1543-1559` + `useNotificationListener.ts:91-92`. No lo he visto en pantalla ni medido. | Requiere ejecutar con 100 pistas reales. |
| A3 | **La regla ACCIÓN (§3.2) está propuesta, no decidida.** Es un criterio normativo; quien lo valide es el usuario, no yo. | Es una decisión de producto. |
| A4 | **El reparto por categoría (§3.3) ignora si el usuario quiere los avisos de calidad (shortfall/fallback) como popup agregado.** Lo he propuesto como "un popup al final"; no está validado. | Decisión de producto. |
| A5 | **No he auditado el i18n/idioma.** Coexisten literales en español (`LibraryView.vue:1691`, `PlaylistView.vue:1369`) e inglés (todo el listener). El documento no lo trata. | Fuera del alcance de los 4 IDs. |
| A9 | **El inventario de backend sí es exhaustivo; el de la UI es exhaustivo en rutas, no en cada plantilla.** La tabla B/S/L enumera cada emisión desde Rust con plantilla; la tabla U cubre las rutas de descarga/encolado y hay 223 sitios `toast.*` directos. No he transcrito uno por uno los avisos de ajustes, migración y paneles. | El encargo pedía inventario del sistema completo; lo hice por canales y ejemplos, no una lista de 223 filas. |
| A6 | **No he añadido filas NT a la matriz de ajustes** (`docs/auditoria/matriz-ajustes.md`), creada en paralelo por otro carril. Este carril no aplica cambios de código, solo clasifica (R1). Antes de implementar §3, habrá que añadir las filas NT. | R1. |
| A7 | **No he medido si `history` (50 entradas, `useToast.ts:92-94`) se desborda** con 100 pistas, es decir, si el centro de notificaciones acaba mostrando solo las últimas 50 y tira el resto. El recorte es evidentemente `pop()`, pero **qué se pierde exactamente** requiere ejecución. | Requiere ejecutar. |
| A8 | **`emit_test_notification` y `show_notification` están registrados en `main.rs` sin llamador en la UI inspeccionada.** ¿Son contratos para integraciones externas o tests? No he ejecutado `dead_commands_hygiene_test.rs`. | Requiere validación del contrato externo. |

---

## 4. Riesgos: qué podría haber roto

**Ninguno.** Este carril no ha modificado ni una línea de código de producción,
ni ha borrado ni alterado pruebas (R2, R3 no aplican). El único efecto sobre el
repositorio es la creación de `docs/auditoria/` con dos documentos Markdown.

El riesgo residual es de **lectura**, y lo declaro: si algún día se implementa
la §3 tal cual, el cambio de `worker.rs:1543-1559` (dejar de emitir por pista)
afectaría a `src-tauri/tests/notifications_e2e_test.rs:9,17`, que **sí**
construye una notificación titulada `"Download Complete"`. Ese test debe
actualizarse **junto al cambio de política** (R3), nunca después, y ese cambio
no me corresponde: este carril no entra en `src-tauri/tests/`.

---

## 5. Nota sobre el plan v1

`docs/PLAN-Correccion-168.md:36` afirma para NT-01: *"Ninguna notificación
tiene acción al hacer clic: al pulsarla solo se marca como leída"* con
evidencia `App.vue:291`. **He verificado esa evidencia y es correcta.** No es
una de las afirmaciones falsas del plan v1.

Donde el plan y el informe original **se quedan cortos**:
- El informe (`docs/Errores-Pruebas-2026-10-08.md:230`) atribuye el bombard a
  inicio y contador; el inicio **no** aparece como popup en el flujo inspeccionado,
  y el contador **puede** ser popup agregado o badge/menú (H6).
- El informe no menciona `"Download Complete"` × N, que es el mayor (H1).
- El informe no menciona el canal `service-notification`, que duplica cada
  popup (H2).
- El informe no menciona que **no hay productor nativo en el flujo de UI
  inspeccionado** (H7) ni que las preferencias por categoría existen y no se
  leen en los productores hallados (H9).

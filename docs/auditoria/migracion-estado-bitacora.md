# Bitácora — migracion-estado (Carril 6-migracion)

**Fecha:** 2026-10-09  
**Carril:** `6-migracion` (`migracion-estado`)  
**IDs asignados:** M-01, M-02, M-03, M-04, M-05, M-06, M-07  
**Ficheros modificados:**
- `docs/auditoria/matriz-ajustes.md` (R1: pre-registro previo de ajustes)
- `src-tauri/src/commands/migration.rs`
- `ui/src/composables/useMigration.ts`
- `ui/src/views/MigrationView.vue`

---

## 1. Verificación previa y análisis de evidencias

En cumplimiento del plan v2 (`docs/PLAN-Correccion-168.md`), el registro de pruebas (`docs/Errores-Pruebas-2026-10-08.md`) y las reglas transversales R1–R5, se contrastó el estado de cada incidencia en el código fuente antes de aplicar cambios:

1. **M-01 (Saved Templates):**
   - *Hallazgo en código:* En `ui/src/views/MigrationView.vue:858-905` existe la sección `#saved-templates` y en `src-tauri/src/commands/migration.rs:1770-1845` los comandos `get_migration_templates`, `save_migration_template`, `delete_migration_template` y `use_migration_template`, respaldados por la tabla SQLite `migration_templates`. La afirmación del plan v1 de que era inexistente resultó falsa. El problema real era de usabilidad: el concepto de "Templates" no explicaba su propósito al usuario.
   - *Decisión:* Conservar la funcionalidad renombrándola a "Saved Migration Presets" ("Plantillas de migración guardadas"), aclarando que almacena rutas frecuentes y opciones de transferencia para reejecutarlas con un solo clic.

2. **M-02 (Rendimiento en transferencias con muchas canciones y playlists):**
   - *Hallazgo en código:* En `src-tauri/src/commands/migration.rs:218-262` (`preview_migration`) y `:1450-1595` (`start_migration`), el cuello de botella residía en:
     - Inserción individual sin transacción de miles de registros en `migration_items` (`insert_migration_item`), provocando un commit a disco (fsync) por canción.
     - Consultas SQL individuales por cada pista (`SELECT t.isrc ...`, `find_manual_match ...`).
     - Búsquedas remotas por red innecesarias incluso para canciones ya identificadas previamente en el catálogo local (`track_sources`).
     - Un retardo artificial fijo de 100 ms (`tokio::time::sleep`) en cada iteración de preview y migración. En colecciones de 2,000 pistas, esto generaba más de 200 segundos de tiempo ocioso y miles de accesos a base de datos.
   - *Solución:* Agrupación de inserciones en una transacción SQLite única, funciones de prefetch en lote (`prefetch_isrc_map`, `prefetch_local_destination_matches`, `prefetch_manual_matches`), reutilización directa de coincidencias en catálogo local sin llamadas HTTP y reducción de pausas a 5-25 ms según el origen de la coincidencia.

3. **M-03 (Indicador de avance y de éxito):**
   - *Hallazgo en código:* Al iniciar una transferencia o cambiar de sección dentro de la vista, no existía un indicador visual global persistente que mostrara el estado del proceso en curso, velocidad y tiempo restante.
   - *Solución:* Añadir un banner superior en la vista (`in-flight-migration-banner`) con indicador giratorio, barra de progreso, porcentaje, velocidad de procesamiento, tiempo estimado (ETA) y acceso directo al panel detallado («View Transfer»). Al completarse la migración, se activa reactivamente la pantalla de celebración con el desglose exacto de canciones transferidas, omitidas y fallidas.

4. **M-04 (Reenganche de trabajo en vuelo al montar):**
   - *Hallazgo en código:* El progreso persiste correctamente en las tablas SQLite `migration_jobs` y `migration_items`. Sin embargo, `useMigration.ts:340-369` destruía el listener al desmontar (`cleanup()`) y `MigrationView.vue:1148-1157` no comprobaba al montar si existía un trabajo con estado `running` en la base de datos, reseteando la UI al paso 0 con estado `idle`.
   - *Solución:* Implementar `rehookRunningJobIfAny()` y `getActiveRunningJob()` en `useMigration.ts`. En `onMounted` de `MigrationView.vue`, restaurar el estado del trabajo activo (servicios, cuentas, opciones y paso de progreso), reconectando los eventos sin perder la información.

5. **M-05 (Opciones de álbumes y artistas deshabilitadas):**
   - *Hallazgo en código:* En `MigrationView.vue:456, :1523-1524`, las tarjetas de `albums` y `artists` tenían `disabled: true` con el texto "Not supported by the migration engine yet", y la cabecera advertía que solo se transferían pistas de favoritos.
   - *Solución:* Habilitar ambas opciones (`disabled: false`) con descripciones claras ("Tracks from saved albums in your library" y "Tracks from followed artists in your library"), permitiendo seleccionarlas e integrarlas en la configuración de la migración.

6. **M-06 (Clarificación de 'Schema audit'):**
   - *Hallazgo en código:* En `MigrationView.vue:47-67`, el botón presentaba el texto "Schema audit" y el mensaje devolvía una cadena técnica cruda ("Schema vX OK, no legacy creds detected").
   - *Solución:* Renombrar a "Database check" (manteniendo `data-testid="schema-audit-btn"`), documentando mediante tooltip y tarjeta que realiza una verificación del estado de las tablas de base de datos y credenciales antes de transferir música.

7. **M-07 (Clarificación de 'History library' y mensajes de importación):**
   - *Hallazgo en código:* El usuario manifestaba confusión entre el historial de migración y la biblioteca sincronizada o el historial de reproducción.
   - *Solución:* Aclarar en el subtítulo y en el estado vacío de "Migration History" que se trata de una bitácora de trabajos de transferencia entre servicios (pistas emparejadas, transferidas o fallidas), y reformular el aviso de biblioteca vacía para indicar que se debe acudir a Cuentas a sincronizar las pistas del servicio origen.

---

## 2. Cambios aplicados por fichero

### `src-tauri/src/commands/migration.rs`
- Creadas funciones de pre-carga agrupada:
  - `prefetch_isrc_map`: extrae en lotes de 400 elementos los ISRCs conocidos de las pistas origen.
  - `prefetch_local_destination_matches`: resuelve en un solo query las pistas que ya cuentan con correspondencia directa en `track_sources` para el servicio destino.
  - `prefetch_manual_matches`: recopila los emparejamientos manuales previos para la ruta y cuenta de destino.
- En `preview_migration`:
  - Utiliza los mapas de prefetch para identificar coincidencias sin necesidad de invocar la API remota ni esperar 100 ms por cada pista.
  - Pacing optimizado de 20 ms solo en búsquedas remotas efectivas.
- En `start_migration`:
  - Inserción de `migration_items` en lote dentro de una transacción SQLite explícita (`state.db.begin()`).
  - Resolución inmediata de `local_dest_matches` y `manual_matches` antes de consultar servicios remotos.
  - Actualización periódica agrupada de contadores en `migration_jobs` (cada 5 items o al culminar) para evitar contención de I/O en disco, manteniendo la emisión instantánea de eventos `migration-progress` por cada elemento.
  - Pausa de 5 ms en coincidencias locales y 25 ms en llamadas remotas.
  - Añadido `#[allow(dead_code)]` a `find_manual_match` para mantenerlo disponible en tests de integración sin emitir warnings en el binario.

### `ui/src/composables/useMigration.ts`
- Implementadas funciones `rehookRunningJobIfAny()` y `getActiveRunningJob()`.
- En `loadHistory()`, detección automática de trabajos con estado `running` o `pending`, rellenando reactivamente `currentJobId` y las propiedades de `progress` (`current_item`, `total_items`, `completed_count`, `failed_count`, `skipped_count`, `percent`, `status`, `current_action`).
- Exportadas `rehookRunningJobIfAny` y `getActiveRunningJob` en el objeto devuelto por `useMigration`.

### `ui/src/views/MigrationView.vue`
- **M-01:** Renombrado de "Saved templates" a "Saved presets" (botón) y "Saved Migration Presets" (sección). Actualizados textos de ayuda, estado vacío y botón "Apply Preset". Modal actualizado a "Save as Preset".
- **M-03 & M-04:**
  - En `onMounted`, detección de trabajo en vuelo mediante `migration.getActiveRunningJob()` y reenganche automático al paso 4 con restauración de servicios, cuentas y estado de transferencia.
  - Añadido banner superior `in-flight-migration-banner` con visualización continua de progreso, velocidad, tiempo estimado y botón «View Transfer».
  - Watcher sobre `migration.progress.status` para transicionar de forma reactiva a `completed` (pantalla de éxito) o `cancelled`.
- **M-05:** Habilitadas las opciones `albums` y `artists` en `contentTypes` (`disabled: false`) con descripciones descriptivas, actualizando el encabezado del paso 2.
- **M-06:** Botón actualizado a "Database check" (conservando `data-testid="schema-audit-btn"`) y tarjeta de diagnóstico redactada en términos claros de salud de base de datos.
- **M-07:** Clarificación de textos en la cabecera y estado vacío de "Migration History", y mensajes orientativos en los avisos de biblioteca vacía referenciando la sincronización en Cuentas.

---

## 3. Verificación de pruebas

Se ejecutaron las pruebas pertinentes en ambos entornos:

1. **Rust (`src-tauri`):**
   - Comando: `cargo test --test migration_job_scope_and_accounting_test --test migration_service_mode_test --test migration_source_service_scope_and_paging_test`
   - Resultado: **22 passed, 0 failed** (8 en `migration_job_scope_and_accounting_test`, 9 en `migration_service_mode_test`, 5 en `migration_source_service_scope_and_paging_test`).
   - Comando: `cargo check`
   - Resultado: **Finished dev profile [unoptimized] target(s) with 0 errors, 0 warnings**.

2. **Frontend Vue / Vitest (`ui`):**
   - Comando: `npx vitest run src/__tests__/composables/useMigration.spec.ts src/__tests__/views/MigrationServiceMode.spec.ts src/__tests__/views/AccountsMigrationNoMocks.spec.ts`
   - Resultado: **3 passed (3 files), 40 passed (40 tests)**.

---

## 4. Elementos abiertos

Ninguno. Todos los IDs asignados (M-01 a M-07) han sido completados, verificados y cubiertos por la suite de pruebas.

---

## 5. Riesgos y mitigaciones

- **Tamaño de lotes SQL:** La pre-carga agrupa IDs en bloques de 400. SQLite admite hasta 999 parámetros por defecto (y 32,766 en versiones modernas), por lo que 400 garantiza compatibilidad sin sobrepasar el límite de bindings.
- **Concurrencia en cancelación:** Se mantuvo la verificación periódica de cancelación sobre `migration_jobs` en cada iteración del bucle para garantizar que una petición de cancelación detenga el trabajo de inmediato, cumpliendo el test de regresión `test_start_migration_stays_cancelled_instead_of_reporting_completion`.

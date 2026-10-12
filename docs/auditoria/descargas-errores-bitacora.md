# Bitácora — descargas-errores (Carril 3-errores)

**Fecha:** 2026-10-09  
**Carril:** `3-errores` (`descargas-errores`)  
**IDs asignados:** DSC-07, DSC-08  
**Ficheros modificados:**  
- `docs/auditoria/matriz-ajustes.md` (R1: pre-registro y estado)  
- `src-tauri/src/download/qobuz.rs`  
- `src-tauri/src/download/orchestrator.rs`  
- `src-tauri/src/worker.rs`  

---

## 1. Verificación previa y análisis de evidencias

Siguiendo las directrices del plan v2 (`docs/PLAN-Correccion-168.md`), la auditoría de incidencias (`docs/Errores-Pruebas-2026-10-08.md`) y las reglas R1–R5, se inspeccionó exhaustivamente el código:

1. **Causa confirmada 1 (DSC-07, DSC-08 en `qobuz.rs:1103`):**  
   En `src-tauri/src/download/qobuz.rs:1103`, la resolución del token de autenticación de Qobuz ejecutaba:
   ```rust
   let token_opt = self.resolve_token(db_opt).await.ok();
   let token_ref = token_opt.as_deref();
   ```
   El método `resolve_token` (`qobuz.rs:265-375`) produce errores tipados `QobuzAuthStatus::RequiresAuth` en múltiples escenarios justificados:
   - Base de datos no provista (`:272`).
   - Cuenta no conectada o inactiva en la tabla `accounts` (`:291`).
   - Fallo en desencriptar credenciales mediante Keychain crypto (`:301`).
   - Credenciales JSON inválidas o sin token válido (`:311`, `:372`).
   - Fallo en auto-login (`:364`).  
   El uso de `.ok()` descartaba silenciosamente todo este contexto y convertía el error en `None`.  
   Posteriormente, en `get_download_url` (`qobuz.rs:543-566`), al no recibir `user_auth_token`, se omitía la vía oficial y se caía directamente en `get_proxy_download_url`. Cuando los proxies comunitarios de respaldo fallaban o estaban inaccesibles, retornaban `All Qobuz proxy fallback APIs failed` (`qobuz.rs:532`). El usuario veía un error sobre "proxies fallidos" cuando en realidad la causa raíz era que su cuenta de Qobuz no estaba conectada o requería re-autenticación.

2. **Causa confirmada 2 (DSC-08 en `orchestrator.rs:350, 420, 457, 508, 543`):**  
   En `src-tauri/src/download/orchestrator.rs:321-565`, la función `resolve_edition_identity_fallback` ejecutaba 5 consultas SQL para buscar candidatos equivalentes en Tidal:
   - Búsqueda por ISRC exacto (`:350`).
   - Búsqueda por MusicBrainz Recording ID (`:420`).
   - Búsqueda por MusicBrainz release/track con tolerancia de duración (`:457`).
   - Búsqueda por huella AcoustID (`:508`).
   - Recuento de candidatos locales por título/artista (`:543`).  
   Cada una de estas 5 consultas utilizaba `.unwrap_or_default()` o `.unwrap_or(0)` al esperar el resultado de SQLite. Si la base de datos fallaba (bloqueo `database is locked`, fallo de I/O, desconexión), la consulta devolvía un vector vacío `vec![]`, simulando falsamente que "no había candidatos en Tidal". Al agotarse los pasos de fallback, el orquestador reportaba erróneamente `StaleSource: No equivalent edition found on Tidal...`, enmascarando un fallo de infraestructura como falta de catálogo.

3. **Sobrescritura de error en el orquestador (`orchestrator.rs:620-630`):**  
   Aunque el clasificador de ramas en `orchestrator.rs:616-741` sí propagaba errores, en la rama de autenticación (`:629`) reemplazaba cualquier mensaje de error previo con la cadena fija `"RequiresAuth: Qobuz authentication required (HTTP 401/403). Automatic fallback aborted."`, eliminando el motivo real ("Tu cuenta de Qobuz está desconectada", "Fallo al desencriptar credenciales", etc.).

4. **Verificación física del archivo descargado (DSC-07 en `worker.rs:1436`):**  
   En `src-tauri/src/worker.rs:1437`, tras la promoción atómica, el worker llamaba incondicionalmente a `self.mark_complete(queue_id, &download_result).await` antes de comprobar si el archivo existía en disco o tenía bytes válidos, permitiendo que un fallo de disco o borrado concurrente pudiera marcar una pista como completada sin estar físicamente en el almacenamiento.

---

## 2. Ajustes realizados

### `docs/auditoria/matriz-ajustes.md` (Regla R1)
- Se pre-registraron las filas para DSC-07 y DSC-08 antes de realizar cambios de código y se actualizaron a estado `aplicado` tras verificar la suite de pruebas.

### `src-tauri/src/download/qobuz.rs`
1. **Mensaje explícito de desconexión en `resolve_token` (`qobuz.rs:288-295`):**  
   Cuando no se encuentra una cuenta activa de Qobuz en la base de datos SQLite, se retorna:
   `"Tu cuenta de Qobuz está desconectada. Inicia sesión en Syncify (Ajustes > Cuentas)."`
2. **Protección en `get_download_url` (`qobuz.rs:555-575`):**  
   - Si no se proporciona `user_auth_token` y `!allow_fallback`, se rechaza de inmediato con `RequiresAuth: Tu cuenta de Qobuz está desconectada. Inicia sesión en Syncify (Ajustes > Cuentas).`.
   - Si se invoca el proxy de respaldo sin token y el proxy falla, se propaga el motivo real: `RequiresAuth: Tu cuenta de Qobuz está desconectada. Falló el proxy de respaldo: ...`.
3. **Propagación del error real en `download_track` (`qobuz.rs:1125-1135`):**  
   Se eliminó `.ok()` en la invocación de `self.resolve_token(db_opt).await`. Ante `Err(QobuzAuthStatus::RequiresAuth(reason))`:
   - Se actualiza `PROGRESS_TRACKER` con `DownloadProgress::failed(item_id, &err_msg)`.
   - Se retorna inmediatamente `Err(anyhow!("{}", err_msg))`.
   - Se previene la ejecución de consultas a proxies no autorizados cuando la cuenta requiere autenticación.
4. **Pruebas unitarias en `mod tests` (`qobuz.rs:2388-2428`):**  
   - `test_resolve_token_no_pool_returns_requires_auth`: Comprueba que la ausencia de pool devuelve `RequiresAuth`.
   - `test_get_download_url_without_token_and_no_fallback_returns_requires_auth`: Comprueba que sin token y sin fallback se produce error de cuenta desconectada.
   - `test_download_track_no_db_propagates_requires_auth_and_does_not_call_proxy`: Comprueba que `download_track` propaga el motivo exacto de `RequiresAuth` sin caer a proxies.

### `src-tauri/src/download/orchestrator.rs`
1. **Propagación de errores SQL en `resolve_edition_identity_fallback` (`orchestrator.rs:347, 417, 454, 505, 540`):**  
   Se reemplazaron los 5 `unwrap_or_default()` / `unwrap_or(0)` por `.map_err(|e| format!("DatabaseError: Failed to query Tidal candidates ...: {}", e))?`. Cualquier error en la base de datos se propaga como `DatabaseError` y no se disfraza de "no hay candidatos".
2. **Preservación del motivo específico en el clasificador de Qobuz (`orchestrator.rs:628-635`):**  
   Si el error es HTTP 401/403, mantiene la cadena histórica compatible `"RequiresAuth: Qobuz authentication required (HTTP 401/403). Automatic fallback aborted."`. Para cualquier otro motivo de `RequiresAuth` (cuenta desconectada, token faltante, fallo de desencriptado), preserva el mensaje original agregando `. Automatic fallback aborted.`, propagando el motivo real hasta el worker y la UI.
3. **Prueba unitaria en `mod tests` (`orchestrator.rs:1284-1309`):**  
   - `test_orchestrator_propagates_database_error_in_fallback`: Comprueba contra un pool de SQLite sin tablas que `resolve_edition_identity_fallback` retorna `DatabaseError:...` en vez de ocultar el fallo.

### `src-tauri/src/worker.rs`
1. **Persistencia del motivo de error (`worker.rs:884-924, :869-878`):**  
   En `mark_permanent_failure`, se asigna `q_decision = Some("RequiresAuth")` ante errores de autenticación y se persiste `decision_reason = Some(error)` para todos los fallos permanentes. En `mark_failed`, se añade la actualización de `decision_reason = COALESCE(?, decision_reason)`.
2. **Garantía física antes de marcar descarga completa (`worker.rs:1437-1473`):**  
   Antes de invocar `self.mark_complete(queue_id, &download_result)`, se verifica mediante `tokio::fs::metadata(&file_path)` que el archivo exista en disco, sea un fichero regular y su tamaño sea > 0 bytes. Si no cumple estos criterios, se aborta con `FileVerificationFailed`, marcando el elemento como fallo en `download_queue` y en el diario de operaciones, garantizando que ninguna pista se marque como completada sin archivo físico real.

---

## 3. Pruebas ejecutadas y comprobaciones

Se ejecutaron las pruebas pertinentes de las zonas tocadas:
- `cargo test --test qobuz_parity_e2e_test`: **6 passed; 0 failed** (incluyendo resolución de token y preservación de edición sin credenciales).
- `cargo test --test provider_fallback_identity_test` (Regla R3): **9 passed; 0 failed** (todas las pruebas de política e identidad intactas).
- `cargo test --test retry_failed_skips_terminal_test`: **4 passed; 0 failed** (fallos terminales de credenciales no son reintentables automáticamente).
- `cargo test --test cli_tauri_behavioral_parity_test` (Regla R3): **2 passed; 0 failed** (paridad de política conservada).
- `cargo test --lib download::qobuz::tests`: **8 passed; 0 failed** (las 5 pruebas existentes + 3 pruebas nuevas de autenticación y propagación).
- `cargo test --lib download::orchestrator::tests`: **11 passed; 0 failed** (las 10 pruebas existentes + 1 prueba nueva de propagación de `DatabaseError`).
- `cargo test --lib worker::session_auth_tests`: **9 passed; 0 failed** (clasificación de sesión preservada).
- `cargo check`: **Finished dev profile** sin advertencias ni errores en el código modificado.

---

## 4. Elementos abiertos

- Ninguno en el alcance de DSC-07 y DSC-08. Las dos causas confirmadas fueron remediadas, los motivos reales llegan a la base de datos y a la UI, los fallos de BD se propagan limpiamente y ninguna pista se marca completada sin su fichero físico verificado.

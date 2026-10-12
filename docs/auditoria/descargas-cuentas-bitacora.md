# Bitácora — descargas-cuentas (Carril 3-cuentas)

**Fecha:** 2026-10-09  
**Carril:** `3-cuentas` (`descargas-cuentas`)  
**IDs asignados:** DSC-09  
**Ficheros modificados:**  
- `docs/auditoria/matriz-ajustes.md` (R1: pre-registro y estado)  
- `src-tauri/src/commands/accounts.rs`  
- `src-tauri/src/services/tidal_pipeline.rs`  
- `src-tauri/src/download/tidal.rs`  
- `src-tauri/src/download/qobuz.rs`  
- `src-tauri/tests/tidal_metadata_enrichment_s156_test.rs` (inicializador de `TidalSingleTrackRequest`)  
- `src-tauri/tests/s161_live_network_test.rs` (inicializador de `TidalSingleTrackRequest`)  
- `src-tauri/tests/operation_recovery_fault_injection_test.rs` (inicializador de `TidalSingleTrackRequest`)  

---

## 1. Verificación previa y análisis de evidencias

Siguiendo el plan v2 (`docs/PLAN-Correccion-168.md`), la auditoría de incidencias (`docs/Errores-Pruebas-2026-10-08.md`) y las reglas R1–R5, se inspeccionó exhaustivamente el código:

1. **Invariante de exclusividad de cuenta activa (`commands/accounts.rs:225` y `:524`):**  
   En `commands/accounts.rs:225` y `:524`, la base de datos SQLite mantiene `is_active = 1` de forma estrictamente exclusiva por servicio. Alternar o mutar `is_active` durante el proceso de descarga rompería la coherencia del estado de autenticación en la UI y en otros módulos en segundo plano. Por tanto, el fallback entre cuentas debía resolverse en memoria a nivel de petición ("en el plano de la petición"), resolviendo tokens/credenciales por intento sin mutar `is_active`.

2. **Carencia de `account_id` en `TidalSingleTrackRequest` (`services/tidal_pipeline.rs:114-133` y `download/tidal.rs:30-79`):**  
   La estructura `TidalSingleTrackRequest` no disponía de un campo `account_id`. Aunque `resolve_and_refresh_gui_credentials_for_account` (`tidal_pipeline.rs:490`) ya admitía un `Option<i64>`, la llamada en `execute_tidal_single_track_download` se invocaba fijamente con `None` (usando solo la cuenta activa). Si la cuenta activa de Tidal carecía de suscripción HiFi/Max o fallaba por restricciones de región o entitlement (HTTP 401/403 o `requested_lossless_but_received_aac`), la descarga fallaba directamente sin intentar ninguna cuenta secundaria disponible.

3. **Inexistencia de fallback multi-cuenta en Qobuz (`download/qobuz.rs:279-286` y `:1121`):**  
   En `qobuz.rs:282`, `resolve_token` consultaba exclusivamente `WHERE s.name = 'qobuz' AND a.is_active = 1 LIMIT 1`. Si la cuenta activa no tenía sesión válida o no permitía la descarga oficial, no se exploraban cuentas hermanas registradas en la tabla `accounts`.

---

## 2. Cambios implementados

1. **Orden de fallback de cuentas sin mutar BD (`src-tauri/src/commands/accounts.rs`):**  
   - Se implementó la función pública `get_service_fallback_account_ids(db, service_name, preferred_account_id)` (`accounts.rs:311-356`).  
   - La consulta ordena las cuentas del servicio por:
     1. Cuenta preferida (si se solicita explícitamente y pertenece al servicio).
     2. Cuentas con credenciales no invalidadas (`credentials_invalid = 0`), con la activa primero (`is_active = 1`) y luego por ID descendente.
     3. Cuentas con `credentials_invalid = 1` como último recurso.
   - Preserva `is_active` en SQLite de forma inmutable.
   - Se añadió la prueba unitaria `test_get_service_fallback_account_ids_priority_and_immutability` (`accounts.rs:1882-1941`).

2. **Fallback multi-cuenta inmediato en el pipeline de Tidal (`src-tauri/src/services/tidal_pipeline.rs` y `src-tauri/src/download/tidal.rs`):**  
   - Se añadió el campo `pub account_id: Option<i64>` con `#[serde(default)]` a `TidalSingleTrackRequest` (`tidal_pipeline.rs:136`).
   - En `execute_tidal_single_track_download_inner` (`tidal_pipeline.rs:801-1275`), se reemplazó la resolución fija de una sola cuenta por una iteración sobre `get_service_fallback_account_ids`:
     - Se resuelven las credenciales con `resolve_and_refresh_gui_credentials_for_account(db, &http_client, Some(cand_account_id), false)`.
     - Si la cuenta falla en auth, búsqueda o resolución de stream (HTTP 401/403, `PlaybackUnauthorized`, `requested_lossless_but_received_aac`), se registra el error concreto en `last_auth_error` para esa cuenta específica en SQLite y se pasa de inmediato a la siguiente cuenta candidata del servicio.
     - Si una cuenta obtiene la resolución de stream con éxito, se captura el contexto y se continúa directamente a la descarga física del audio sin penalización de rendimiento.
     - Si todas las cuentas candidatas fallan, se propaga el error correspondiente con evento `CandidateRejected`.
   - En `download/tidal.rs:74`, se inicializa `account_id: None` al construir `TidalSingleTrackRequest`.
   - Se añadió la prueba unitaria `test_tidal_multi_account_fallback_priority_and_exclusivity` (`download/tidal.rs:381-470`).

3. **Fallback multi-cuenta en el descargador de Qobuz (`src-tauri/src/download/qobuz.rs`):**  
   - Se extrajo el método `resolve_token_for_account(&self, pool, account_id: Option<i64>)` (`qobuz.rs:282-390`), manteniendo `resolve_token` como delegador hacia la cuenta activa.
   - En `download_track` (`qobuz.rs:1145-1310`), cuando hay un pool de base de datos disponible, se obtienen las cuentas candidatas con `get_service_fallback_account_ids(pool, "qobuz", None)`. Si la cuenta activa falla en autenticación o resolución de stream oficial, se intenta de inmediato la siguiente cuenta de Qobuz antes de reportar error o recurrir a proxies comunitarios.
   - Se añadió la prueba unitaria `test_qobuz_multi_account_token_resolution_and_fallback_order` (`qobuz.rs:2600-2675`).

4. **Compatibilidad en tests (`src-tauri/tests/`):**  
   - Se añadió `account_id: None` a las inicializaciones literales de `TidalSingleTrackRequest` en `tidal_metadata_enrichment_s156_test.rs:482`, `s161_live_network_test.rs:296` y `operation_recovery_fault_injection_test.rs:834`.

---

## 3. Verificación y pruebas ejecutadas

Se ejecutaron pruebas focalizadas en el backend de Rust con las siguientes salidas:

1. **Pruebas de fallback multi-cuenta:**
   - Comando: `cargo test --lib fallback --manifest-path src-tauri/Cargo.toml`
   - Resultado: 11 passed; 0 failed.
   - Pruebas clave validadas:
     - `commands::accounts::accounts_tests::test_get_service_fallback_account_ids_priority_and_immutability ... ok`
     - `download::tidal::tests::test_tidal_multi_account_fallback_priority_and_exclusivity ... ok`
     - `download::qobuz::tests::test_qobuz_multi_account_token_resolution_and_fallback_order ... ok`

2. **Suite de autenticación multi-cuenta existente:**
   - Comando: `cargo test --test multi_account_auth_test --manifest-path src-tauri/Cargo.toml`
   - Resultado: 18 passed; 0 failed.

3. **Suite de autenticación y descarga de Tidal:**
   - Comando: `cargo test --test tidal_auth_sync_download_test --manifest-path src-tauri/Cargo.toml`
   - Resultado: 4 passed; 0 failed.

4. **Suite E2E de paridad de Qobuz:**
   - Comando: `cargo test --test qobuz_parity_e2e_test --manifest-path src-tauri/Cargo.toml`
   - Resultado: 6 passed; 0 failed.

5. **Verificación de compilación de pruebas completas:**
   - Comando: `cargo check --tests --manifest-path src-tauri/Cargo.toml`
   - Resultado: Compilación limpia sin errores.

---

## 4. Qué queda abierto

- **Selección explícita de cuenta por ítem de cola:** Actualmente la cola de descargas procesa pistas sin forzar un `account_id` específico, aplicando el orden de fallback automático (cuenta activa primero, luego secundarias). Si en el futuro se desea permitir al usuario forzar qué cuenta específica descarga una pista desde la UI de la cola, la infraestructura ya está preparada (bastará con mapear `account_id` a `TidalSingleTrackRequest`).

---

## 5. Riesgos evaluados

- **Impacto en usuarios con una sola cuenta:** Nulo. Cuando existe una sola cuenta para el servicio, el orden de fallback contiene un único elemento y se ejecuta exactamente como antes.
- **Invariante de exclusividad:** Preservado al 100%. `is_active` nunca se muta en la base de datos durante la resolución o el cambio de cuenta para la descarga.
- **Rendimiento:** El fallback es inmediato y en memoria; no introduce bloqueos de red innecesarios ni ciclos de reintento en cola.

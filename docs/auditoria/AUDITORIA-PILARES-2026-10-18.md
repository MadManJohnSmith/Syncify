# Auditoría Syncify contra pilares de programación y optimización para agentes

**Commit auditado:** `c0ade98` (`chore/limpieza-y-conciliacion`, padre = `origin/dev`)
**Árbol:** limpio (`git status --porcelain` vacío) al empezar y al terminar.
**Toolchain:** cargo/rustc 1.98.1, node v26.11.1, npm 12.2.1, Python 3.14.8 (`.venv`, 58 paquetes).
**Modo:** solo lectura sobre el producto. Cero ediciones, renombrados, borrados, formato ni cambios de dependencias. Este documento es el único entregable.

---

## 1. Veredicto ejecutivo

| Puerta | Comando | Resultado |
|---|---|---|
| Frontend build | `npm --prefix ui run build` | **VERDE** (exit 0, 21.61 s, vue-tsc + vite) |
| Frontend tests | `vitest run` | **VERDE** (125 ficheros, 1075 tests) |
| Python | `unittest discover -s scripts/tests` | **VERDE** (307 tests, 2 skip) |
| Rust check | `cargo check --workspace --all-targets --locked` | **VERDE** (exit 0, 0 warnings, 3m54s) |
| Rust clippy | `cargo clippy --workspace --all-targets --locked -- -D warnings` | **ROJO** (exit 101, 1 lint) |
| Rust fmt | `cargo fmt --all -- --check` | **ROJO** (exit 1, 23 hunks / 9 ficheros) |
| Rust tests | `cargo test --locked` | **ROJO** (fail-fast en el primer binario) |
| Rust tests (completo) | `cargo test --locked --no-fail-fast` | **ROJO en `c0ade98`: 318 objetivos, 2753 pasan, 4 fallan, 13 ignorados**. Tras la reparación de §16: **316 objetivos ok, 2756 pasan, 2 fallan, 13 ignorados** |

**El titular:** 3 de las 5 puertas Rust están en rojo en HEAD. Dos fallos son deriva de toolchain (no hay `rust-toolchain.toml`; CI instala `@stable` flotante), uno es un contrato IPC roto de verdad, y dos son fixtures de test que violan una FK del propio esquema.

**Lo bueno, que es mucho y hay que decirlo:** 48 sitios de pánico en 120 466 líneas de producción (0,40 por 1000 líneas), 0 `panic!`, 1 bloque `unsafe`, 0 canales sin cota, 27 `spawn_blocking`, 0 nombres de test perezosos sobre 2184 funciones de test, y puertas de higiene escritas por el propio repo que vigilan artefactos, docs, README y simetría de eventos.

---

> **Estado de reparación:** la auditoría se ejecutó primero en solo lectura (producto intacto en `c0ade98`). La fase de reparación corre en la rama `fix/puertas-rust-verdes`; el registro commit a commit, con la evidencia rojo→verde de cada uno, está en **§16**.

## 2. Método y desviaciones (leer antes de los números)

- Cada cifra de este informe sale de un comando ejecutado; los recuentos heurísticos van etiquetados **OBSERVED** (medido directamente) o **INFERRED** (proxy declarado).
- **Convención de rutas:** todas las citas resuelven desde la raíz del repo `Syncify/`; se abrevian a la ruta mínima inequívoca (la ambigüedad de basename está eliminada: 0 citas dependen de adivinar el fichero).
- **Método de recuento:** los totales de producción excluyen solo bloques `#[cfg(test)] mod … { }` (llaves balanceadas), **no** "todo lo que va después del primer `#[cfg(test)]`". Ese atajo infravaloraba ficheros con un ítem de test a mitad; al corregirlo, producción son 121 ficheros / 120 466 líneas (los módulos de test inline suman 20 543 líneas aparte) y los recuentos de pánico subieron de 45 a 48.
- **Desviación 1 (sandbox):** la política de ficheros pasó a `workspace-write` a mitad de la auditoría y dejó `/home/alan/.cache` en solo lectura para comandos nuevos. El `CARGO_TARGET_DIR` que pediste (`~/.cache/syncify-audit/target`) quedó inservible; la suite completa se relanzó con el target en `/home/alan/Documents/Syncify/.dsh-scratch/` (fuera del repo, dentro del workspace). Los 8,5 G de `~/.cache/syncify-audit/target` y los 8,7 G de `.dsh-scratch/syncify-audit-target` son residuos de la auditoría, borrables cuando quieras.
- **Desviación 2 (skill no invocable):** `improve-codebase-architecture` tiene `disable-model-invocation`: no puedo cargarla, hay que invocarla tú. Usé el vocabulario de `codebase-design` (módulo, interfaz, implementación, profundidad, seam, adapter, leverage, locality) en su lugar.
- Dos correcciones de mi propia contabilidad, porque casi reporto falsos positivos: (a) mi primera matriz IPC decía "50 comandos inalcanzables" y era mi regex truncando el bloque `generate_handler!` de 358 líneas; (b) dije "23 ficheros con drift de formato" cuando son **23 hunks en 9 ficheros**. Ambos corregidos abajo.

---

## 3. A1 — Corrección en Rust

**Veredicto: notable alto. Es el eje más sano del repo.** La disciplina de panic es de nivel producción real.

| Métrica (producción, sin tests) | Valor | Etiqueta |
|---|---|---|
| Ficheros / líneas | 121 / 120 466 | OBSERVED |
| `.unwrap()` | 24 | OBSERVED |
| `.expect(` | 19 | OBSERVED |
| `panic!` | 0 | OBSERVED |
| `todo!`/`unimplemented!` | 0 | OBSERVED |
| `unreachable!` | 5 | OBSERVED |
| Total pánico | **48 (0,40 / 1000 líneas)** | OBSERVED |
| `unsafe {` | 1 | OBSERVED |
| Líneas en módulos de test inline | 20 543 | OBSERVED |

Los sitios de pánico de producción son, casi todos, legítimos y de arranque: regex estáticas compiladas una vez (`src-tauri/src/logs_history.rs:150` con `expect`, `crates/syncify-lyrics-domain/src/lib.rs:301` con `unwrap`), inicialización del runtime y de la BD (`src-tauri/src/main.rs:87`, `:187`, `:1325`), migración embebida (`src-tauri/src/db.rs:201`, con el mensaje "la migración 0063 está embebida en el binario") y `SystemTime::duration_since(UNIX_EPOCH)` (`crates/syncify-metadata-domain/src/lib.rs:994`).

Excepciones que sí son rutas de pánico en caliente, no de arranque:

- `commands/tools.rs:526` y `tools.rs:551`: `.expect("library watchers mutex poisoned")` — un pánico ajeno envenenaría el mutex y convertiría el siguiente comando en pánico. **55 líneas de diferencia entre ambos.**
- `commands/playback.rs:307`: `Response::builder().status(500).body(Vec::new()).unwrap()` **dentro de una ruta de error**. Es un pánico al construir la respuesta del 500, es decir, doble fallo.
- `commands/queue.rs:562` `valid_candidates.into_iter().next().unwrap()`, `commands/favorites.rs:2364` `track.expect("checked above")`, `download/qobuz.rs` y `flac-writer:1249` con `unwrap()` sobre `Option` calculado: correctos hoy, frágiles ante el próximo refactor.

**Concurrencia y async (lo verifiqué uno a uno, no por grep):**

| Patrón | Sitios | Riesgo |
|---|---|---|
| `std::sync::Mutex/RwLock` | 21 | Revisados; **ninguno** cruza un `.await` real |
| `tokio::sync::Mutex` | 12 | Correcto por diseño |
| `block_on` | 9 | En arranque/API síncrona, no en handlers async |
| `tokio::spawn` | 26 | Tareas delegadas; sin `unbounded` |
| canales sin cota | **0** | Bien: hay backpressure |
| `spawn_blocking` | 27 | Bien: no se bloquea el executor |

Los tres candidatos que mi heurística marcó como "lock cruzando await" resultaron **correctos** y conviene registrarlo para que nadie los "arregle":
`services/musicbrainz.rs:299-308` cierra el guard en un bloque antes del `sleep(...).await` (patrón de manual);
`src-tauri/src/services/operation_recovery.rs:26-29` usa `tokio::sync::Mutex`;
`crates/syncify-tidal-downloader/src/lib.rs:3311` está dentro de un `#[cfg(test)]` que envenena el lock a propósito.

**Único hallazgo real de ciclo de vida:** `cmd_utils.rs:331` `run_command_with_timeout` envuelve `cmd.output()` en `tokio::time::timeout` sin `kill_on_drop(true)`. Al expirar, el futuro se descarta pero el hijo **no** se mata: queda un proceso huérfano. Mitigante importante: hoy **no tiene ningún llamador en producción** (solo `src-tauri/tests/cmd_utils_test.rs:341-382`), así que es una trampa latente para el próximo que lo use, no un bug activo. Los sitios que sí matan al hijo son `tools.rs:367`, `auth.rs:127`, `mb_dump_import.rs:147`.

**El hallazgo grande de A1 no son los panics, es el silencio:** 575 `let _ =` en producción, de los cuales **243 son `let _ = sqlx::query(...)`** — escrituras cuyo `Result` se descarta sin log. Por tipo de sentencia: **125 `UPDATE`**, 82 `INSERT` (incluye `OR IGNORE`/`OR REPLACE`), 11 `DELETE`, y 25 son consultas dinámicas sin literal a la vista. Muchas están dentro de una transacción cuyo `commit` sí se comprueba, así que el riesgo real no es "se pierde todo" sino **"se pierde parte y la transacción confirma"** sin rastro en el log. Reparto: `services/enrichment.rs` 45, `commands/service.rs` 28, `services/qobuz.rs` 21, `commands/favorites.rs` 19, `commands/library.rs` 18, `services/tidal.rs` 17, `services/musicbrainz.rs` 13, `services/operation_recovery.rs` 11.

Contexto que evita exagerar: el resto de familias de `let _ =` sí es descarte deliberado y correcto — 58 `tokio::fs::remove_file` + 33 `std::fs::remove_file` (limpieza de temporales), 63 de la familia `emit` (si no hay ventana, el emit falla y da igual), 12 `tx.send`.

---

## 4. A2 — Integridad de persistencia

**Veredicto: la base es sólida; el rojo está justo aquí, en las pruebas.**

Lo que está bien y es caro de conseguir:

- Migraciones **embebidas en el binario** (`db.rs:40` `sqlx::migrate!("./migrations")`), aplicadas al arrancar (`db.rs:74`), 97 ficheros numerados 1..101 (huecos 26–29, sin ficheros `down`; sqlx no los necesita).
- Pool: `max_connections(10)`, `acquire_timeout(10 s)` y, en `after_connect` (`db.rs:57-64`), `PRAGMA foreign_keys = ON`, `journal_mode = WAL`, `busy_timeout = 30000`. Esto es lo correcto y mucha gente lo olvida.
- Idempotencia apoyada en el esquema, no en el código: `UNIQUE(service_name, service_track_id)` (`0001_init.sql:25,50`), `UNIQUE(service_id, email)` (`0002:47`), `UNIQUE(track_id, service_id)` (`0002:125`), `UNIQUE(account_id, track_id)` (`0002:144`), `UNIQUE(playlist_id, track_id)` (`0002:170`), `UNIQUE(track_id, format)` (`0002:229`), índice único `downloads(file_path)` (`0003:60`), `library_snapshots(snapshot_date)` (`0015:21`). Refuerzos: 123 `ON CONFLICT`, 124 `INSERT OR IGNORE/REPLACE`, 26 transacciones.
- Verifiqué el falso positivo obvio: `idx_downloads_unique_path` se crea en `0003:60` **y** en `0004:72`, pero no es un bug: `0004` recrea la tabla (`ALTER TABLE downloads_new RENAME TO downloads`), con lo que el índice viejo desaparece con ella.
- Dos relajaciones **deliberadas** de unicidad, documentadas en migración: `0064:185 DROP INDEX idx_tracks_isrc_unique` y `0076:8 DROP INDEX idx_artists_name_unique`. La deduplicación de ISRC y de nombres de artista pasó a lógica de aplicación. No es deuda por descuido; es una decisión con coste: sin índice, dos pistas con el mismo ISRC solo se detectan si el código las compara.

**Los dos rojos de persistencia (y su causa raíz, que localicé):**

1. `featuring_and_appearances_test::test_enrichment_associates_featured_in_track_artists` (`:219`) →
   `library entry persistence failed: error returned from database: (code: 787) FOREIGN KEY constraint failed`.
2. `social_metadata_backfill_test::test_enrichment_engine_canonical_year_and_genre_derivation` (`:398`) →
   `favorite item persistence failed: ... (code: 787) FOREIGN KEY constraint failed`.

`787` = `SQLITE_CONSTRAINT_FOREIGNKEY`. La FK viene del esquema original: `library_entries.account_id INTEGER NOT NULL REFERENCES accounts(id)` (`src-tauri/migrations/0002_normalize_schema.sql:138`). El test conecta con `SqlitePool::connect(":memory:")` y pasa `account_id: 1` **sin insertar la cuenta**. La prueba de que es el fixture y no el producto: de los 11 ficheros de test que llaman a `enrich_and_persist_sync_track`, **9 siembran `accounts` y 2 no** — y son exactamente esos 2 los que fallan (`featuring_and_appearances` accounts=0, `social_metadata_backfill` accounts=0). El producto tampoco crea la cuenta en ese camino (`src-tauri/src/services/enrichment.rs:3478` inserta en `library_entries` y asume la cuenta preexistente), lo cual es razonable: la cuenta la crea el flujo de importación.

**No lo toqué, y no hay que arreglarlo relajando el esquema.** El arreglo correcto es que el fixture siembre la cuenta (o que un helper común de fixture lo haga por todos), no quitar la FK ni saltarse el test.

El tercer rojo no es de FK sino de identidad de edición: `track_identity_and_tagging_test::test_same_artist_and_title_in_different_albums` (`:137`) espera "edition review" y el motor devuelve `is_already_present: true, is_new_global_track: false`. Este sí puede ser una regresión de comportamiento (resolución de identidad) y no una fixture: **es el único de los cuatro que exige análisis de producto, no de test.**

---

## 5. A3 — Contratos IPC

**Veredicto: arquitectura de contrato buena con tres grietas concretas, una de ellas funcional y silenciosa.**

Números medidos con parsing real (llaves balanceadas, sin regex ingenuo):

| Superficie | Cantidad |
|---|---|
| `#[tauri::command]` declarados | **304** |
| Entradas en `generate_handler!` (`main.rs:965`, bloque de 358 líneas) | **306** |
| Ficheros TS/Vue que consumen | 233 |
| Emisores `.emit(` en Rust | **80** (0 `emit_to`, 0 `emit_filter`) |
| Constantes en el catálogo `TauriEvents` (`ui/src/api/tauri.ts`) | 41 |

**Grieta 1 — comando registrado que nadie invoca (es el rojo de la suite):**
`resolve_playback_source` está declarado (`commands/playback.rs:38`) y registrado (`main.rs:1115`), pero no aparece en `ui/src`. La puerta del propio repo lo caza: `dead_commands_hygiene_test::test_every_registered_command_is_invoked_by_the_frontend` → `orphans: ["resolve_playback_source"]`. Mi análisis independiente llegó al mismo único huérfano antes de leer ese test, lo que da confianza al método.

**Grieta 2 — la UI llama a un comando que no existe (funcionalidad muerta silenciosa):**
`ui/src/views/LibraryView.vue:1688` hace `await invoke('set_max_concurrency', { maxConcurrent: threads })`. Ese comando **no existe en Rust** (grep en todo el repo: solo aparece en la UI, en un mock de test y en un borrador `.zcode/`). La llamada está envuelta en `try/catch` que solo hace `console.warn`, así que el control de "concurrencia viva" de la interfaz **no hace nada y no avisa**. Peor: `ui/src/__tests__/views/LibraryView.spec.ts:1220` lo mockea, de modo que el test pone verde una capacidad que en producción no existe. Es el ejemplo perfecto de test como especificación invertida (B3).

**Grieta 3 — atributo espurio:** `src-tauri/src/commands/settings.rs:15-17`: el `#[tauri::command]` de la línea 15 va pegado a `perform_get_service_preferences`, un helper que recibe `db: &crate::DbPool` y no está registrado (correctamente). Además el orden es raro: atributo, luego doc-comment, luego `fn`. No rompe nada; es basura de contrato.

Falsos positivos que descarté (no los reportes como fallo): `analyze_library_bpm`, `cancel_bpm_analysis` y `update_track_bpm_manual` usan el alias `use tauri::{command, AppHandle}` (`commands/tempo.rs:6`) en vez de `#[tauri::command]`; están declarados y registrados.

**Eventos: aquí el repo está por delante de la media.** La UI no compara strings sueltos: escucha a través de un catálogo de constantes (`TauriEvents`) duplicado en `@/api/tauri` y `@/composables/useEventBus`, con un test que (a) exige que ambos catálogos sean idénticos y (b) escanea Rust buscando `.emit("<evento>"` para comprobar que todo lo emitido tiene constante (`ui/src/__tests__/api/tauri_events_symmetry.spec.ts`). Verifiqué la cobertura real del escáner: hay 80 `.emit(` y **cero** `emit_to`/`emit_filter`/comilla simple, así que el regex no tiene puntos ciegos hoy. El día que alguien use `emit_to`, el escáner se queda ciego: eso es una grieta de la puerta, no del contrato.

**Puente Rust↔Python: el mejor diseño del repo.** `src-tauri/src/cmd_utils.rs` es un **módulo profundo** de verdad: interfaz pequeña (`create_python_tokio_command`, `create_python_std_command`, `resolve_tool`, `apply_bundled_tool_env`, `find_scripts_dir`, `packaged_python_candidates`) que esconde resolución de binarios empaquetados, `PYTHONUNBUFFERED`, `PYTHONIOENCODING=utf-8`, `PYTHONPATH`, `current_dir` y candidatos de Python empaquetado. Es el patrón a replicar, no a tocar. Encima hay 10 ficheros de test de contrato en Python (`test_auth_bridge_action_contract.py`, `test_download_bridge_contract.py`, `test_fingerprint_bridge_contract.py`, `test_metadata_bridge_robustness.py`, `test_bridge_cli_documentation.py`…) y los 307 tests Python pasan.

---

## 6. A4 — Red de seguridad

**Veredicto: red amplia y bien nombrada, con cuatro agujeros de ejecución y ocho módulos sin referencia.**

- **318 binarios** de test Rust; **2753 tests pasan, 4 fallan, 13 ignorados** (medido en `c0ade98`; tras la reparación, ver §16). La suite se declara en **302 ficheros** (`src-tauri/tests` 295 + `crates/*/tests` 7) con **534 `#[test]` + 1090 `#[tokio::test]` + 13 `#[ignore]`**. Además **125 ficheros / 1075 tests de UI** (exit 0, re-verificado) y **307 tests Python OK** (2 skip, re-verificado).
- **Nombres: 0 perezosos** sobre las **2184 funciones** declaradas en `tests/` (1346 de ellas `fn test*`): ningún `test`, `test_1`, `t1`. Los nombres declaran el contrato: `test_every_registered_command_is_invoked_by_the_frontend`, `test_generate_handler_has_no_duplicate_registrations`, `test_purged_in5_commands_are_gone_from_source_and_handler`.
- **Los 13 `#[ignore]` son legítimos**, todos con razón explícita: red viva/credenciales/almacenamiento físico/benchmarks (`streaming_live.rs` ×4, `batch_cache_optimization_and_parity_test.rs` ×2, y 1 en cada uno de `tidal_sync_persistence`, `s167_live_preflight_50_audit`, `s167_live_execution_50_audit`, `s164_fresh_import`, `s161_live_network`, `live_network_pilot_20_audit`, `live_network_pilot_10_audit`).
- **Drift doc↔código:** `README.md:278` afirma "The 9 ignored tests"; en HEAD hay **13**. El párrafo está acotado al tag `v0.6.0`, así que no es una mentira histórica, pero un lector en HEAD lee un número falso. INFERRED.
- **Cobertura por módulo (proxy declarado):** de 86 módulos de producción ≥60 líneas, 78 aparecen referenciados por tests y **8 no**: `services/mpris.rs` (570), `services/silence_trimmer.rs` (482), `services/catalog_identity_audit.rs` (472), `logs_history.rs` (450), `system_conditions.rs` (273), `services/migration_tests.rs` (269, que es en sí un `cfg(test)`), `sync_scheduler.rs` (140), `services/user_reports.rs` (134). Que `mpris.rs` (570 líneas, D-Bus/MPRIS) no tenga ninguna referencia es coherente con lo que el propio README declara como no probado.
- **Puertas de higiene propias, que son un activo:** `test_tracked_artifact_hygiene.py`, `test_docs_consistency.py` (valida que las rutas en backticks de los docs existan), `test_python_hygiene.py`, `test_ci_packaging_exclusions_hygiene.py`, `test_readme_claims.py`, `dead_commands_hygiene_test.rs` (8 aserciones), `tauri_events_symmetry.spec.ts`.
- **El agujero de ejecución:** `cargo test --locked` es fail-fast, así que en CI el primer binario rojo (`dead_commands_hygiene_test`, alfabéticamente temprano) **oculta los otros tres**. Mi corrida con `--no-fail-fast` es la que destapó los 4.

---

## 7. A5 — Puertas automáticas

**Veredicto: las puertas existen y son serias; en HEAD están rojas por tres motivos distintos, y ninguno se ve desde el badge.**

- CI (`.github/workflows/ci.yml`) ejecuta, en orden: `cargo check --all-targets --locked` (l.96-97), `cargo test --locked` (99-100), `cargo test --test batch_50_audit_test -- --test-threads=1` (102-104), clippy estricto (106-107), `cargo fmt --all -- --check` (109-110), y en otros jobs `npm ci && npm run build`, `vue-tsc --noEmit`, `vitest run`, `unittest`.
- **No hay `rust-toolchain.toml`** y CI instala `dtolnay/rust-toolchain@stable` (ci.yml:75-78): la puerta de formato y la de lints se mueven con cada release de Rust. Eso explica los dos rojos "cosméticos": `unnecessary_map_or` es un lint de Rust 1.98, y 23 hunks de rustfmt son deriva de versión, no código sucio. **Consecuencia práctica: el mismo commit puede estar verde un mes y rojo al siguiente sin que nadie toque una línea.**
- **Rojo 1 (clippy):** `crates/syncify-tidal-downloader/src/lib.rs:1054` `if best.map_or(true, |(_, best_score)| score > best_score)` → clippy pide `is_none_or`. Exit 101.
- **Rojo 2 (fmt):** exit 1; 23 hunks en 9 ficheros: `crates/syncify-core-domain/src/metadata.rs` (3), `crates/syncify-fmp4-flac/src/lib.rs` (4), `crates/syncify-fmp4-flac/tests/fmp4_flac.rs` (1), `crates/syncify-tidal-downloader/src/lib.rs` (5), `src-tauri/src/download/identity.rs` (2), `src-tauri/src/download/orchestrator.rs` (2), `src-tauri/src/services/streaming.rs` (2), `src-tauri/tests/s197_live_isrc_resolution_test.rs` (1), `src-tauri/tests/streaming_live.rs` (3). Son reescrituras de rustfmt (líneas partidas/agrupadas), sin cambio semántico.
- **Rojo 3 (tests):** los 4 fallos de A2/A3.

---

## 8. B1 — Módulos profundos

**Veredicto: el punto más débil del repo. Hay un módulo profundo modélico (`cmd_utils.rs`) y, al lado, funciones de 4 556 líneas.**

Medido sobre `src-tauri/src` descontando bloques `#[cfg(test)]` (**1863 funciones**; tamaño = distancia hasta la siguiente declaración `fn`): mediana **23 líneas**, p90 **106**, p99 **445**, máximo **4559**. Hay **211 funciones ≥100 líneas, 74 ≥200, 25 ≥400 y 7 ≥1000**.

| Líneas | Función | Ubicación |
|---|---|---|
| **4556** | `perform_sync_service_with_emitter_inner` | `commands/service.rs:3285` (fin verificado en 7840) |
| **2860** | `resolve_token_for_account` | `src-tauri/src/download/qobuz.rs:417` (fin verificado en 3276) |
| 2668 | `execute_tidal_single_track_download_inner` | `services/tidal_pipeline.rs:1002` |
| 1402 | `enrich_and_persist_sync_track` | `src-tauri/src/services/enrichment.rs:2155` |
| 1289 | `main` | `src-tauri/src/main.rs:38` |
| 1216 | `resolve_exhaustive_track_metadata_with_force` | `src-tauri/src/services/enrichment.rs:268` |
| 1209 | `perform_sync_favorites` | `commands/favorites.rs:1465` |
| 1036 | `process_downloaded_staging` | `src-tauri/src/download/qobuz.rs:2240` |
| 946 | `process_download_internal` | `worker.rs:1334` |
| 825 | `auto_resolve_duplicates_inner` | `commands/library.rs:3543` |

Verifiqué los dos mayores con cierre de llaves real: **no es artefacto de la heurística**. Una función de 2 860 líneas llamada `resolve_token_for_account` (resolver un token) es la definición de módulo superficial: interfaz de una línea, implementación inabarcable, sin seam interno donde meter una prueba.

**Lo que hay que copiar, no tocar:** `cmd_utils.rs` (interfaz pequeña / implementación grande), `db.rs` (pool, pragmas y migraciones en un solo sitio), `services/import_pagination.rs::normalize_added_at` (seam diminuto y testeable). Ese es el vocabulario y el nivel al que hay que llevar el resto.

**Diagnóstico de fondo:** el acoplamiento no está entre módulos, está *dentro* de las funciones. `perform_sync_service_with_emitter_inner` mezcla orquestación de sincronización, escrituras SQL y emisión de eventos en un solo cuerpo: no hay costura entre "decidir qué hacer" y "hacerlo", que es exactamente lo que impide testear sin base de datos.

---

## 9. B2 — Descubribilidad para un agente nuevo

**Veredicto: buena documentación humana, pésima puerta de entrada para agentes.**

- **No existe `AGENTS.md`, ni `CLAUDE.md`, ni `.cursorrules`, ni `.github/copilot-instructions.md`** en ningún sitio del árbol (búsqueda exhaustiva). Un agente nuevo no tiene mapa; tiene que grep-ear 354 796 líneas (141 102 de Rust en `src/`, 112 558 en `tests/`, 83 473 de TS/Vue y 17 663 de Python).
- El único artefacto para agentes es `.agents/skills/syncify-screenshots/SKILL.md` (1 fichero versionado). Además hay un directorio `skills/` en la raíz **vacío y sin versionar**: residuo, no intención.
- Lo humano está muy bien: `README.md` + `README.es.md`, `docs/README.md` + `docs/README.en.md`, `CONTRIBUTING.md` bilingüe con los comandos, `docs/design-system.md`, 3 `docs/DECISION_*.md`, 7 `docs/features/`, `docs/auditoria/`, `docs/diagrams/`, notas de release y un `docs/Deuda_Tecnica_y_UX.md`. Los comandos deterministas están escritos (`README.md:289-315`).
- **El fallo que más duele y que ya se cobró una víctima:** sin toolchain fijada, un agente no puede reproducir CI. La puerta de formato/lints se mueve sola y el agente que "arregla" el rojo reformateando 9 ficheros mezcla un cambio semántico-adyacente con el arreglo real.
- No hay `GLOSSARY.md` (la skill `domain-modeling` lo espera) ni ADRs con formato ADR: las decisiones viven en `docs/DECISION_*` con prosa, no con contrato.

---

## 10. B3 — Tests como especificación

**Veredicto: disciplina alta, con un antipatrón localizado y muy dañino.**

- 0 nombres perezosos sobre 2184 funciones de test; los tests declaran el contrato protegido.
- El repo usa tests como puertas de arquitectura, no solo de comportamiento: `dead_commands_hygiene_test` (8 aserciones sobre el registro IPC), `tauri_events_symmetry.spec.ts` (catálogo de eventos), `test_readme_claims.py`, `test_docs_consistency.py`. Eso es exactamente "tests como especificación".
- **Antipatrón:** `ui/src/__tests__/views/LibraryView.spec.ts:1220` mockea `set_max_concurrency`, un comando que no existe en Rust. El test verifica la *forma* de una capacidad imaginaria y la pone verde. Es tautológico por construcción: el mock es la única fuente de verdad de un contrato que no existe.
- Segundo detalle de calidad: `#[allow(dead_code)]` aparece **188 veces** en producción, y **89 de ellas** llevan la justificación "Cubierta por `tests/...`" en la misma línea. Es una práctica legítima (API que solo ejercitan los tests de integración) pero es un riesgo de descubribilidad: un agente que lea `cmd_utils.rs` ve funciones `pub` y puede creer que están en el camino de producción. `run_command_with_timeout` es el caso puro: pública, sin llamadores, viva solo por sus tests.

---

## 11. B4 — Navegabilidad y acoplamiento

**Veredicto: el repo se navega mal por tamaño, no por desorden.**

25 ficheros por encima de 2 300 líneas — 20 de producción Rust y 5 vistas Vue, ninguno de test. Reparto medido: **producción Rust 121 ficheros / 120 466 líneas** (+20 543 en módulos de test inline), **tests Rust en `tests/` 302 ficheros / 112 558 líneas**, **UI `ui/src` 233 ficheros / 83 473 líneas**, **Python `scripts/` 36 ficheros / 17 663 líneas**. El producto escrito a mano ronda las 221 600 líneas; el resto es red de seguridad:

| Líneas | Fichero |
|---|---|
| 8482 | `src-tauri/src/commands/service.rs` |
| 7762 | `src-tauri/src/commands/library.rs` |
| 5025 | `src-tauri/src/services/tidal_pipeline.rs` |
| 4643 | `src-tauri/src/services/enrichment.rs` |
| 4344 | `src-tauri/src/download/orchestrator.rs` |
| 4050 | `src-tauri/src/download/qobuz.rs` |
| 3790 | `src-tauri/src/commands/lyrics.rs` |
| 3700 | `crates/syncify-flac-writer/src/lib.rs` |
| 3575 | `src-tauri/src/commands/settings.rs` |
| 3337 / 3325 / 3325 | `services/tidal.rs` / `download/lyrics.rs` / `crates/syncify-tidal-downloader` |
| 3290 / 3212 / 3185 | `commands/favorites.rs` / `ui/src/views/MetadataView.vue` / `ui/src/views/LyricsView.vue` |
| 3184 / 3058 / 3002 / 2955 | `worker.rs` / `commands/accounts.rs` / `commands/queue.rs` / `ui/src/views/LibraryView.vue` |

**Conceptos duplicados por capas** (esto es acoplamiento, no tamaño): "qobuz" vive en `download/qobuz.rs` (4050) + `services/qobuz.rs` (2080) + `scripts/services/qobuz_auth.py` (742) + `qobuz_service.py` (1532). "tidal" vive en `services/tidal.rs` (3337) + `services/tidal_pipeline.rs` (5025) + `crates/syncify-tidal-downloader` (3325) + `tidal_auth.py` (442) + `tidal_service.py` (1026). Cuatro capas para el mismo proveedor significa que un cambio de contrato de Qobuz toca 4 sitios en 3 lenguajes.

Los tests tampoco ayudan a navegar: 112 558 líneas en 302 ficheros, con ficheros de 1 319, 1 260, 1 252, 1 170 y 1 157 líneas.

---

## 12. Brechas ordenadas por riesgo × esfuerzo

| # | Brecha | Riesgo | Esfuerzo | Etiqueta |
|---|---|---|---|---|
| 1 | `set_max_concurrency`: la UI llama a un comando inexistente y se traga el error (`LibraryView.vue:1688`) | **Alto** (funcionalidad muerta silenciosa) | Bajo | OBSERVED |
| 2 | `resolve_playback_source` registrado sin consumidor: puerta `dead_commands_hygiene_test` en rojo | **Alto** (CI rojo) | Bajo | OBSERVED |
| 3 | 2 fixtures sin sembrar `accounts` → FK 787 (`featuring_and_appearances:219`, `social_metadata_backfill:398`) | **Alto** (CI rojo) | Bajo | OBSERVED |
| 4 | `track_identity_and_tagging:137` espera "edition review" y recibe `is_already_present: true` | **Alto** (posible regresión de identidad) | Medio | OBSERVED / causa INFERRED |
| 5 | Sin `rust-toolchain.toml`: fmt y clippy derivan con cada estable | Medio (rojo intermitente) | **Muy bajo** | OBSERVED |
| 6 | clippy `unnecessary_map_or` (`crates/syncify-tidal-downloader/src/lib.rs:1054`) | Medio (CI rojo) | Muy bajo | OBSERVED |
| 7 | 23 hunks de rustfmt en 9 ficheros | Bajo (cosmético, pero bloquea CI) | Muy bajo | OBSERVED |
| 8 | 243 `let _ = sqlx::query` (118 `UPDATE`) sin log | **Medio-alto** (pérdida parcial invisible) | Medio | OBSERVED patrón / INFERRED impacto |
| 9 | `run_command_with_timeout` no mata al hijo (latente, sin llamadores) | Medio (huérfanos al usarlo) | Muy bajo | OBSERVED |
| 10 | 2 `expect` sobre mutex envenenado (`tools.rs:526,551`) y un `unwrap` en ruta de error (`playback.rs:307`) | Medio | Bajo | OBSERVED |
| 11 | Funciones de 4 556 / 2 860 líneas sin seam | **Alto estructural** | Alto | OBSERVED |
| 12 | 8 módulos sin referencia en tests (encabeza `mpris.rs`, 570) | Medio | Medio | INFERRED |
| 13 | Sin `AGENTS.md` ni mapa por área; toolchain/documentación no determinista | Medio (coste por sesión de agente) | Bajo | OBSERVED |
| 14 | README dice "9 ignored tests", hay 13 | Bajo | Muy bajo | INFERRED |
| 15 | `#[tauri::command]` espurio en `src-tauri/src/commands/settings.rs:15` | Muy bajo | Muy bajo | OBSERVED |
| 16 | `skills/` vacío sin versionar; `.zcode/` no versionado citado por docs | Muy bajo | Muy bajo | OBSERVED |

**NO TOCAR (marcadas explícitamente):** las 188 anotaciones `#[allow(dead_code)]` (89 justificadas como cubiertas por tests) — no son código muerto demostrado; los 13 `#[ignore]`; las migraciones ya aplicadas, en particular `0002` y las relajaciones deliberadas `0064`/`0076`; el bloque `generate_handler!` de `main.rs`; el módulo `cmd_utils.rs`; el `.venv`, los dos `node_modules` y la BD personal `~/.local/share/com.syncify.app`.

---

## 13. Plan por fases (cada fase: cambio, test que lo respalda, no-pérdida, reversión)

**Fase 0 — Sanear las puertas sin tocar comportamiento (1 commit).**
Cambio: `cargo fmt --all`; `is_none_or` en `crates/syncify-tidal-downloader/src/lib.rs:1054`; añadir `rust-toolchain.toml` con `1.98.1`.
Test: las propias puertas (`fmt --check`, `clippy -D warnings`) pasan a verde. El `rust-toolchain` fija lo que CI ya instala.
No-pérdida: formato y un método equivalente de `Option`; cero cambio semántico.
Reversión: `git revert` del commit (el fmt es un commit aislado, por eso va primero y solo).

**Fase 1 — Cerrar los dos contratos IPC (red → green).**
Cambio A: `resolve_playback_source`. Dos opciones, hay que elegir con criterio: (i) cablearlo en la UI si la capacidad se quiere, o (ii) quitar su línea de `generate_handler!` y su `#[tauri::command]`. Yo recomiendo (ii) porque no hay consumidor, y si algún día se necesita se re-registra.
Cambio B: `set_max_concurrency`. O se implementa el comando (con el `worker` como destino real) o se borra la llamada de `LibraryView.vue:1688` y su mock del test. **Prohibido** dejarlo como está: hoy es un botón que miente.
Test: `dead_commands_hygiene_test` verde; un test de UI nuevo que afirme el comportamiento real de `setConcurrency` (si se borra, que no haya `invoke`; si se implementa, que el estado del worker cambie).
No-pérdida: si se elige borrar, se documenta en el commit que la capacidad nunca existió en backend.
Reversión: trivial, son líneas aisladas.

**Fase 2 — Reparar la red de seguridad de persistencia.**
Cambio: sembrar `accounts` en `featuring_and_appearances_test` y `social_metadata_backfill_test` (idealmente vía un helper de fixture compartido, para que el próximo test no repita el error). Para `track_identity_and_tagging:137`, **primero** un test que reproduzca el contrato de edición con datos mínimos y decidir si la expectativa o el motor es lo incorrecto.
Test: los 3 binarios pasan a verde; CI deja de estar rojo.
No-pérdida: no se toca esquema ni producto; la FK sigue vigente.
Reversión: `git revert`; los fixtures son test-only.

**Fase 3 — Endurecer sin cambiar comportamiento (aditivo).**
Cambio A: `kill_on_drop(true)` en `create_*_command` (o dentro de `run_command_with_timeout`).
Test nuevo (rojo antes, verde después): lanzar `sleep 5`, timeout de 100 ms, y afirmar que el hijo ya no existe.
Cambio B: helper `exec_best_effort(tx, query)` que hace `tracing::warn!` y sustituye los `let _ = sqlx::query(UPDATE…)` de forma incremental (empezando por `enrichment.rs`, el mayor).
Test nuevo: forzar un fallo (p.ej. `UPDATE` sobre columna inexistente en un fixture) y afirmar que se emite el warning; comportamiento observable del producto idéntico.
No-pérdida: por definición, el camino feliz no cambia; solo se añade observabilidad y muerte del hijo.
Reversión: cada helper es un punto único de reversión.

**Fase 4 — Seams donde hoy no hay costura (el trabajo caro).**
Cambio: extraer de `resolve_token_for_account` (2 860) y `perform_sync_service_with_emitter_inner` (4 556) subfunciones con nombre, **una por commit**, precedidas de tests de caracterización que fijen el comportamiento actual.
Test: caracterización primero (verde sobre el código sin tocar), refactor después (verde sin cambios en el test). Ese es el contrato de "no se perdió comportamiento".
No-pérdida: los tests de caracterización son la prueba; si un extracto cambia algo, el test se pone rojo.
Reversión: commits pequeños y mecánicos.

**Fase 5 — Puerta de entrada para agentes (aditivo puro).**
Cambio: `AGENTS.md` en la raíz (comandos deterministas, mapa de áreas, qué no tocar), `AGENTS.md` por área en `src-tauri/`, `ui/`, `scripts/`, `crates/`, y un `GLOSSARY.md` con el vocabulario del dominio (pista, edición, cuenta, entrada de biblioteca, disponibilidad…).
Test: extender las puertas de higiene existentes (`test_docs_consistency.py` ya valida rutas en backticks) para que también exija que `AGENTS.md` exista y que sus rutas resuelvan; así el mapa no se pudre.
No-pérdida: solo documentación y una puerta nueva.
Reversión: borrar los ficheros y la aserción.

---

## 14. Qué NO se debe tocar y por qué

1. **Las 188 anotaciones `#[allow(dead_code)]`** (89 con "Cubierta por `tests/…`"). No hay prueba de que nadie las use: la prueba es que los tests de integración las usan. Borrarlas rompe la suite. Si algún día se limpian, es con un test que demuestre que su cobertura desapareció, no por grep.
2. **Los 13 `#[ignore]`.** Son la única barrera que impide que la suite toque red real y credenciales. Quitarlos la convierte en una suite que falla sin red.
3. **Las migraciones aplicadas, y en particular `0002`** (FK y unicidad de las que depende todo) y las relajaciones deliberadas **`0064:185`** y **`0076:8`**. Un fichero de migración ya aplicado no se edita jamás: se añade uno nuevo.
4. **El bloque `generate_handler!` de `main.rs:965`** (358 líneas). Es la superficie de contrato: se toca de una entrada en una en una, nunca en bloque.
5. **`cmd_utils.rs`.** Es el módulo profundo modélico del repo. Solo se le añade `kill_on_drop`, nada más.
6. **Los 4 tests rojos no se arreglan relajándolos.** Quitar la FK, marcar `#[ignore]` o borrar la aserción de `resolve_playback_source` convierte la puerta en decorado. Se arreglan por la causa: fixture, cableado o implementación.
7. **`.venv`, `node_modules` (raíz y `ui`), los tres lockfiles.** Son el entorno de desarrollo; ya pagamos una vez el precio de borrarlos y el script de prueba manual se rompió.
8. **La BD personal `~/.local/share/com.syncify.app`, la biblioteca musical y las credenciales.** La auditoría entera corrió con fixtures en memoria (`:memory:`) precisamente por esto.
9. **`.zcode/`** (no versionado) pese a que `docs/DECISION_ALCANCE_BETA.md:13` lo cita: comprobé que `test_docs_consistency` pasa aunque `.zcode` desaparezca, así que no hay fallo latente en un clon limpio. No lo borres sin actualizar esa cita.
10. **El estilo de formato de los ficheros no tocados.** Si la Fase 0 reformatea, que sea en su propio commit: un `fmt` mezclado con lógica hace irrevisable el diff.

---

## 15. Reproducir cada número

```
git -C /home/alan/Documents/Syncify/Syncify rev-parse --short HEAD   # c0ade98
CARGO_TARGET_DIR=<fuera del repo> cargo check --workspace --all-targets --locked      # exit 0, 0 warnings
CARGO_TARGET_DIR=<fuera del repo> cargo clippy --workspace --all-targets --locked -- -D warnings   # exit 101
CARGO_TARGET_DIR=<fuera del repo> cargo fmt --all -- --check                          # exit 1, 23 hunks / 9 ficheros
CARGO_TARGET_DIR=<fuera del repo> cargo test --locked --no-fail-fast                  # c0ade98: 318 objetivos, 2753/4/13
CARGO_TARGET_DIR=<fuera del repo> cargo test --locked --no-fail-fast                  # rama de reparación: 316 ok, 2756/2/13
npm --prefix ui run build                                                            # exit 0
```
Los recuentos de A1/A3/B1/B4 salen de scripts de análisis en `/tmp/` (`census_panics.py`, `lockawait.py`, `ipc_matrix2.py`, `events_matrix.py`, `coverage.py`, `func_size.py`) y los logs completos de cada puerta están en `/home/alan/.cache/syncify-audit/` (lectura) y `.dsh-scratch/syncify-audit/` (última corrida).


## 16. Estado de reparación (rama `fix/puertas-rust-verdes`)

Ejecutada después de validar los claims, sobre `c0ade98`, en una rama propia. Cada commit
lleva su evidencia rojo→verde y su reversión (`git revert`).

| # | Commit | Fase | Qué cierra | Evidencia |
|---|--------|------|-----------|-----------|
| 1 | `ff11167` | 0 | `cargo fmt --all -- --check` | rojo (23 hunks/9 ficheros) → **exit 0**. Prueba de cero semántica: normalizando todo el whitespace **y las comas**, los 9 ficheros son idénticos a `HEAD` |
| 2 | `3174642` | 0 | 1.er lint clippy | `unnecessary_sort_by` en `src-tauri/src/download/qobuz.rs:1216` → `sort_by_key(Reverse(...))`, mismo orden y estabilidad |
| 3 | `f5e2317` | 1b | bug funcional + puerta que faltaba | ver abajo |
| 4 | `86f6ca4` | 1a | **revert** de la retirada del registro huérfano | ver abajo por qué |
| 5 | `41b2f08` | 2 | 2 objetivos de test rojos | fixtures sin cuenta → FK 787 |
| 6 | `b4a9588` | 5 | **B2: no había mapa para un agente** | `AGENTS.md` (87 líneas) + `test_docs_consistency` ahora lo escanea: con una referencia rota inyectada → `{'AGENTS.md': ['src-tauri/src/no_existe_jamas.rs']}`; sin ella → OK |
| 7 | `1e35108` | 5b | claim falso del README (9 vs 13) | puerta nueva: `README.md claims 9 ignored tests; the tree has 13` → **OK** tras corregir los dos front pages |
| 8 | `0ee9b63` | 5c | documentación desactualizada | deuda abierta D-05…D-08 + cierres C-12…C-18, nota de ruta retirada en `docs/features/PLAYER_PHASE1_MPRIS.md`, puntero a `AGENTS.md` en los dos índices; **308 tests Python OK** |
| 9 | `dd54731` | 5d | **B4: no había vocabulario de dominio** | `GLOSSARY.md` (90 líneas, 18 términos) que desambigua «migración» (SQL vs. `MigrationJob`) y `accounts` vs. la histórica `service_accounts`; escaneado por la puerta |
| 10 | `1167121` | 1a | **D-05**: subsistema de reproducción retirado aún registrado | `resolve_playback_source`, el protocolo y sus tests borrados; `playback.rs` 628 → 194 líneas; puerta 9/9 · escucha local 11/11 |
| 11 | `dd7fa12` | 2 | **D-07**: misma grabación en otra edición pasaba sin revisión | guarda ED-04 antes de escribir: álbum canónico ≠ álbum entrante sin evidencia de release → `needs_review`; test 4/4 y **suite completa 2740/0/13** |
| 12 | `987cf8b` | 3 | **D-06**: 243 escrituras silenciosas podían crecer sin control | ratchet en `scripts/tests/test_silent_write_ratchet.py`; demo en rojo real (244 > 243 con una sonda) y verde tras retirarla |
| 13 | `db56981` | 3 | **D-06**: la política de fallo estaba repetida en 243 sitios | `crate::db::write_or_log` con 3 tests; `commands/library.rs` 18/18 migrado; ratchet 243 → **225**; suite 2744/0/13 |
| 14 | `0da1cca` | 3 | **D-06**: los silencios de los caminos calientes de metadatos | `services/enrichment.rs` 45/45 migrado (`apply_to_database` 25, `enrich_and_persist_sync_track` 19); ratchet 225 → **180**; suite 2744/0/13 |
| 15 | `a742119` | 3 | **D-06**: cierre del top-5 y medida sobrecontada | `favorites` 9 + `tidal` 9 + `qobuz` 21 + `service` 28 migrados; corrección 243 → **225 reales** (18 propagan); ratchet 180 → **113**; suite 2744/0/13 |
| 16 | `5d19ad8` | 4 | **D-08**: no había contrato del orquestador antes de cortarlo | 3 tests de caracterización sin red (eventos, terminalidad, determinismo); determinismo medido, no supuesto |
| 17 | `f1f14ca` | 4 | **D-08**: dos `return Err` de credenciales dentro del brazo SoundCloud | `soundcloud_credentials` pura + 5 tests; equivalencia de evento, mensaje y flujo; suite 2760/0/13 |
| 18 | `b8a8037` | 4 | **D-08**: el brazo SoundCloud dentro de la función de 4 546 líneas | `sync_soundcloud_arm` (249 líneas) + `ArmOutcome` de 19 acumuladores; orquestador 4 546 → **4 330**; contrato del brazo y suite 2762/0/13 |
| 19 | `7f29a77` | 4 | **D-08**: el brazo Deezer (585 líneas) dentro del orquestador | `sync_deezer_arm` (591) + `ArmOutcome` de 26 + métricas de expansión; siembra y volcado completos; orquestador 4 330 → **3 809**; suite 2764/0/13 |
| 20 | `b05f394` | 4 | **D-08**: el brazo Spotify (828 líneas) dentro del orquestador | `sync_spotify_arm` (841) + `reported_*` en `ArmOutcome` (29 campos); orquestador 3 809 → **3 057**; suite 2766/0/13 |
| 21 | `a4804bc` | 4 | **D-08**: el brazo TIDAL (1 097 líneas), el de más fases | `sync_tidal_arm` (1 098) + 4 campos de disponibilidad/expansión (33); orquestador 3 057 → **2 054**; casi-accidente del literal `ServiceSyncResult` parado por `assert`; suite 2768/0/13 |
| 22 | `0b1609c` | 4 | **D-08**: el brazo Qobuz (1 182 líneas), el último | `sync_qobuz_arm` (1 237) + `purchases_total` (34 campos); orquestador **958**; los cinco brazos fuera; suite 2770/0/13 |
| 23 | `46e0a57` | 4 | **Prueba manual 2026-10-19**: el guardián ED-04 rechazaba ediciones legítimas | 16 766 errores de ítem en seis pasadas; favoritos de Tidal 16 087/17 492; `misma_familia_de_album` + 2 tests con los pares reales; lib 476 |
| 24 | `c7325a7` | 4 | **Prueba manual 2026-10-19**: 51 altas perdidas por `tracks.isrc` UNIQUE | `INSERT_PISTA_SQL` con `ON CONFLICT(isrc) DO UPDATE`; test rojo→verde con el código 2067; lib 477 |

**Fase 1b — el bug que la suite no veía.** `ui/src/views/LibraryView.vue` invocaba
`invoke('set_max_concurrency', { maxConcurrent })`, un comando **inexistente**; el error moría en un
`catch` que solo hacía `console.warn`, y el spec mockeaba el comando inventado. El selector de hilos
nunca funcionó. El backend ya tenía el comando real: `set_max_concurrent_downloads(max: usize)`
(`src-tauri/src/commands/queue.rs:2628`), con wrapper y tests propios. Rojo→verde:
`vitest` 1 failed/44 passed → **45 passed**; puerta Rust `phantom commands: ["set_max_concurrency"]`
→ **9 passed**.

Se añadió la puerta que faltaba, `test_every_ui_invoked_command_exists_in_the_backend`: IN-5 vigilaba
comandos registrados que nadie invoca (sobran) pero **nadie** vigilaba comandos que la UI invoca y no
existen (faltan). Y se corrigió un punto ciego del parser de la propia puerta:
`extract_registered_commands()` solo entendía `commands::` y `tray::`, así que
`logs_history::read_log_history` / `export_log_range` (registrados en `main.rs:1306-1307`) eran
invisibles en **ambas** direcciones.

**Fase 1a — por qué se revirtió.** Quitar el registro huérfano (`resolve_playback_source`) ponía la
puerta en verde, pero destapaba el subsistema retirado completo como código muerto bajo
`-D warnings`: `MAX_GRANTS`, `resolve_playback_source` y `validate_playback_file`
(`error: never used`). La evidencia de que la ruta `syncify-media://` está retirada es sólida:
`grants()` (`playback.rs:24`) solo lo puebla esa función, el handler (`playback.rs:373`) **deniega
todo** si el set está vacío, el reproductor usa el motor nativo `player_load_track`
(`ui/src/composables/usePlayer.ts` documenta la retirada) y ni el bundle construido ni ningún test
contienen el nombre. Pero completar la retirada son ~450 líneas de `playback.rs` más el protocolo
registrado en `main.rs:104` y las docs que lo describen como vigente. Es una decisión de producto con
revisión propia, no un paso mecánico, así que se revirtió y la puerta queda **roja a propósito**: es
un positivo verdadero que señala un subsistema retirado, no un test que estorba.

**Fase 2 — fixtures, no producto.** `featuring_and_appearances_test` y `social_metadata_backfill_test`
fallaban con `SQLITE_CONSTRAINT_FOREIGNKEY (787)`: pasaban `account_id: 1` sin insertar la cuenta.
De los 11 ficheros que llaman a `enrich_and_persist_sync_track`, 9 la sembraban y fallaban justo los 2
que no. Arreglo solo de fixture (reutilizar el servicio ya sembrado por `0002:261` + insertar la
cuenta 1). Resultado: **4/4 y 5/5 verdes**. El producto no se tocó.

**Correcciones al informe que salieron de la validación:** el clippy rojo **no era 1 lint** sino 2 —
clippy detiene cada crate en el primer error, así que arreglar `unnecessary_map_or` en
`syncify-tidal-downloader` reveló `unnecessary_sort_by` en `syncify-tauri` (comportamiento de cebolla
que el informe no recogía).

### 16.1 Recuento final medido (rama de reparación)

**Comandos de la misión, ejecutados sobre el estado final.** `npm --prefix ui run build` → **exit 0** (vue-tsc + vite, incluida la vista reparada); `cargo check --workspace --all-targets --locked` → **exit 0**; `cargo fmt --all -- --check` → **exit 0**; `cargo clippy --workspace --all-targets --locked -- -D warnings` → **exit 0**; `cargo test --locked` → **exit 0** tras cerrar Fase 1a y D-07 (antes se detenía en el rojo deliberado); 12 objetivos focalizados de IPC/recuperación/reconciliación → **73 tests, 0 fallos, exit 0**.

**Artefacto B1 (`improve-codebase-architecture`).** Informe HTML autocontenido fuera del repo
(`/tmp/architecture-review-1792396400.html`, 15,3 KB) con cuatro oportunidades de profundización
—frontera de escritura para los 243 silencios, seam de IPC, brazos de sincronización y sesión de
Qobuz—, cada una con antes/después, palanca, localidad y fuerza de recomendación; la recomendación
principal es la frontera de escritura. No se modificó ningún fichero del repo para producirlo.

**Cierre de los dos rojos (Fase 1a + D-07).** La suite completa queda **verde por primera vez**:
318 objetivos, **2 740 pasan, 0 fallan, 13 ignorados**, `cargo test --locked` → **exit 0**. El rojo del
subsistema retirado desapareció al borrarlo (D-05 → C-19) y el de identidad por edición al insertar la
guarda ED-04 (`needs_review` cuando el álbum canónico difiere del entrante y no hay evidencia de
release; D-07 → C-20). El total de tests baja respecto a `c0ade98` (2 753 → 2 740) porque se
eliminaron los tests del protocolo retirado: cobertura de código inexistente, no pérdida de
comportamiento. `cargo fmt --check` y `cargo clippy -D warnings` siguen en **exit 0**.

**Nota de método (2026-10-19).** Los totales de la suite cuentan cada test una vez por objetivo
en que se compila. Un test unitario nuevo dentro de la crate aparece **dos veces** (`lib` y `bin`), así
que 8 tests añadidos suben el agregado en 16 (2 744 → 2 760). Los totales de objetivos (318) y de
ignorados (13) no cambian por esto.

**Cierre de D-08 (2026-10-19).** Los cinco brazos están extraídos y cada uno vive en su
función: `sync_soundcloud_arm` (249), `sync_deezer_arm` (591), `sync_spotify_arm` (841),
`sync_tidal_arm` (1 098) y `sync_qobuz_arm` (1 237). `perform_sync_service_with_emitter_inner`
pasó de 4 546 a **958 líneas** (−79 %), y el `match` de servicios es hoy una llamada y un
volcado por brazo. Diez contratos offline (credenciales de SoundCloud, ARL de Deezer,
credenciales de Spotify, cuenta de TIDAL, token de Qobuz y «sin fases no se toca nada») más
tres tests de caracterización del orquestador sostienen la equivalencia junto con la suite.
Lo que **no** está probado y queda anotado: ningún test recorre los brazos contra un
proveedor real, así que la secuencia de progreso completa y el recuento por servicio con red
siguen pendientes de una caracterización contra proveedor simulado.

**Prueba manual en escritorio (2026-10-19).** El usuario ejecutó
`~/.local/bin/syncify-prueba-limpia.sh` (`npm run dev` con `SYNCIFY_LOG_DIR` en
`~/.local/state/syncify-manual-test`) e hizo un sync completo. Los logs prueban dos
cosas a la vez:

1. **La extracción aguanta la app real.** Los cinco brazos corrieron en producción, con
   los tiempos del orquestador: Tidal 670 s (enriquecimiento 348 s, persistencia 124 s),
   Qobuz 81 s, Apple Music 29 s, Deezer 4 s, SoundCloud 1,4 s; Tidal importó 16 087 de
   17 492 favoritos (92 %) y 27 832 de 27 832 pistas de playlist, sin panic ni cuelgue.
2. **El D-07 tenía un coste que la suite no podía ver.** `[sync] N item error(s)` sumó
   16 766 en seis pasadas (Tidal 7 644, Qobuz 5 098, Spotify 4 023, SoundCloud 1) y los
   motivos dominantes eran la guarda nueva, con pares que son la misma grabación en dos
   ediciones («Nothing But Thieves» ← «(Deluxe)», «1» ← «(Remastered 2015)», «Hybrid
   Theory (DMD Album + 3 Bonus Tracks)» ← «(Bonus Edition)»). Corregido en `46e0a57`.
   El mismo sync destapó 51 altas perdidas por `tracks.isrc` UNIQUE que antes eran
   invisibles (`let _ =`): corregidas en `c7325a7`.

Límite del método: el guion **no aísla** perfil ni base de datos (usa
`~/.local/share/com.syncify.app/syncify.db`), así que la prueba es real pero no
repetible sobre un estado limpio; y los eventos de progreso van a la UI, no al log, de
modo que la secuencia de fases se reconstruye por los resúmenes del orquestador.

**Corrección de medida (2026-10-19, al migrar D-06).** El recuento inicial de 243
`let _ = sqlx::query(...)` era un **límite superior**: al pasar los sitios a la frontera se vio que 18
terminan en `.await.map_err(..)?` y por tanto propagan el error (10 en `commands/favorites.rs`, 8 en
`services/tidal.rs`); nunca fueron silencios. Silenciosos reales: **225**, de los que **130** están
migrados a `crate::db::write_or_log` (los cinco ficheros de mayor riesgo) y **95** siguen pendientes,
congelados por el ratchet en 113 (95 silenciosos + 18 que propagan). La corrección se aplicó al
registro de deuda y a la cabecera del ratchet, no solo al informe.

**Documentación (Fase 5c).** El registro de deuda abría D-05 (subsistema de reproducción retirado,
con la evidencia de que el protocolo ya no puede servir nada), D-06 (243 escrituras SQLx silenciosas,
con reparto por fichero), D-07 (contrato de identidad por edición) y D-08 (las dos funciones monstruo
con su medida), y cerraba C-12…C-18 con el commit de cada uno. D-07 se afinó contra
`docs/DECISION_SELECCION_EDICION.md:204-205`: la ADR define la identidad «solo grabación» como
ISRC + título + artista + duración y prohíbe igualar en silencio versiones distintas, pero **no
resuelve** el caso del test (álbum original vs. recopilación); queda documentada la decisión que
falta en vez de insinuar una respuesta. La puerta de consistencia documental obligó a no meter esas
entradas en la tabla de alcance condicionado (que declara solo lo que espera a un tercero): funcionó
como contrato.

**`AGENTS.md` (Fase 5).** B2 era el eje más débil: no existía ninguna puerta de entrada para un
agente. El mapa responde solo a lo que no se deduce del árbol (superficies, comandos deterministas,
puertas y qué protegen, minas, módulo profundo vs. funciones monstruo) y apunta a las docs en vez de
duplicarlas. Para que no envejezca, la puerta de consistencia documental lo escanea y resuelve cada
referencia: se demostró inyectando `src-tauri/src/no_existe_jamas.rs` (falla nombrando el fichero) y
quitándola (pasa).


```
CARGO_TARGET_DIR=<fuera del repo> cargo test --locked --no-fail-fast
-> 318 objetivos con resultado: 316 ok, 2 FAILED | 2756 passed, 2 failed, 13 ignored
```
| Métrica | `c0ade98` | Rama de reparación |
|---|---|---|
| Objetivos de test rojos | 4 | **2** |
| Tests que pasan | 2753 | **2756** |
| `cargo fmt --all -- --check` | exit 1 (23 hunks / 9 ficheros) | **exit 0** |
| `cargo clippy -D warnings` | exit 101 | **exit 0** |
| UI (`vitest`) | 125 ficheros / 1075 tests | **igual, 1075 pasan** |
| Python (`unittest`) | 307 OK (2 skip) | **308 OK** (2 skip): +1 puerta nueva |

Los **2 rojos que quedan son deliberados y están explicados**:

1. `dead_commands_hygiene_test::test_every_registered_command_is_invoked_by_the_frontend`
   → `orphans: ["resolve_playback_source"]`. Positivo verdadero: señala el subsistema de
   reproducción retirado (§16, Fase 1a). Cerrarlo exige borrar ~450 líneas más el protocolo, con
   decisión de producto. **No se relajó la puerta.** El test nuevo de la dirección contraria
   (`test_every_ui_invoked_command_exists_in_the_backend`) **pasa**: ya no hay comandos fantasma.
2. `track_identity_and_tagging_test` → contrato de identidad por edición. Caracterizado: las dos
   entradas solo difieren en `album` (`"Gorillaz"` vs `"The Singles Collection 2001-2011"`) y
   `service_track_id`, con el mismo ISRC; el segundo `enrich_and_persist_sync_track` devuelve
   `is_already_present: true, is_new_global_track: false` en vez del `Err("needs_review: ...")` que
   el test espera. El producto tiene 4 guardas de revisión
   (`src-tauri/src/services/enrichment.rs:2716, :2731, :2755, :3412`); la de conflicto de release
   (`:3412`) es inalcanzable en este caso porque el camino de "ya presente por ISRC" retorna antes.
   **Decisión pendiente y deliberada:** o el producto debe restaurar la revisión de edición, o el
   contrato correcto es "identidad por grabación, pertenencia a álbum aparte" y el test codifica un
   contrato superado. Se resuelve leyendo la autoridad de dominio (`docs/`) antes de tocar nada, no
   ajustando la aserción.

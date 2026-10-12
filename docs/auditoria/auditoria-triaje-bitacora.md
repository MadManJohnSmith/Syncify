# Bitácora — carril `auditoria-triaje`

Fecha: 2026-10-08. IDs: L-01, L-02, DSC-01..DSC-06, Q-01, PR-03, PR-04, MET-02, MET-03,
LY-04, LY-06, LY-07, LY-09, LY-11, LY-12, LOG-01..LOG-04, AQ-02, MT-03, BR-01..BR-03 (28).

## Qué cambié

**Nada en código.** Este carril fue de solo lectura sobre `ui/src/**` y `src-tauri/src/**`.
Ficheros creados (los únicos, todos en `docs/auditoria/`):

- `triaje.md` — tabla ID / clase / fichero / función / acción para los 28.
- `matriz-ajustes.md` — filas pre-registradas para los 14 REAL (regla R1), más los
  falsos positivos marcados como "no abrir ajuste" y las decisiones que bloquean.

Ningún test se borró ni modificó (R2 no aplica). No toqué
`provider_fallback_identity_test.rs` ni `cli_tauri_behavioral_parity_test.rs` (R3 no aplica).

## Con qué evidencia

Todo se leyó contra el código en esta sesión. Comandos ejecutados y su salida:

| Qué | Comando | Resultado |
|---|---|---|
| No hay virtualización en ninguna vista | `grep -rln "DynamicScroller\|RecycleScroller\|virtual-list\|useVirtual" ui/src/views/ ui/src/components/` | **sin salida** |
| Metadata pide 500 y las pinta enteras | `grep -n "getLibrary(0," ui/src/views/MetadataView.vue ui/src/views/LyricsView.vue` | `MetadataView.vue:1464`, `LyricsView.vue:1476` |
| MBID no aparece fuera de Metadata | `grep -rn "MBID\|musicbrainz_id" ui/src/views/*.vue ui/src/components/*.vue \| grep -v MetadataView` | solo `SettingsView.vue:222` (default) y `MusicBrainzMatchModal.vue:286` |
| Sin portadas animadas en Metadata | `grep -n "animated" ui/src/views/MetadataView.vue` | **sin salida** |
| El buffer de logs está alimentado por `tracing` | lectura de `services/logging.rs:543-589` (`LogBufferLayer::on_event`) y `:705-712` (registro en la cadena) | `tracing_subscriber::registry().with(fmt).with(buffer_layer.with_filter(filter))` |
| No hay mocks en el pipeline de logs | `grep -n "mock\|Mock\|fake\|dummy\|sample" ui/src/composables/useLogs.ts ui/src/api/logs.ts ui/src/views/LogsView.vue` | único acierto: el comentario `useLogs.ts:6` "without mock logs" |
| El init de logging solo ocurre una vez y con `None` | `grep -rn "init_logging_system\|set_logging_config\|update_logging" src-tauri/src/` | 2 aciertos: la definición (`logging.rs:689`) y la llamada (`main.rs:84`), ambas `None, None` |
| El historial se lee de disco | `grep -n "pub fn read_log_history\|read_dir\|\.log" src-tauri/src/logs_history.rs` | `logs_history.rs:317`, `read_dir` en `:222`, filtro `syncify*.log` en `:233` |
| `error_message`/`last_error` se escriben | `grep -rn "error_message" src-tauri/src/ \| grep -i update` | `worker.rs:871`, `:905`, `:1921` (y `migration.rs`, otro módulo) |
| `commands/download.rs` no registra errores | `grep -n "tracing::error!\|tracing::warn!" src-tauri/src/commands/download.rs` | **sin salida** (solo 7 `tracing::info!`); el worker real está en `worker.rs` |

### Los dos puntos con atención especial

**LY-12 — qué hace `Delete` de verdad.**
`delete_lyrics` (`src-tauri/src/commands/lyrics.rs:753-778`) ejecuta, según el caso,
`DELETE FROM lyrics WHERE track_id = ? AND format = ?` (`:762`) o
`DELETE FROM lyrics WHERE track_id = ?` (`:770`). **Nada más.** No borra filas de
`tracks`, ni de `downloads`, ni el archivo del disco. La afirmación del registro en crudo
("borra la canción y su letra") es **falsa**.

Lo que el usuario ve: `deleteTrackLyrics` (`LyricsView.vue:1813-1825`) borra la letra y
recarga con `loadTracks()` (`:1473-1483`, `getLibrary(0, 500)`) + `loadLyricsStats()`.
`loadTracks` recarga **toda** la biblioteca, así que la pista no desaparece de `tracks`;
pero `filteredTracks` (`:1295-1331`) aplica el filtro activo, y con *Synced* o *Unsynced*
descarta la fila porque `mapLyricsType(t.lyrics_type)` ya no coincide (`:1306-1315`).
`lyrics_type` sale de un LEFT JOIN sobre `lyrics` (`commands/library.rs:218-224`) y tras
el borrado resuelve a `'none'`. Con filtro *All* la fila se queda y solo cambia la
etiqueta a "None".

Hallazgo adicional en sentido contrario: **el borrado no es duradero** si la letra está
embebida en el FLAC o existe un sidecar, porque `probe_track_lyrics`
(`lyrics.rs:1677-1710`) la reinserta con `upsert_lyrics`. La guarda `diskProbedTracks`
(`LyricsView.vue:1504`) es por sesión, así que la resurrección ocurre al reabrir la app.

**LOG-01..04 — ¿logs reales o mocks?**
**Son reales.** `LogBufferLayer` (`logging.rs:543-589`) es un `tracing_subscriber::Layer`
que convierte cada evento en un `SystemLogEntry` y lo mete en el ring buffer
(`LogBuffer::push`, `:77`), y está montado en el subscriber (`:705-712`). El buffer es de
capacidad acotada con expulsión (`:98`). Los tres filtros de la UI (búsqueda `:55-61`,
nivel `:106-118`, módulo `:120-142`) son reales y comparan insensible a mayúsculas por
subcadena (`:377-384`).
- **LOG-02**: el defecto no es "no hay filtros" sino que la lista de módulos está
  hardcodeada y **omite `Lyrics`**, valor que sí emiten los dos normalizadores
  (`logging.rs:798-799`, `useLogs.ts:92`).
- **LOG-03**: el motivo de un fallo de descarga se guarda (`worker.rs:871`, `:905`) y se
  muestra en Descargas (`DownloadsView.vue:1602`, `classifyFailureReason`), pero el brazo
  `Err` del worker (`worker.rs:1601-1650`) **no emite ningún `tracing::error!` con el
  motivo** → en la pestaña Logs la descarga falla sin explicación.
- **LOG-04**: *Copy* funciona (buffer en memoria). *Export* / *Load History* leen de disco,
  y en release `log_to_file` es siempre `false` (`logging.rs:482-486`) porque
  `main.rs:84` pasa `None` y `INIT_ONCE` (`:693-696`) impide re-configurar. El toggle de
  Ajustes Avanzados (`SettingsAdvanced.vue:22-23`) no tiene efecto en runtime.

## Hallazgos que corrigen al plan v2

- **LY-09 ya estaba implementado** (`LyricsView.vue:298`), antes de este plan. Falso positivo.
- **LY-12 no borra la canción** — el informe en crudo es incorrecto; el defecto real es
  otro (desaparición por filtro + borrado no duradero).
- **DSC-02, DSC-04, DSC-05, Q-01, BR-01** son functionality ya presente.
- **LOG-02** no es "no hay filtros": hay tres y funcionan.

## Qué queda abierto

1. **PR-04** — no se puede cerrar sin perfilar en la máquina del usuario. No medido.
2. **DSC-03** — "verificad que los datos sean ciertos" exige una BD real; este carril solo
   tiene lectura de código.
3. **LY-07** — el abanico en paralelo cambia consumo de red y el contrato de búsqueda:
   necesita criterio de qué hacer cuando dos proveedores devuelven niveles distintos.
4. **LY-11** — versionar filas exige migración de esquema; la alternativa barata es
   renombrar el panel. Decisión de producto.
5. **LY-12b / LOG-04** — dos decisiones de política bloqueadas (ver
   `matriz-ajustes.md`, sección final).
6. **8 IDs NECESITA-DECISION** (DSC-01, DSC-03, DSC-06, MET-02, MET-03, LY-06, LOG-01,
   AQ-02) no se pueden cerrar leyendo código: son peticiones de diseño sin especificación.

## Comprobaciones no ejecutadas

- **No corrí la suite**, por instrucción explícita del ask ("no ejecutes la suite completa").
- **No corrí `cargo check --target x86_64-pc-windows-msvc`** (R4): ningún ID de este carril
  es código `#[cfg(windows)]`; todo lo tocado es UI o lógica de datos multiplataforma.
- **No compilé nada.** Este carril no modificó código, así que no había nada que verificar
  compilando; todas las citas son de lectura estática.
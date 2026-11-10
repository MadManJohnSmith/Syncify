# AGENTS.md

Mapa para agentes que trabajan en este repo. Es el único fichero de orientación:
no dupliques su contenido en otros sitios.

## Qué es

App de escritorio (Tauri 2) para sincronizar y descargar música. Cinco superficies:

| Superficie | Ruta | Lenguaje |
|---|---|---|
| Núcleo, comandos IPC y servicios | `src-tauri/src/` | Rust |
| Crates del workspace | `crates/` | Rust |
| Interfaz | `ui/src/` | Vue 3 + TypeScript |
| Puentes de proveedores | `scripts/services/` | Python |
| Esquema | `src-tauri/migrations/` | SQL (SQLx) |

## Comandos deterministas

Desde la raíz. El target de Rust va **fuera del repo** para no llenar el disco:

```bash
CARGO_TARGET_DIR=/tmp/syncify-target cargo check --workspace --all-targets --locked
CARGO_TARGET_DIR=/tmp/syncify-target cargo clippy --workspace --all-targets --locked -- -D warnings
cargo fmt --all -- --check
CARGO_TARGET_DIR=/tmp/syncify-target cargo test --locked --no-fail-fast   # 318 objetivos de test: recuento real
CARGO_TARGET_DIR=/tmp/syncify-target cargo test --locked --test batch_50_audit_test -- --test-threads=1
npm --prefix ui run build && npm --prefix ui run test:run                  # 131 ficheros / 1139 tests
python3 -m unittest discover -s scripts/tests                             # 324 tests (2 skipped)
```

Todo corre sin red salvo los 13 tests `#[ignore]`, que necesitan servicios reales.
No los des-ignores en CI ni añadas `--skip`.

### Puerta previa al push

`tools/prepush-check.sh` adelanta a la máquina local el veredicto de CI:
fmt + clippy con las mismas banderas de arriba (~2 min en caliente);
`--with-tests` añade la suite completa. Las exportaciones específicas de la
máquina (p. ej. `PKG_CONFIG_PATH` para OpenSSL) van en el entorno o en el hook,
no en el script. La receta del hook local opcional está en la cabecera del
script: no versiones el hook, que no es reproducible entre clones.

### Verificación manual

`tools/prueba-limpia.sh` arranca la app con `npm run dev` (`tauri dev`), fija
`RUST_LOG=info` y `RUST_BACKTRACE=1`, y duplica la salida a
`~/.local/state/syncify-manual-test/`: el log de terminal y, vía
`SYNCIFY_LOG_DIR`, los logs nativos de la app. Cerrar la app termina la captura.

No versiones ni compartas esos logs: pueden contener cuentas y biblioteca.
`SYNCIFY_REPO` apunta el script a otro clon.

## Puertas que no se relajan

El repo se vigila a sí mismo; si una falla, el fallo es el mensaje:

| Puerta | Protege |
|---|---|
| `src-tauri/tests/dead_commands_hygiene_test.rs` | IPC sin contrato: comandos registrados que nadie invoca, y comandos que la UI invoca y no existen |
| `ui/src/__tests__/api/tauri_events_symmetry.spec.ts` | Simetría de eventos emitidos y escuchados |
| `scripts/tests/test_docs_consistency.py` | Que las docs no citen ficheros o ramas inexistentes |
| `scripts/tests/test_readme_claims.py` | Cifras del README contra el árbol real |
| `scripts/tests/test_tracked_artifact_hygiene.py` | Artefactos generados que no deben estar versionados |

Arregla la causa: no añadas excepciones a esas listas, no pongas
`#[allow(dead_code)]` en un `#[tauri::command]` y no cambies una aserción para
que pase.

## Minas

- **Migraciones aplicadas:** nunca edites una migración existente ni reutilices su
  número. Una BD `:memory:` recién migrada **no** demuestra que el upgrade funcione.
- **Datos personales:** no toques `~/.local/share/com.syncify.app`, la biblioteca de
  música ni las descargas reales. Usa fixtures desechables y BD `:memory:`.
- **Ruta de reproducción retirada (ya eliminada):** el motor nativo `player_load_track`
  sustituyó a `resolve_playback_source` y al protocolo `syncify-media://`, que se
  borraron (C-19 en `docs/Deuda_Tecnica_y_UX.md`). `commands/playback.rs` conserva
  solo la escucha local (`record_local_play`).
- **Escrituras silenciosas:** hay ~113 `let _ = sqlx::query(...)` en producción (el
  registro D-06 de `docs/Deuda_Tecnica_y_UX.md` lleva el ratchet y no debe subir): una
  escritura puede fallar y la transacción confirmar sin rastro. Si tocas una, propaga
  el error o registra con `tracing::warn!`.
- **Entorno:** `.venv`, `node_modules` y los lockfiles son el entorno; no los
  regeneres sin motivo. `src-tauri/gen/` no está en el árbol: no lo recrees.

## Dónde está el código

- `src-tauri/src/cmd_utils.rs` es el módulo profundo del repo: resolución de
  herramientas, entorno de los puentes Python y timeouts tras una sola interfaz.
  Úsalo en vez de construir `Command` a mano.
- `src-tauri/src/db.rs` monta el pool y las migraciones. El `generate_handler!` con
  todos los comandos está en `src-tauri/src/main.rs`.
- **Evita** `src-tauri/src/commands/service.rs` (~9,4k líneas; ahí vive el orquestador
  `perform_sync_service_with_emitter_inner`) y `src-tauri/src/download/qobuz.rs`
  (~4,1k líneas; ahí vive `resolve_token_for_account`): ahí un cambio se vuelve
  invisible. Si hay que tocarlos, tests de caracterización delante.
- `crates/README.md` explica qué hace cada crate del workspace.
- `ui/src/api/` es la única puerta a Rust desde la UI: cada comando tiene su wrapper.
  No llames a `invoke` desde una vista; así nació el bug de `set_max_concurrency`.

## Más contexto

- `docs/README.md` (y `docs/README.en.md`): índice completo de la documentación,
  con qué fichero es la referencia vigente de qué.
- `GLOSSARY.md`: vocabulario de dominio (track vs. track source vs. edición,
  y los dos sentidos de «migración»).
- `docs/adr/*.md`: decisiones de alcance y arquitectura.
- `docs/Deuda_Tecnica_y_UX.md`: deuda conocida y su prioridad.
- `docs/features/`: comportamiento por área (reproductor, biblioteca, descargas).
- Registros vivos en `docs/`: la Deuda, las colas `docs/COLA_*.md` y
  `docs/PLAN_MEJORA_POST_BETA.md`; el ledger de ajustes es
  `docs/auditoria/matriz-ajustes.md`. En `docs/historico/` está el registro
  fechado (handoffs, Errores-Pruebas, perf, release notes): evidencia, **no**
  estado actual.
- `README.md` §"Building from source" y §"Verification and limitations": lo que
  **no** está verificado en escritorio real (GUI, audio, bandeja, MPRIS/D-Bus, red viva).

## Agent skills

### Issue tracker

Los issues viven en GitHub Issues de `MadManJohnSmith/Syncify`, vía la CLI
`gh`. Ver `docs/agents/issue-tracker.md`.

### Triage labels

Cinco roles canónicos con la etiqueta igual a su nombre (`needs-triage`,
`needs-info`, `ready-for-agent`, `ready-for-human`, `wontfix`). Ver
`docs/agents/triage-labels.md`.

### Domain docs

**single-context**: un `GLOSSARY.md` en la raíz y las decisiones en
`docs/adr/*.md`. Ver `docs/agents/domain.md`.

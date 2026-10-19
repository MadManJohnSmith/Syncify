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
CARGO_TARGET_DIR=/tmp/syncify-target cargo test --locked --no-fail-fast   # 318 objetivos: recuento real
CARGO_TARGET_DIR=/tmp/syncify-target cargo test --locked --test batch_50_audit_test -- --test-threads=1
npm --prefix ui run build && npm --prefix ui run test:run                  # 125 ficheros / 1075 tests
python3 -m unittest discover -s scripts/tests                             # 307 tests
```

Todo corre sin red salvo los 13 tests `#[ignore]`, que necesitan servicios reales.
No los des-ignores en CI ni añadas `--skip`.

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
- **Ruta de reproducción retirada:** el motor nativo `player_load_track` sustituyó a
  `resolve_playback_source` y al protocolo `syncify-media://`
  (`src-tauri/src/commands/playback.rs`). Siguen registrados y la puerta de higiene
  los señala; no los tomes por el camino vivo.
- **Escrituras silenciosas:** hay ~243 `let _ = sqlx::query(...)` en producción: una
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
- **Evita** `src-tauri/src/commands/service.rs`
  (`perform_sync_service_with_emitter_inner`, ~4,5k líneas) y
  `src-tauri/src/download/qobuz.rs` (`resolve_token_for_account`, ~2,9k): ahí un
  cambio se vuelve invisible. Si hay que tocarlos, tests de caracterización delante.
- `ui/src/api/` es la única puerta a Rust desde la UI: cada comando tiene su wrapper.
  No llames a `invoke` desde una vista; así nació el bug de `set_max_concurrency`.

## Más contexto

- `docs/DECISION_*.md`: decisiones de alcance y arquitectura.
- `docs/Deuda_Tecnica_y_UX.md`: deuda conocida y su prioridad.
- `docs/features/`: comportamiento por área (reproductor, biblioteca, descargas).
- `README.md` §"Building from source" y §"Verification and limitations": lo que
  **no** está verificado en escritorio real (GUI, audio, bandeja, MPRIS/D-Bus, red viva).

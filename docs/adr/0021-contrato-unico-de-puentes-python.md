# ADR-0021 — Contrato único de puentes Python: `BridgeClient` y `bridge_protocol`

**Estado:** aceptado · **Fecha:** 2026-11-05
**Origen:** parte P3 del informe de arquitectura «optimizar el codebase para agentes».
**Spec:** PR de P3 (rama `p3/bridge-client`). **Tickets:** ninguno.

## Contexto

En el árbol había **tres formas de lanzar un puente Python** — el lanzador genérico
`run_bridge_command` (`src-tauri/src/commands/tools.rs`), el de autenticación
(`auth.rs:run_auth_bridge_subprocess`) y el de ajustes
(`settings.rs:test_lyrics_provider`) — y **cuatro structs de envelope idénticos**
(`MetadataResult`, `FingerprintResult`, `BridgeResult`, `AuthResult`), todos
`{success, data, error}`.

En Python, cada script repetía su propio `argparse`, captura de stdout, envelope JSON
(`json_response`) y códigos de salida: **10 definiciones privadas de `json_response`
con 3 variantes de serialización** (ASCII-escape en auth, `ensure_ascii=False+**extra`
en lyrics, `ensure_ascii=False+default=str` en el resto). Añadir un puente costaba
~80 líneas de ceremonia y un bug de protocolo se arreglaba en cada script.

## Decisión

- **Rust:** un único `crate::bridges::BridgeClient` con dos operaciones:
  - `capture(script, args, CallOptions { timeout, stdin })` — lanza el puente, fija el
    entorno Python compartido (`PYTHONIOENCODING=utf-8`, `PYTHONUNBUFFERED=1`,
    `PYTHONPATH`, `CREATE_NO_WINDOW`) y devuelve la salida cruda con su `ExitStatus`.
  - `call::<T>()` — además deserializa el envelope `BridgeEnvelope<T>`
    `{success, data, error}`. Se parsea stdout aunque el exit sea distinto de cero
    (un fallo de dominio llega con su envelope y exit 1); solo stdout vacío con exit
    fallido es error.
  - El **timeout es obligatorio** en `CallOptions`: ningún call site hereda en
    silencio un presupuesto pensado para otro (auth 330 s, genéricos 45 s, test de
    lyrics 45 s — antes no tenía límite).
  - `BridgeError` tipado (`Spawn`, `StdinTimeout`, `StdinWrite`, `Wait`, `Timeout`,
    `ExitFailure`, `Parse`) cuyo `Display` reproduce los mensajes que la UI ya
    mostraba.
  - Auth conserva su interpretación del payload (`parse_auth_bridge_output`, cubierta
    por `auth_bridge_json_error_preserved_test.rs`) y aporta stdin, redacción en logs
    y parseo tolerante por marcador; el lanzamiento pasa por `capture`.
- **Python:** un único módulo `scripts/services/bridge_protocol.py`:
  - `run(handler, parse=None)` — modo IPC. Handler devuelve datos → envelope
    `{success: true, data}` con exit 0; `BridgeFailure` → `{success: false, error}`
    con exit 1; un dict con clave `success` se emite **verbatim** (payloads
    históricos de auth y lyrics); `FailureEnvelope` emite verbatim con exit 1 (fallo
    con payload propio, p. ej. la URL de verificación de Tidal); una excepción
    inesperada escribe el traceback en stderr y sale con 1 **sin envelope**, para
    que un bug siga siendo un error de proceso y no un fallo de dominio.
  - `run_cli(handler, parse=None)` — modo humano para los CLIs de mantenimiento.
  - Una sola serialización: `ensure_ascii=False` + `default=str` + stdout UTF-8.
- **Alcance:** migran los 8 puentes vivos y los 7 CLIs de mantenimiento. Los puentes
  legacy `download_bridge.py` y `organizer_bridge.py` **no** se tocan: están
  protegidos por `src-tauri/tests/python_module_hygiene_test.rs` y el camino de
  descarga es nativo en Rust (ADR-0009).

## Consecuencias

- La ceremonia repetida desaparece: no quedan `json_response` privados en scripts
  vivos, ni `argparse` duplicado en los puentes, ni tres lanzadores. La prueba de
  borrado deja `BridgeClient` + `BridgeEnvelope`/`BridgeError` + `bridge_protocol`.
- El contrato IPC con la UI no cambia: el envelope serializado conserva
  `success`/`data`/`error`, así que los tipos TypeScript y las vistas siguen igual.
- Los tests unitarios que afirmaban sobre `sys.exit` de `json_response` se adaptaron
  a la superficie nueva (`main()` devuelve el código; los handlers devuelven datos o
  lanzan `BridgeFailure`); los contract tests de CLI (subprocess) quedan intactos.
- Un puente nuevo declara un handler y su parser; el lanzamiento y el envelope ya no
  se reescriben.

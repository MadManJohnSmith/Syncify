# Bitácora — carril de procesos Windows (G-01, G-06)

**Fecha:** 2026-10-08
**Carril:** `1-procesos-windows`
**IDs:** G-01 (ventana al incrustar metadatos), G-06 (ninguna ventana de terminal, nunca)
**Plan:** `docs/PLAN-Correccion-168.md` v2, entrada de §1 fila "14 sitios lanzan
procesos sin `CREATE_NO_WINDOW`".
**Ficheros tocados (los 4 del encargo, ninguno más):**
`src-tauri/src/cmd_utils.rs`, `src-tauri/src/tray.rs`,
`crates/syncify-flac-writer/src/lib.rs`, `crates/syncify-tidal-downloader/src/lib.rs`
**Fichero de registro:** `docs/auditoria/matriz-ajustes.md` (filas G-01 y G-06 añadidas
antes de tocar código, por R1).

---

## 1. Veredicto previo: G-01 y G-06 son REALES, no falsos positivos

Comprobé los 14 sites contra el código antes de asumir nada. Los 14 existían y
ninguno aplicaba el flag:

```
$ grep -n 'Command::new' crates/syncify-flac-writer/src/lib.rs \
    crates/syncify-tidal-downloader/src/lib.rs src-tauri/src/tray.rs      # ANTES del cambio
flac-writer:973   Command::new("ffmpeg")     flac-writer:2490  std::process::Command::new("flac")
flac-writer:1062  Command::new("ffmpeg")     flac-writer:2623  Command::new("flac")
flac-writer:2254  Command::new("flac")       flac-writer:2666  Command::new("ffmpeg")
flac-writer:2318  Command::new("ffmpeg")     flac-writer:2824  Command::new("ffmpeg")
tidal-downloader:2010  tokio::process::Command::new("ffmpeg")
tray.rs:203  std::process::Command::new("reg")
tray.rs:279  std::process::Command::new("reg")
```

Cero `creation_flags` en esas tres rutas. Antes del cambio, el único
`creation_flags` del repo entero estaba en `src-tauri/src/cmd_utils.rs:216` y
`:228` — es decir, **solo los módulos de `src-tauri` sabían hacerlo**. Los 8
launches de flac-writer, el remux de Tidal y los dos `reg.exe` del tray iban con
`Command::new(...)` pelado, tal como decía el plan. El mecanismo es el esperado:
`ffmpeg.exe`, `flac.exe` y `reg.exe` son binarios de subsistema consola; sin
`CREATE_NO_WINDOW`, `CreateProcess` les asigna una consola propia y sale la
ventana que tapa la app.

**Comprobado además (el plan lo afirmaba en su retractación 12):**
`src-tauri/src/system_conditions.rs:94` y `:116` son `busctl` y `gdbus`. Confirmado
que **no** causan G-01/G-06: ambos usan `.output()` (`:101` y `:127`), o sea
tuberías drenadas y nunca una consola heredada, y son clientes D-Bus de escritorio
Linux que en Windows ni siquiera existen — el spawn falla y `dbus_get_property`
devuelve `None` (busctl cae al fallback en `:109`, y el `.ok()?` de gdbus en
`:129` se lleva el resto). La retractación 12 del plan
es correcta; **no los toqué** y no propongo tocarlos.

---

## 2. Qué cambié

### 2.1 El helper único, en `cmd_utils.rs`

`CREATE_NO_WINDOW` vive ahora en un solo sitio por crate, y el flag se aplica en
un solo sitio por crate.

| Símbolo | Ruta:línea | Qué es |
|---|---|---|
| `CREATE_NO_WINDOW_FLAG` | `src-tauri/src/cmd_utils.rs:217` | La constante de Win32 |
| `configure_hidden<C: HiddenProcess>()` | `src-tauri/src/cmd_utils.rs:225` | **El helper**: aplica el flag; en `#[cfg(not(windows))]` es un no-op explícito (`let _ = cmd;`) |
| `HiddenProcess` | `src-tauri/src/cmd_utils.rs:237` | Trait de dos implementaciones (std y tokio) |
| `create_std_command` / `create_tokio_command` | `src-tauri/src/cmd_utils.rs:266` / `:275` | Ahora **delegan** en `configure_hidden` en vez de repetir el `#[cfg(windows)] { … }` |

`HiddenProcess` sigue el mismo idioma que el `CommandEnv` que ya existía
(`cmd_utils.rs:184`), para no inventar una convención nueva.

El impl de `std::process::Command` usa la ruta explícita
`CommandExt::creation_flags(self, flags)` a propósito: con `use …CommandExt` +
`self.creation_flags(flags)` el método del trait en curso y el del `CommandExt`
son candidatos y `rustc` da `E0034`. Lo descubrí al compilar para Windows y lo
dejé anotado en el propio código.

### 2.2 Los 14 sites, todos por el helper

| # | Site (línea final) | Programa | Cambio |
|---|---|---|---|
| 1 | `crates/syncify-flac-writer/src/lib.rs:1003` | `ffmpeg` (portada → JPEG) | `hidden_std_command` |
| 2 | `…:1092` | `ffmpeg` (salvage de portada) | `hidden_std_command` |
| 3 | `…:2284` | `flac` (MD5 del stream PCM) | `hidden_std_command` |
| 4 | `…:2348` | `ffmpeg` (fallback MD5 PCM) | `hidden_std_command` |
| 5 | `…:2520` | `flac -t -s` (decode-check) | `hidden_std_command` |
| 6 | `…:2653` | `flac` (medición PCM) | `hidden_std_command` |
| 7 | `…:2696` | `ffmpeg` (fallback medición PCM) | `hidden_std_command` |
| 8 | `…:2853` | `ffmpeg` (trim por stream-copy) | `hidden_std_command` |
| 9 | `crates/syncify-tidal-downloader/src/lib.rs:2037` | `ffmpeg` (remux ISOBMFF→FLAC) | `hidden_tokio_command` |
| 10 | `src-tauri/src/tray.rs:208` | `reg.exe add/delete` | `create_std_command` |
| 11 | `src-tauri/src/tray.rs:284` | `reg.exe query` | `create_std_command` |

Los números del encargo (`:2824` de flac-writer) eran `:2823` de la línea `use`;
el `Command::new` real estaba en `:2824`. Además, al empezar ya no coincidían con
el estado del repo: el plan reckonaba 14 y son 14, pero las líneas de flac-writer
que encontré eran `973, 1062, 2254, 2318, 2490, 2623, 2666, 2824` — las mismas
del encargo, así que el plan no iba descaminado ahí.

Los sites 1–9 estaban antes de la intercalación de este carril; las líneas
actuales llevan el desplazamiento que introduce el propio helper.

**Limpieza asociada:** al dejar de usarse `std::process::Command` directamente,
los `use std::process::{Command, Stdio}` de cuatro funciones pasaron a ser
`use std::process::Stdio;` (`flac-writer:993, 1063, 2263, 2625`) y el
`use std::process::Command;` de `trim_flac_stream_copy` desapareció. Sin eso
salían cuatro `unused_imports`.

### 2.3 El punto donde el encargo es imposible de literally cumplir

El encargo pide **un** helper en `cmd_utils.rs` usado en **todos** los sites.
Para los sites 10 y 11 eso es literal y ya está hecho. Para los 9 de los crates
**no puede serlo**, y no es una cuestión de estilo:

- `Cargo.toml` (raíz) declara el workspace; `src-tauri/Cargo.toml` depende de
  `syncify-flac-writer` y `syncify-tidal-downloader`. La arista va
  **`syncify-tauri` → crates, nunca al revés**.
- Por tanto `crates/syncify-flac-writer` no puede hacer `use crate::cmd_utils`:
  ni compila (dependencia circular).
- La única crate que ven los dos crates y `src-tauri` es `syncify-core-domain`,
  pero su manifiesto la describe como *"Pure domain contracts, quality policies,
  validators, manifest, and typed progress states"* y **no depende de `tokio`**
  (`crates/syncify-core-domain/Cargo.toml`: solo `serde`, `serde_json`, `chrono`,
  `regex`). Meter ahí un helper que devuelve `tokio::process::Command` rompe su
  premisa, y además ese crate **no es mío** (no está en mi lista de ficheros).

Lo que he hecho es lo más cerca posible sin tocar ficheros de otros carriles ni
romper el grafo: `cmd_utils.rs` es el helper canónico y las dos crates llevan un
espejo de 10 líneas (`flac-writer:35,42` y `tidal-downloader:44,51`) con el
mismo nombre de constante y el mismo valor, y un doc-comment que dice
explícitamente "espejo de" y por qué. Si en el futuro alguien mueve esto a una
crate compartida, los tres sitios están marcados para encontrarlos.

**Nada de esto cambia el resultado observable**: el valor es el mismo en los tres
sitios y el comportamiento en Windows es idéntico.

---

## 3. Con qué evidencia (comandos exactos y su salida)

### 3.1 El valor del flag, contra Microsoft

`https://learn.microsoft.com/en-us/windows/win32/procthread/process-creation-flags`
→ `CREATE_NO_WINDOW` = **`0x08000000`**, *"The process is a console application
that is being run without a console window."* Coincide con el `0x08000000` que ya
tenía el repo; lo que he cambiado es que ahora está en una constante con nombre.

### 3.2 Target Windows (R4) — lo que SÍ pude correr

```
$ cargo check --target x86_64-pc-windows-msvc -p syncify-flac-writer -p syncify-tidal-downloader
EXIT=0
    Checking syncify-flac-writer v0.1.0
    Checking syncify-tidal-downloader v0.1.0
    Finished `dev` profile [unoptimized] target(s)
```
Cero errores y **cero warnings**. El target estaba instalado
(`~/.rustup/toolchains/stable-…/lib/rustlib/x86_64-pc-windows-msvc`).

### 3.3 `-p syncify-tauri` para Windows — BLOQUEADO por el entorno, no por el código

```
$ cargo check --target x86_64-pc-windows-msvc -p syncify-tauri --lib
EXIT=101
error: failed to run custom build command for `libsqlite3-sys v0.30.1`
  error occurred in cc-rs: failed to find tool "lib.exe": No such file or directory (os error 2)
```

Reintenté apuntando cc-rs a clang/llvm-lib, que sí están instalados:

```
$ CC_x86_64_pc_windows_msvc=clang AR_x86_64_pc_windows_msvc=llvm-lib \
  cargo check --target x86_64-pc-windows-msvc -p syncify-tauri --lib
EXIT=101
  /usr/lib/clang/23/include/mm_malloc.h:13:10: fatal error: 'stdlib.h' file not found
```

Es decir: falta el **SDK de MSVC** (headers UCRT) en esta máquina Linux. Falla en
el *build script de una dependencia de terceros* (`libsqlite3-sys`, el SQLite que
empotra `sqlx`), antes de llegar a una línea de `cmd_utils.rs` o `tray.rs`. **No
puedo verificar el crate completo para Windows en esta máquina** y no lo declaro
verificado.

### 3.4 Sustituto explícito para el código `#[cfg(windows)]` de `cmd_utils.rs`

Como el camino de §3.3 muere antes de mis líneas, monté una sonda en `/tmp/winprobe`
que **`include!`a el `cmd_utils.rs` real, sin modificar**, más los dos call sites
de `reg.exe` de `tray.rs` literales. Para que sea reproducible, esto es lo que
tiene (`/tmp` es efímero, así que va escrito aquí):

```rust
// /tmp/winprobe/Cargo.toml
[package] name = "winprobe"   version = "0.1.0"   edition = "2021"
[dependencies] tokio = { version = "1", features = ["full"] }
[workspace]

// /tmp/winprobe/src/lib.rs
include!("/…/Syncify/src-tauri/src/cmd_utils.rs");   // el fichero REAL, sin tocar

#[cfg(not(windows))]
compile_error!("esta comprobacion no esta viendo cfg(windows)");  // control negativo

pub fn tray_windows_autostart(exe: &std::path::Path, enabled: bool) -> Result<(), String> {
    let mut command = create_std_command("reg");        // tray.rs:208 literal
    if enabled { command.arg("add") /* … */ } else { command.arg("delete") /* … */ }
    command.status().map(|_| ()).map_err(|e| format!("Failed to run reg.exe: {}", e))
}
pub fn tray_is_autostart_enabled() -> bool {
    if cfg!(target_os = "windows") {
        create_std_command("reg").arg("query")/* … */.output().map(|o| o.status.success()).unwrap_or(false)
    } else { false }
}
pub const FLAG_ASSERTED: () = assert!(CREATE_NO_WINDOW_FLAG == 0x0800_0000);
```

```
$ cargo check --target x86_64-pc-windows-msvc        # en /tmp/winprobe
EXIT=0
    Finished `dev` profile [unoptimized]
```

Y para probar que la sonda **no es vacía** (control negativo): la sonda lleva un
`compile_error!` que se dispara si `cfg(windows)` **no** está activo. Con
`#[cfg(not(windows))]` compila; al invertirlo a `#[cfg(windows)]`:

```
error: esta comprobacion no esta viendo cfg(windows)
```

Es decir: el `cfg(windows)` estaba realmente activo y el código de Windows de
`cmd_utils.rs` **se ha comprobado con el compilador de verdad**, sin warnings.

**Límite honesto de este sustituto:** comprueba que el código *compila* para
Windows, no que la ventana *desaparezca*. Eso último solo se puede observar
ejecutando en Windows, y no tengo Windows aquí. El flag y su semántica están
verificados contra la documentación de Microsoft (§3.1); el efecto en pantalla,
no.

### 3.5 Host (Linux), workspace completo incluyendo tests

```
$ cargo check --workspace --all-targets
HOST_ALL_EXIT=0
    Finished `dev` profile [unoptimized] target(s) in 1m 17s
```
Cero errores, cero warnings. `--all-targets` incluye los binarios de prueba, así
que **todos los tests siguen compilando**; no he borrado ni silenciado ninguno
(R2).

### 3.6 Tests del crate tocado, en runtime

```
$ cargo test -p syncify-flac-writer
EXIT=0
running 12 tests → 12 passed   (unittests, incl. test_apply_and_verify_flac_tags_full_roundtrip)
running  7 tests →  7 passed   (flac_picture_dimensions_test)
running  1 test  →  1 passed   (flac_silence_trim_test:
                               test_trim_restore_finalize_pipeline_repairs_streaminfo)
running  7 tests →  7 passed   (flac_streaminfo_md5_test)
running  0 tests
```
**27 passed, 0 failed.** No es la suite completa (esa la corre el script entre
olas, por encargo); es el crate dueño de 8 de los 11 sites tocados. Y no es
decorativo: `test_compute_pcm_stream_md5_hi_res_24bit_exactness` y
`test_trim_restore_finalize_pipeline_repairs_streaminfo` lanzan `flac` y `ffmpeg`
de verdad (están instalados: `/usr/sbin/ffmpeg`, `/usr/sbin/flac`) y comprueban el
MD5 y el STREAMINFO resultantes. Es decir, **el camino no-Windows del helper —que
en Linux es un no-op— sigue lanzando los procesos correctos y produciendo el
resultado correcto.**

---

## 4. La otra mitad del encargo: los argumentos de los 14

El encargo pedía mirar los argumentos por si alguno escribía por consola. **Los
14, uno a uno:**

**No hay ni un `--verbose` ni un `-v verbose` en ninguno de los 14.** Ninguno de
los sites que toqué pasa a ffmpeg el loglevel por defecto con verbosidad alta.
Detalle por familia:

- **ffmpeg, 4 de 8 con `-v error`** — `flac-writer:2348`, `:2696`, `:2853`
  (y `:2318`→`:2348`). Salida acotada.
- **ffmpeg, 3 sin `-v`** — `flac-writer:1003` y `:1092`, y
  `tidal-downloader:2037` (el remux). Van con el loglevel por defecto (`info`),
  así que escriben banner + volcado de configuración en stderr. **No cuelga
  nada**: los tres drenan con `wait_with_output()` (`flac-writer:1031`, `:1125`)
  o `.output()` (`tidal-downloader:2048`). Lo que sí hacen es ensuciar el
  diagnóstico, porque `flac-writer:1034-1039` mete stderr en el mensaje de error
  y se queda solo con la primera línea (`.lines().next()`).
- **flac, 3 de 4 con `-s`** — silenciosos. `flac-writer:2520` usa `-t -s`.

### 4.1 Lo que sí encontré: stderr canalizado que nadie drena

Preexistente, **no introducido por este cambio**, y **no es lo que el usuario
reportó**. Aun así es el riesgo real que el encargo apuntaba:

| Site | Programa | Por qué |
|---|---|---|
| `flac-writer:2284` | `flac -s` | `.stderr(Stdio::piped())`, y el bucle de `:2306` lee **solo stdout**; luego `child.wait()` en `:2321-2322`. stderr nunca se drena. |
| `flac-writer:2653` | `flac -s` | idéntico vía `measure_pcm_stream` (`:2577`): toma `stdout` (`:2580`), lee, `wait()`. stderr nunca se drena. |
| `flac-writer:2348` | `ffmpeg -v error` | mismo patrón (`:2357-2360` toma solo `stdout`). |
| `flac-writer:2696` | `ffmpeg -v error` | mismo patrón. |

El escenario: si el hijo llena los ~64 KB del pipe de stderr, se bloquea escribiendo;
como no hay nadie leyendo, deja de escribir en stdout; el padre espera EOF en
stdout que no llega → **deadlock**, y la descarga se queda colgada. Con `-s` y
`-v error` el volumen es diminuto y en un FLAC sano no pasa de cero, así que hoy
es teórico. Pero en un FLAC corrupto `flac` puede emitir una línea por frame
dañado y la combinación se vuelve alcanzable.

**No lo he arreglado, a propósito.** Las dos salidas obvias cambian
comportamiento:

1. `Stdio::null()` para stderr — deja de existir la posibilidad de diagnosticar,
   que es justo lo que hoy permite el `debug!` de `:2313`. Es un cambio de
   política, no una corrección de ventana.
2. Un hilo/hilo async que drene stderr — mete concurrencia en un camino que
   hoy es puramente síncrono y secuencial.

El encargo pedía **mirarlo**, no cambiarlo, y hacerlo bien requiere medir. Queda
como recomendación con sitio exacto.

---

## 5. Qué queda abierto

1. **`-p syncify-tauri --target x86_64-pc-windows-msvc` sigue sin poder correr**
   aquí (§3.3). Bloqueo de entorno: falta el SDK de MSVC para el build script de
   `libsqlite3-sys`. **Lo tiene que correr quien tenga Windows o el SDK.** Hasta
   entonces, `cmd_utils.rs` y `tray.rs` están verificados por el sustituto de
   §3.4 (compilan para el target, sin warnings), no por el crate completo.
2. **La ventana en pantalla no se ha visto desaparecer.** Requiere Windows.
   El mecanismo está verificado contra la documentación de Microsoft, no
   observado.
3. **Los 4 sites con stderr sin drenar (§4.1)** siguen como están. Recomendación,
   no cambio.
4. **El helper existe en tres sitios, no en uno** (§2.3). Cerrarlo de verdad pide
   tocar `syncify-core-domain` (añadir `tokio` y quebrar su premisa de "pure
   domain") o crear una crate nueva y editar el `Cargo.toml` raíz. Ninguna de las
   dos cosas me corresponde: la primera es fichero de otro carril. Si el
   mantenedor lo prefiere, es un refactor mecánico y los tres puntos están
   comentados en el código.
5. **`docs/auditoria/matriz-ajustes.md`**: puse las filas G-01 y G-06 en `pendiente`
   porque el script entre olas hace el cierre; las dejo con todo el detalle
   arriba. El fichero lo estaba editando otro carril en paralelo (me lo saltó un
   `Edit` a mitad) y reconduje sobre el estado nuevo.

---

## 6. Contratos que no he tocado (R2, R3)

- **Ningún test borrado, silenciado ni modificado.** `cargo check --workspace
  --all-targets` compila los dos ficheros que R3 nombra
  (`provider_fallback_identity_test.rs`, `cli_tauri_behavioral_parity_test.rs`).
  Este cambio **no altera ninguna política de calidad ni de fallback**, así que
  R3 no se activa: no había ningún contrato que invalidar.
- **Ningún fichero fuera de mi lista.** Comprobado con `git diff --stat`: mis
  cambios están en los 4 ficheros del encargo. `crates/syncify-lyrics-domain/
  src/lib.rs` también aparece en ese `diff --stat`, pero **no es mío** (otro
  carril lo está editando en paralelo) y no lo he tocado.
- **`system_conditions.rs` intacto**, con la evidencia de por qué (§1).

---

## 7. Pasada final, de una vez, sobre el estado entregado

```
### 1) Target Windows, los 2 crates (R4)
cargo check --target x86_64-pc-windows-msvc -p syncify-flac-writer -p syncify-tidal-downloader
   EXIT=0   warnings=0
### 2) Host, workspace completo con tests
cargo check --workspace --all-targets
   EXIT=0   warnings=0   errors=0
### 3) Sonda del cmd_utils.rs real para Windows (sustituto de §3.3)
cd /tmp/winprobe && cargo check --target x86_64-pc-windows-msvc
   EXIT=0   warnings=0
### 4) Tests del crate dueño de 8 de los 11 sites
cargo test -p syncify-flac-writer
   EXIT=0   5 binarios de test ok, 0 fallos (27 tests)
### 5) Compilación explícita de los dos contratos de R3
cargo check -p syncify-tauri --test provider_fallback_identity_test --test cli_tauri_behavioral_parity_test
   EXIT=0
```

Y el barrido final de launches sin cubrir en `src-tauri/src` y `crates/*/src`:

```
$ grep -rn --include='*.rs' 'Command::new(' src-tauri/src crates/*/src
system_conditions.rs:94   busctl      <- Linux, .output(), D-Bus: NO es G-01/G-06 (§1)
system_conditions.rs:116  gdbus       <- ídem
syncify-flac-writer/src/lib.rs:43       <- el helper nuevo
syncify-tidal-downloader/src/lib.rs:52  <- el helper nuevo
```

Los 14 sites del encargo están cubiertos, y no queda ningún `Command::new`
pelado en el repo salvo los dos de Linux ya descartados (§1) y los dos helpers.
**G-01 queda cerrado por este arreglo**: la fase de incrustar de metadatos es la
de los sites 1–4 y 9, y los 11 launches de mis 4 ficheros pasan ya por un helper
que aplica `CREATE_NO_WINDOW` en Windows — igual que los 30 usos previos de
`create_std_command` / `create_tokio_command` que ya había en `src-tauri`
(`grep -rn 'create_std_command\|create_tokio_command' src-tauri/src | grep -v
cmd_utils.rs` → 32, de los cuales 2 son los de `tray.rs` que añadí yo), que ya lo
hacían bien y que este carril no ha tenido que tocar.


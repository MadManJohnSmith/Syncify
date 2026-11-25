# Cola técnica: composición WebKitGTK sin perder compatibilidad Intel Iris Xe

**Estado:** pendiente de diseño, reproducción y benchmark; sin cambio de código.
**Fecha de origen:** 2026-10-13. **Última revisión:** 2026-10-25 (nota de estado). *(Nota 2026-10-25: el motivo original para separar este fichero —ficheros «reservados» por el workflow de descargas concurrente— ya no aplica; la cola sigue viva por sí misma y su estado canónico está en `docs/auditoria/matriz-ajustes.md`. El incidente del pánico de bandeja de su sección «Incidente» quedó resuelto por C-22 (`0e53dad`, ver `docs/Deuda_Tecnica_y_UX.md`).)*
**Nota 2026-11-22:** WK-01 (#52) se ejecutó el 2026-11-21 (PR #353, `f97ff19`, en `main` vía el consolidado #354): harness `tools/wk01-composicion.sh` e informe `docs/historico/wk01-composicion-webkitgtk-2026-11-21.md`. El veredicto vale solo para la máquina de carriles; #52 sigue abierto a propósito hasta medir el portátil Intel Iris Xe. El «Estado» de arriba es del 2026-10-25 y no se reescribe.
**Ejecución**: [#52](https://github.com/MadManJohnSmith/Syncify/issues/52), [#53](https://github.com/MadManJohnSmith/Syncify/issues/53), [#54](https://github.com/MadManJohnSmith/Syncify/issues/54) y [#55](https://github.com/MadManJohnSmith/Syncify/issues/55) (spec [#35](https://github.com/MadManJohnSmith/Syncify/issues/35)).

## Incidencia y evidencia

El usuario observa retardos superiores a un segundo al cambiar de pantalla o pestaña, y scroll que tarda y da tirones en toda la app. En `src-tauri/src/main.rs:55-81` se asignan, antes de crear el webview y salvo que ya estén definidas, `WEBKIT_DISABLE_DMABUF_RENDERER=1` y `WEBKIT_DISABLE_COMPOSITING_MODE=1`. Ambas estaban presentes en `/proc/<WebKitWebProcess>/environ` durante 10 descargas simultáneas. El comentario del código explica que desactivar solo DMABuf no evitaba la creación de un display EGL, y que desactivar la composición mantiene el webview en una ruta Cairo sin EGL.

El workaround se introdujo por un abort real en la AppImage v0.3.0: «Could not create default EGL display: EGL_BAD_PARAMETER. Aborting...» en un equipo Intel Iris Xe con KDE/Wayland. El usuario identifica ese equipo como **Dell Latitude 5420, Core i5-1145G7**. No se conoce aún la versión de WebKitGTK, Mesa, firmware, kernel ni el camino preciso de fallo en ese portátil. No afirmar que sea un error exclusivo del chip ni de DMABuf: el comentario solo documenta que quitar DMABuf no bastó en esa reproducción.

En el equipo medido actualmente (NVIDIA GTX 1050 Ti, driver 580.178.04, KDE/Wayland; la ejecución release posterior usó `GDK_BACKEND=wayland`), `eglinfo -B` informa EGL 1.5 y renderer NVIDIA para X11, Wayland y GBM. Tiene WebKitGTK 2.54.1-1 y Mesa 26.2.4-1. **Esto no certifica que el compositor WebKitGTK funcione en él:** solo comprueba que EGL está disponible fuera de WebKit. La primera medición fue en debug; posteriormente se midió release bajo otra carga. Con descargas en marcha se observaron ~35% de CPU en Rust y 48–58% en WebKitWebProcess; no se midieron FPS, duración real de navegación ni comparación A/B. La composición desactivada es un sospechoso de alto impacto, **no una causa raíz comprobada de los retardos**.

## Invariantes de compatibilidad

1. El Latitude 5420 debe seguir arrancando y mostrando contenido aunque EGL/DMABuf fallen. **Nunca** retirar globalmente la ruta compatible ni usar la app principal como sonda de una configuración que puede abortarla.
2. `LIBGL_ALWAYS_SOFTWARE=1` global no es solución: degrada todas las GPU, incluidas las funcionales. `eglinfo` exitoso no basta para habilitar aceleración WebKit.
3. Una preferencia explícita `Compatible` debe prevalecer sobre auto-detección. También debe existir una recuperación fuera de la UI (argumento de arranque o variable de entorno documentada) para pantalla negra o fallo anterior a que la UI sea operable.
4. No activar aceleración basándose solo en la marca/modelo de la GPU ni asumir que X11 y Wayland comparten comportamiento. Tener en cuenta cambios de driver, WebKitGTK, Mesa, sesión, dispositivo DRM y display.

## Diseño propuesto para investigación/implementación

- **Primero, experimento aislado y medido.** En el equipo NVIDIA ejecutar un webview mínimo de WebKitGTK *en un proceso hijo independiente* con entorno acelerado, mismo backend gráfico que la app y tiempo máximo de espera. Registrar código/senal de salida, pantalla visible y frames. Comparar con la ruta compatible y con un build release sobre las mismas pantallas y carga. No tocar ni reiniciar la instancia de Syncify que descarga pistas.
- **Sonda segura.** Si se confirma que WebKitGTK puede inicializar, componer y presentar contenido en ese entorno, usar un proceso hijo supervisado con timeout y kill del hijo colgado. La prueba debe observar presentación real (no solo `eglInitialize`) y aislar el posible abort. La propia sonda, su dependencia de display y cómo detectar un frame negro requieren prototipo: no se presume una API fiable de detección automática.
- **Política de selección.** `Automático` solo habilitaría aceleración con sonda positiva y mecanismo de recuperación probado; `Compatible` fuerza la configuración actual. `Acelerado` sería opt-in avanzado con aviso y posibilidad de volver a Compatible sin abrir la UI. Cachear resultado por fingerprint de entorno (backend X11/Wayland, GPU/dispositivo, driver, WebKitGTK/Mesa) y revalidar si cambia; ante fallo o incertidumbre usar Compatible. Una estrategia de doble arranque necesita marcador de inicio/éxito persistido y tratamiento de cierres de energía, para no confundirlos con fallo del webview.
- **Persistencia:** preferencia en datos locales; si requiere esquema, migración nueva. Las variables se deben decidir antes de crear WebKit, no tras montar la UI. Respetar overrides existentes (`main.rs` solo las fija cuando no hay valor explícito) y documentar los valores para recuperar compatibilidad desde terminal.

## Pruebas de aceptación

1. Latitude 5420: arranque y contenido visible en Compatible, también después de un intento fallido de Automático y después de una actualización de WebKit/Mesa; sin pantalla negra ni bucle de aborts.
2. Equipo NVIDIA: medir p50/p95 de entrada→primer frame y tiempo de cambio de vista; latencia de scroll/fotogramas perdidos, CPU de WebKitWebProcess y memoria. A/B compatible vs acelerado, debug vs release, con igual carga de descargas y en reposo. Definir umbrales a partir de baseline; no atribuir mejora a la GPU sin control de build y carga.
3. Tests automatizados puros para precedencia (override explícito > preferencia > sonda > fallback), invalidación de fingerprint, timeout/crash del hijo, recuperación tras fallo y ausencia de bucle de arranque. Smoke gráfico manual en X11 y Wayland; CI sin GPU no sustituye hardware Intel afectado.
4. Trazas de decisión sin datos sensibles: modo solicitado/efectivo, razón de fallback, versiones y clave de entorno; limitar volumen y no registrar información personal del equipo.

## Frontera de trabajo

No cambiar `main.rs`, Cargo, migraciones ni parámetros de arranque mientras el workflow de descargas esté ejecutando su puerta de compilación y el usuario descargue pistas. Este documento registra el trabajo pendiente para hacerlo después. No editar los dos ficheros de cola reservados hasta su liberación.

---

## Mediciones del 2026-10-13 (app real, sin descargas en vuelo)

El usuario mantuvo la app abierta y navegó por pestañas mientras se medía. Después
detuvo la interacción. Se compiló un build release desde el árbol actual
(`cargo build --release --bin syncify-tauri`, 9m23s, binario de 48 MB, frente a los
119 MB del debug) y se arrancó con `SYNCIFY_ENV=release`.

**Reposo tras arranque, instancia release:**

| Proceso | RSS | Hilos | CPU en 30s |
|---|---:|---:|---:|
| `syncify-tauri` | 267 MB | 35 | 1008 ticks (33%) |
| `WebKitWebProcess` | 315 MB | 21 | 116 ticks |

**El mismo proceso en build de debug, medido antes:** 26 ticks/10s (2%) y WebKit 10
ticks/10s. Es decir, el release consume **más CPU en reposo que el debug** en este
momento, con el log de aplicación sin escribir una línea nueva en 30 s.

**Dónde se va el CPU del hilo principal:** `stime` (tiempo de sistema) estable en
~23% en ventanas de 2 s, con el hilo principal aparcado en `ppoll`. El número de
descriptores relacionados con SQLite se desglosan como 10 de `syncify.db` (coincide con `max_connections(10)`), 1 del `-shm` y 10 del `-wal`, frente a
`max_connections(10)` en `src-tauri/src/db.rs:56-65`. El WAL no cambia de tamaño
durante 4 s de muestreo y no hay escrituras nuevas, así que el despertar del event
loop no viene de throughput de disco.

**No se pudo atribuir a un culpable concreto** en esa fase; `strace` y `perf`
se instalaron y utilizaron después (véase revisión de evidencia).
Tampoco se hicieron medidas de navegación (ping/pong por IPC, tiempo de cambio de
vista, frames por segundo) porque eso requiere instrumentar la UI; **no se hizo**.

**Consecuencia para esta incidencia:** la hipótesis "composición acelerada" sigue
sin probarse en A/B y el A/B es el paso que falta. La diferencia debug/release en
reposo, más el número de descriptores de base de datos, abre una **segunda línea de
investigación paralela** (conexiones SQLite y event loop) que no estaba en esta
entrada y conviene abrir como incidencia propia.

**Acción ejecutada por precisión:** se cerró la instancia `tauri dev` y se arrancó el
binario release. Si se quiere volver al modo desarrollo hay que relanzar
`npm run tauri dev`.

---

## Revisión de evidencia del 2026-10-13

La primera versión de esta sección atribuía 5988 despertares/s al hilo principal,
22,09 % del CPU total a `libpixman`, y un techo de 30 FPS al compositor. Ninguna
de esas tres cifras tenía una definición/denominador correcto. Se retiran.
La comparación debug/release de arriba tampoco controló la carga: había descargas
y distintas tareas en momentos distintos, por lo que **no establece** qué build
responde mejor. El equipo medido estaba en KDE/Wayland con `GDK_BACKEND=wayland`
en `/proc/<pid>/environ` de la ejecución release posterior; la afirmación previa
de que se había medido el binario release con `GDK_BACKEND=x11` no es correcta.

### Trazado de la ejecución release bajo strace

En `/tmp/syncify-trace.log`, PID 518005, 23:52:16–00:05:04, se observaron
88.123 llamadas `ppoll` que terminaron indicando un descriptor listo (87.863 en
el hilo principal), 45.357 que expiraron y 149.557 líneas `unfinished`; estas
últimas **no** son llamadas terminadas adicionales. El fd 9 del proceso era un
`anon_inode:[eventfd]`. El conteo agregado abarca una ejecución con descargas
en vuelo y no puede interpretarse como una tasa de reposo ni como el número de
operaciones del mismo productor. En todo el intervalo, los 87.863 retornos listos del hilo principal
equivalen a una media aproximada de 114/s; una clasificación anterior de
`unfinished`/`resumed` los duplicó y produjo la cifra errónea de 19.000/s.
El origen de las señales al `eventfd` no se identificó; `ptrace_scope=1` impidió
adjuntar `strace` a la ejecución sin trazador. No se debe llamar *busy loop*
del hilo principal a esta evidencia, ni atribuirlo a Tokio, SQLx o a WebKit.

En una nueva ejecución release sin `strace` (PID 520373) y con descargas activas,
medidas de `/proc/<pid>/task/*/status` durante 5 segundos mostraron unos
20.883 cambios de contexto/s para el proceso, de los cuales la mayoría eran
voluntarios: cuatro hilos `tokio-runtime-worker` a 1.783–3.769/s y varios de
los diez `sqlx-sqlite-worker` a 1.006–1.325/s. Es una tasa elevada y verificable,
pero estos son cambios de contexto de **todos** los hilos, no retornos de
`ppoll` del hilo principal. Los hilos se atendían junto a descargas, hashes,
FLAC y operaciones SQLite; por ello no se deduce un bucle vacío ni causalidad
con la latencia de navegación. Los diez workers SQLx concuerdan con las diez
conexiones máximas configuradas en `src-tauri/src/db.rs`; diez ficheros de base,
diez WAL y un SHM no son 21 conexiones. La revisión estática de sleeps e
intervalos no encontró ningún bucle periódico activo en reposo que explique
miles de iteraciones/s; tampoco excluye eventos de una ruta no inspeccionada.

Un nuevo `perf record -F 99 --call-graph fp -p 520373` obtuvo 3252 muestras,
con muestras de `syncify_flac_writer::measure_pcm_stream`, SHA-256 y
`sqlite3VdbeExec`. El perfil anterior bajo `strace` tenía muestras de
`libpixman`, por lo que hay trabajo de rasterizado CPU en aquella ejecución,
pero el porcentaje de overhead de una tabla filtrada de símbolos no prueba
que fuese 22 % del CPU total de la app ni que cause el retardo. Las muestras
de CPU en librerías de dibujo y en la descarga varían con la carga y el lugar
del muestreo.

### Prueba aislada de WebKitGTK

La prueba de GTK/WebKit que invalida un área cada ~33 ms registró 258
**callbacks de temporizador** en unos 8,5 s tanto con las variables de
compatibilidad como sin ellas. No registró fotogramas realmente presentados:
no es una medición de FPS ni demuestra un límite del compositor o igualdad de
rendimiento entre las rutas. La variante sin variables emitió
`Failed to create GBM buffer ... Invalid argument`, aunque no abortó; esto
no confirma aceleración funcional ni autoriza quitar el workaround que
protege el Dell Latitude 5420. Un intento posterior de inspector remoto
con `WEBKIT_INSPECTOR_SERVER` no abrió puerto en esta versión.

### Incidente en la ejecución de medición

El proceso release sin `strace` acabó con un pánico en el hilo principal:
`tray-icon-0.25.1/src/lib.rs:455:19: RefCell already borrowed`, inmediatamente
después de finalizar una descarga (registro en `/tmp/syncify-clean.log`,
líneas 3413–3419). Es un problema separado; no se demostró que lo causaran
WebKit, `perf` o el sondeo de `/proc`. No se modifica el código de bandeja
ni el resolver mientras el trabajo concurrente siga sin coordinarse.

### Veredicto y datos faltantes

Hay actividad de rasterizado por CPU y una tasa alta de cambios de contexto
bajo carga de descarga. **No está justificada** la conclusión de que desactivar
composición sea la causa de los >1 s de retardo, que SQLx esté bloqueando la UI,
ni que se hayan medido 30 FPS. Quedan sin medir las latencias entrada→primer
frame de cambio de vista y scroll en la ventana real, un A/B controlado con
igual carga y un smoke en el Latitude con fallback. Hasta entonces se mantiene
`WEBKIT_DISABLE_COMPOSITING_MODE=1` de manera predeterminada. Para cerrar la
causalidad harán falta trazas sincronizadas con interacciones reales, no otro
conteo de callbacks de una sonda aislada.

## WK-02 · sonda de arranque supervisada (2026-11-24)

La implementación vive en `src-tauri/src/webkit_probe/` y se invoca desde
`src-tauri/src/main.rs` antes de construir Tauri; el reporte se registra cuando
el subscriber de logging ya está instalado. **Todavía no cambia la política de
render:** la app principal conserva
`WEBKIT_DISABLE_DMABUF_RENDERER=1` y `WEBKIT_DISABLE_COMPOSITING_MODE=1` por
defecto y respeta overrides explícitos; la selección Automático/Compatible y la
recuperación pertenecen a #54.

### Ejecución y evidencia

- El padre recopila el fingerprint sin inicializar GL y lanza el propio binario
  con `--syncify-webkit-probe-child`. El hijo usa GTK 3 + WebKitGTK 4.1, un
  perfil XDG temporal con permisos `0700` en `XDG_RUNTIME_DIR` (o `/tmp`, nunca
  un `TMPDIR` potencialmente remoto), `GSETTINGS_BACKEND=memory` y un grupo de
  procesos propio. Su entorno se reconstruye con una allowlist de sesión,
  locale y librerías gráficas: no hereda credenciales de Syncify ni variables de
  `.env`, y `HOME` apunta al perfil temporal.
- El hijo quita las variables compatibles y fuerza la política WebKit
  `HardwareAccelerationPolicy::Always`. Carga una fixture local de 480×360 con
  bandas de color y un bloque rojo en movimiento; no carga recursos de red.
- El supervisor concede 8 s, envía TERM al grupo y espera 1 s antes de KILL.
  La espera del reap tras KILL está acotada a 500 ms y, si hace falta, un hilo
  propio espera al hijo sin depender del runtime de Tauri. El grupo se barre al
  salir. El hijo tiene un deadline propio de 6 s. Un timeout, señal, aborto,
  error de escritura o pánico se clasifica sin propagar el fallo a la app
  principal.
- `requestAnimationFrame` cuenta producción del web process y `after-paint`
  cuenta pintado de GTK. Ninguno basta por sí solo:
  - **X11:** solo una captura de la ventana con al menos 300 píxeles rojos de la
    fixture **en ambas capturas** y al menos 300 píxeles cambiados produce
    `supported/content_observed`.
  - **Wayland:** `after-paint` y el snapshot se registran, pero cualquier
    resultado de presentación —incluidos no-ready, sin paints o sin frames— es
    `inconclusive`: no hay prueba de píxeles visibles.
  - Si no hay paints y el snapshot sí contiene el marcador, se registra
    `unsupported/rendered_not_presented`, reproduciendo la señal medida en
    X11/acelerada por WK-01.

### Fingerprint y privacidad

La clave versionada (`probe_contract_version = 1`, fixture
`wk02-red-marker-v1`) cubre backend efectivo, sesión y escritorio, endpoints de
display, GPU/PCI y versión de driver por dispositivo, nodos DRM, kernel,
WebKitGTK, Mesa y overrides de render. Los overrides de Mesa/GLX/EGL, selección
`DRI_PRIME`, `LD_LIBRARY_PATH` y `LD_PRELOAD` se incorporan mediante hash, sin
registrar rutas o valores sensibles. WebKitGTK se lee de la biblioteca enlazada;
Mesa procede del gestor de paquetes y la clave incluye su procedencia (`dpkg`,
`pacman` o `rpm`), que no garantiza que sea la biblioteca cargada por un
AppImage. Un cambio material invalida la clave y un fingerprint incompleto no
permite un positivo. Las trazas incluyen la clave y un resumen de esos campos;
no incluyen nombre de host, usuario, PID, carga ni rutas temporales.

### Límite de la detección de negro

El umbral (`max_channel <= 8`, sin píxeles coloreados) es un **prototipo atado a
la fixture**, no un detector universal de pantalla negra. Una captura X11 negra
se marca `inconclusive/suspected-black` salvo que la ausencia de paints y un
snapshot con contenido permitan afirmar `rendered_not_presented`. La sonda no
distingue oclusión por otra ventana, pantalla apagada, política del compositor,
captura remota ni ambigüedad de Wayland. Tampoco prueba que el render use una GPU
física: la política WebKit y la presentación visible son señales distintas. Por
eso un resultado positivo no autoriza por sí solo activar aceleración.

### Verificación realizada y pendiente

Las pruebas automatizadas cubren selección de backend/sesión, invalidación del
fingerprint, clasificación de evidencia, parsing de píxeles y timeout/TERM/KILL.
Smoke del 2026-11-24:

- **X11 bajo Xvfb:** `ready=true`, 145 frames y 145 paints en el intervalo;
  3600 píxeles rojos en ambas capturas y 7515 cambiados. La evidencia cumple el
  umbral positivo. Con `GDK_SCALE=2`, ventana y snapshot midieron 960×720 con
  14400 píxeles rojos y 31293 cambiados. Mesa advirtió que Xvfb no ofrece DRI3:
  no se interpreta como prueba de aceleración GPU.
- **Wayland real (`wayland-1`):** `ready=true`, 144 frames y 144 paints; snapshot
  con 3600 píxeles rojos y captura de ventana no disponible. El verdicto permanece
  inconcluso por falta de prueba de píxel.
- El servidor XWayland `:0` de la sesión exigió autorización que el harness no
  tenía; no se extrajo ni compartió su cookie. La cobertura X11 viene de Xvfb.
- **Pendiente:** repetir el smoke en el Dell Latitude 5420 y en una sesión X11
  autorizada; CI sin GPU no sustituye esa evidencia.

## WK-04 · precedencia, fallback y bucle de arranque (2026-11-25)

Esta sección fija el contrato que la política no puede invertir. El modelo puro
vive en `src-tauri/src/webkit_probe/policy.rs` y **no está cableado**: no lee el
entorno, no escribe ficheros ni construye el webview. #54 conecta la decisión
antes de crear Tauri; aquí solo se prueba que la precedencia, el fallback y el
corte del bucle se comportan como se documenta. Los tests están en
`src-tauri/src/webkit_probe/tests.rs` y corren con `nextest run -p
syncify-tauri --lib`, sin GPU, sin webview y sin captura de pantalla.

### Orden de decisión

De mayor a menor prioridad; cada nivel ignora los inferiores.

| # | Entrada | Ruta efectiva | Razón registrada |
|---|---|---|---|
| 1 | Override explícito `Compatible` | compatible | `override-compatible` |
| 2 | Override explícito `Acelerado` | acelerada | `override-accelerated` |
| 3 | Preferencia persistida `Compatible` | compatible | `preference-compatible` |
| 4 | Preferencia persistida `Acelerado` | acelerada | `preference-accelerated` |
| 5 | `Automático` con fallo previo de la misma clave | compatible | `previous-failure-fingerprint` |
| 6 | `Automático` con fingerprint incompleto | compatible | `fingerprint-incomplete` |
| 7 | `Automático` con sonda `Supported` | acelerada | `probe-supported` |
| 8 | `Automático` con sonda `Unsupported`/`Inconclusive`/`Skipped` | compatible | `probe-*` |

Un override explícito `Automático` no fuerza aceleración: manda sobre la
preferencia persistida y vuelve a consultar la sonda con las mismas guardas de
los niveles 5–8. El marcador de fallo solo frena el modo `Automático`: ni
`Compatible` ni un opt-in `Acelerado` quedan secuestrados por él. La salida de
recuperación fuera de la UI es el override `Compatible`: no depende de que la UI
llegue a ser operable.

### Matriz de driver y sesión conocidos

Las filas son **evidencia fechada**, no una tabla de GPUs soportadas. La
decisión no se toma por marca ni modelo (invariante 4); el fingerprint cubre
backend efectivo, sesión, display, GPU/PCI, driver por dispositivo, DRM, kernel,
WebKitGTK, Mesa y overrides de render, y un cambio material rearma el intento.
Una fila sin versión medida lo dice: no se rellena por analogía.

| Equipo / driver | Sesión · backend | WebKitGTK · Mesa | Evidencia | Lectura para la política |
|---|---|---|---|---|
| Dell Latitude 5420, Intel Iris Xe (i5-1145G7); driver y kernel sin medir | KDE/Wayland; AppImage v0.3.0 | sin medir | Abort real `Could not create default EGL display: EGL_BAD_PARAMETER` (2026-10, origen del workaround) | Ruta compatible obligatoria. `Automático` solo puede acelerar allí con sonda positiva y recuperación probada; no hay ninguna medición posterior. |
| NVIDIA GTX 1050 Ti, driver 580.196.07; kernel 7.3.4-1-cachyos | X11/XWayland `:0` | 2.54.3 · sin registrar | WK-01 2026-11-21: ruta acelerada 0 frames rAF, 0 paints, ventana 0/0 px, `Failed to create GBM buffer` 3/3 | Acelerar sobre X11 en esta máquina no presenta contenido; la sonda debe devolver `Unsupported`, no confiar en `eglinfo`. |
| NVIDIA GTX 1050 Ti, driver 580.196.07 | Wayland `wayland-1` | 2.54.3 · sin registrar | WK-01: ~58 paints/s sin errores EGL/GBM; captura de ventana no disponible | Señal de composición, no de píxel: el contrato WK-02 la clasifica `Inconclusive`, nunca `Supported`. |
| Runner de CI con Xvfb (Mesa sin DRI3) | X11 virtual | la del runner | Smoke WK-02 2026-11-24: `supported/content_observed` con 3600 px rojos en ambas capturas | Banco de la suite, no prueba de GPU física; no autoriza aceleración en hardware real. |
| Sesión XWayland `:0` real del 2026-11-24 | X11 | sin registrar | Sin cookie de autorización; no se extrajo ni compartió | No medido: no se infiere un resultado de la fila de Xvfb. |
| Sesión `tty` o sin display gráfico | ninguna | — | Sonda omitida: `skipped/no-graphical-session` | Compatible: una sonda omitida nunca acelera. |
| Clave nueva o sonda no ejecutada | cualquiera | — | fingerprint incompleto o sin resultado para esa clave | Compatible hasta una sonda nueva de esa clave. |

### Bucle de arranque

`AttemptLedger` guarda una sola clave: la del fingerprint cuyo arranque
acelerado no llegó a la UI. Repetir el mismo fallo es idempotente y no acumula
intentos; un arranque acelerado completo borra el marcador. Mientras la clave
coincida, `Automático` resuelve compatible las veces que haga falta: no hay
segundo, tercer ni enésimo intento que pueda repetir el abort. Una clave distinta
—driver, WebKitGTK, Mesa, sesión o display cambiados— rearma el intento, y la
clave antigua sigue bloqueada si se vuelve a ella. Un cierre de energía no borra
un marcador ya escrito: quitarlo exige la señal positiva de que la UI llegó a
mostrarse. El marcador de "intento iniciado" para la primera prueba y su
detección tras un corte pertenecen a #54.

### Cobertura y límites

| Test | Criterio que protege |
|---|---|
| `wk04_explicit_compatible_outranks_preference_and_positive_probe` | Preferencia explícita `Compatible` gana incluso con sonda positiva. |
| `wk04_override_outranks_the_stored_preference_in_both_directions` | Override explícito > preferencia persistida, incluido `Automático`. |
| `wk04_negative_incomplete_or_skipped_probe_falls_back_to_compatible` | Sonda negativa, inconclusa, omitida o fingerprint incompleto caen a compatible. |
| `wk04_accelerated_failure_is_not_retried_in_a_loop` | Un fallo previo no se repite en bucle; otra clave rearma; el éxito limpia. |
| `wk04_failure_ledger_only_gates_auto_not_an_explicit_opt_in` | El marcador frena solo `Automático`; no secuestra `Compatible` ni un opt-in `Acelerado`. |
| `wk04_modes_parse_strictly_and_verdicts_map_without_upgrading` | Parseo estricto de modos y mapeo de veredictos sin ascender señales. |

La matriz de sesión/backend no se duplica aquí: ya la cubre
`backend_matrix_keeps_session_and_effective_backend_separate`. Estos tests no
sustituyen el smoke en el Latitude 5420 ni la medición A/B: prueban la
decisión, no que una GPU concreta presente píxeles.

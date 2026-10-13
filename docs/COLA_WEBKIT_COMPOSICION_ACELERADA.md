# Cola técnica: composición WebKitGTK sin perder compatibilidad Intel Iris Xe

**Estado:** pendiente de diseño, reproducción y benchmark; sin cambio de código.
**Fecha:** 2026-10-13. Este fichero es separado porque `docs/Errores-Pruebas-2026-10-11.md` y `docs/auditoria/matriz-ajustes.md` están reservados por el workflow de descargas concurrente. Integrar la entrada a la cola canónica cuando se libere.

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

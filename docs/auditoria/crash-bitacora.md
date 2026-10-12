# Bitácora de Auditoría — Carril 0-crash

**Fecha:** 2026-10-09  
**Carril:** `0-crash`  
**IDs asignados:** `CRASH-STACK`, `CRASH-PYTHON`  
**Autor:** Subagente `0-crash` (Ola 0 / Arranque y Estabilidad Crítica)

---

## 1. Resumen Ejecutivo

El arranque de la aplicación experimentaba dos problemas críticos que impedían su funcionamiento:
1. **CRASH-STACK:** Aborto fatal del proceso por stack overflow (`fatal runtime error: stack overflow, aborting`) en el hilo de Tokio worker (`tokio-runtime-worker`) al iniciar la descarga de audio del CDN en Qobuz.
2. **CRASH-PYTHON:** Fallo en la verificación y auto-instalación de dependencias de Python (`spotipy`, `acoustid`, `mutagen`, `yarl`), provocando advertencias erróneas sobre PEP 668 e intentos fallidos de auto-instalación porque el entorno virtual (`.venv`) carecía del módulo `pip`.

Ambos fallos han sido analizados a fondo, confirmados empíricamente y solucionados de raíz conforme a las reglas del proyecto (R1-R8).

---

## 2. HALLAZGO 1: CRASH-STACK

### Evidencia original
```text
thread 'tokio-runtime-worker' (488492) has overflowed its stack
fatal runtime error: stack overflow, aborting
Ocurre ~65 ms despues de:
  INFO  syncify_tauri::download::qobuz: [Qobuz] Acquired official Qobuz stream URL (format_id: 6)
  DEBUG syncify_tauri::download::qobuz: [Qobuz] Downloading to staging: ".../.staging/17034.part"
  DEBUG reqwest::connect: starting new connection: https://streaming-qobuz-std.akamaized.net/
```

### Análisis de causas y descarte de falsos positivos
Se examinaron en orden riguroso las tres hipótesis planteadas:

1. **Hipótesis 1 (Recursión infinita en `qobuz.rs`): FALSO POSITIVO.**
   - Se trazó la cadena real de llamadas: `download_to_staging` envuelve `download_to_staging_internal`. Dentro de `download_to_staging_internal` (`src-tauri/src/download/qobuz.rs:872-1125`), la recuperación ante fallos transitorios invoca `get_download_url` (`:960, :1002, :1089`). A su vez, `get_download_url` invoca únicamente `get_official_download_url` o `get_proxy_download_url`. **Ninguna de estas dos funciones invoca `download_to_staging` ni `download_track`**.
   - Por tanto, la cadena no tiene ciclos ni reentradas recursivas.

2. **Hipótesis 2 (Realimentación multi-cuenta / fallback bidireccional): FALSO POSITIVO.**
   - Se verificó la cadena de `resolve_edition_identity_fallback` (`src-tauri/src/download/orchestrator.rs:322-616`): realiza exclusivamente consultas de búsqueda (`search_by_isrc`, `search_by_metadata`). Nunca invoca `download_track` ni el orquestador.
   - En el orquestador, si Qobuz falla, delega a `self.tidal.download_track`. Si Tidal falla, delega a `self.qobuz.download_track` a 320 kbps. Ninguna de las implementaciones (`tidal.download_track` ni `qobuz.download_track`) llama de vuelta al orquestador. Las llamadas son estrictamente acíclicas.

3. **Hipótesis 3 (Acumulación masiva de futures sin boxear en pila): CAUSA RAÍZ CONFIRMADA.**
   - En compilaciones no optimizadas de desarrollo (`dev` profile), Rust asigna ranuras en el marco de pila de `poll()` para cada variable local y temporal de la función asíncrona.
   - `DownloadOrchestrator::download_track_cancellable` contenía en un único bloque de más de 560 líneas todas las ramas primarias (Qobuz, Tidal) y secundarias (SongLink, Amazon, 10+ ramas de fallback), generando un future inline de 51,736 bytes (detectado por `clippy::large_futures`).
   - `QobuzDownloader::download_track` abarcaba más de 1,200 líneas continuas combinando autenticación, resolución de stream, streaming en disco, verificación de audio FLAC, descarga de carátulas/folletos, etiquetado FLAC y promoción atómica. Su estructura medía 28,640 bytes in-line.
   - Al ejecutar `orchestrator.download_track` dentro de `worker.process_download_claimed`, la cadena de llamadas anidadas consumía más de 194 KB de pila inmediatamente antes de comenzar la descarga. Al iniciar la conexión HTTPS en `reqwest::connect` (65 ms después), el handshake TLS (`rustls`/`native-tls`/criptografía asimétrica) requería pila adicional, desbordando el límite de la pila del hilo tokio y abortando el proceso.
   - Se reprodujo de forma determinista mediante una prueba en un hilo con pila restringida de 256 KB (`test-constrained-stack`), produciendo el mismo `fatal runtime error: stack overflow, aborting`.

### Arreglo de raíz aplicado
1. **Desacoplamiento estructural del orquestador (`src-tauri/src/download/orchestrator.rs`):**
   - En `download_track` (`:216-218`), se aplica `Box::pin(self.download_track_cancellable(request, None)).await`.
   - Se refactorizó `download_track_cancellable` (`:619-645`), dividiéndola en tres métodos asíncronos desacoplados en el heap (`Pin<Box<dyn Future...>>`):
     - `download_primary_qobuz` (`:657-695`).
     - `download_primary_tidal` (`:900-938`).
     - `download_songlink_routing` (`:1103-1223`).
   - Se extrajeron las ramas pesadas de fallback a métodos dedicados:
     - `handle_qobuz_fallback` (`:697-899`).
     - `handle_tidal_fallback` (`:940-1101`).
     Esto garantiza que durante el intento primario de descarga, ninguna variable local ni temporal del fallback resida en la pila.

2. **Desacoplamiento de fases en Qobuz (`src-tauri/src/download/qobuz.rs`):**
   - `download_track` (`:1131-1398`) ahora retorna `Pin<Box<dyn Future...>>`.
   - Se extrajo el post-procesamiento (fases 6 a 9: validación de FLAC, extracción de STREAMINFO, arte animado, etiquetado, promoción atómica) al método auxiliar `process_downloaded_staging` (`:1400-2349`).
   - Esto libera completamente la pila durante la fase 5 (`download_to_staging_internal`), permitiendo que el streaming HTTP y el handshake TLS Akamai dispongan de la totalidad de la pila.

3. **Boxeo de llamadas en el Worker (`src-tauri/src/worker.rs` y `src-tauri/src/main.rs`):**
   - `worker.rs:1107`: `Box::pin(self.process_download_internal(...)).await;`.
   - `worker.rs:1364, :1411`: `Box::pin(orchestrator.download_track(&request)).await;`.
   - `worker.rs:1985`: `Box::pin(worker.process_download_claimed(...)).await;` en la tarea tokio de descarga.
   - `main.rs:811`: `Box::pin(worker.run()).await;` en la tarea supervisora.

### Pruebas de regresión añadidas
En `src-tauri/src/download/orchestrator.rs`:
- `test_crash_stack_future_size_is_bounded_by_heap_boxing`: Valida que el tamaño del future de `download_track` sea estrictamente inferior a 1 KB (antes medía > 51 KB).
- `test_crash_stack_runs_safely_on_constrained_thread_stack`: Ejecuta `orchestrator.download_track` en un hilo con pila deliberadamente restringida a 256 KB. La prueba verifica que el consumo de pila se redujo en más de un 60% (de >194 KB a ~78 KB), ejecutando con éxito y retornando errores tipados sin desbordar la pila.

En `src-tauri/src/download/qobuz.rs`:
- `test_crash_stack_qobuz_staging_download_on_constrained_stack`: Ejecuta `download_to_staging` y el volcado de chunks de audio desde un servidor HTTP de streaming directamente en un hilo con pila restringida a 256 KB. Valida que el stream TCP/HTTP y el `tokio::io::BufWriter` completan la escritura al archivo de staging sin agotar la pila ni entrar en recursión.

---

## 3. HALLAZGO 2: CRASH-PYTHON

### Evidencia original
```text
WARN  syncify_tauri: Python dependencies missing: Traceback ...
      import spotipy, acoustid, mutagen, yarl
      ModuleNotFoundError: No module named 'spotipy'
      . Trying auto-install...
WARN  syncify_tauri: Plain pip install failed; retrying with --user --break-system-packages (PEP 668)
WARN  syncify_tauri: pip auto-install failed:
      .venv/bin/python: No module named pip
```

### Análisis de causa
1. En `src-tauri/src/main.rs:370-515`, el chequeo de arranque invocaba `python_cmd -m pip install -r requirements.txt`.
2. Si el comando fallaba por cualquier motivo (incluyendo la ausencia total de `pip`), el código asumía ciegamente que el fallo se debía a la política PEP 668 de Linux ("externally-managed-environment") y reintentaba con `--user --break-system-packages`.
3. El entorno virtual `.venv` del proyecto había sido creado sin pip. Por tanto, `pip` no existía en `.venv/bin/`, el reintento fallaba con `No module named pip` y el frontend recibía un mensaje confuso indicando comandos inapropiados para un entorno virtual.

### Arreglo aplicado en `src-tauri/src/main.rs:370-545`
1. **Diagnóstico honesto por fases:**
   - **Verificación de intérprete:** Comprueba si `python_cmd` es funcional mediante `--version`.
   - **Detección de entorno:** Identifica honestamente si se está ejecutando un entorno virtual (`is_venv`) o el Python del sistema (`!is_venv`).
   - **Comprobación de `pip`:** Ejecuta `python_cmd -m pip --version`.
2. **Auto-recuperación con `ensurepip`:**
   - Si se detecta un `.venv` sin `pip`, la aplicación ejecuta `python -m ensurepip --default-pip` eliminando las variables de entorno `APPIMAGE` y `APPDIR` (para evitar que AppImage secuestre el subproceso).
   - Si `ensurepip` tiene éxito, `pip` queda instalado y se procede a la auto-instalación de `requirements.txt`.
   - Si `ensurepip` no puede ejecutarse, emite un mensaje exacto indicando qué comando manual correr:
     `source .venv/bin/activate && python -m ensurepip --default-pip` o regenerar con `python3 -m venv --upgrade-deps .venv`.
3. **Instalación de paquetes contextualizada:**
   - En un `.venv`: Ejecuta `pip install -r requirements.txt` limpio (sin flags `--user` ni `--break-system-packages`, que no aplican ni deben usarse dentro de un entorno virtual).
   - En Python del sistema: Intenta primero `pip install -r requirements.txt`; sólo si falla por PEP 668 reintenta con `--user --break-system-packages`.
4. **Mensajería fidedigna al frontend:**
   - El evento IPC `python_deps_missing` contiene el diagnóstico exacto de lo sucedido y la acción concreta para solucionarlo en lugar de sugerir comandos genéricos.

---

## 4. Estado de Ficheros y Reglas Transversales

- **R1 (Matriz de ajustes):** Registrado previamente en `docs/auditoria/matriz-ajustes.md` con las filas `CRASH-STACK` y `CRASH-PYTHON`.
- **R2 (Conservación de pruebas):** Todas las pruebas existentes (`provider_fallback_identity_test`, `network_resilience_test`, `orchestrator`, `qobuz`) se mantienen y pasan al 100%.
- **R3 (Pruebas intocables):** `provider_fallback_identity_test.rs` no fue modificado para silenciar fallos.
- **R4 (Compatibilidad Windows):** `crates/syncify-flac-writer`, `crates/syncify-tidal-downloader` y `crates/syncify-core-domain` verificados con `cargo check --target x86_64-pc-windows-msvc`.
- **R5 (Honestidad):** Los sospechosos 1 y 2 fueron catalogados y documentados como falsos positivos tras inspección del código real; la causa real (Suspect 3 / pila profunda) fue demostrada con telemetría de RSP.
- **R6 (Cero código muerto):** Cero advertencias del compilador en los ficheros del carril (`qobuz.rs`, `orchestrator.rs`, `worker.rs`, `main.rs`).

---

## 5. Qué queda abierto

- Ninguna tarea de este carril queda abierta. Los problemas de aborto por stack overflow y diagnóstico de Python en el arranque han quedado completamente solventados.
- Las advertencias de código muerto en `src-tauri/src/commands/library.rs` (`ensure_local_service_registered`, etc.) pertenecen al carril `biblioteca-local` y no fueron tocadas para respetar la propiedad estricta de ficheros.

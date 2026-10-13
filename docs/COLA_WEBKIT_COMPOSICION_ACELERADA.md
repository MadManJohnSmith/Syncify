# Cola técnica: composición WebKitGTK sin perder compatibilidad Intel Iris Xe

**Estado:** pendiente de diseño, reproducción y benchmark; sin cambio de código.
**Fecha:** 2026-10-13. Este fichero es separado porque `docs/Errores-Pruebas-2026-10-11.md` y `docs/auditoria/matriz-ajustes.md` están reservados por el workflow de descargas concurrente. Integrar la entrada a la cola canónica cuando se libere.

## Incidencia y evidencia

El usuario observa retardos superiores a un segundo al cambiar de pantalla o pestaña, y scroll que tarda y da tirones en toda la app. En `src-tauri/src/main.rs:55-81` se asignan, antes de crear el webview y salvo que ya estén definidas, `WEBKIT_DISABLE_DMABUF_RENDERER=1` y `WEBKIT_DISABLE_COMPOSITING_MODE=1`. Ambas estaban presentes en `/proc/<WebKitWebProcess>/environ` durante 10 descargas simultáneas. El comentario del código explica que desactivar solo DMABuf no evitaba la creación de un display EGL, y que desactivar la composición mantiene el webview en una ruta Cairo sin EGL.

El workaround se introdujo por un abort real en la AppImage v0.3.0: «Could not create default EGL display: EGL_BAD_PARAMETER. Aborting...» en un equipo Intel Iris Xe con KDE/Wayland. El usuario identifica ese equipo como **Dell Latitude 5420, Core i5-1145G7**. No se conoce aún la versión de WebKitGTK, Mesa, firmware, kernel ni el camino preciso de fallo en ese portátil. No afirmar que sea un error exclusivo del chip ni de DMABuf: el comentario solo documenta que quitar DMABuf no bastó en esa reproducción.

En el equipo medido actualmente (NVIDIA GTX 1050 Ti, driver 580.178.04, KDE/Wayland pero `GDK_BACKEND=x11`), `eglinfo -B` informa EGL 1.5 y renderer NVIDIA para X11, Wayland y GBM. Tiene WebKitGTK 2.54.1-1 y Mesa 26.2.4-1. **Esto no certifica que el compositor WebKitGTK funcione en él:** solo comprueba que EGL está disponible fuera de WebKit. La app actual es `target/debug/syncify-tauri`, no un build release. Con descargas en marcha se observaron ~35% de CPU en Rust y 48–58% en WebKitWebProcess; no se midieron FPS, duración real de navegación ni comparación A/B. La composición desactivada es un sospechoso de alto impacto, **no una causa raíz comprobada de los retardos**.

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

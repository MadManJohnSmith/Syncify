# Cola técnica pre-beta: orden y solapamiento de etapas dentro de la descarga de una pista

**Estado:** DLX-01…06 y DFL-01 ya prerregistrados en la matriz; sin baseline comparable ni aceptación de implementación. DLX-02 y DLX-06 están condicionados, no aplicados. No acelerar antes de medir cronología, p95, throughput y fallos (`docs/auditoria/matriz-ajustes.md:580-590`). Los diez gates exit 0 aportados en el encargo antecedieron a la ampliación final de la prueba Qobuz comunicada por el carril residual: no cubren el candidato final ni cierran seis gates rojos heredados. GUI/cuentas/Symfonium NOT_EXERCISED.
**Fecha:** 2026-10-14. **Origen:** petición expresa del propietario sobre cómo se construye una descarga
individual (orden de metadatos y audio, incrustado de etiquetas, y qué se ejecuta en paralelo o en
serie), no sobre el volumen del lote ni sobre subir conexiones.

**Frontera de trabajo:** `docs/auditoria/matriz-ajustes.md`, el worker, los adaptadores y
`src-tauri/src/services/tidal_pipeline.rs` están siendo editados por otro carril. Este documento no
los modifica; sus IDs deben registrarse/actualizarse en la matriz (R1) por el único escritor antes
de tocar código, coordinado con ED-01…04 y DL-SVC-01…03. **Revalidar cada línea citada antes de
editar código:** el árbol cambió durante esta revisión (Tidal pasó de ~2.900 a 4.802 líneas y ganó
ramas adicionales).

---

## 1. Lo que se sabe y lo que NO se puede saber de los clientes comerciales

Hay que separar dos cosas: **qué está documentado** y **qué es interno de sus aplicaciones**.

- **Spotify:** su página de [escucha offline](https://support.spotify.com/us/article/listen-offline/)
  solo habla de límites y condiciones (hasta 10.000 pistas en hasta 5 dispositivos, conexión cada 30
  días para recoger datos de reproducción, y que no se pueden descargar canciones sueltas sino
  listas). **No documenta** si pide primero los metadatos, si incrusta etiquetas ni cuántas pistas
  descarga a la vez.
- **Apple:** publica una [API de descarga offline](https://developer.apple.com/documentation/avfoundation/avassetdownloadurlsession)
  que permite crear una tarea pasando el recurso y datos como la portada **antes** de iniciarla, y
  delegar la descarga al sistema fuera del proceso de la app. Eso describe una capacidad de la API,
  **no demuestra** que la app Apple Music use exactamente ese pipeline internamente. Además, una
  compra de iTunes Store y una descarga offline de suscripción Apple Music son productos distintos.
- **Tidal:** no dispuse de fuente oficial utilizable (la página de soporte devolvió error 403 en esta
  revisión), así que **no afirmo nada** sobre su secuencia interna.

**Consecuencia práctica:** no se toma de esos clientes una secuencia concreta de etapas, porque no
está publicada. Lo que sí está documentado y sí se puede tomar como referencia técnica es el
**transporte HLS** ([RFC 8216](https://www.rfc-editor.org/rfc/rfc8216)), que es una especificación
abierta: describe listas maestras y de medios, segmentos con URI y sus **rangos de bytes dentro de un
recurso mayor**, y recomienda traer segmentos por delante de la reproducción para absorber la
variabilidad de red. **No prescribe** ni un número de descargas paralelas ni un modelo asíncrono, y
no habla de etiquetado. HLS es una referencia de *transporte*, no de *pipeline de producto*.

Lo que **no** se copia en ningún caso: caché ligada a cuenta/aplicación, verificación periódica en
línea ni mecanismos para eludir restricciones de contenido. Syncify produce ficheros
propios con calidad, etiquetas, letras y manifiesto, y su pipeline se juzga solo con medición local.

---

## 2. Flujo actual de una sola pista, verificado en el código

Las dos ramas siguen el mismo orden, y las fases de **una** pista son prácticamente secuenciales.

| # | Etapa | Qobuz (`src-tauri/src/download/qobuz.rs`) | Tidal (`src-tauri/src/services/tidal_pipeline.rs`) |
|---|---|---|---|
| 1 | Auth / credenciales | `~:1279` | `:817` |
| 2 | Resolución de pista, edición y calidad; URL o manifiesto del audio | `:1350`, `:1459` | `:906` |
| 3 | Transferencia del audio a fichero temporal | `:1531` | `:1350` |
| 4 | Validación del payload y extracción de métricas físicas | `:1597` (STREAMINFO `:1616`) | `:1433` |
| 5 | Portada | `:1742` | `:1560` (y rama alternativa `:2014`) |
| 6 | Letras | `:1849` | `:1667` (rama `:2116`) |
| 7 | Enriquecimiento adicional (MusicBrainz) | `:1999` | `:1737` (rama `:2178`) |
| 8 | Etiquetado y verificación | `:2174` | `:1935` (rama `:2251`) |
| 9 | Promoción a ruta final (rename atómico) | `:2211` | `:2548` |
| 10 | Persistencia del libro mayor y métricas finales | `:2557`, `:2567` | `:2827` |

Con respecto a las preguntas concretas del propietario:

1. **¿Metadatos antes o después del audio?** La resolución (paso 2) va **antes**, porque sin ella no se
   puede ni pedir el recurso correcto ni saber qué calidad/edición se está bajando. La portada, las
   letras y el enriquecimiento van **después** de tener el audio en disco, etapa por etapa y en serie.
2. **¿Vienen incrustados o se incrustan después?** El audio recibido **no se presupone etiquetado**: se
   valida el payload, se extraen métricas físicas reales (`:1616` en Qobuz) y luego se escriben y
   verifican las etiquetas propias (paso 8). Es un requisito de producto: el fichero final debe ser
   portable y coherente con la edición y calidad elegidas, no un recurso de reproducción interna.
3. **¿Qué es síncrono y qué asíncrono?** Dos planos distintos que conviene no confundir:
   - **Entre pistas:** el worker ejecuta varias a la vez (por defecto 2, techo configurable), reclamando
     cada fila con `UPDATE ... WHERE status='queued'` (`src-tauri/src/worker.rs:499-537`) y lanzando una
     tarea por pista (`:2243-2260`).
   - **Dentro de una pista:** las fases se encadenan esperando la anterior. Que el código use `async`
     y `await` significa solo que **no bloquea el hilo** mientras espera red o disco; no implica que
     portada, letras, enriquecimiento y audio se estén bajando a la vez. Tampoco se lanzan las
     descargas «en segundo plano» mientras la del usuario se queda esperando.
4. **Costes concretos observados:** el bucle de segmentos DASH de Tidal los pide **uno a uno**
   (`crates/syncify-tidal-downloader/src/lib.rs:1892`, con límite `MAX_DASH_SEGMENTS` y reintentos por
   segmento), y la validación vuelve a **leer el temporal completo** para comprobar el payload
   (`src-tauri/src/services/tidal_pipeline.rs:1433` y siguientes). Esos son gastos distintos de la
   descarga en sí y hay que medirlos por separado.

**Lectura del flujo:** la identidad y la calidad solo pueden ir antes (son condiciones para pedir el
audio). Pero pasos 5, 6 y 7 **no dependen del audio** en muchos casos: las URLs de portada y letras se
conocen por metadatos de pista y álbum. Ahí está el solapamiento disponible sin cambiar lo que se
descarga ni de dónde.

---

## 3. Objetivo: medir el orden, y solo entonces solapar lo que es seguro

Nada de lo siguiente se activa por suposición. La palanca principal deja de ser «más conexiones» y
pasa a ser **no gastar tiempo de más por pista**.

### DLX-01 — Cronología de etapas por pista (prerrequisito de todo lo demás)

Instrumentar, por pista, el instante de inicio y fin de cada etapa: resolución, transferencia,
validación, portada, letras, enriquecimiento, etiquetado, promoción y persistencia; más tiempo
descargando bytes de audio y tiempo total. Guardar la cronología sin credenciales, URLs firmadas ni
datos privados. Medir con servidor local y red de prueba, separando red, disco, CPU y espera de SQLite,
para **saber qué fracción del tiempo de una pista es audio y qué fracción son esperas evitables**.
Sin esa medición, cualquier cambio de orden es ciego.

*Prueba:* una pista reproducida con servidor local deja cronología válida; en la línea base
se explica el tiempo total con las etapas y esperas. Tras introducir solapamiento, el tiempo total
se compara con la **unión de intervalos**, no con su suma (que contaría dos veces el trabajo
paralelo). El informe compara dos corridas.

### DLX-02 — Iniciar portada y letras durante la transferencia (solapamiento acotado)

Solo si DLX-01 demuestra espera significativa: permitir que las etapas que **no dependen de los bytes
del audio** se inicien cuando sus entradas ya estén resueltas, sin esperar a que termine la descarga
del audio. Requisitos duros:

- la portada y las letras deben provenir de la **misma identidad de pista/álbum** que el audio en
  curso (nunca de un título parecido de otra edición); si hay ambigüedad ED pendiente, no se solapa y
  se sigue el orden actual;
- separar claramente «complementos terminados y válidos», «complementos fallidos» y «audio
  terminado»: un fallo de portada o letras **no debe** tumbar la descarga del audio, y el audio no se
  promueve hasta estar completo y validado;
- la cancelación del usuario debe abortar **todos** los trabajos abiertos de esa pista, y ninguna tarea
  suelta debe seguir escribiendo tras cancelar;
- límite global de peticiones de complementos en vuelo, respetando el limitador por servicio, para no
  robar ancho de banda al audio;
- si una etapa enmascarada era error lógico real, el nuevo orden debe conservar el mensaje y el estado
  de la cola equivalentes.

*Prueba:* con servidor local que retrase portada y letras a propósito, el tiempo por pista no debería
incluir esa espera en serie, el FLAC final es idéntico (bytes de audio, etiquetas, portada incrustada,
manifest y ruta), un fallo de portada no interrumpe el audio, y la cancelación a mitad aborta todo y no
deja fichero promovido.

### DLX-03 — Etiquetado y verificación sin trabajo repetido

Medir el coste real de las relecturas del temporal (validación tras descarga, verificación tras
etiquetado, lectura post-promoción para métricas) y decidir, con datos, qué lecturas se pueden
combinar o eliminar sin perder la garantía de que el fichero final es válido. **No se elimina ninguna
verificación que proteja calidad o integridad**; solo se evita leer dos veces lo mismo.

*Prueba:* el mismo lote mantiene las verificaciones actuales activas y las métricas físicas
coinciden; medir antes/después por etapa.

### DLX-04 — Tidal: segmentos DASH secuenciales y payload leído completo

Medir y decidir dos cosas por separado: el bucle de segmentos uno a uno
(`crates/syncify-tidal-downloader/src/lib.rs:1892`) y la lectura completa del temporal
(`src-tauri/src/services/tidal_pipeline.rs:1433`). El RFC 8216 permite rangos de bytes dentro de un
recurso mayor y no prohíbe paralelismo, pero **no prescribe ninguno**, así que paralelizar segmentos
requiere verificar antes el contrato del proveedor, el orden de concatenación y los límites reales, y
mantener la cancelación y los reintentos. Si la medición muestra que no aporta, se descarta por
decisión documentada.

*Prueba:* comparar p50/p95 de transferencia en serie frente a la variante, con concatenación correcta
y sin degradar la validez del FLAC; si se paraleliza, probar cancelación a mitad y agotamiento.

### DLX-05 — Recurso repetido por pista: deduplicar lo que realmente se repite

Conservar la caché acotada por URL (con *single-flight*) para la portada, evitando que cada pista de un
álbum pida la misma imagen, con clave que incluya URL y contexto de acceso cuando aplique, y sin
compartir staging mutable ni mezclar ediciones distintas. No es la palanca principal, pero sí un ahorro
de red por pista que se acumula en lotes.

*Prueba:* dos y doce pistas concurrentes con la misma portada producen una sola petición válida por
clave; llaves, accesos o ediciones distintas no se fusionan; fallos, cancelación y expiración no rompen
el resto.

### DLX-06 — Concurrencia y carga: palanca secundaria y condicionada

El techo sigue siendo el ajuste del usuario (`dl_concurrent_downloads`). Cualquier control adaptativo
solo se evalúa **después** de DLX-01…05, y con el criterio de que bajar ante 429/5xx persistentes y
subir tras ventanas de éxito nunca debe confundir autorización, identidad o DRM con congestión. Ninguna
descarga por rangos paralelos ni subir a 10 por defecto.

### DFL-01 — Puerta transversal

Nada de esta cola altera el estado de ED-01…04 ni DL-SVC-01…03. Orden: (1) DLX-01; (2) DLX-02
condicionado; (3) DLX-03/04 medidos; (4) DLX-05; (5) DLX-06 solo si sigue habiendo cuello; (6) prueba
integrada. Cada ID se marca hecho solo con pruebas offline y medición reproducible, o se descarta con
evidencia y decisión del propietario. Descargas y cuentas reales, GUI y Windows siguen pendientes hasta
la puerta de beta: un build verde no es rendimiento demostrado.

---

## 4. Integración con las listas existentes

- `docs/DECISION_ALCANCE_BETA.md` (punto 7): requisito de cierre; no amplía proveedores ni rangos.
- `docs/auditoria/matriz-ajustes.md`: filas R1 DLX-01…06 y DFL-01 **antes** de código; el workflow
  activo coordina un único escritor.
- `docs/Errores-Pruebas-2026-10-13.md`: DL-SVC-01…03 siguen como defectos separados; esta cola no los
  marca resueltos.

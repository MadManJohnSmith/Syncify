# Errores y peticiones detectados en pruebas manuales

Registro en crudo de lo que se observó usando la aplicación. **No está planificado ni
priorizado**: es la lista tal cual salió de las pruebas, agrupada solo por pantalla para
poder localizarla. Cuando se planifique, el orden de este archivo no implica orden de
ejecución.

Origen: pruebas manuales del usuario (2026-10-08), sobre todo en Windows.

---

## Global / arranque

- **G-01 — Se abre una terminal visible al descargar.** Al poner una descarga, en la fase
  de incrustar metadatos aparece una ventana de consola que no debería verse. Abrirla en
  modo escondido.
- **G-02 — Tema por defecto.** Cambiar el tema por defecto de la aplicación al de tinta.
- **G-03 — La navbar superior no ocupa todo el ancho de la aplicación.** La barra de progreso
  global, que va en esa navbar, tampoco llega al ancho completo porque el menú lateral
  (Dashboard, Biblioteca, Playlists, Downloads) la ocupa en parte. El menú lateral debe
  aparecer **justo debajo** de la navbar, no al lado.
- **G-04 — Eliminar la píldora de tarea activa** que está justo encima de la barra de
  progreso (recordatorio de BP-01; lo vuelve a mencionar aquí).
- **G-05 — Eliminar por completo el botón "Help & Support".** Está junto al de
  notificaciones y solo lleva a un sitio con información de relleno (artículos, tutoriales,
  preguntas frecuentes, contacto). No debe existir.
- **G-06 — Ninguna ventana de terminal debe verse, nunca.** En Windows aparecen ventanas de
  terminal por encima de la app, hasta arriba de la pantalla, y bloquean la interacción con
  la aplicación. Deben ejecutarse siempre ocultas. Es la regla general detrás de G-01.

---

## Dashboard

- **D-01 — Eliminar "System Diagnostics".** Solo muestra si ffmpeg / chromaprint están
  instalados; ya no aplica porque se instalan con el instalador. Es información inútil.
- **D-02 — El widget "Library Growth" no muestra nada.** No funciona.
- **D-03 — "Storage Use" es demasiado grande** para la información que muestra.
- **D-04 — "Lyrics Coverage" es demasiado grande** para lo que es (solo una barra de
  porcentaje).
- **D-05 — "Sources" mal formateado.** Spotify muestra un texto corrupto ("que buscate el
  deserto") y Apple Music trae un guión bajo pegado. Debe quedar bien formateado.
- **D-06 — Barrido de todo el dashboard** para asegurar que no queden más errores de
  formato como los de Sources.
- **D-07 — Widgets con el contenido cortado en pantallas pequeñas.** Ejemplo: el widget
  "Library overview" muestra la información de playlists, downloader y active downloads
  cortada y montada encima de la línea divisoria de "23.220 streaming only / 1% of the
  library on disk". Mismo tipo de fallo que L-11 y L-13: hay que revisar el desbordamiento
  de los widgets en pantallas estrechas.

---

## Biblioteca

- **L-01 — Reproducir desde el menú contextual falla.** Clic derecho → *Play now* →
  error "no se puede reproducir". Debería hacer streaming de la pista usando las fuentes
  donde esté disponible.
- **L-02 — "Añadir a las playlists" no hace nada.**
- **L-03 — "Añadir a favoritos" no es explícito.** No queda claro si es favorito local, de
  una fuente concreta o de todas. Debe indicarse.
- **L-04 — Eliminar el botón "Check availability".** No hace nada; quitarlo de todos lados.
- **L-05 — "Show folder" en pistas no descargadas.** Avisa que no puede mostrarla. El botón
  debería desaparecer o quedar deshabilitado cuando la pista no está descargada.
- **L-06 — La columna "Special" no muestra nada** y no se entiende qué hace. Había un plan
  antiguo sobre si la pista solo está disponible en un servicio; hay que decidir qué
  información debe mostrar realmente.
- **L-07 — Filtros de biblioteca.** "High res / lossless" y "Multisource" no están
  disponibles. Revisar la implementación de todos los filtros y ver cuáles funcionan y
  cuáles no.
- **L-08 — Acciones destructivas sin confirmación.** Por ejemplo, "Remove from library" en
  el clic derecho pregunta directamente. Añadir advertencia a **todas** las acciones
  destructivas.
- **L-09 — Tamaño de portadas en la vista Grid.** Según la pantalla las portadas quedan
  muy grandes o muy pequeñas. Permitir al usuario cambiar el tamaño.
- **L-10 — El botón de agrupar** debería estar disponible solo en la vista Grid.
- **L-11 — El menú emergente de selección de pistas tapa contenido en pantallas pequeñas.**
  Al seleccionar pistas aparece un menú con opciones como "Download" y "Check availability"
  (esta última ya va a eliminarse, ver L-04) que reduce cuántas pistas se ven. Hay que
  encontrar otra colocación para esos botones.
- **L-12 — Aprovechar el espacio libre encima del título "Library".** Hay al menos la mitad
  de esa franja sin usar. Replicar en Biblioteca lo ya hecho en Descargas (ver DSC-01):
  estadísticas de pistas activas, en cola, completadas, falladas y seleccionadas, número de
  hilos y opciones de detalle de cola, junto al título de la pestaña.
- **L-13 — El menú contextual (clic derecho) se corta y oculta las dos últimas opciones.**
  Incluso en pantallas grandes, al hacer clic en la pista más abajo visible, "Show metadata"
  y "Remove from library" quedan ocultos y no se ven ni se pueden pulsar. En pantallas
  pequeñas es peor.

Lo que sí funciona en Biblioteca: el scroll infinito y la información de la tabla, la
vista de tabla/lista, y el botón "Show metadata" (redirige a la pestaña de metadatos; el
problema con él es que queda cortado en el menú contextual, ver L-13).

---

## Playlists

- **P-01 — El botón "Sync all" no hace nada** (o al menos no se sabe qué hace).
- **P-02 — Importar playlist por URL:** no se sabe si funciona.
- **P-03 — El botón "Smart":** no se sabe qué hace.
- **P-04 — Las playlists no muestran su portada.** La portada de la propia playlist, no la
  de las pistas (las pistas dentro sí se ven bien).
- **P-05 — No se pueden reordenar las pistas dentro de una playlist.** Debe poder hacerse,
  y propagarse a los servicios donde esa playlist esté disponible.
- **P-06 — Reproducir desde Playlists falla** si la pista no está descargada.

---

## Descargas

- **DSC-01 — Reorganizar toda la pantalla.** Según el tamaño de la ventana puede no verse
  nada de la información de descargas y solo los detalles de la cola.
- **DSC-02 — "Queue details"**: la información que muestra gusta, hay que conservarla.
- **DSC-03 — Verificar que los datos de "Queue details" son ciertos.** Tras tantos cambios
  en descargas y sincronización, la información puede estar desactualizada.
- **DSC-04 — Panel de pistas completadas con filtros**: gusta, pero debe poder buscarse.
- **DSC-05 — La barra de progreso de la cola no se muestra bien.**
- **DSC-06 — El menú flotante duplica información** y satura. Ordenar: quitar lo duplicado
  y lo que no aporta.
- **DSC-07 — Hay pistas que deberían descargarse y no se descargan.** El usuario tenía
  sincronizada solo su biblioteca de Qobuz, así que todas las pistas deberían estar
  disponibles ahí. Algunas fallaron sin causa visible: no se sabe si fue por formato, por el
  nombre, por el ISRC o por otra cosa. Si la pista se importó del mismo servicio al que se
  le pide la descarga, debería poder descargarse siempre.
- **DSC-08 — Cuando una pista no se puede descargar, hay que mostrar el error real y
  concreto.** Ahora el fallo aparece sin explicación, lo que hace que la app parezca no
  funcionar y le da mala reputación al programa. Este es un requisito, no solo un fallo de
  interfaz: ver también L-06 (columna de disponibilidad) y FS-05 (política de fallback).
- **DSC-09 — Fallback entre cuentas del mismo servicio.** Al usar varias cuentas, puede que
  solo una de ellas permita descargar y las demás no. Hay que añadir un fallback a nivel de
  cuenta: si una cuenta del servicio no puede descargar la pista, probar con otra cuenta del
  mismo servicio. Es el caso complementario de FS-05 (ahí el recorrido es entre servicios;
  aquí es entre cuentas del mismo servicio), y ambos deben funcionar.

---

## Migración / transferencia

- **M-01 — Eliminar "Templates".** No se sabe para qué sirve.
- **M-02 — La transferencia tarda muchísimo** con servicios que tienen muchas canciones y
  muchas playlists.
- **M-03 — No hay indicador de avance ni de éxito** durante la transferencia: no se sabe si
  va funcionando ni cuánto falta.
- **M-04 — El progreso no se guarda al cambiar de pantalla o de pestaña.** Al empezar una
  migración y cambiar de vista, al volver todo el progreso ha desaparecido. Falta guardar
  el estado.
- **M-05 — Opciones deshabilitadas de álbumes y artistas "no soportados".** Deben estar
  soportadas. Todo lo que la aplicación muestre tiene que funcionar.
- **M-06 — "Auditoría" (Audit).** No se entiende qué hace ni qué significa; al pulsarla solo
  dice que el esquema está ok y que no hay credenciales detectadas. Explicar/documentar.
- **M-07 — "History library".** No queda claro qué significa al importar el historial;
  hay que ser más explícito.

---

## Cuentas

- **C-01 — La vista no muestra lo importante al abrirla.** Solo aparece al hacer scroll, y
  lo primero que se ve son las credenciales de Spotify.
- **C-02 — Las credenciales de Spotify deben ir en un modal**, ocultas salvo que se busquen
  deliberadamente. Al abrir la pestaña debe verse: las cuentas del usuario, sincronizar
  entre ellas, añadir cuentas, etc.
- **C-03 — "Importar desde URL":** no se sabe si funciona; si no, arreglarlo.
- **C-04 — La lista de servicios soportados está desactualizada.** Actualmente solo tiene
  emojis.
- **C-05 — "Importar desde archivo":** no se explica cómo funciona ni qué formatos admite.
  Debe ser explícito y decir cuál es el estándar que se sigue.
- **C-06 — El login de Tidal está roto.** Antes abría el navegador; ahora no lo abre en
  ningún sistema operativo e impide iniciar sesión.
- **C-07 — Cualquier cambio en la sincronización de cuentas activa la barra de progreso
  global aunque no se haya modificado nada de esas funciones.**
- **C-08 — Valores por defecto de la sincronización.** Activos: favoritos, álbumes,
  artistas, playlists. Desactivados: compras, historias, apariciones.
- **C-09 — No se puede añadir una cuenta desde la pestaña del servicio** en Ajustes →
  Cuentas. Solo se puede desde la tarjeta. Debería poder hacerse en ambos sitios.
- **C-10 — La tarjeta de Apple Music avisa de que hace falta la "biblioteca de iCloud
  activada".** No se explica qué significa ni dónde se hace: no aparece en ningún sitio.

---

## Ajustes → Cuentas → Qobuz

- **Q-01 — No se puede añadir una cuenta desde dentro de la pestaña del servicio** (solo
  desde la tarjeta exterior). Ver C-09.

---

## Ajustes → Metadatos

- **MD-01 — Botones sin explicación:** "Analizar calidad", "Merge con estadísticas",
  "Review title"/"Prepared dry run", "Applied / reverse history". Hay que explicar qué hace
  cada uno.
- **MD-02 — "Merge con estadísticas" está bien, pero solo si las estadísticas son reales y
  están al día** con todos los cambios.
- **MD-03 — Comprobar que "Applied / reverse history" funciona** y que muestra los datos
  actualizados.

---

## Barras de progreso (global)

- **BP-01 — Eliminar la píldora sobre la barra de progreso.** Está siempre girando con
  "enriqueciendo datos con MusicBrainz" aunque no haya nada real que mostrar. Añadirla a la
  lista negra y quitar la píldora.
- **BP-02 — La barra de progreso global solo debe usarse para procesos en segundo plano**
  (sincronización de música, procesamiento de metadatos en segundo plano, etc.), y la
  información debe aparecer solo en "Detalles de sincronización", abajo a la izquierda.
- **BP-03 — Eliminar la tercera barra de progreso**, la de la parte inferior central.
- **BP-04 — Renombrar "Sync Details".** Si va a mostrar solo procesos en segundo plano, el
  nombre actual engaña; debe algo tipo "Procesos en segundo plano".

---

## Rendimiento (Linux)

- **PR-01 — El scroll está mal calibrado.** Al bajar con el ratón empieza muy suavizado y
  luego acelera demasiado y salta demasiado lejos. Hay que corregir la curva de aceleración
  y frenado.
- **PR-02 — Muchos tirones y saltos (lag).** El movimiento no es fluido.
- **PR-03 — El cambio de pestaña es muy lento.** Tardan mucho en cargar al pasar de una a
  otra.
- **PR-04 — Auditar por qué la app va tan mal en Linux.** La biblioteca del usuario tiene
  alrededor de **23.000 pistas** con sus metadatos, todos los servicios y todo lo demás; no
  se sabe qué parte es la que degrada tanto el rendimiento. Hay que trabajar mucho en el
  rendimiento en Linux.

## Notificaciones

- **NT-01 — El menú de notificaciones no hace nada al hacer clic en una notificación.**
  Cada notificación debería ejecutar alguna acción.
- **NT-02 — Demasiadas notificaciones al descargar.** Poner 100 canciones en Windows generó
  un bombardment de notificaciones: "empieza a descargar", "N canciones en la cola", "una
  canción no está disponible en X servicio", etc.
- **NT-03 — Criterio de notificaciones:**
  - solo las **importantes** deben salir como pop-up,
  - las que no son importantes deben ir al botón de notificaciones o a los logs,
  - las operaciones internas de descarga/sincronización no deben notificar al usuario final.
- **NT-04 — Hacer un análisis de todas las notificaciones actuales:** cuáles se emiten, cuáles
  deberían emitirse y cuáles no, y en qué sitio deben mostrarse. Revisar por servicio.

---

## Barra inferior / reproductor

- **PL-01 — El menú lateral izquierdo (Dashboard, Biblioteca, Playlists, …) debe poder
  esconderse** detrás de un menú de hamburguesa.
- **PL-02 — El botón de Ajustes queda tapado por la reproducción** cuando algo está sonando.
  Reubicarlo en el menú de abajo (el de Online, Sync Details, Storage Details, Network
  status). Es la opción preferida por el usuario, por encima de dejarlo en la navbar.
- **PL-03 — El botón de Ajustes debe cambiar de tamaño** según haya reproducción en curso o
  no.
- **PL-04 — No hay vista grande de "reproduciendo ahora".** No se pueden ver las letras que
  están sonando, ni la portada animada, ni la portada en alta definición, ni el resto de
  metadatos que la app extrae. Toda esa información no se puede apreciar dentro de la app.
- **PL-05 — Los booklets** que se descargan en algunos álbumes tampoco se muestran.
- **PL-06 — Eliminar el botón de colapsar** la barra inferior (el de la derecha). El usuario
  no quiere poder colapsarla.
- **PL-07 — Eliminar el FAB (botón flotante).** Funciona muy mal.
- **PL-08 — Reubicar el reproductor / "reproduciendo ahora" en la barra de estado inferior,
  en el centro**, en lugar de donde está ahora.
- **PL-09 — Reproductor grande o de pantalla completa** al hacer clic en la portada de la
  pista, con una señal en la propia portada cuando se le hace hover, indicando que se puede
  expandir.
- **PL-10 — Acceso al reproductor de pantalla completa también desde el menú lateral**
  (el que va escondido tras el menú de hamburguesa, ver PL-01).

---

## Funcionalidad ausente: biblioteca local

- **F-01 — La app empezó siendo un CLI cuyo objetivo era escanear la biblioteca local**
  (archivos del disco) y comprobar que estaban actualizados frente a los servicios de
  streaming. Esa función desapareció y hay que recuperarla.
- **F-02 — Tratar la biblioteca local como un servicio más:** escanear duplicados, hacer
  todo lo que hacen las sincronizaciones, añadirla a la base de datos, y permitir
  reproducir (streaming) desde los archivos locales, ver y actualizar sus metadatos, etc.
  Todo lo que la app sabe hacer con una fuente online debe poder hacerse con la local.

---

## Pantalla de Metadatos

- **MET-01 — La pantalla de Metadatos se refresca entera de repente.** Ocurre de forma
  intermitente y no debería pasar: no debe repintarse la pantalla completa.
- **MET-02 — La píldora "enriqueciendo datos con MusicBrainz" también aparece dentro de la
  pestaña de Metadatos.** Este tipo de tareas solo deben mostrarse abajo a la izquierda, en
  "Sync Details" (ver BP-01/BP-02).
- **MET-03 — El botón de portada animada debería estar aquí.** En Lyrics no funciona y no
  debería estar; en Metadatos sí debe poder verse la portada animada.

---

## Pantalla de Letras (Lyrics)

- **LY-01 — Los tiempos de sincronización se muestran siempre.** Hay letras sincronizadas
  por sílabas que aun así muestran los detalles de los segundos. No deben verse salvo en
  modo edición; en modo escucha no deberían ser visibles.
- **LY-02 — Créditos al principio de la letra.** Muchas letras empiezan con el nombre de la
  canción y del artista, que no es parte de la letra. Hay que eliminarlos automáticamente
  del archivo de letras.
- **LY-03 — El re-chequeo de letras en modo karaoke tiene que poder ir en segundo plano.**
  Ahora la única forma de seguir usando la app es cancelar el proceso, y se termina.
- **LY-04 — El botón de portada animada está en Lyrics y no funciona.** Quitarlo de aquí
  (ver MET-03).
- **LY-05 — Los botones de sidecar no funcionan:** "Sidecar", "portada sidecar" y
  `sidecar.lrc`.
- **LY-06 — La forma de edición no es intuitiva.** Muestra el texto plano. Investigar cuál
  es la mejor manera de editar letras e implementarla.
- **LY-07 — No se sabe si la búsqueda de letras usa todos los proveedores posibles** en esta
  pantalla.
- **LY-08 — El botón "Embed lyrics":** no se sabe si funciona.
- **LY-09 — "Embed lyrics" debería estar deshabilitado si la pista no está descargada.**
  Sin pista no hay nada donde incrustar la letra.
- **LY-10 — El botón "Export lyrics" parece correcto**, pero investigar la mejor forma.
- **LY-11 — No se sabe si "versiones guardadas" funciona.**
- **LY-12 — El botón "Delete" borra la letra y también la canción.** Al ser una acción
  destructiva, debe pedir confirmación.

---

## Ajustes → Letras

- **CFGL-01 — "Fuentes de letras y preferencias"** supposedly sirve para probar los
  servicios. No está claro que funcione.
- **CFGL-02 — El idioma preferido está en inglés** y por defecto debería ser el original.
- **CFGL-03 — Formatos de almacenamiento:** no se sabe si todos están realmente
  disponibles/seleccionables (por ejemplo, al cambiar a TTML, ¿se descarga como TTML?).
  Se había acordado usar solo LRC por compatibilidad, más texto plano incrustado en las
  pistas.
- **CFGL-04 — "Reintentar fallos":** no se sabe si funciona; si no, arreglarlo.
- **CFGL-05 — Las otras dos opciones extra no se sabe si están sincronizadas**
  ("buscar letra automáticamente al importar" y "reintentar pistas cuya búsqueda falló").
  En opinión del usuario no deberían estar ahí.

---

## Logs

- **LOG-01 — No se sabe si la pestaña funciona bien**, si los logs son reales o si hay mocks
  o datos falsos.
- **LOG-02 — No se sabe si los logs se están guardando** aunque no se esté dentro de esa
  pantalla.
- **LOG-03 — No se sabe si los filtros funcionan** o si están disponibles.
- **LOG-04 — Todos los módulos dentro de la pestaña parecen estar bien**; la pantalla ya se
  ve mucho mejor que antes.

---

## Ajustes (pestaña general)

Auditoría amplia pendiente: no se sabe qué partes de Ajustes funcionan, cuáles están
sincronizadas y cuáles siguen siendo legacy pero todavía se muestran. Hay mucho que revisar
dentro de esta pestaña.

- **AJ-01 — "Ubicación de la base de datos" está deshabilitado.** No deja hacer nada; no se
  sabe si funciona.
- **AJ-02 — "Ubicación de archivos temporales":** igual, no se sabe si funciona.
- **AJ-03 — Revisar que la base de datos se migra/inicializa correctamente.** Se han hecho
  muchas migraciones y muchos cambios; hay que comprobarlo y actualizar si no lo está.
- **AJ-04 — Tema por defecto "tinta"** (recordatorio de G-02).
- **AJ-05 — Nuevos temas con la paleta de cada servicio.** Si Apple Music, Spotify, Tidal,
  etc. están disponibles, poder seleccionar su paleta de colores como tema, con la
  interfaz teñida. Los temas van **debajo**, separados de los de apariencia normal, dentro
  del menú de apariencia.
- **AJ-06 — Tema dinámico por carátula.** Todos esos temas deben poder take sus colores de
  la portada de la pista que se está reproduciendo, y poder activarse/desactivarse.
- **AJ-07 — El botón "Añadir servicio" de Ajustes debe funcionar igual** que el de la
  pestaña de Cuentas, y estar sincronizado con el resto de la app (ver C-09).
- **AJ-08 — Nada de ajustes duplicados.** Cualquier ajuste que esté en Ajustes debe estar
  sincronizado y ser el mismo en cualquier otra parte de la app, para no tener dos
  controles con valores distintos y no saber cuál manda.
- **AJ-09 — El botón "Guardar cambios" de Ajustes: no se sabe si funciona.**
- **AJ-10 — El botón "Borrar a default" / restablecer: no se sabe si funciona.**
- **AJ-11 — Documentar cuáles deben ser los valores por defecto de todos los ajustes.**
  No se conoce la lista de defaults; hay que establecerla y comprobarla.

---

## Ajustes → Servicios y prioridades

- **SP-01 — Arrastrar para reordenar la prioridad de descarga no funciona en Windows.** En
  Linux sí deja arrastrar; en Windows parece agarrar el elemento pero no lo mueve. Hay que
  arreglarlo.
- **SP-02 — Autoimport debe estar activado por defecto en todos los servicios.**
- **SP-03 — Mostrar el nombre de usuario o el correo de la cuenta** en vez de ese string
  opaco (no se entiende qué significa).
- **SP-04 — "Desconectar" / "Eliminar cuenta" son acciones destructivas** y deben pedir
  confirmación.
- **SP-05 — El "Autoimport" de la tabla de prioridades no está sincronizado.** Refleja
  autoimport activo en Apple Music pero al elegir Apple no deja descargar; en Qobuz sí
  cambia, en "Musix"/Qobuz no aparece. Hay que comprobar si el resto de ajustes se
  actualizan igual que este (ver AJ-08).

---

## Ajustes → Audio Quality

- **AQ-01 — "Techo efectivo de descarga" arriba del todo, con un texto que dice dónde
  encontrarlo.** No debería pasar: si un ajuste está ahí, no debe decir "está en el otro
  lado". El ajuste debe simplemente estar en su sitio.
- **AQ-02 — Reestructurar la pantalla de ajustes si hace falta**, siempre justificado y
  para mejorar la app. Ni tantos ajustes que no sirvan ni tan pocos que no se pueda usar la
  aplicación bien.
- **AQ-03 — El techo global de calidad debe estar junto al resto de techos por servicio.**
- **AQ-04 — Añadir por servicio una opción para tomar el techo global** o fijar una calidad
  de descarga específica para ese servicio.
- **AQ-05 — Revisar en toda la aplicación que lo no disponible no sea seleccionable.**
  Servicios como Spotify quizá no permitan descargar en alta resolución; si no lo permiten,
  esa opción no debe aparecer en el selector.
- **AQ-06 — Calidad global de descarga sin "subir" la calidad artificialmente.** Si se pide
  solo FLAC y el servicio entrega MP3 o una calidad inferior, no debe re-codificar ni subir
  la calidad: solo sube el tamaño del archivo sin mejorar nada.

---

## Ajustes → Metadata y Tags

- **MT-01 — No se sabe cuál de estos ajustes funciona y cuál no**, ni si realmente se aplican:
  si se están buscando los tags y escribiéndolos al descargar una pista.
- **MT-02 — La pantalla se queda en negro y deja de mostrar los ajustes.** Hay que salir de
  la pestaña y volver a entrar para recuperar el texto. Reproducido dos veces. Ocurre cada
  ~20 segundos aproximadamente (dato para auditarlo).
- **MT-03 — Con el tema de tinta, los toggles activos no se leen bien.** Hay que ajustar el
  color para que se distingan activados; ahora solo se ven bien cuando están desactivados.

---

## Transversal: matriz de ajustes

- **TR-01 — Matriz de todos los ajustes de la aplicación.** Hace falta un inventario de
  dónde vive cada ajuste en toda la app (no solo en la pestaña de Ajustes), con:
  - en qué pantalla(s) aparece,
  - si está conectado / funciona,
  - si es el sitio correcto para él,
  - si está duplicado y si esa duplicación está justificada por comodidad.
- **TR-02 — Peticiones sueltas de esta auditoría, ya detectadas:** "Techo efectivo de
  descarga" en Audio Quality (AQ-01), autoimport en la tabla de prioridades (SP-05), tema
  por defecto (G-02/AJ-04), "Añadir servicio"/"Añadir cuenta" (AJ-07, C-09, Q-01), y las
  acciones destructivas sin aviso dispersas por toda la app (L-08, C-04/SP-04, LY-12).
- **TR-03 — Requisito de fondo:** ningún ajuste duplicado. Si un ajuste está disponible en
  más de un sitio, todos deben leer y escribir el mismo estado, de modo que no haya dos
  controles con valores distintos ni que haya que adivinar cuál manda.

---

## Ajustes → Scoring de calidad de metadatos

- **SC-01 — Los umbrales se mueven solos al actualizar.** Se pueden modificar, pero cuando
  algo se actualiza todos los valores se desplazan y se pierden los cambios.
- **SC-02 — No se sabe si estos ajustes se usan realmente** al calcular la calificación de
  metadatos. Hay que comprobarlo y, si no se usan, implementarlos.
- **SC-03 — Debe aplicar también a la biblioteca local.** El scoring de la biblioteca
  existente (la local) es el caso que menos se conoce; hay que añadirlo.
- **SC-04 — Botón de interrogación con tooltip en los ajustes.** Al hacer hover sobre
  cualquier ajuste (p. ej. un botón "?"), mostrar qué hace exactamente, para que no haya
  sorpresas al ejecutarlo.

---

## Análisis de BPM y procesos queued

- **BPM-01 — No se sabe si el análisis de BPM está al día** con los cambios actuales.
- **BPM-02 — Añadir un ajuste para activarlo durante la descarga** (analizar BPM mientras
  se descarga).
- **BPM-03 — Los procesos queued (BPM, descarga de metadatos) deben poder ejecutarse en
  segundo plano.** Hoy son síncronos y bloquean la descarga; hay que pasarlos a asíncronos.

---

## Ajustes → Letras (motor de descarga)

- **LYM-01 — La pestaña de Ajustes → Letras es engañosa.** El motor de descarga de letras
  es mucho más complejo de lo que se muestra ahí.
- **LYM-02 — Mostrar cómo funciona realmente el motor:** prioridades de descarga, fallbacks,
  orden de proveedores, etc., y permitir modificarlo a gusto de cada usuario. Que no solo
  se vea, sino que sea editable.

---

## Ajustes → Duplicados

- **DUP-01 — No se sabe si la detección de duplicados funciona.** Está activada por defecto,
  pero el usuario tiene muchísimos duplicados en su biblioteca y no está claro que se esté
  detectando.
- **DUP-02 — Resumen explicativo de cómo funciona cada función** en los ajustes: un texto
  pequeño junto a la opción (y un botón "?" al hacer hover, ver SC-04) que explique qué
  hace, para que no haya sorpresas.

---

## Ajustes → Audio Processing

- **AP-01 — Está desactivado por defecto y no se explica cómo ni cuándo funciona** ni
  cuándo debería activarse.
- **AP-02 — La incrustación de letras en los archivos de audio** está activada por defecto,
  pero no se sabe si funciona ni si al desactivarla se desactiva también al descargar.
- **AP-03 — No se sabe si funcionan "máximo de tamaño del artwork" ni "ReplayGain".**
- **AP-04 — Añadir en cada ajuste individual una indicación de qué hace** (no solo en la
  categoría), para que el usuario entienda cada opción.

---

## Ajustes → Folder Structure

- **FS-01 — No se sabe si alguno de estos ajustes funciona.** Hay que revisar todos los
  ajustes de la pestaña.
- **FS-02 — El "techo de descargas" aparece aquí dentro de Folder Structure.** No debería
  estar anidado aquí (ver AQ-03).
- **FS-03 — Hay menús desplegables (dropdowns):** hay que checar si realmente están
  mostrando lo que deberían.
- **FS-04 — Verificar que el techo de descarga funciona**, no solo el individual por servicio
  sino también el global, porque se hicieron cambios en descargas.
- **FS-05 — La "service and quality fallback policy" no está bien hecha.** Comportamiento
  deseado: si el servicio por defecto (p. ej. Qobuz) no tiene la pista en la calidad usada
  (p. ej. CD/FLAC), pero sí la tiene en MP3 320, probar el siguiente servicio y el siguiente
  hasta encontrar la calidad buscada; solo si no se encuentra en ninguno, descargar en 320
  pero nunca por debajo de eso. Hoy el fallback va directo a la calidad inferior en vez de
  recorrer servicios.
- **FS-06 — Hay otro duplicado de la "calidad máxima global" más abajo** en esta misma
  pantalla. Eliminar el duplicado.
- **FS-07 — El "nivel de concurrencia de descargas" está aquí.** No se sabe si funciona ni si
  es el sitio correcto.
- **FS-08 — Los presets de folder y nomenclatura de escritura no están claros:** hay que
  revisar que todos sean correctos y que ninguno esté duplicado.

---

## Ajustes → Estructura de carpetas / Symphonium

- **SYM-01 — El modo "Estándar" es el que se muestra por defecto, pero el usuario quiere que
  se elimine y que el default sea "Optimizado para Symphonium".**
- **SYM-02 — Comprobar que el modo Optimizado para Symphonium carga de verdad** y funciona.
- **SYM-03 — Investigar cuál es la mejor forma de organizar descargas, artistas, pistas y
  sidecars** para que Symphonium lo lea bien. Otras aplicaciones ofrecen más opciones de
  reglas/nombrado; hoy las opciones de nombrado son muy pocas.

---

## Ajustes → Sync and Scheduling

- **SS-01 — No se sabe si está sincronizado a través de toda la aplicación.** Otro caso de la
  matriz de ajustes (TR-01).

---

## Ajustes → Backup and Restore

- **BR-01 — No se sabe si la descarga del export está actualizada** (el archivo que genera la
  descarga/export).
- **BR-02 — No se sabe si "Restore" funciona.** Con tantas migraciones, hay que comprobar
  que el restore es válido y que el respaldo (backup) es bueno.
- **BR-03 — No se sabe qué hace "Physical Library Integration and Reconciliation"** ni si
  funciona.

---

## Nota de criterio sobre Ajustes

Se da libertad para modificar Ajustes: cambiar su organización, las pestañas internas,
añadir o quitar opciones. Pero **solo si mejora la aplicación y su facilidad de uso**, no por
cambiar por cambiar: ni dejar ajustes que no sirvan ni quitar los que hagan falta.
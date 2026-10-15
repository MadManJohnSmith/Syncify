# Plan de mejora post-beta — registro de funcionalidades diferidas

**Propósito:** que ninguna funcionalidad estudiada y decidida como diferida se pierda.
La decisión de alcance vive en `docs/DECISION_ALCANCE_BETA.md`; los ítems que esa
decisión excluye del beta están enumerados, con evidencia, en este fichero.

**Cómo se usa este fichero.** Es una lista de espera, no una cola de trabajo: **ninguno
de estos ítems se empieza mientras el beta no esté cerrado** (ver `DECISION_ALCANCE_BETA.md`
§4 para la condición exacta). Cuando el beta se cierre, este fichero se consume por
prioridad, no por orden de aparición.

**Estado de cada ítem:** `registrado` = anotado, sin tocar. Un ítem pasa a `en curso`
solo cuando alguien abre trabajo real sobre él.

**Nota sobre las evidencias:** todas las citas de código se verificaron el 2026-10-12
contra el árbol de trabajo. Un ítem con evidencia "no usado" significa que el código o
los datos existen pero nadie los consume —esos son los más baratos de todo el fichero,
y están marcados con `◆`.

---

## Grupo A — Sincronización continua y biblioteca espejo

_A-01 · `registrado` · **Motor de delta real (borrados y pertenencia).** La
sincronización hoy es unidireccional de acumulación: cada brazo recorre los endpoints
del proveedor y hace upsert, pero **ninguna línea borra una fila local porque el
proveedor ya no la devuelve**. No hay tumbas, ni cursor, ni tabla de snapshot. Evidencia:
las únicas sentencias `DELETE FROM` del repo son borrados locales iniciados por el
usuario (`src/commands/playlists.rs:117`, `:128`, `:164`, `:213`, `:349`) y drops de
migración. Consecuencias concretas: una canción que se quita de favoritos en el proveedor
sigue marcada como favorita para siempre; un track que sale de una playlist remota se
queda en `playlist_tracks`. **Diseño obligatorio:** no borrar nunca una fila de catálogo
(dispara `ON DELETE CASCADE` y podría llevarse por delante audio ya descargado). El
patrón es diff de pertenencia → tabla de "ya no está en el proveedor" → sugerencia en la
UI, es decir cuarentena, no borrado.

_A-02 · `registrado` · **`snapshot_id` en huella de contenido.** La columna existe desde
`src-tauri/migrations/0048_playlist_sync.sql:11` y **nunca se escribe ni se lee** en
ningún sitio del árbol. El plan es escribir un hash ordenado de los IDs de track del
proveedor: si la huella no cambió, saltar la expansión track a track entera. Hoy cada
sincronización re-enriquece todos los tracks de todas las playlists aunque no haya
cambiado nada. Es el ítem con mejor relación coste/beneficio de todo este fichero: cambio
de schema cero, y reduce trabajo en vez de añadirlo. `◆`_

_A-03 · `registrado` · **`incremental_sync` — interruptor muerto.** El ajuste se
persiste, se expone en la UI de ajustes y se pasa al motor de importación
(`src-tauri/src/settings.rs:249-269`, `commands/types.rs:499`, `models.rs:54`)… y
**ningún código lo lee para decidir qué sincronizar**. La sincronización siempre es un
recorrido completo. Es un no-op de punta a punta: o se implementa (encima de A-02), o se
retira el ajuste para que deje de mentir en la UI. `◆`_

_A-04 · `registrado` · **`auto_download_favorites` — el motor sin disparador.** El
ajuste existe, tiene onboarding (`ui/src/OnboardingWizard.vue:850`) y un motor completo
de encolado (`src-tauri/src/commands/favorites.rs:2719`) con guardrails, clasificación de
fallos 404/stale/ambiguo y **preview seco ya implementado** (`favorites.rs:3077`). Lo que
no existe es el disparador: **cero lecturas en Rust**. La feature pedida está al 90%.
Encadenamiento recomendado: primero modo propuesta/notificación (la infraestructura de
preview ya está), después auto-encolado opt-in. `◆`_

_A-05 · `registrado` · **Auto-descarga por artista seguido.** No hay watcher de
nuevos lanzamientos. La condición natural es: sigue a X → sale álbum → se encola la
descarga o se notifica. Depende de A-04 (mismo motor de disparo) y de A-01 (para saber
qué artistas se siguen de verdad, no solo los guardados en la app).

_A-06 · `registrado` · **Auto-descarga por cambios en playlist.** Al igual que A-05 pero
por playlist: seguidas, automáticas, o de terceros que cambian solas. Nota de diseño:
hoy las playlists propias y las seguidas **son indistinguibles**; `owner_id` se guarda
pero nunca se compara con la identidad de la cuenta. La derivación ya es posible
(`is_owned = owner_id == external_user_id`), solo falta el campo calculado.

_A-07 · `registrado` · **Avisos de novedades / pantalla de descubrimiento.** No existe
ninguna pantalla de "explorar" ni avisos de lanzamientos. Como referencia de formato (no
de producto): SpotiFLAC resuelve esto con un feed de lanzamientos semanales
(`lib/providers/weekly_releases_provider.dart`) más pantalla de artistas favoritos
(`lib/screens/favorite_artists_screen.dart`), ambos con pull-to-refresh manual.

_A-08 · `registrado` · **Registro de capacidades por proveedor.** El botón "Sincronizar"
miente en los proveedores que no permiten escritura o carecen del concepto: Qobuz y
Deezer no permiten escribir favoritos; SoundCloud solo like/unlike; Apple Music no tiene
concepto de favoritos (la biblioteca *es* el favorito, documentado en
`src-tauri/src/services/apple_music.rs:1127`); Deezer no expone historial de escucha
(`src-tauri/src/service.rs:6892`). Hay que declarar capacidad por proveedor para que la
UI deje de prometer lo imposible. Ninguna API permite simetría completa, en ningún
proveedor: esto es límite del ecosistema, no deuda nuestra.

---

## Grupo B — Reproductor para ser app principal

_B-01 · `registrado` · **Atajos de teclado de reproducción.** El composable existe
(`ui/src/composables/useKeyboardShortcuts.ts`) pero **no contiene ni un binding de
reproducción**: ni play/pausa, ni siguiente, ni anterior, ni volumen. Es P0 de este
grupo y cuesta muy poco. `◆` (el andamio ya está; solo hay que colgar los handlers)._

_B-02 · `registrado` · **Verificar MPRIS en sesión gráfica real.** La integración existe
en código pero **nunca se ha comprobado en una sesión gráfica real con D-Bus**
(`docs/README.md:16` lo declara explícitamente sin verificar). Sin esa verificación, las
teclas multimedia del sistema probablemente no lleguen fuera de la ventana en primer
plano. Un usuario no puede dejar la app abierta todo el día si no responde a las teclas
del teclado. Es P0 junto a B-01 por ser verificable en una sesión, no por ser código. `◆`_

_B-03 · `registrado` · **ReplayGain aplicado en reproducción.** Se calcula ReplayGain
2.0 / EBU R128 completo y **se escribe en los tags**
(`src-tauri/src/download/audio_inspector.rs:10-23`, `:465-488`)… y **nadie lo lee al
reproducir**: cero coincidencias de `replaygain` en `commands/playback.rs` ni en
`usePlayer.ts`. Para una herramienta de audio Hi-Res es una lástima. Ruta barata:
Web Audio GainNode alrededor del `<audio>` que ya existe, usando el valor del tag. `◆`_

_B-04 · `registrado` · **Las dos rejillas de favoritos que existen pero no se ven.** El
backend está completo y funcional —`get_favorites_albums`
(`src-tauri/src/commands/favorites.rs:346`) y `get_favorites_artists`
(`favorites.rs:503`)— y las envolturas de API existen (`ui/src/api/library.ts:420`,
`:428`, exportadas en `:1362-1363`), pero **no hay un solo sitio de llamada en ninguna
vista ni componente**. Los datos existen, no se muestran. Coste muy bajo, y de paso
alimenta A-05 y A-07. `◆`_

_B-05 · `registrado` · **Búsqueda en proveedores (descubrimiento).** La búsqueda actual
es **solo local**: todo el filtrado es SQL con `LIKE` sobre SQLite
(`src-tauri/src/commands/search.rs:117-129`, `:180`, `:251`); no hay ninguna consulta a
API de proveedores. No se puede descubrir música que no se posee ya. Depende de decidir
primero si el descubrimiento entra en el reproductor (es decir, si se revoca el veredicto
S194) o se resuelve con el sheet de "buscar en otro servicio" (ver C-01).

_B-06 · `registrado` · **Características de motor de reproducción:** gapless, crossfade,
ecualizador y normalización en reproducción. Ninguno existe. B-03 es el prerrequisito
natural de la normalización.

---

## Grupo C — Más fuentes y ecosistema de plugins

_C-01 · `registrado` · **Sheet de "buscar este mismo ítem en otro servicio".** De
SpotiFLAC (`lib/services/cross_extension_share_service.dart`): en cualquier ítem, un
atajo para buscarlo en los demás proveedores. Es la pieza de descubrimiento más barata
del catálogo y conecta directamente con el fallback por ISRC de C-02. Además es
compatible con el veredicto S194: no abre el catálogo completo, solo resuelve el ítem
que ya tienes.

_C-02 · `registrado` · **Cascada de fallback por ISRC multi-fuente.** SpotiFLAC resuelve
un track por ISRC en Deezer cuando la fuente original falla. Syncify ya busca por ISRC
(`src-tauri/src/services/deezer.rs:975`) pero solo la usa como metadata, y su fallback
actual es una única dependencia externa Python (`src-tauri/src/download/amazon.rs:226`).
Esto multiplica el valor de cada descargador nuevo: cada fuente sumada se convierte en
respaldo de todas las demás.

_C-03 · `registrado` · **Seis descargadores nuevos** (en orden de valor estudiado):
Deezer (el más barato de todos: el cliente ARL completo ya existe,
`src-tauri/src/services/deezer.rs:167`; solo falta la resolución del stream), SoundCloud
(OAuth token, likes y playlists ya resueltos, `services/soundcloud.rs:312-370`; solo
falta el endpoint de stream), JioSaavn (API pública, sin auth, AAC), YouTube Music
(pool más grande para librerías incompletas, pero el más frágil por InnerTube), Amazon
(nativo, sustituyendo el relay Python actual; es el que más deuda técnica arrastra), y
Apple Music Audio (Widevine/ALAC sobre m3u8; requiere descifrado serio). **Nota de
contexto que evita redescubrirlo:** SpotiFLAC anuncia 12 fuentes pero **solo 7
descargan audio** —Spotify Web, Apple Music, Bugs, Melon y Genie son solo metadata y
búsqueda— y de esos 7, tres pasan por relés de terceros (Deezer y Qobuz por proxy;
Amazon por `zarz.moe`). El gap real es 3 descargadores frente a 7, no frente a 12.

_C-04 · `registrado` · **Sistema de extensiones/plugins estilo SpotiFLAC.** Runtime JS,
contenedor `.sflx`, tienda con verificación SHA-256. **Es arquitectura nueva, no un
hueco que rellenar**: es construir una plataforma. Registrado con sus tres piezas
reutilizables por si algún día se hace —el contenedor ZIP endurecido
(módulo externo de SpotiFLAC rust_backend/crates/extensions/src/package.rs, parseo del directorio central con
ZIP64, límite de 2048 entradas y rechazo de symlinks), el flujo de registro con SHA-256
verificado, y el modelo de capacidades por host en lugar de APIs globales— más la
advertencia de que **el SHA-256 protege integridad pero no intención**: un registro
malicioso puede servir un plugin válidamente firmado que robe tokens de usuario.

_C-05 · `registrado` · **Proxy de red.** El ajuste `use_proxy`/`proxy_url` existe en la
configuración pero **ningún código lo consume**: no está implementado, a diferencia del
proxy real que SpotiFLAC aplica en su política de red
(módulo externo de SpotiFLAC rust_backend/crates/network/src/policy.rs). Pendiente de decidir si es deuda que se
paga en beta (es ajuste visible que no hace nada) o va a este grupo.

_C-06 · `registrado` · **Transcodificación de formato de salida.** No existe. Todos los
ingredientes están (ffmpeg-kit-audio ya se enlaza en Android, y el pipeline de tagging
acepta FLAC/MP4), pero no hay ruta que convierta el formato descargado.

---

## Grupo D — Versión Android

_D-01 · `registrado` · **Port a Android.** Hito propio, posterior al beta, posterior
también a estabilizar el alcance de escritorio. Nota de contexto: la app ya fue
construida para Android desde el inicio en algunos aspectos —los tests de paridad de
tags con Symfonium (`tests/symfonium_tag_parity_test.rs`,
`tests/symfonium_visibility_contract_test.rs`) y el doble etiquetado BPM/TEMPO y
LABEL/RECORDLABEL "para compatibilidad total con reproductores Android"
(`docs/auditoria/procesos-y-carpetas-bitacora.md:118`) existen precisamente por esto.
Lo que no existe es ningún plan, ni checkout, ni configuración de build móvil.

_D-02 · `registrado` · **Almacenamiento y servicio en primer plano para móvil.** SAF /
scoped storage, notificación de descarga persistente, y comportamiento en segundo plano.
Es el trabajo real de D-01, no un extra: es lo que hace que un descargador funcione en
móvil.

---

## Grupo E — Verificación y deuda que condiciona todo lo anterior

_E-01 · `registrado` · **Verificación real que el README declara pendiente.** El README
excluye expresamente de su evidencia de release el login en vivo, el enriquecimiento, las
descargas, la resolución de letras y la comprobación real de GUI/audio/tray/MPRIS
(`README.es.md:246-263`). Nada de lo anterior debe darse por bueno sin estalista
reducida. Relacionado: el handoff advierte de no fiarse de los "verificado" de las
bitácoras de carril donde el carril no pudo ejecutar (`docs/HANDOFF-2026-10-11.md:295-297`).

_E-02 · `registrado` · **Windows: target de compilación y D-Bus/MPRIS en máquina real.**
Hoy bloqueado en esta máquina (`docs/HANDOFF-2026-10-11.md:6`). Es prerrequisito de B-02
y de cualquier promesa de release multiplataforma.

_E-03 · `registrado` · **Commitear el árbol de trabajo.** 21 entradas sin commitear
(16 modificadas, 5 nuevas), incluyendo el importador de dumps de MusicBrainz
(`src-tauri/src/services/mb_dump_import.rs`, 374 líneas) que está registrado en
`services/mod.rs:21` pero sin flujo web/UI conectado. El handoff lo marcaba como el paso
más urgente por riesgo de pérdida el 2026-10-11 y sigue siéndolo el 2026-10-12. **Nota:
esto está en el beta, no aquí; aparece en este grupo solo para que no se pierda la
referencia.**

_E-04 · `registrado` · **Vaciar la cola de triaje de pruebas manuales del 2026-10-11**
(`docs/Errores-Pruebas-2026-10-11.md`). Solo Q-01 está cerrado. El caso general de
overlays flotantes frente a la `.status-bar` no se ha comprobado visualmente. Al igual
que E-03, es alcance beta y se lista aquí únicamente como referencia viva.

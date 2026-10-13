# Decisión de alcance — qué entra antes de salir de beta y qué queda fuera

**Estado:** DECIDIDO · **Fecha:** 2026-10-12 · **Autoriza:** decisión verbal del propietario
en sesión de producto (2026-10-12), tras revisar el repositorio
`spoflacapp/SpotiFLAC-Mobile` y el registro de plugins de `SpotiFLAC-Extension`.

**Antes de este documento no existía ningún roadmap en el reposito.** Verificado por
búsqueda exhaustiva: `roadmap|hoja de ruta` → 0 coincidencias en todo el árbol y en el
directorio padre; `android|apk|mobile` → 0 planes, solo menciones incidentales
(compatibilidad de tags con Symfonium en Android, y una instrucción de un subagente para
no usar un APK durante una investigación). La única aparición de "beta" en el árbol era
una linea suelta en una automatización de ZCode
(`.zcode/workflows/resolutor-unico-descargas.dwf.ts:177`, sobre no hacer i18n hasta
después de la beta). Todo lo demás de esta conversación vivía solo en la sesión.

---

## 1. La decisión

**El objetivo inmediato es sacar a Syncify de la beta con lo que ya está en el árbol.
No se añade alcance nuevo — de ninguna clase — hasta que eso esté cerrado.**

En particular, **queda fuera del alcance beta**, explícitamente, aunque se haya
estudiado esta semana:

| Fuera de alcance (post-beta) | Motivo de la exclusión |
|---|---|
| Descargadores nuevos: Deezer, YouTube Music, SoundCloud, JioSaavn, Amazon-prime, Apple Music Audio, Bugs, Melon, Genie | Son el carril "más fuentes"; ya se decidió posponerlos. Ninguno entra antes del beta. |
| El sistema de extensiones/plugins estilo SpotiFLAC (runtime JS, formato `.sflx`, tienda con SHA-256) | Arquitectura entera nueva. No es arreglar huecos: es construir una plataforma. |
| Cualquier servicio de sincronización continua o delta (auto-descarga de nuevos favoritos, watchers de playlists, avisos de nuevos lanzamientos por artista seguido) | Es la visión de "reproductor centralizado". Post-beta. |
| Visión de reproductor dedicado: búsqueda en proveedores, descubrimiento, pantalla de explorar | Post-beta. |
| Versión Android (además de la de escritorio) | Post-beta, y declarada como hito propio, no como rama del beta. |
| i18n | Ya excluido previamente: la app es English-first hasta después de la beta. |

La razón operativa es la que da el propietario: **lo que hay que resolver primero es que
lo existente funcione perfecto**. El riesgo de sumar alcance ahora es doble, porque el
repositorio ya está en estado delicado: HEAD está en `docs/v0.6.0-released-status`
(un commit por delante y uno por detrás de `origin/main`), con **21 entradas sin
commitear** (16 modificados + 5 nuevas), incluyendo un importador de MusicBrainz de 374
líneas (`src-tauri/src/services/mb_dump_import.rs`) que está registrado en
`services/mod.rs:21` pero **sin flujo web/UI conectado**. El handoff
(`docs/HANDOFF-2026-10-11.md:303`) ya marcaba esto como el paso más urgente por riesgo
de pérdida, y sigue siendo verdad el 2026-10-12.

---

## 2. Lo que SÍ es alcance beta

Beta no significa "el repositorio se congela": significa cerrar lo que ya está empezado
y verificado solo a medias. Concreto:

1. **Comitear el árbol de trabajo por carriles.** Es la primera tarea, antes que cualquier
   otra. Dos carriles identificados: la revisión de entitlements de Qobuz
   (`services/qobuz.rs`, `commands/accounts.rs`, `commands/auth.rs`, `download/qobuz.rs`
   + 3 ficheros de test) y el transcriptor de dumps de MusicBrainz
   (`services/mb_dump_import.rs` + `migrations/0096_mb_dump_catalog.sql` + test).
   La segunda está **sin conectar**: decidir si se conecta ahora o se aparta.
2. **Vaciar la cola de triaje de pruebas manuales del 2026-10-11**
   (`docs/Errores-Pruebas-2026-10-11.md`). Solo Q-01 está cerrado (con test
   `LayoutSpacing.spec.ts`, 8/8). El resto (Q-02 en adelante) sigue pendiente. El caso
   general de overlays flotantes frente a la `.status-bar` no se ha comprobado
   visualmente.
3. **Las tres palancas que ya existen pero no hacen nada**, en la medida en que sea
   arreglar lo de dentro y no construir lo de fuera: `incremental_sync` se persiste y se
   lee de ajustes pero **ningún código lo usa para decidir qué sincronizar**
   (`commands/settings.rs:249-269`; cero lectores en el motor);
   `playlist_sources.snapshot_id` existe desde `migrations/0048_playlist_sync.sql:11` y
   **nunca se escribe ni se lee**; `auto_download_favorites` se guarda en ajustes, tiene
   onboarding (`OnboardingWizard.vue:850`) y tiene motor completo de encolado
   (`favorites.rs:2719`, con preview seco en `:3077`) pero **ningún disparador en Rust**.
   Estas tres se resuelven o se documentan como post-beta; no se dejan a medias. El motor
   de `auto_download_favorites` es el que más duele dejar muerto, porque ya está
   construido y probado.
4. **El reproductor mínimo defendible**, pero solo lo que ya está dentro del veredicto
   S194 que declaró el propio proyecto: streaming de proveedores y búsqueda externa
   quedan fuera (`commands/playback.rs:14`, `ui/src/composables/usePlayer.ts:8-9`). Lo
   que sí es scope: atajos de teclado de reproducción, verificar MPRIS en sesión gráfica
   real (nunca se ejecutó: `docs/README.md:16` lo declara sin verificar), y las dos
   rejillas de favoritos de álbumes y artistas cuyo backend está completo
   (`favorites.rs:186`, `:363`) pero sin un solo sitio de llamada en la UI.
5. **Cerrar la verificación que el README declara pendiente**: sin login en vivo, sin
   descargas reales, sin GUI/audio/tray/MPRIS comprobados (`README.es.md:246-263`). Un
   beta honesto necesita esa lista reducida, no ampliada.
6. **Tests y CI en verde en Windows**, que hoy está bloqueado en esta máquina
   (`HANDOFF-2026-10-11.md:6`).

---

## 3. Del estudio de SpotiFLAC: qué se guarda para el post-beta

Se revisó `SpotiFLAC-Mobile` (main @ `5a89526`) y el registro completo de
`SpotiFLAC-Extension` (12 plugins, commit `0290b26`, los 12 SHA-256 verificados contra
los `.sflx` empaquetados). Dos hallazgos que condicionan el post-beta y que conviene
dejar escritos para no redescubrirlos.

**No hay nada que copiarles en sincronización.** No tienen sincronización continua de
cuentas: el perfil de usuario es una identidad local
(`lib/services/user_profile_store.dart`) y la importación de biblioteca es manual (URL
de playlist pegada, CSV/M3U/M3U8 en `lib/services/csv_import_service.dart`). No hay
watcher de favoritos, ni auto-descarga por like nuevo, ni por artista seguido: la
pantalla de novedades (`weekly_releases_provider.dart`,
`favorite_artists_screen.dart`) es pull-to-refresh contra artistas guardados **en la
app**, no contra los follows de la cuenta. **La visión de "biblioteca espejo viva" no es
alcanzar paridad con ellos: es saltar por delante en la única dimensión donde se les
gana.**

**El conteo de fuentes suyo es inferior al que parece.** De los 12 plugins del registro,
**solo 7 descargan audio**: Amazon, Deezer, JioSaavn (AAC), Qobuz, SoundCloud, Tidal,
YouTube Music. Los otros 5 (Spotify Web, Apple Music, Bugs, Melon, Genie) son solo
metadata y búsqueda; Apple Music añade letras. Y de esos 7, tres pasan por relés de
terceros (Deezer y Qobuz por proxy; Amazon por `zarz.moe` con ticket firmado), mientras
que los descargadores propios de Syncify son nativos y directos. **La ventaja de ellos es
número, no robustez.** Dos apuntes de cadena de suministro para el post-beta: 10 de las 12
`download_url` apuntan al owner `zarzet` en vez de `spotiflacapp` (parece un string
heredado, no un atacante, porque el repo resuelve al mismo contenido), y el SHA-256
protege integridad pero no intención — un registro malicioso puede servir un plugin
válidamente firmado que robe tokens.

Con lo que ya tiene Syncify hoy (Qobuz con URL oficial directa
`download/qobuz.rs:542-577`; Tidal nativo con OAuth propio y DASH
`crates/syncify-tidal-downloader/src/lib.rs:1285-1315`), el gap real al llegar el
post-beta es: **3 descargadores frente a 7**, no 12. Queda anotado el orden de valor
estudiado —Deezer primero, porque el cliente ARL y el matching por ISRC ya existen
(`services/deezer.rs:167`, `:975`); después SoundCloud y JioSaavn (baratos, el segundo
con API pública sin auth); YouTube Music como el salto grande— pero **eso es carril de
después del beta y no se empieza antes.**

---

## 4. Regla para futuras peticiones de alcance

Cualquier feature que encaje en la tabla de "fuera de alcance" se responde con este
documento, no con debate. La condición para desbloquear cualquiera de ellas es: **beta
cerrada**, definida como los seis puntos del apartado 2 en verde y verificados, no solo
commiteados.

Este fichero es el roadmap que faltaba. Si hay que ampliarlo, se amplía aquí.

**Cada ítem de las tablas de "fuera de alcance" está enumerado con su evidencia en
[docs/PLAN_MEJORA_POST_BETA.md](PLAN_MEJORA_POST_BETA.md)**, que es la lista de espera
post-beta. Los dos ficheros se leen juntos: este dice qué NO se toca ahora, el otro dice
exactamente qué se hará después, con qué coste relativo y apoyándose en qué código ya
existente.

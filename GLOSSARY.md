# Syncify

Syncify importa, sincroniza y descarga música entre servicios. Este glosario fija
el lenguaje de dominio: si un término no está aquí y hace falta, se añade; si dos
compiten por lo mismo, se queda uno.

## Identidad

**Track**:
La grabación canónica. Una fila de `tracks` es una grabación, no una edición ni una
aparición en un servicio; su identidad se define en `docs/adr/0001-identidad-de-tracks.md`.
_Evitar_: 'TrackSource' o 'LibraryItem' para referirse a la grabación.

**TrackSource**:
La aparición de un track en un servicio concreto. Una fila de `track_sources` guarda
el `service_track_id` del proveedor y es **evidencia**, no identidad: un track puede
tener varias, y borrar el track no debe borrar la evidencia de que existió.
_Evitar_: 'Track' para referirse a la aparición.

**ISRC**:
Código internacional de grabación. Identifica la **grabación**, no la edición: el
mismo ISRC puede aparecer en un álbum y en una recopilación.

**Provider item**:
El ítem tal como lo identifica un servicio, por su id propio dentro de ese servicio.
Es la unidad que llega en favoritos, álbumes, playlists y URLs.
_Evitar_: 'Track' (la grabación canónica) y 'LibraryItem'.

**Ledger de edición**:
El registro de qué Track se asignó a cada Provider item cuando se decidió una
**Edition**. Es una decisión ya tomada, no una prueba independiente: la escribe el
propio importador.
_Evitar_: 'TrackSource', que es la evidencia de que el ítem existe en la cuenta.

**Aparición**:
Un Provider item que se vio y quedó asignado a un **Track** sin mover su álbum
canónico: la grabación ya vivía en otro álbum y llegó desde un recopilatorio, una
banda sonora o una playlist de catálogo. Queda `needs_review` para que una persona
decida; no es **Evidencia** ni modifica el Track.
_Evitar_: 'colisión', 'conflicto de identidad'.

**Evidencia**:
La aparición de un Provider item en la cuenta, registrada como **TrackSource**. Manda
sobre el **Ledger de edición**: si hay evidencia, el Track que liga es el bueno y el
ledger solo puede confirmarlo; el ledger decide únicamente cuando todavía no hay
evidencia.

**Edition / Release / Recording**:
El modelo de ediciones (`edition_recordings`, `edition_releases`,
`edition_provider_items`, `edition_item_tracks`), gobernado por
`docs/adr/0003-seleccion-de-edicion.md`. Una grabación puede pertenecer a varias
ediciones materialmente distintas.

**needs_review**:
Resultado explícito de identidad o edición: hay conflicto relevante entre **Evidencia**
y **Ledger de edición**, o falta el dato mínimo para decidir. No es un error: es un
estado que la UI debe mostrar.
_Evitar_: 'colisión de identidad del ítem' — no hay colisión cuando un ítem solo está
en el ledger; eso es ausencia de evidencia y tiene su propia regla (ED-02a).

**Fallback**:
Permitido para la **misma** edición (cuenta o calidad alternativa). Un cambio de
edición exige autorización separada; no lo cubre el `allow_fallback` de
servicio/calidad.

## Catálogo y biblioteca

**Service**:
Proveedor de música (Qobuz, Tidal, Spotify, SoundCloud, Apple Music). Tabla
`services`, sembrada por `src-tauri/migrations/0002_normalize_schema.sql`.

**Servidor personal**:
Biblioteca de música propiedad del usuario servida por Plex, Jellyfin, Emby,
Subsonic o Kodi. Es una fuente de reproducción, no un proveedor comercial: no
tiene catálogo ni derechos de descarga, y sus ítems se tratan como Provider items.
_Evitar_: 'Service' para referirse a un servidor personal; 'proveedor' a secas.

**Account**:
Cuenta conectada de un servicio. Puede haber varias por servicio y no hay cuenta
activa: la biblioteca se agrega con filtros. Estados: habilitada, desactivada,
archivada y eliminada (ADR-0007). Tabla `accounts`: la antigua `service_accounts` de la
0001 fue renombrada y migrada en la 0002, así que ese nombre solo aparece en
migraciones históricas.

**Perfil de login**:
Directorio de datos del webview asociado a una cuenta, o a un intento de login antes de
que la cuenta exista. Aísla cookies y almacenamiento entre cuentas del mismo servicio.
_Evitar_: «sesión» para referirse al directorio; la sesión es el artefacto cifrado.

**LibraryItem**:
Pertenencia de un track a la biblioteca del usuario. Tabla `library_items`, creada en
`src-tauri/migrations/0067_migration_schema_alignment.sql`.
_Evitar_: 'Track' (la grabación) y 'Download' (el fichero).

**Biblioteca**:
Vista agregada de las pistas importadas de todas las cuentas y colecciones, con una fila
por pista canónica y filtros por servicio y cuenta. Excluye la cuarentena por defecto.
_Evitar_: llamar «biblioteca» a una sola cuenta o a la suma de entradas sin deduplicar.

**Playlist / PlaylistTrack**:
Lista del usuario y su pertenencia ordenada (`playlists`, `playlist_tracks`,
`playlist_sources` para el vínculo con el origen).

**Playlist local**:
Playlist sin cuenta, con UUID estable; no depende de ninguna cuenta y puede publicarse
en un proveedor. Su identidad no cambia al renombrarla.
_Evitar_: 'playlist del sistema' o 'playlist de la cuenta 1'.

**Fuente de playlist**:
Vínculo entre una playlist local y su copia remota (`playlist_sources`). Una playlist
puede tener varias fuentes; el modo de escritura se decide por fuente.
_Evitar_: 'playlist remota' para referirse a la entidad local.

**Playlist huérfana**:
Playlist cuya fuente remota desapareció. Se conserva localmente, se avisa y no se
empuja; no se borra sola.
_Evitar_: 'playlist borrada'.

**Publicar playlist**:
Crear la playlist en un proveedor y enlazarla como fuente, conservando la identidad
local. No es Sync.
_Evitar_: 'sincronizar' para una publicación puntual.

**Push de playlist**:
Escritura de los cambios locales de una playlist en su fuente remota. Es opt-in por
fuente, ocurre al guardar y no forma parte de Sync.
_Evitar_: 'sync bidireccional' para este push.

**Smart playlist**:
Playlist dinámica definida por reglas; no almacena pertenencia en `playlist_tracks`.
Su contenido se evalúa al abrirla y su portada es un mosaico del resultado.
_Evitar_: 'playlist automática' si se refiere a una lista importada.

## Trabajo y recuperación

**Sync**:
Importación unidireccional de las colecciones de un servicio —favoritos, playlists,
álbumes y artistas— a la biblioteca local (`sync_log`, `service_favorites`). Un «sync»
no descarga audio ni escribe en el proveedor; el push de playlists es una operación
aparte.
_Evitar_: «actualizar la biblioteca» para referirse solo a importar.

**Modo de sincronización**:
Contrato elegido para una cuenta o servicio: solo importar (default), espejo con
cuarentena, espejo estricto o bidireccional (modelado y deshabilitado en ADR-0006;
el push de playlists no lo activa). Cada modo está limitado por las capacidades
declaradas del proveedor.
_Evitar_: confundirlo con la cadencia (manual, intervalo) o con el alcance por entidad.

**Cobertura**:
Relación entre lo que el proveedor reporta y lo que la biblioteca local tiene
importado. Puede ser desconocida cuando el proveedor no publica un total; nunca se
inventa un porcentaje.
_Evitar_: usar «completo» para decir «sin errores».

**Cuarentena**:
Estado de una pista, pertenencia o playlist que el proveedor ya no devuelve. Se
conserva la fila y la evidencia; se excluye de listados y contadores por defecto.
_Evitar_: «borrado» o «eliminado»: en cuarentena nada se borra.

**Corrida de sincronización**:
Una ejecución de Sync para una cuenta. Tiene estado (terminó / parcial / falló),
cobertura y errores; el historial conserva las últimas.
_Evitar_: «sync completado» cuando la cobertura no cuadra.

**Download / DownloadQueue**:
Descarga de un fichero y su estado en cola (`downloads`, `download_queue`). La cola
sobrevive al reinicio; su estado es parte del contrato, no un detalle interno.

**Job de descarga**:
Agrupación con id, tipo, filtro y contadores de una acción de descarga del usuario
(p. ej. «descargar favoritos»), sobre las filas de `download_queue`. No es un segundo
planificador: la cola persistente sigue siendo la fuente de verdad.
_Evitar_: confundirlo con **MigrationJob** (migración de biblioteca) o con la cola misma.

**Fuente (de una descarga)**:
Servicio del que procede la identidad de la pista en la biblioteca (`track_sources`).
Puede no ser el servicio que entrega el audio.
_Evitar_: llamarlo «destino» o confundirlo con la cuenta.

**Destino (de una descarga)**:
Proveedor con adaptador que entrega el audio de una descarga. Se elige por preferencia
explícita y, si no, por evidencia; puede ser distinto de la fuente.
_Evitar_: usar «servicio» sin decir si es fuente o destino.

**Estado de descarga (JobState)**:
Uno de los cinco valores de `download_queue.status` —`queued`, `downloading`,
`complete`, `failed`, `cancelled`— con transiciones legales declaradas en la máquina
de estados (`crates/syncify-core-domain`). El string es contrato con la UI y con las
filas existentes; el tipo es la fuente de verdad de qué transiciones existen
(`docs/adr/0022-maquina-de-estados-de-descargas.md`).
_Evitar_: escribir un estado a mano o comparar strings sueltos en consultas nuevas.

**RetryVerdict**:
Respuesta de un job fallido a «¿se reintenta?»: `Permanent` o `RetryAt(instante)` con
backoff 1m/5m/15m/1h (tope 4h). Una clase de fallo terminal fuerza `Permanent`. El
instante se persiste en `next_attempt_at` y el claim lo respeta.
_Evitar_: inventar un segundo presupuesto de reintentos por call site.

**Clase de fallo (FailureClass)**:
Categoría tipada de un fallo de descarga —reintentable, terminal, requiere acción del
usuario— definida en la taxonomía de errores del dominio y persistida en
`failure_class` con la transición a `failed`.
_Evitar_: clasificar reintentos parseando mensajes de error.

**Recuperación de la cola**:
Transición explícita `downloading → queued` que se aplica una sola vez al arrancar
para los jobs interrumpidos (`RecoverySweep`). No revive `failed` ni `cancelled`.
_Evitar_: barridos SQL sueltos que reseteen estados.

**Cancelación**:
Transición legal a `cancelled` (desde `queued` o `downloading`) seguida del
`CancellationToken` del job en vuelo, que hace abortar la transferencia; el staging
parcial se conserva (`resumable`) para reanudar. Un job cancelado no se reintenta ni
se recupera.
_Evitar_: «eliminar» para referirse a cancelar.

**MigrationJob**:
Migración de **biblioteca** de un servicio a otro (`migration_jobs`, con
`source_service` y `destination_service`;
`src-tauri/migrations/0017_migration_history.sql`). Un job puede llevar favoritos de
pista, álbumes favoritos, artistas seguidos y playlists; cada ítem declara su tipo.
_Evitar_: confundirlo con **migración SQL**; las de esquema viven en
`src-tauri/migrations/` y son otra cosa. El doble uso es deliberado en el vocabulario
del producto, pero nunca en el mismo contexto.

**Colección migrable**:
Cada una de las colecciones que un job puede transferir: favoritos de pista, álbumes
favoritos, artistas seguidos y playlists. No todas existen en todos los destinos.
_Evitar_: «biblioteca completa» cuando solo se migra una colección.

**Capacidad de destino**:
Lo que un proveedor declara que puede escribir por colección (guardar pista, guardar
álbum, seguir artista, crear playlist). Lo no declarado no se ofrece.
_Evitar_: inferirla de un intento fallido.

**Journal de migración**:
Registro de cada ítem que un job creó en destino, con su id y cuenta, para poder
deshacer sin tocar lo que ya existía.
_Evitar_: confundirlo con el journal de operaciones de descarga.

**Undo de migración**:
Retirada best-effort de lo que un job creó en destino. No borra lo preexistente y
puede dejar restos si el proveedor no permite eliminar.
_Evitar_: «rollback» para prometer una reversión completa.

**Preflight de migración**:
Comprobación previa de esquema, credenciales, capacidades y cuentas antes de escribir.
Si falla, el job no empieza.
_Evitar_: llamar «dry-run» a un preview que no comprueba credenciales.

**OperationJournal / OperationRecovery**:
Diario de operaciones y su recuperación tras un fallo (`operation_journal`,
`operation_recovery_audit`, `repair_history`). Es lo que permite que un cierre a
mitad de descarga no deje la biblioteca a medias.

**Enrichment**:
Completar metadatos (MusicBrainz, Last.fm, AcoustID) sobre un track ya identificado.
Enriquecer no cambia la identidad; si la pondría en duda, el resultado es
`needs_review`.

**ListenSession**:
Una escucha contable (`playback_listens`), idempotente por `listen_session_id`:
registrar dos veces la misma sesión no suma dos escuchas.

## Reproducción

**Cola de reproducción**:
La lista ordenada de lo que va a sonar. La posee el backend y se persiste en SQLite
junto con la pista actual, la posición y los modos; la UI, MPRIS y la bandeja leen
el mismo estado.
_Evitar_: 'playlist', que es una lista de catálogo o de usuario.

**Entrada efímera**:
Una entrada de la cola que apunta a un Provider item que no está en la biblioteca.
No crea `Track` ni `TrackSource` mientras no se promocione.
_Evitar_: 'track temporal' o 'pista fantasma'.

**Promoción**:
El paso de una entrada efímera a track de biblioteca tras 30 s reales de escucha:
se resuelve su identidad, se crea o enlaza el `Track` con su `TrackSource` y se
acredita la escucha. Es idempotente.
_Evitar_: 'importación automática' para previsualizaciones que aún no sonaron.

**Ganancia de reproducción**:
El modo con el que el motor aplica ReplayGain al sonar (`off`, `track`, `album`),
separado del modo de etiquetado de descarga. La ganancia sale de las columnas de
`tracks`.
_Evitar_: 'replay_gain_mode' para ambos sentidos sin decir cuál.

**AGC**:
Nivelación dinámica del volumen en reproducción. Es un modo alternativo a
ReplayGain, no un complemento.
_Evitar_: 'normalización' a secas, que también nombra el etiquetado de descarga.

**Calidad de fuente**:
Codec, profundidad de bits y frecuencia de muestreo que entrega el proveedor o el
archivo local, tal como los reporta el resolver. No implica que el dispositivo los
reproduzca sin remuestreo.
_Evitar_: 'calidad' a secas; 'QualityTier', que es la calidad anunciada frente a
la entregada en descargas.

**Calidad de salida**:
Formato que el dispositivo de audio entrega realmente (frecuencia, profundidad,
canales) y si hubo remuestreo. Puede diferir de la calidad de fuente; la UI debe
mostrar ambas cuando difieran.
_Evitar_: 'calidad de reproducción' para la de fuente.

**Preflight de cola**:
Comprobación previa al encolado o la reproducción de que la cuenta y el proveedor
pueden servir la entrada (entitlement, credenciales, estado del proveedor). Si
falla, la entrada se marca para reconectar en vez de fallar a mitad.
_Evitar_: 'validación' genérica.

**Estado de proveedor**:
Situación de una cuenta o servicio para reproducir: `connected`, `expired`,
`preview-only` o `unavailable`. Se muestra en la búsqueda y la cola; no es lo
mismo que el estado de una descarga.
_Evitar_: 'conectado' sin decir si es cuenta, proveedor o descarga.

**Renderer**:
Dispositivo de salida de terceros (UPnP/DLNA o Chromecast) que recibe el audio
por red y lo reproduce. Syncify lo controla; no es el dispositivo de audio local
ni el motor nativo.
_Evitar_: 'reproductor' para el renderer; 'salida' a secas.

## Calidad y entrega

**QualityTier**:
Calidad anunciada por el servicio frente a la realmente entregada
(`quality_preferences`, `track_availability`). Anunciada ≠ bytes reales.
_Evitar_: confundirla con la calidad de fuente o de salida de reproducción (ver
Reproducción).

**Evidencia de entrega**:
Registro de que una combinación servicio + cuenta sirvió una pista concreta, con la
calidad entregada. Se reutiliza para enrutar descargas futuras; no es una garantía
permanente: caduca y un fallo la invalida.

**Grant** *(retirado y eliminado)*:
Concesión en memoria del protocolo `syncify-media://`, sustituido por el motor
nativo. El subsistema se borró (C-19 en `docs/Deuda_Tecnica_y_UX.md`): hoy no queda
ni el comando ni el protocolo.

## Letras

**Fuente de letras**:
Uno de los orígenes independientes de los que Syncify resuelve una letra (LRCLIB,
Apple Music TTML, Kugou, …). No es un proveedor de música: sirve letras para
cualquier pista, venga o no de su biblioteca.
_Evitar_: 'proveedor' a secas, reservado a los servicios de música.

**Nivel de sincronía**:
Granularidad temporal que garantiza una letra: por sílaba, por palabra, por línea o
sin marcas (plain). Ordena la calidad entre versiones: más fino, mejor.
_Evitar_: 'calidad' a secas; 'tipo de letra' para el nivel.

**Enhanced LRC (ELRC)**:
LRC con marcas de tiempo por palabra o sílaba dentro de la línea. Es el formato de
preservación de la sincronía fina; degradarlo a LRC de línea es pérdida.
_Evitar_: 'LRC mejorado', 'karaoke LRC'.

**Sidecar**:
Fichero `.lrc` hermano del audio que entrega la letra a reproductores externos. Es
una proyección de la letra guardada, no su origen.
_Evitar_: 'fichero de letra' para el almacenamiento canónico.

**Versión de letra**:
Una letra concreta que una **Fuente de letras** produjo para un idioma y formato
dados. Varias versiones pueden convivir para la misma pista; el formato es una
propiedad del contenido, no su identidad.
_Evitar_: 'formato' para la versión.

**Versión activa**:
La versión de letra que la reproducción y la UI muestran de una pista. La fija un
**Pin de versión** del usuario o, en su ausencia, la regla de selección (nivel de
sincronía, idioma preferido, prioridad de fuente).
_Evitar_: 'letra actual' sin más; el badge de UI no persiste la elección.

**Pin de versión**:
Elección explícita del usuario que fija la **Versión activa**. La resolución
automática, los reintentos y el re-fetch karaoke no la degradan: solo la acción del
usuario la cambia.
_Evitar_: 'favorito', 'seleccionada' como estado efímero.

**Marca de borrado**:
Intención persistente del usuario de que una pista no tenga letra, por formato o
completa. La resolución automática la respeta; solo guardar o restaurar
explícitamente ese formato la anula.
_Evitar_: tratarla como caché de la resolución: es voluntad del usuario, no un
resultado.

**Cosecha**:
Barrido del disco local que incorpora letras ya presentes en ficheros (tags
embebidos o sidecars) a la biblioteca. No resuelve en red.
_Evitar_: 'escaneo' genérico; 'descarga', que no toca.

**Letra instrumental**:
Resultado válido de resolución que declara que la pista no tiene letra cantada. No
es un fallo ni ausencia: se muestra como tal.
_Evitar_: 'sin letra', que sugiere que falta resolverla.

**Sonda**:
Comprobación puntual de que una fuente responde con su endpoint, sin resolver una
pista concreta. Distingue falta de credencial de fuente caída.
_Evitar_: 'test de proveedor'; 'ping'.

## Ajustes y backup

**Ajuste**:
Una preferencia del usuario con nombre, valor y superficie de persistencia (tabla
tipada o KV genérica). Persistido no significa consumido: un ajuste sin lector no
es una preferencia, es una promesa falsa.
_Evitar_: 'opción' para el ajuste sin lector; 'config' a secas.

**Ajuste huérfano**:
Ajuste visible o persistido que ningún código lee para decidir comportamiento. No
es deuda menor: miente en la UI. Su destino es cablearse u ocultarse, nunca
quedarse.
_Evitar_: 'ajuste decorativo'; 'dead setting'.

**Superficie de ajustes**:
El almacén atómico de un grupo de ajustes: una tabla tipada (singleton o por fila)
o la KV genérica. Cada valor vive en una sola superficie; la duplicación entre
superficies es un defecto, no un patrón.
_Evitar_: 'tabla de ajustes' para la KV genérica.

**Consumidor**:
El código que lee un ajuste para decidir comportamiento. Es lo que acredita que el
ajuste existe de verdad: escribirlo no basta.
_Evitar_: 'persistido' como sinónimo de consumido.

**Defaults**:
Valores de fábrica de una superficie. Viven en un solo sitio y de ahí comen
arranque, reset y seed; las copias de defaults en la UI están prohibidas.
_Evitar_: 'seed' para el valor leído en runtime.

**Reset**:
Restaurar una o todas las superficies a sus defaults. Es una operación del backend
sobre los almacenes, no una compensación que la UI hace en cliente.
_Evitar_: confundirlo con el borrado de biblioteca; 'factory reset' si solo toca
ajustes.

**Manifest de backup**:
El JSON de exportación con checksum y versionado de esquema que lleva biblioteca y
ajustes, nunca secretos. Restaurarlo en otra máquina reproduce biblioteca y
ajustes; las cuentas piden reautenticar.
_Evitar_: llamar 'backup' a la copia pre-mutación interna.

**Copia pre-mutación**:
Snapshot interno de la BD que una operación peligrosa deja antes de escribir. No es
un backup de usuario: no sale del equipo, no se programa y no se lista en la UI.
_Evitar_: 'backup automático'.

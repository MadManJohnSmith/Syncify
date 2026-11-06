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

## Trabajo y recuperación

**Sync**:
Importación unidireccional de las colecciones de un servicio —favoritos, playlists,
álbumes y artistas— a la biblioteca local (`sync_log`, `service_favorites`). Un «sync»
no descarga audio ni escribe en el proveedor.
_Evitar_: «actualizar la biblioteca» para referirse solo a importar.

**Modo de sincronización**:
Contrato elegido para una cuenta o servicio: solo importar (default), espejo con
cuarentena, espejo estricto o bidireccional. Cada modo está limitado por las
capacidades declaradas del proveedor.
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

**MigrationJob**:
Migración de **biblioteca** de un servicio a otro (`migration_jobs`, con
`source_service` y `destination_service`;
`src-tauri/migrations/0017_migration_history.sql`).
_Evitar_: confundirlo con **migración SQL**; las de esquema viven en
`src-tauri/migrations/` y son otra cosa. El doble uso es deliberado en el vocabulario
del producto, pero nunca en el mismo contexto.

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

## Calidad y entrega

**QualityTier**:
Calidad anunciada por el servicio frente a la realmente entregada
(`quality_preferences`, `track_availability`). Anunciada ≠ bytes reales.

**Evidencia de entrega**:
Registro de que una combinación servicio + cuenta sirvió una pista concreta, con la
calidad entregada. Se reutiliza para enrutar descargas futuras; no es una garantía
permanente: caduca y un fallo la invalida.

**Grant** *(retirado y eliminado)*:
Concesión en memoria del protocolo `syncify-media://`, sustituido por el motor
nativo. El subsistema se borró (C-19 en `docs/Deuda_Tecnica_y_UX.md`): hoy no queda
ni el comando ni el protocolo.

## Puentes Python

**Puente**:
Proceso Python que la app lanza para hablar con un proveedor o una herramienta
externa (`scripts/*.py`). Todos comparten el mismo contrato de envelope JSON por
stdout, el mismo intérprete resuelto y el mismo presupuesto de timeout declarado en
cada llamada (`docs/adr/0021-contrato-unico-de-puentes-python.md`).
_Evitar_: «script» para referirse a un puente cuando se habla de su contrato.

**Envelope de puente**:
Sobre JSON `{success, data, error}` que todo puente escribe por stdout. `success` es
obligatorio; `data` y `error` llegan según el desenlace. Un fallo de dominio viaja
como envelope con `success: false` y exit 1; un bug del puente es traceback en stderr
sin envelope.
_Evitar_: llamar «resultado» al envelope cuando se habla del shape del contrato.

**BridgeClient**:
Única vía Rust para lanzar un puente e interpretar su envelope
(`src-tauri/src/bridges/`). `capture` devuelve la salida cruda; `call<T>` deserializa
el envelope tipado.
_Evitar_: construir `Command` para un puente fuera de este módulo.

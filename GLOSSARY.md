# Glosario de Syncify

Lenguaje de dominio del proyecto. Es la referencia para nombrar módulos, tests y
columnas: si un término no está aquí y hace falta, se añade; si dos términos
compiten por lo mismo, se queda uno.

Las decisiones que fijan estos términos viven en `docs/DECISION_IDENTIDAD_TRACKS.md`
y `docs/DECISION_SELECCION_EDICION.md` (no en `docs/adr/`: el repo usa ese otro
nombre de fichero desde antes de este glosario y moverlo rompería las citas).

## Identidad

**Track** — la grabación canónica. Una fila de `tracks` es una grabación, no una
edición ni una aparición en un servicio. Su identidad se define en
`docs/DECISION_IDENTIDAD_TRACKS.md`.
*No confundir con* **TrackSource** ni con **LibraryItem**.

**TrackSource** — la aparición de un track en un servicio concreto. Una fila de
`track_sources` guarda el `service_track_id` del proveedor y es **evidencia**, no
identidad: un track puede tener varias. Borrar un track no debe borrar la
evidencia de que existió.
*No confundir con* **Track**.

**ISRC** — código internacional de grabación. Identifica la **grabación**, no la
edición; el mismo ISRC puede aparecer en un álbum y en una recopilación.

**Provider item** — el ítem tal como lo identifica un servicio: su id propio dentro
de ese servicio. Es la unidad que llega en favoritos, álbumes, playlists y URLs.
*No confundir con* **Track** (la grabación canónica) ni con **LibraryItem**.

**Ledger de edición** — el registro de qué Track se asignó a cada Provider item
cuando se decidió una **Edition**. Es una decisión ya tomada, no una prueba
independiente: la escribe el propio importador.
*No confundir con* **TrackSource**, que es la evidencia de que el ítem existe en
la cuenta.

**Aparición** — un Provider item que se vio y quedó asignado a un **Track** sin
mover su álbum canónico: la grabación ya vivía en otro álbum y llegó desde un
recopilatorio, una banda sonora o una playlist de catálogo. Queda `needs_review`
para que una persona decida; no es **Evidencia** ni modifica el Track.
*Evitar*: «colisión», «conflicto de identidad».

**Evidencia** — la aparición de un Provider item en la cuenta, registrada como
**TrackSource**. Manda sobre el **Ledger de edición**: si hay evidencia, el Track
que liga es el bueno y el ledger solo puede confirmarlo. El ledger decide
únicamente cuando todavía no hay evidencia.

**Edition / Release / Recording** — el modelo de ediciones (`edition_recordings`,
`edition_releases`, `edition_provider_items`, `edition_item_tracks`), gobernado por
`docs/DECISION_SELECCION_EDICION.md`. Una grabación puede pertenecer a varias
ediciones materialmente distintas.

**needs_review** — resultado explícito de identidad/edición: hay conflicto relevante
entre **Evidencia** y **Ledger de edición**, o falta el dato mínimo para decidir. No es un
error: es un estado que la UI debe mostrar. (12 usos en `src-tauri/src`.)
_Evitar_: «colisión de identidad del ítem» — no hay colisión cuando un ítem solo está en el
ledger; eso es ausencia de evidencia y tiene su propia regla (ED-02a).

**Fallback** — permitido para la **misma** edición (cuenta o calidad alternativa).
Un cambio de edición exige autorización separada; no lo cubre el `allow_fallback`
de servicio/calidad.

## Catálogo y biblioteca

**Service** — proveedor de música (Qobuz, Tidal, Spotify, SoundCloud, Apple Music).
Tabla `services`, sembrada por `src-tauri/migrations/0002_normalize_schema.sql`.

**Account** — cuenta conectada de un servicio. Tabla `accounts`: la antigua
`service_accounts` de la 0001 fue renombrada y migrada en la 0002, así que ese
nombre solo aparece en migraciones históricas.

**LibraryItem** — pertenencia de un track a la biblioteca del usuario. Tabla
`library_items` (creada en `src-tauri/migrations/0067_migration_schema_alignment.sql`).
*No confundir con* **Track** (la grabación) ni con **Download** (el fichero).

**Playlist / PlaylistTrack** — lista del usuario y su pertenencia ordenada
(`playlists`, `playlist_tracks`, `playlist_sources` para el vínculo con el origen).

## Trabajo y recuperación

**Sync** — importar favoritos/likes desde un servicio a la biblioteca
(`sync_log`, `service_favorites`). Un «sync» no descarga audio.

**Download / DownloadQueue** — descarga de un fichero y su estado en cola
(`downloads`, `download_queue`). La cola sobrevive al reinicio; su estado es parte
del contrato, no un detalle interno.

**MigrationJob** — migración de **biblioteca** de un servicio a otro
(`migration_jobs`, con `source_service` y `destination_service`;
`src-tauri/migrations/0017_migration_history.sql`).
*No confundir con* **migración SQL**: las de esquema viven en
`src-tauri/migrations/` y son otra cosa. Ese doble uso es deliberado en el
vocabulario del producto, pero nunca en el mismo contexto.

**OperationJournal / OperationRecovery** — diario de operaciones y su recuperación
tras un fallo (`operation_journal`, `operation_recovery_audit`, `repair_history`).
Es lo que permite que un cierre a mitad de descarga no deje la biblioteca a medias.

**Enrichment** — completar metadatos (MusicBrainz, Last.fm, AcoustID) sobre un
track ya identificado. Enriquecer no cambia la identidad; si la pondría en duda,
el resultado es `needs_review`.

**ListenSession** — una escucha contable (`playback_listens`), idempotente por
`listen_session_id`: registrar dos veces la misma sesión no suma dos escuchas.

## Calidad y entrega

**QualityTier** — calidad anunciada por el servicio frente a la realmente
entregada (`quality_preferences`, `track_availability`). Anunciada ≠ bytes reales.

**Grant** *(retirado y eliminado)* — concesión en memoria del protocolo
`syncify-media://`, sustituido por el motor nativo. El subsistema se borró (C-19 en
`docs/Deuda_Tecnica_y_UX.md`): hoy no queda ni el comando ni el protocolo.

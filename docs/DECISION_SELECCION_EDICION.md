# Decisión — selección determinista de edición y frontera de importación

**Estado:** DECIDIDO (entra dentro del alcance beta, autorizado por el propietario
2026-10-13) · **Prerrequisito:** este documento + las filas `ED-*` de
[docs/auditoria/matriz-ajustes.md](auditoria/matriz-ajustes.md).

**Justificación de alcance:** `DECISION_ALCANCE_BETA.md` §1 prohíbe añadir alcance
nuevo. El propietario autorizó expresamente esta corrección dentro de la beta
(2026-10-13). Cambia la implementación de importación y descarga existentes para
evitar asociaciones erróneas; no añade proveedores ni reorganiza datos de usuarios.

**Este documento es el registro permanente.** Recoge el hallazgo, su evidencia,
las correcciones que han sufrido las hipótesis iniciales y las invariantes que
debe respetar cualquier implementación futura. Existe para que nadie repita la
investigación ni contradiga una decisión ya tomada.

---

## 1. El síntoma que lo originó

El usuario encontró 11 carpetas de recopilaciones bajo `~/Music/Syncify/Various Artists`
(*The Matrix Reloaded*, *Ocean's Twelve*, *Atomic Blonde*, *Synth Pop*, *Saturday Night
Fever*, etc.). Su lectura correcta fue en dos tiempos:

1. Primero pensó que era un problema de carpetas (plantilla `{AlbumArtist}`). **Es
   falso**: la carpeta es normal y estándar, y el etiquetado de recopilación es
   intencional (`crates/syncify-flac-writer/src/lib.rs:268-340` fuerza
   `album_artist = "Various Artists"` cuando detecta recopilación, y
   `crates/syncify-core-domain/src/layout.rs:374-375` añade `{Artist}` al nombre de
   archivo precisamente en esas rutas). No hay bug que arreglar ahí.
2. Después identificó el problema real: **no quiere descargar la versión de
   recopilación cuando existe la de álbum/single**, porque esas versiones suelen ser
   edits o de peor calidad.

El punto 2 es un problema de **selección de fuente**, no de carpetas. Pero al
profundizar resultó que el síntoma visible (carpetas) y el síntoma real
(contaminación silenciosa por asociación de edición incorrecta) no son el mismo
problema, y el segundo es más grave: puede ocurrir sin que ninguna carpeta se vea mal.

---

## 2. Diagnóstico verificado y límites de la evidencia



### Selección y fallback

- `src-tauri/src/download/qobuz.rs:746-895`: `search_by_isrc` toma el primer ISRC
  coincidente con duración ±10 s; `search_by_metadata` toma el primer título/artista
  aproximados con duración. La búsqueda usa `limit=50`: no garantiza cobertura de
  todas las ediciones ni estabilidad del orden. `clean_title` (`:2652`) quita
  calificadores live/remix/remaster y `title_matches` (`:2611`) acepta subcadenas:
  útiles para descubrimiento, no para afirmar identidad.
- `src-tauri/src/download/tidal.rs:134-192` también busca por ISRC y metadatos.
  Inventariar su ruta con/sin BD antes de cambiar adaptadores.
- `src-tauri/src/download/resolver.rs:483-605,608-712`: el resolver ordena fuentes
  ya asociadas a `tracks.id` por prioridad, disponibilidad y calidad anunciada;
  una fuente fijada se promueve. `src-tauri/src/worker.rs:1308-1385` vuelve a
  resolver antes de acceder a la red. `quality_score` no mide confianza de edición.
- `src-tauri/src/download/orchestrator.rs:325-610` ya devuelve `AmbiguousSource`
  para ciertas alternativas cross-service. No es verdad que **todas** las rutas
  elijan el primer resultado. Auditar las rutas con/sin plan y fallback antes de
  modificar el comportamiento.

### Importación y persistencia

- `src-tauri/migrations/0002_normalize_schema.sql:113-129`: `track_sources` impone
  `UNIQUE(track_id,service_id)` desde el origen, así que **no puede** contener dos
  items del mismo proveedor para un mismo track. La migración
  `0064_pipeline_hardening_and_integrity.sql:188-190` añadió además el índice único
  global `idx_track_sources_service_track_unique` sobre `(service_id,service_track_id)`
  (también en `src-tauri/src/db.rs:528-532`), y lo hizo **borrando antes** las
  filas duplicadas (`DELETE ... WHERE id NOT IN (SELECT MIN(id) ... GROUP BY
  service_id, service_track_id)`). El esquema vigente aplica las dos reglas de
  unicidad, y la segunda se impuso con pérdida de filas: auditar qué colisiones
  se descartaron antes de diseñar tablas encima.
- `src-tauri/src/services/enrichment.rs:2686-2707` busca primero por item de
  servicio (`:2690`) y luego, si no hay match y existe ISRC, por
  `SELECT id FROM tracks WHERE isrc = ? LIMIT 1` (`:2707`); el ISRC no prueba
  misma edición.
- `src-tauri/src/download/qobuz.rs:2320` y `:2472` resuelven el id canónico con
  `SELECT id FROM tracks WHERE isrc = ? LIMIT 1`: misma trampa, dentro del
  propio descargador, no solo en el motor de importación.
- `src-tauri/src/import_cache.rs:229-262` usa título normalizado + artista (o
  `va:` + título) como clave de álbum, y puede desempatar por completitud.
- `src-tauri/src/commands/library.rs:5654-5775` enlaza un archivo local por
  ruta/hash, luego ISRC `LIMIT 1`, luego título+artista primario. Una única
  coincidencia por nombre no prueba que sea la misma grabación o versión.
- `src-tauri/src/commands/url_import.rs:15-39,95-151,184-311`: la URL parsea
  servicio/tipo/id (`:15-39`) pero SongLink puede elegir un **servicio/item
  destino** (`:95-151`), conservando el origen parseado como procedencia de
  fallback, no como item obligatorio a descargar; SongLink propone un item
  cross-service; se puede crear un track sin ISRC/álbum/duración. El upsert
  de `track_sources` por `(track_id,service_id)` reemplaza un ID anterior
  (`:253-276`). Equivalencia SongLink no demuestra misma edición. Importar
  catálogo de streaming **no implica descargar**; importar URL puede encolar
  y el sync local asocia un archivo existente sin moverlo.
- `src-tauri/src/download/qobuz.rs:746-802` y `:822-855` son las únicas rutas que
  buscan antes de descargar; `src-tauri/src/download/tidal.rs:134-192` convive
  con el pipeline `services/tidal_pipeline.rs` cuando hay BD: verificar qué
  ruta actúa antes de cambiar una sola de ellas.
### Tres identidades que el diseño debe separar

| Identidad | Qué representa | Límite de la prueba |
|---|---|---|
| Grabación/versión | Interpretación o master compatible | ISRC solo no garantiza mismo edit/master ni release; corroborar artista, título con calificadores y duración |
| Release/edición | Álbum o single concreto y su relación con la pista | Título/fecha solos no son un ID global; posición puede cambiar entre ediciones |
| Item de proveedor | `(service_id,service_track_id)` descargable | Calidad anunciada no es calidad física ni acredita identidad de release |

`docs/DECISION_IDENTIDAD_TRACKS.md` decide el modelo canónico existente. Esta
propuesta lo complementa; debe resolver explícitamente la tensión entre su dedup
ISRC-first y la necesidad de no unir ediciones incompatibles. El catálogo
`migrations/0096_mb_dump_catalog.sql` almacena grabaciones e ISRC, no releases;
`services/mb_dump_import.rs` lo escribe pero no hay lector conectado al flujo.
No contar con ese dump para verificar una edición.

## 3. Invariantes decididas

1. **Respetar origen explícito.** Un item de proveedor seleccionado por URL,
   importación o usuario mantiene su lock. No se cambia por una recopilación o
   álbum "mejor" sin autorización de sustitución de edición. Si falla identidad
   o calidad estricta, bloquear con motivo. Cambiar de cuenta manteniendo el
   mismo item no es cambiar de edición.
2. **Descubrimiento no es confirmación.** ISRC, SongLink, MusicBrainz recording,
   título flexible, fecha o calidad anunciada generan candidatos. Ninguno de
   ellos solo certifica misma edición. Ausencia de dato es `unknown`, no `false`.
3. **Identidad antes que preferencia.** Solo entre candidatos ya acreditados
   como versión compatible se prefiere álbum/single no recopilatorio. Fecha
   más antigua, más pistas, posición y calidad anunciada no salvan un conflicto.
4. **Fail closed.** Conflicto, empate material, cobertura parcial de búsqueda,
   error de API o falta de evidencia para la acción solicitada => `needs_review` /
   `AmbiguousSource`; no vincular filas ni descargar. Un resultado único de
   una búsqueda incompleta no prueba que sea la única edición existente.
5. **Auditable y reproducible.** Persistir origen, observaciones, evidencia
   positiva/negativa, estado de cobertura, versión de regla, veredicto y causa.
   Orden estable solo entre candidatos que la política considera equivalentes;
   nunca usar el ID menor como desempate de identidad. No guardar credenciales
   ni URLs firmadas en logs.

## 4. Diseño para implementación (especificación, no SQL cerrado)

### ED-01 — Modelo y migración aditiva de identidades

Separar `RecordingEvidence`, `ReleaseEvidence` y `ProviderItemEvidence` (con
`Unknown` explícito). Añadir por migración nueva —nunca editar una ya aplicada,
ver `src-tauri/src/db.rs`— almacenamiento de items por servicio, relación N:M
con pista canónica, contexto de release separado por ID de proveedor cuando
exista y procedencia/estado de validación. `(service_id,service_track_id)` debe
identificar un item globalmente cuando los datos lo permitan; NO basta con
`UNIQUE(track_id,service_id,service_track_id)`, que deja asociar el mismo item a
dos pistas distintas. Sin ID de álbum no fabricar uno con título/año.

Mantener el resolver existente durante la transición, pero definir una única
fuente de verdad y un bridge transaccional con `track_sources`; inventariar
colisiones previas y exponerlas, nunca borrarlas ni reasignarlas por `LIMIT 1`.
El ID de release y el item elegido deben viajar en la intención de cola y el
plan; el texto de búsqueda no sustituye a una referencia verificada.

### ED-02 — Frontera de importación sin fusiones inciertas

Conservar item original, cuenta/servicio y release original (si se conoce) en
favoritos, álbumes, playlists, URL/SongLink, migración y sync para todos los
proveedores existentes; registrar qué dato ofrece cada API, no inventar
metadatos. Revisar todos los brazos de `docs/MATRIZ_PARIDAD_IMPORTACION.md`.

Eliminar matches que silenciosamente asignan ISRC `LIMIT 1` o álbum de igual
título; sustituir por consulta de todos los candidatos, corroboración de
versión y release, y resultado explícito `matched | new | needs_review`.
Un match tentativo no puede sobrescribir título/álbum, adjuntar `track_sources`
a otra pista ni vincular un download local. Ruta/hash establecen identidad del
archivo físico; etiquetas internas, duración y datos de álbum se usan para
validar un posible vínculo con catálogo. Sin match confiable se admite archivo
local independiente, no se fuerza sobre homónimo. No mover ni borrar archivos.

Eliminar upserts destructivos de ID por `(track_id,service_id)` en URL/import;
preservar el origen y hacer idempotentes reimportaciones. Revalidar lock en
worker antes de acceder a red. No implementar nuevos proveedores.

### ED-03 — Descubrimiento acotado y selector puro

Inventariar adaptadores Qobuz/Tidal, SongLink, cola, resolver, orchestrator y
fallback, incluyendo rutas sin DB/plan. En búsquedas enumerar páginas hasta
límite documentado y registrar `complete | partial | failed`; `limit=50` en
`qobuz.rs:746,815` no garantiza cobertura. Consultar detalles de API solo
para candidatos plausibles con presupuesto fijo. No inferir recopilación por
número de pistas. Qobuz `QobuzTrack/QobuzAlbum` (`qobuz.rs:59-116`) no
incluye necesariamente release_type/compilation; los valores de
`qobuz.rs:2028,2071` proceden de enriquecimiento posterior. Señal
`album_artist != artist` tampoco es prueba concluyente: colaboraciones y
bandas sonoras tienen excepciones.

Selector de dominio puro, sin red ni SQL, con gates según intención:

- **Item fijado:** comprobar servicio/ID y evidencia disponible. Si hay
  contradicción de título calificado, artista, ISRC o duración: bloquear. Sin
  corroboración suficiente: revisión, no búsqueda que reemplace el lock.
  Fallback de cuenta para el mismo item sigue permitido; cambio de edición
  exige autorización separada del `allow_fallback` de servicio/calidad.
- **Release fijado:** exigir relación item↔release mediante ID verificado.
  Número de pista/disco corrobora solo dentro del **mismo** release. No es
  gate universal entre álbum y single, donde legítimamente cambian.
- **Solo grabación:** ISRC exacto normalizado y además título calificado
  (no `contains` ni quitar "live", "radio edit", "remix", "remaster"), artista
  y duración con tolerancia absoluta+relativa documentada. Falta o conflicto
  relevante => revisión. No igualar silenciosamente versiones distintas.
- **Preferencia de edición:** solo entre versiones ya verificadas, usar
  clasificación positiva de álbum/single y recopilación. Si no hay álbum de
  origen y quedan varias ediciones materialmente distintas, revisión; si hay
  una única candidata verificada con cobertura completa, puede ser
  recopilación. Fecha y calidad anunciada son desempates débiles solo entre
  equivalentes y jamás acreditan versión.
- **Calidad/entitlement:** comprobar disponibilidad, límite de calidad y
  cuenta después de identidad; distinguir calidad anunciada de bytes reales.

### ED-04 — Ejecutar, verificar antes de confirmar y exponer revisión

Resolver devuelve decisión tipada (item/release/versión de política); worker
revalida sin sustituir un lock válido; Qobuz/Tidal ejecutan item fijado o
rechazan. Toda alternativa y reintento pasa otra vez por ED-03, sin bucles
entre la misma combinación item/cuenta/calidad.

Antes de marcar `downloads` completo (`src-tauri/src/worker.rs:1593`) o mover
staging a ruta final, contrastar con evidencia independiente: metadatos recibidos del item/release de API,
ISRC si está realmente disponible, duración **medida** del audio, códec y
tier físico. No verificar un tag escrito desde la petición contra esa misma
petición: sería tautológico. Si faltan pruebas suficientes, estado pendiente
de revisión y ningún vínculo definitivo con catálogo. Registrar candidatos,
razones y procedencia saneadas. UI/cola deben mostrar acción de revisión
manual, no un error genérico ni reintentos automáticos infinitos.

## 5. Orden y pruebas de aceptación

1. **ED-01**: contrato y migración con DB temporal: múltiples items Qobuz/Tidal
   para una grabación, mismo item con dos tracks en conflicto, reimportación
   idempotente y colisión concurrente. Mantener selección previa intacta
   hasta que ED-02/03 estén listos; no activar a medias.
2. **ED-02**: tests de cada importador, local sync, URL y migración: origen
   conservado, ISRC compartido entre releases, homónimos, mismo álbum de
   distinto año/proveedor, reintentos que no reemplazan edición.
3. **ED-03**: fixtures de API paginada y desordenada: álbum vs recopilación
   vs single, live/edit/remix/remaster, ISRC reutilizado, datos ausentes,
   título parcial, cover/soundtrack/collab, duraciones divergentes y posición
   distinta entre releases. Shuffle de orden = mismo veredicto.
4. **ED-04**: integración simulada DB+cola+worker sin red: locks, fallback
   autorizado/no, multi-cuenta, strict quality, caída/reinicio, resultado
   ambiguo y file staging. Mocks no acreditan calidad real: verificarla
   aparte en escritorio con cuentas reales cuando estén disponibles.
5. Gates: `cargo fmt --all -- --check`, `cargo check`, tests SQLx y migración,
   contratos importadores, resolver/worker y UI de revisión. **No está
   terminado** mientras alguna ruta legacy acepte el primer match y persista
   una asociación sin pasar por los gates nuevos.

**No se necesita migración de biblioteca de usuarios ni reorganizar las carpetas
actuales:** la app está en desarrollo y el propietario lo ha excluido. Sí se
necesitan pruebas de compatibilidad del esquema y conflictos para instalaciones
de desarrollo. El éxito es ausencia verificable de contaminación silenciosa,
no contar cuántas carpetas `Various Artists` quedaron.

# Análisis del volcado de MusicBrainz (2026-10-11)

Dump fuente: `ftp.musicbrainz.org/pub/musicbrainz/data/fullexport/20261011-143401/`
Verificado con `SHA256SUMS` oficial.

## Tamaños descargados

| Fichero | Tamaño |
|---|---|
| `mbdump.tar.bz2` (núcleo) | 7,1 GB |
| `mbdump-derived.tar.bz2` (derivado) | 499 MB |

Espacio en disco durante el análisis: ~147 GB libres en `/home`.

## Filas reales por tabla (medido, no estimado)

| Tabla | Filas |
|---|---|
| `isrc` | 6.478.709 |
| `recording` | 40.532.448 |
| `artist` | 3.010.451 |
| `artist_credit_name` | 7.283.230 |
| `release` | 5.844.569 |
| `track` | 17.074.548 |
| `artist_meta` (derivado) | 3.010.451 |
| `artist_tag` (derivado) | 772.344 |
| `recording_meta` (derivado) | 40.532.448 |
| `medium_index` (derivado) | 5.573.725 |

Formato de fila observado (separado por tabulaciones, con `\N` como nulo):
```
isrc:      75478  10  GBAAA9000038  0  2011-05-16 16:08:20.288158+00
recording: 20937085  0f42ab32-...  Travelling Man  2001233  260000  ...
artist:    2252039  fadeb38c-...  Доктор Сатана  Доктор Сатана  \N  ...
```

## Qué consulta Syncify

Rutas reales en `src-tauri/src/services/musicbrainz.rs`:
* ISRC → grabación: `GET /ws/2/recording?query=isrc:XX&fmt=json`
* Búsqueda por artista/título/álbum: `/ws/2/recording`
* Detalle por MBID: `/ws/2/recording/{mbid}?inc=genres+isrcs`
* Artista externo (URLs Spotify/Tidal): `/ws/2/artist/{mbid}?inc=url-rels`
* Lanzamiento por código de barras y por título/artista
* Lanzamiento con pistas: `/ws/2/release/{id}?inc=recordings+artists+labels`

Todas son consultas por clave. Ninguna recorre el catálogo. Eso es lo que hace viable un índice parcial.

## Opciones medidas

### Opción A — Índice de identidades (recomendada)

Importar `isrc` + `recording` + `artist` + `artist_credit_name`. Permitiría resolver ISRC presentes en el volcado, pero aún hay que construir y probar el índice, y validar la paridad de búsqueda por texto.

Filas a importar: ~56 millones. Estimación de tamaño en SQLite con índice:
no medida todavía: faltan importación e índices reales. Se almacenaría en el directorio de datos, no en el paquete.

### Opción B — Solo ISRC y grabación

Importar `isrc` + `recording`. ~47 millones de filas; tamaño real pendiente de medir con el índice construido.

Cubre consultas `lookup_by_isrc` cuando el código esté conectado al índice; las búsquedas por texto seguirían en la API. También hay otras rutas MusicBrainz limitadas a 1 req/s.

### Opción C — Espejo íntegro

Núcleo + derivado + ediciones + editores + eventos + relaciones + estadísticas.
`mbdump-edit.tar.bz2` pesa 16,9 GB, según cabecera HTTP consultada; no se descargó. El tamaño de un espejo íntegro importado no se ha medido.

No se ha demostrado que mejore las rutas actuales. El espejo oficial importa a PostgreSQL; un índice selectivo adaptado a SQLite sería otro producto. La opción íntegra queda pendiente de evaluación técnica, no implementada.

## Decisión

**Propuesta pendiente de implementación:** empezar por ISRC/grabación y medir tiempo, tamaño real y cobertura; añadir artista y `artist_credit_name` solo si esas métricas lo justifican. No se ha creado un importador ni un espejo íntegro.

Notas de diseño que fija esta medición:
* El dump crece dos veces por semana; un índice local necesita refresco, no
  reconstrucción, por lo que conviene registrar la fecha del dump importado.
* La ingesta debe ser un proceso que lea el `.tar.bz2` en streaming, sin
  descomprimirlo entero a disco.
* La licencia: el núcleo es CC0, pero los datos derivados (tags, géneros) son
  CC BY-NC-SA y **no** deben redistribuirse con la app.

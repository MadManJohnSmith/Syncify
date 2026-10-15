# Prueba de rendimiento aislada de MusicBrainz (2026-10-12)

**Alcance:** se copió la base SQLite de `~/.local/share/com.syncify.app/syncify.db` a
`/home/alan/perf-probe/probe.db` antes de ensayar cambios. No se modificó la base viva, el
código, las migraciones ni los ficheros del workflow de descargas. El otro workflow puede
seguir usando el mismo `target`; esta prueba solo usó Python `sqlite3` y copias locales.

## Resultado reproducido

El predicado de pertenencia de MusicBrainz L3:

```sql
SELECT 1 FROM tracks t
WHERE UPPER(REPLACE(REPLACE(TRIM(t.isrc), '-', ''), ' ', '')) = ?
LIMIT 1;
```

no usa un índice de búsqueda. `EXPLAIN QUERY PLAN` del workflow mostró `SCAN t EXISTS
USING COVERING INDEX idx_tracks_isrc`. En una copia de la base real (30.527 pistas):

| Prueba (250 ISRC) | Sin índice de expresión, mediana / p95 | Con índice de expresión, mediana / p95 |
|---|---:|---:|
| `EXISTS` con ISRC presente | 0,0746 / 0,1259 ms | 0,0049 / 0,0053 ms |
| `EXISTS` con ISRC ausente | **11,5366 / 13,3684 ms** | **0,0043 / 0,0047 ms** |
| UPSERT L3 real, ISRC ausente | **13,0179 / 15,4024 ms** | **0,0098 / 0,0112 ms** |

Una repetición independiente de 2.000 ISRC ausentes, con caché SQLite de 2 MiB,
dio mediana **12,0639 ms** y p95 **13,6605 ms** sin índice. El índice temporal de
expresión ocupó **596 KiB** en la copia; su coste de mantenimiento en otras escrituras
a `tracks` no se midió.

El índice probado **solo en la copia** fue:

```sql
CREATE INDEX probe_idx_norm_isrc ON tracks(
  UPPER(REPLACE(REPLACE(TRIM(isrc), '-', ''), ' ', ''))
);
```

Como control, 200 UPSERT L2 de payload real sin conflicto externo, cada uno con
`commit` independiente, dieron mediana 0,892 ms, p95 1,476 ms, máximo 5,618 ms;
200 UPSERT L3 sin predicado `EXISTS` dieron mediana 0,969 ms, p95 1,983 ms,
máximo 6,977 ms. Esto no reproduce los segundos del log SQLx: **el tiempo alto
del registro sigue sin atribución causal**. Podría incluir espera por otro escritor,
checkpoint o recursos del sistema. Tampoco se midió el coste del `EXISTS` en un
flujo concurrente real.

## Implicaciones

- El escaneo de `EXISTS` sí es un coste aislado y reproducido: los ISRC ausentes
  obligan a examinar todo el índice de `tracks`. La opción del índice de expresión
  parece prometedora, pero requiere migración nueva, pruebas de igualdad semántica
  con el ISRC normalizado y medición de coste de mantenimiento. No se implementó.
- La lectura L3 cuando `mb_library_index` no contiene ese ISRC corta por la PK,
  por lo que no ejecuta el `EXISTS`; no se debe extrapolar los 12 ms a todos los
  fallos L3.
- Los 98 y 32 slow statements de caché del workflow son observaciones del log
  sobre el umbral de 1 s, no medias de todas las operaciones ni prueba de que la
  caché sea el origen de la espera. Esta prueba lo confirma: aislados, los UPSERT
  están por debajo de 2 ms en p95.

## Reproducción

Los scripts de la prueba están fuera del repositorio en `/home/alan/perf-probe/`:
`bench.py`, `bench_hit.py` y `bench_exists.py`; las copias probadas son
`probe-no-index.db` y `probe-with-index.db`. El informe general, con 23 hallazgos
confirmados, está en `docs/perf-run-2026-10-12.md`.

## Ensayos adicionales sin interferir con el workflow de descargas

Todas las escrituras de prueba se hicieron exclusivamente en copias bajo
`/home/alan/perf-probe/`. No hubo compilación ni cambios en `target`.

### Mantenimiento del índice al actualizar pistas

Con copias nuevas de la base viva y orden alterno entre ambas variantes, se
hicieron 250 `UPDATE tracks SET isrc=? WHERE id=?` reales con `commit` por
operación en cada copia. Esta prueba sustituye el ensayo inicial de
`SET columna=columna`, que solo descartaba regresión macroscópica:

| Copia | Mediana | p95 | Total |
|---|---:|---:|---:|
| Sin índice de expresión (ronda 1) | 1,00 ms | 1,55 ms | 276,0 ms |
| Con índice (ronda 1) | 1,067 ms | 2,281 ms | 322,8 ms |
| Con índice (ronda 2) | 1,086 ms | 1,947 ms | 311,3 ms |
| Sin índice (ronda 2) | 1,059 ms | 1,521 ms | 291,4 ms |

La mediana con índice y sin índice se superpasa entre rondas, y el total difiere
unos 11%; **este ensayo no permite afirmar que el índice tenga un coste
despreciable**. Para medirlo haría falta muchas más rondas, con la misma
clase y frecuencia de escritura que ve el worker, que no conozco. Lo que sí
muestra el orden alterno es que no hay un efecto macroscópico visible.

### Consultas que sostienen sondeos

En la base viva abierta con URI `mode=ro` (5 ejecuciones de cada SQL):

| Consulta | Mediana | Máximo observado |
|---|---:|---:|
| Página simplificada de `get_library` (100 filas) | 1,297 ms | 77,3 ms |
| Misma página solo con JOIN a albums | 0,127 ms | 0,266 ms |
| Selección de la próxima pista del worker (1 fila) | 3,412 ms | 63,252 ms |
| Contadores base de cola | 0,009 ms | 0,726 ms |
| Tres COUNT de estado del enriquecimiento | 6,346 ms | 14,165 ms |

La consulta de biblioteca es **una proyección reducida**, no la sentencia completa
de producción; las diferencias entre ambas no deben interpretarse como una mejora
medida del comando `get_library`. Al repetir 12 veces por tamaño de caché SQLite
(2, 64 y 256 MiB), la consulta del worker se estabilizó entre 2,3 y 2,7 ms de
mediana y el estado de enriquecimiento entre 5,9 y 7,2 ms. Los máximos de 63 y
77 ms de la primera pasada no se reprodujeron: probablemente son arranque frío,
pero no está demostrada su causa.

### Coste de JS y escaneo de almacenamiento

En Node 26, con objetos **sintéticos** que imitan el comparador de `LibraryView.vue`,
40 iteraciones tras cinco de calentamiento:

| Pistas cargadas | Copia + sort con comparador 0 (`dateAdded`) | Sort por título | Filtro + sort por título |
|---|---:|---:|---:|
| 100 | 0,005 ms | 0,017 ms | 0,021 ms |
| 5.000 | 0,101 ms | 0,297 ms | 0,438 ms |
| 30.000 | 0,546 ms | 1,810 ms | 2,702 ms |

Eso mide V8, no WebKit ni render DOM. El coste crece con las pistas cargadas,
pero no justifica llamar a la ordenación, por sí sola, la causa de lag visible.

El directorio de descargas configurado en la base (`/home/alan/Music/Syncify`)
contiene 29 archivos. Un recorrido con `os.walk` y `stat` repetido 100 veces dio
mediana 0,253 ms, p95 0,347 ms. El comando Rust de `StatusBar.vue` usa `WalkDir`
y `sysinfo::Disks`, por lo que el cronometraje Python no mide el comando entero;
sí descarta que los **29 archivos** sean una exploración masiva. Es un candidato
de bajo impacto, a revisar solo si cambia la raíz o crece la carpeta real.

### Estado de la app viva

Al comenzar estas pruebas no había un proceso `syncify-tauri` ni `WebKitWebProcess`
activo: las métricas de CPU/swap de una sesión anterior no se pudieron reproducir.
No se inició otra instancia para no compartir recursos con el workflow concurrente.

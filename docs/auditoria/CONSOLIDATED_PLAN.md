# Plan de implementación consolidado — Syncify

> **Errata (2026-10-25):** plan histórico ya ejecutado en lo esencial (A-0/A-1/A-2/A-4a
> y B-1 aplicados; B-4 superado por el throttle de PR-01/PR-02; B-5 aplicado). La
> referencia a `/home/alan/Documents/Syncify/Syncify` y al commit `5036217` del §0 ya
> no resuelve: el repo vive en la raíz y el commit no está en la historia alcanzable.
> Su valor residual es la lista de «Rechazados y por qué» (§4) y el registro de las
> decisiones de clasificador de auth que Track A implementó (`AuthVerdict`, guard de
> transporte, decay del latch en `src-tauri/src/worker.rs`).

Consolida la auditoría de sesión OAuth (A) y la de rendimiento (B) sobre el **mismo**
repo. Deduplicadas, priorizadas y con un hallazgo de B **corregido**: la reescritura
de favoritos tal como estaba descrita es más lenta que la original.

**Repo:** `/home/alan/Documents/Syncify/Syncify` (el cwd de las tools es el directorio
padre; el repo real es el subdirectorio `Syncify/`). Rutas relativas a ese repo.

---

## 0. Corrección de partida: la premisa de la Auditoría A está desfasada

La Auditoría A afirma que el árbol de trabajo tiene arreglos **sin commitear** y que
`real_credential_rejection_still_marks_session_dead` **falla**. Verifiqué el estado
real antes de planificar nada:

```
$ git status --porcelain          # en Syncify/Syncify
(sin salida — árbol limpio)

$ git log --oneline -3
5036217 fix(auth): un corte de red invalidaba la sesión y obligaba a re-loguear
```

Los arreglos están **commiteados** en `5036217`, no pendientes. Y el test que A reporta
rojo pasa:

```
$ cargo test --manifest-path src-tauri/Cargo.toml --lib
test result: ok. 272 passed; 0 failed; 0 ignored; 0 measured; 266 filtered out
   ... worker::session_auth_tests::real_credential_rejection_still_marks_session_dead ... ok
   ... worker::session_auth_tests::transport_failure_never_marks_session_dead ... ok
   ... worker::session_auth_tests::invalid_grant_survives_transport_guard ... ok
   ... worker::session_auth_tests::transport_markers_are_detected ... ok
```

Verifiqué el código instalado: `classify_session_auth_failure`
(`src-tauri/src/worker.rs:26`) ya contiene `e.contains("401")`, `e.contains("403")` y
`e.contains("authentication failed")` en `worker.rs:44-50`, y el guard de transporte
corre **antes** en `worker.rs:33`.

**Consecuencia:** los hallazgos A#4 (clasificador) y A#2 parcial quedan **cerrados** y
salen del plan. No los reperformo. El resto de A (Deezer, Apple Music, latch) **sigue
vivo** — lo verifiqué por código, línea a línea.

Suite frontend, línea base antes de tocar nada:

```
$ cd ui && npx vitest run --reporter=dot
Test Files  86 passed (86)
     Tests  698 passed (698)
  Duration  12.67s
```

---

## 1. Hallazgos unificados y deduplicados

Estado: `V` = verificado por mí en esta sesión. Descartado = no entra al plan.

| # | Hallazgo | Ubicación | Estado | Severidad |
|---|---|---|---|---|
| **A1** | **Deezer invalida por cualquier fallo de `init()`** — 4 de 5 salidas son transporte/parse (`.send()` falla, leer body, status no-2xx, JSON inválido), no rechazo de credenciales. Un 503 o un corte de wifi deja `credentials_invalid = 1` para siempre. El comentario en el caller afirma "explicit provider rejection": es falso. | `src-tauri/src/commands/service.rs:5937-5939` + `src-tauri/src/services/deezer.rs:169,174,182-186,190-196,204-209` | **V** | **Crítica** |
| **A2** | **Latch sin decay.** `is_service_available()` es lectura pura de BD; sin re-verificación, una vez escrito el flag solo lo borran login/logout/refresh exitosos. Sin ruta de autocuración por tiempo. | `src-tauri/src/download/orchestrator.rs:837` (gate), resets en `auth.rs:683,703,1247`, `tidal_pipeline.rs:623`, `spotify.rs:514,522`, `main.rs:495,556` | **V** | **Alta** |
| **A3** | **Apple Music convierte cualquier ≠200 y toda excepción en "Token expired".** 401, 403, **429, 500, 503** y timeout/DNS caen en el mismo `else` → `False` → `"Token expired"`. No escribe en BD, pero alimenta el estado de UI. | `scripts/services/apple_music_auth.py:186-188` y `:190-192`; consumidor `get_status` en `:345-368` | **V** | **Alta** |
| **A4** | **`validate_all_sessions` reporta `valid:false` por fallo de subproceso** y pone `connected: true` incondicionalmente, incluso con `valid: false`. La UI ve una sesión no válida pintada como conectada. | `src-tauri/src/commands/auth.rs:785-786` (match `Err`) y `:794` (`connected: true`) | **V** | Media |
| **A5** | **`TidalGuiCredentials::is_expired` devuelve `true` sin expiry** si hay `refresh_token` → refresh en cada llamada. Mitigado aguas abajo por `tidal_pipeline.rs:686-690`. | `crates/syncify-tidal-downloader/src/lib.rs:114-118` | **V** | Baja |
| **B1** | **`get_library` escanea la tabla entera por página.** `SCAN t` + `TEMP B-TREE FOR ORDER BY`: los 6 LEFT JOIN + `GROUP BY` impiden ordenar por índice. La reescritura de dos fases lo acota a las 50 filas pedidas. | `src-tauri/src/commands/library.rs:241-251` (también `:376-392`) | **V + medido** | **Alta** |
| **B2** | **Favoritos: mismo escaneo completo.** Reescribible igual, **pero** solo si se elimina el `WHERE t.is_favorite=1` exterior (ver §3). | `src-tauri/src/commands/library.rs:1846-1850` | **V + medido** | **Alta** |
| **B3** | **Doble spawn de Python en cada `fetchData()` del Dashboard.** `fetchData()` llama `fetchSystemDiagnostics()`, que a su vez lanza `checkFfmpeg()` + `checkFingerprint()` en paralelo — 2 procesos por carga. | `ui/src/views/DashboardView.vue:688` → `:642-649` | **V + medido** | Media |
| **B4** | **`handleScroll` sin throttle** en el contenedor de scroll; cada evento fuerza reflow síncrono. `loadMore()` ya es idempotente. | `ui/src/views/LibraryView.vue:1372-1380` (bind en `:500`) | **V** | Baja |
| **B5** | **`LibraryView` renderiza toda la lista sin virtualizar**; `DownloadsView` ya tiene el patrón de ventana en el mismo repo. | `ui/src/views/LibraryView.vue:502` vs `ui/src/views/DownloadsView.vue:1719-1739` | **V** | Media (riesgosa) |
| — | Clasificador `classify_session_auth_failure` no invalida rechazos reales | `worker.rs:26-50` | **V — ya arreglado en `5036217`** | — |
| — | Caché con TTL de `crypto::decrypt` | — | **Rechazado** (§4) | — |
| — | Índices nuevos en `tracks` | — | **Rechazado** (§4) | — |
| — | Cachear `get_metadata_stats` | `metadata.rs:275-289` | **Rechazado** (§4) | — |
| — | `qobuz_auth.get_status` lee estado en memoria | `scripts/services/qobuz_auth.py` | **No auditado por mí** | Fuera |
| — | `get_artist_detail` awaits secuenciales | `library.rs:622-643` | **Rechazado** (§4) | — |

### Mediciones que ejecuté (no heredadas)

Reconstruí el esquema real con los 83 ficheros de `src-tauri/migrations/*.sql`
(0 errores, 48 tablas) y lo poblé con 300 000 tracks / 100 000 favoritos.

| Consulta | Antes | Después | ¿Filas idénticas? |
|---|---|---|---|
| `get_library` LIMIT 50 OFFSET 0 | 161 / 156 / 158 ms | 7 / 8 / 7 ms | `diff` → **IDENTICAL** |
| `get_library` OFFSET 100 000 | 728 ms | 10 ms | `diff` → **IDENTICAL** |
| Favoritos LIMIT 50 OFFSET 0 | 162 / 168 / 170 ms | 44 / 43 / 45 ms | `diff` → **IDENTICAL** |
| Favoritos OFFSET 200 | — | — | `diff` → **IDENTICAL** |
| `conversion_bridge.py check` | 114 / 103 / 103 ms | — | — |
| `fingerprint_bridge.py check` | 86 / 84 ms | — | — |

También forcé empates de título (`UPDATE tracks SET title='SAME' WHERE id%7=0`) y el
`diff` siguió dando idéntico: el `id ASC` del sub-select es un desempate estable.

Caveat honesto: mi dataset sintético tiene las tablas de join vacías, así que mi
ganancia en `get_library` es ~22×, no los ~72× que reporta B. La **dirección** y el
cambio de plan de ejecución se confirman; la magnitud absoluta depende de los datos.

---

## 2. TRACK A — Sesión OAuth

**Invariante que se respeta en todos los pasos:**
> Un rechazo real (401/403/400 del proveedor, `invalid_grant`) **SÍ** invalida.
> Un fallo de transporte (timeout, DNS, 5xx, 429, error de parseo) **NUNCA** invalida.

### A-0. Estado de tres valores (base del track) — *no cambia comportamiento observable*

Hoy el veredicto es implícito: no escribir en BD = "válida", escribir = "muerta". Eso
obliga a que **cada** llamador tenga su propia idea de qué es un rechazo. La fuente de
los bugs A1/A3 es exactamente esa ambigüedad.

Añadir en `src-tauri/src/worker.rs`, junto a `is_transport_failure` (`:59`):

```rust
pub enum AuthVerdict { Revoked, Valid, UnverifiedTransient(String) }
```

`Revoked` solo para evidencia de rechazo explícita; `UnverifiedTransient` para "no se
pudo comprobar" (5xx, timeout, parseo, subproceso caído).

**Refactorizar DENTRO de `classify_session_auth_failure`, conservando su firma
`bool`** → no cambia comportamiento: hoy `true` = revocado, `false` = no revocado. Los
tests de `worker.rs:1854-1896` siguen siendo el contrato. Añadir
`pub fn classify_auth_verdict(&str) -> AuthVerdict` y hacer el `bool` un wrapper. Esto
da el estado de tres valores a los nuevos callers sin romper a los 12 existentes.

### A-1. Deezer — *no cambia comportamiento para rechazos reales*

`service.rs:5937-5939` es el bug crítico. Tipar la salida de `init()` en
`src-tauri/src/services/deezer.rs`:

- `deezer.rs:169` (`.send()`), `:174` (leer body), `:190-196` (parseo) → `Transient`
- `deezer.rs:182-186` (`!status.is_success()`) → `Transient` **salvo** 401/403
- `deezer.rs:204-209` (`api_token.is_none()`, "ARL may be invalid") → `Revoked`

```rust
// services/deezer.rs
pub enum DeezerInitError { Rejected(String), Transient(String) }
// init() -> Result<(), DeezerInitError>
```

En `commands/service.rs:5938`:

```rust
match client.init().await {
    Err(DeezerInitError::Rejected(e)) => {
        let _ = mark_account_credentials_invalid(db, "deezer", &e).await;
        let err_msg = format!("RequiresAuth: Deezer session rejected ({})", e);
        emit(SyncProgressEvent::requires_auth(&service_normalized, Some(account_id), &err_msg));
        return Err(err_msg);
    }
    Err(DeezerInitError::Transient(e)) => {
        return Err(format!("Deezer sync failed (transient, session intact): {}", e));
    }
    Ok(()) => {}
}
```

**Comportamiento que cambia:** un 503 de Deezer deja de invalidar la cuenta. Ese es
exactamente el objetivo. Un 401 o un ARL inválido real siguen invalidando igual. El
segundo call-site, `service.rs:1488-1494`, ya **no invalida** (solo loguea) → no tocar.

Tests: los 3 rechazos reales invalidan; timeout / 503 / 429 / JSON inválido no.

### A-2. Apple Music — *cambia comportamiento: deja de mentir la UI*

`scripts/services/apple_music_auth.py:186-188` (el `else` captura cualquier status) y
`:190-192` (la excepción). Convertir a tri-estado `Optional[bool]`:

```python
if response.status_code == 200:
    ...
    return True, storefront
if response.status_code in (401, 403):
    return False, f"Invalid token (HTTP {response.status_code})"   # rechazo real
return None, f"Could not verify (upstream HTTP {response.status_code})"  # no verificable
# except:  return None, f"Could not verify (network: {e})"
```

En `get_status` (`:345-368`), tratar `None` como **no concluyente**, nunca como
"expirada": `{"connected": True, "status": "Could not verify (network)", "storefront": None}`.
Devolver `connected: True` en el caso no verificable es lo correcto aquí: la credencial
sigue guardada y el estado de la BD no cambió; afirmar "expirada" es lo que produce el
síntoma. Documentar la decisión en un comentario.

**Comportamiento que cambia:** la UI deja de mostrar "Token expired" ante un 503 o un
timeout. Antes mentía; ahora dice que no pudo comprobar.

### A-3. `validate_all_sessions` — *cambia comportamiento: tri-estado real*

`commands/auth.rs:785-786` y `:794`. Separar tres casos: sesión válida / sesión
rechazada / **no verificable** (subproceso caído, `Err(e)`). Sacar `connected: true`
incondicional de `:794` y derivarlo del veredicto. `Err(e)` de un spawn fallido no
debe leerse como sesión inválida — es la misma clase de bug que A2.

### A-4. Decay del latch — *el que más superficie toca; hacer último y por partes*

`download/orchestrator.rs:828-846` es una lectura pura de BD con el gate de `:837`. Dos
opciones, **no ambas**:

- **4a (mínima, recomendada).** El gate sigue, pero un rechazo re-verificable abre la
  puerta: añadir el criterio de edad usando `last_auth_error_at`, que **ya existe**
  (`src-tauri/migrations/0055_account_auth_telemetry_and_dedupe.sql:4`). Si el flag es
  más viejo que 24 h, no negar el servicio; que la descarga falle sola con su 401 real
  y `classify_session_auth_failure` (`worker.rs:26`) decida. **No cambia el
  comportamiento de un rechazo recién ocurrido** — solo evita que un veredicto viejo
  bloquee para siempre. Necesita migración solo si se quiere el índice; la columna ya
  está.
- **4b (más amplia).** Borrar el gate y confiar en el 401 del proveedor. Rechazo
  explícitamente: degrada el mensaje de error y reescribe la semántica de A1/A2. No la
  recomiendo como primer movimiento.

### A-5. Spotify — *cierre, no trabajo nuevo*

Verificado: el refresh proactivo ya existe con buffer de 300 s
(`src-tauri/src/services/spotify.rs:485-486`, `expires_at = now + expires_in` en
`:497`, persiste token y refresco rotado en `:514`) y un `Err` explícito cuando no hay
`refresh_token` (`:530-548`). **No hay trabajo pendiente.** Lo incluyo solo para dejar
constancia de que ambas auditorías lo señalaban como faltante y no lo está.

### A-6. `is_expired` de Tidal — *opcional, bajo valor*

`crates/syncify-tidal-downloader/src/lib.rs:114-118`: sin expiry pero con
`refresh_token` devuelve `true`. Está mitigado aguas abajo
(`tidal_pipeline.rs:686-690`). **Dejarlo** salvo que se vea refresh masivo en logs;
cambiarlo toca un crate compartido por mucho Benefiticio pequeño.

---

## 3. TRACK B — Rendimiento

Regla: misma salida, mismos datos, solo menos trabajo repetido.

### B-1. `get_library` dos fases — **SEGURO**, verificado

`src-tauri/src/commands/library.rs:241-251`. Sustituir:

```sql
FROM tracks t
... (6 LEFT JOIN) ...
GROUP BY t.id
ORDER BY t.title ASC
LIMIT ? OFFSET ?
```

por:

```sql
FROM tracks t
JOIN (SELECT id FROM tracks ORDER BY title, id LIMIT ? OFFSET ?) p ON p.id = t.id
... (6 LEFT JOIN, idénticos) ...
GROUP BY t.id
ORDER BY t.title ASC
```

Plan medido: `SCAN t` (300k) → `CO-ROUTINE p → SCAN tracks USING COVERING INDEX
idx_tracks_title`. Los índices **ya existen** (`0002_normalize_schema.sql:97`,
`0020_favorites.sql:3`, `0044_add_favorite_timestamps.sql:3`) → **sin migración**.

**No cambia comportamiento** — demostrado con `diff` byte a byte en offset 0 y 100 000,
incluyendo el caso de empates de título. Aplicar también a `:376-392` (misma forma).

### B-2. Favoritos dos fases — **SEGURO, pero distinto al propuesto**

`library.rs:1846-1850`. Aquí la reescritura **tal como la describe la auditoría B es
más lenta que la original**: conservar el `WHERE t.is_favorite=1` exterior cambia el plan
a `SEARCH t USING INDEX idx_tracks_favorite (is_favorite=?)` en vez de la búsqueda por
rowid, y la consulta no terminó en 4+ minutos. Eliminar ese `WHERE` exterior — el
sub-select ya garantiza `is_favorite = 1` — corrige el plan:

```sql
FROM tracks t
JOIN (SELECT id FROM tracks WHERE is_favorite = 1
      ORDER BY favorite_at DESC, title ASC, id ASC LIMIT ? OFFSET ?) p ON p.id = t.id
... (6 LEFT JOIN) ...
GROUP BY t.id
ORDER BY t.favorite_at DESC, t.title ASC
```

170 ms → 44 ms en mi medición, `diff` idéntico en offset 0 y 200. **Aplicar la forma
corregida, no la de la auditoría.**

### B-3. Deduplicar diagnósticos en vuelo — **SEGURO con nota**

`ui/src/views/DashboardView.vue:688` → `:642-649`. Un `Promise` en vuelo compartido en
`ui/src/api/tools.ts` para que `checkFfmpeg()` y `checkFingerprint()` compartan una
sola invocación de los dos bridges, limpiado en `.finally()`. Ahorra ~190 ms por carga
(medido). Solo coalesce llamadas **concurrentes**; una invocación posterior vuelve a
comprobar de verdad, así que nunca hay resultado caducado.

**No cambia comportamiento.** Riesgo residual: si el usuario instala ffmpeg mientras
las dos comprobaciones están en vuelo, la UI puede pintar el estado viejo unos
milisegundos — la misma ventana que ya existe entre las dos llamadas paralelas actuales.

### B-4. Debounce en `handleScroll` — **SEGURO**

`ui/src/views/LibraryView.vue:1372-1380`, con el patrón ya usado en el repo
(`PathSelector.vue`, `CommandPalette.vue`). `loadMore()` ya es idempotente
(`:1351`), así que agrupar disparos no cambia el conjunto final de páginas.

**No cambia comportamiento.**

### B-5. Virtualizar `LibraryView` — **SEGURO conceptualmente, pero rompe tests: NO lo apliques sin tocar los tests**

`LibraryView.vue:502` renderiza `filteredTracks` entero; el patrón de ventana ya existe
en `DownloadsView.vue:1719-1739`.

**Riesgo concreto y verificado:** `ui/src/__tests__/views/LibraryView.spec.ts:461-468`
hace aserciones **posicionales** (`rows[0]`, `rows[1]`) sobre `wrapper.findAll('.track-row')`.
Cualquier ventana de render cambia el DOM observable y esos asserts fallan. Audit B ya
lo señaló; lo confirmo leyendo el spec. Además `index % 2` (`:511`) pinta las filas en
cebra por posición — con ventana hay que conservar el índice **global** o la zebra
parpadea al hacer scroll.

Dejarlo fuera de este lote. Es una mejora real pero exige revisar los asserts, y el
beneficio (≈56 nodos/fila) no compensa el riesgo de regresión visual en este cambio.

---

## 4. Riesgos rechazados y por qué

| Rechazado | Por qué |
|---|---|
| **Índices nuevos en `tracks`** | El `GROUP BY t.id` fuerza el escaneo completo sea cual sea el índice. Audit B lo midió (186→186 ms, 66→64 ms) y lo descarto. B-1/B-2 atacan la causa correcta sin índices nuevos. |
| **Caché con TTL de `crypto::decrypt`** | Microbenchmark de Audit B: 0,48–6,26 µs por operación. Peor caso realista ≈30 µs por carga. La clave ya está en `OnceLock` (`crypto.rs:16`). Añade invalidación y ventana de credenciales obsoletas a cambio de nada. |
| **Cachear `get_metadata_stats`** (`metadata.rs:275-289`) | Cambia el dato mostrado entre cargas (p. ej. tras un enriquecimiento). Los 70 ms son reales pero el dato debe ser fresco. |
| **Paralelizar los awaits de `get_artist_detail`** (`library.rs:622-643`) | Cambia el efecto observable: hoy un fallo de `appearances` no impide que corra `albums`; en paralelo sí. |
| **Borrar el gate `orchestrator.rs:837` por completo** (A-4b) | Degrada el mensaje de error y reescribe la semántica de A1/A2. Se queda con la variante 4a (decay por edad). |
| **Virtualizar `LibraryView` ahora** (B-5) | Rompe aserciones posicionales en `LibraryView.spec.ts:461-468` y la zebra de `index % 2`. |
| **Rehacer A4/clasificador** | Ya commiteado en `5036217`; los 272 tests pasan. Rehacerlo es trabajo duplicado. |

---

## 5. Orden de ejecución

**Track A** (cada paso gateado por `cargo test --manifest-path src-tauri/Cargo.toml --lib`):
1. A-0 `AuthVerdict` (interno, sin cambios de comportamiento)
2. A-1 Deezer — *crítico, primer paso de valor real*
3. A-2 Apple Music
4. A-3 `validate_all_sessions`
5. A-4 decay del latch (A-4a) — *el que más toca, va último*
6. A-5/A-6 cierre, sin trabajo

**Track B** (independiente de A, gateado por `cd ui && npx vitest run`):
1. B-1 `get_library` (`:241-251` y `:376-392`)
2. B-2 favoritos — **con la forma corregida, sin el `WHERE` exterior**
3. B-4 debounce de scroll
4. B-3 deduplicación de diagnósticos
5. B-5 virtualización — *otro cambio, con sus tests*

A y B no se solapan en archivos: A toca `worker.rs`, `deezer.rs`, `service.rs`,
`auth.rs`, `orchestrator.rs`, `apple_music_auth.py`; B toca `library.rs`, `metadata.rs`
(no) y `ui/src/`. Se pueden hacer en paralelo.

---

## 6. Lo que NO verifiqué

- **No ejecuté la app ni reprodujo el síntoma** contra un proveedor real. A1, A2 y A3
  están verificados por lectura de código y por las measurements de SQL, no de extremo
  a extremo con Deezer/Apple Music reales.
- **No compilé el backend completo** (`cargo build`). Ejecuté `cargo test --lib`, que sí
  compila la librería y pasa.
- **No leí completos** `src-tauri/src/services/apple_music.rs` (49 KB) ni los 83
  ficheros de migración más allá de los que cito. Un trigger SQL de invalidación podría
  existir y no lo descarto.
- **No verifiqué** `scripts/qobuz_auth.py` (`get_status` leyendo estado en memoria);
  lo excluí por no haberlo comprobado.
- **Mi dataset de SQL es sintético** (tablas de join vacías): los planes de ejecución y
  la equivalencia de filas son válidos, pero los múltiplos absolutos de latencia no
  necesariamente transfieren a la base real.

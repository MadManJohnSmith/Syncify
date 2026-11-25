# DLX-05 — Deduplicar la portada que se repite

**Issue:** [#46](https://github.com/MadManJohnSmith/Syncify/issues/46)  
**Rama de evidencia:** `feat/ola2-46-dlx05-dedupe`  
**Medición:** 2026-11-25, offline, sin cuentas ni red reales.

## Recurso medido

La línea base de `src-tauri/tests/dlx05_cover_dedupe_test.rs` reproduce el
`reqwest::get` por pista que existía en Qobuz, Tidal y el gancho de entrega MP3.
Con una portada PNG sintética de 70 bytes y un servidor local, dos y doce pistas
concurrentes hacen una petición por pista. El recurso repetido es, por tanto, la
**portada estática del álbum**, no el audio ni los metadatos de cada pista.

La deduplicación cubre esas mismas rutas: Qobuz FLAC, Tidal FLAC/M4A y su
recuperación de sidecar, y `DownloadOrchestrator::tag_mp3_delivery`. La portada
animada y las reparaciones ajenas al flujo de descarga no entran en esta caché.

## Identidad y límites

`download::cover_art::CoverIdentity` solo se construye cuando están disponibles
el proveedor, la cuenta y la edición del álbum. La clave combina:

- URL canónica: se normalizan esquema/host/puerto por defecto y se quita el
  fragmento; la query se conserva porque puede seleccionar tamaño o acceso;
- proveedor y cuenta, para no cruzar contextos de acceso;
- id de edición del álbum, para no mezclar portadas de ediciones distintas.

Si falta identidad, la URL contiene user-info o es inválida, se hace fetch sin
caché. Los errores y los payloads vacíos no se cachean. La caché es de proceso,
con TTL de 10 minutos, límite de 64 entradas y 32 MiB, máximo de 64 vuelos en
curso y máximo de 16 MiB por payload. Un vuelo compartido no pertenece a la
cancelación de un waiter: cancelar una llamada no aborta a las demás.

Los valores son `Arc<[u8]>` inmutables. Cada adaptador conserva su propio staging
y escribe su sidecar; no se comparte un fichero mutable entre pistas.

## Antes/después

Corrida ilustrativa del test, no una promesa de latencia:

| Pistas | Peticiones antes → después | Bytes de red antes → después | Reutilizaciones | Bytes entregados |
|---:|---:|---:|---:|---:|
| 2 | 2 → 1 | 140 → 70 | 1 | 140 |
| 12 | 12 → 1 | 840 → 70 | 11 | 840 |

El ahorro determinista es del 50 % de peticiones/bytes con dos pistas y del
91,7 % con doce. Los milisegundos se imprimen para inspección, pero no se usan
como garantía: el servidor local y el scheduler introducen ruido.

## Pruebas

- `dlx05_cover_dedupe_test`: baseline, ahorro 2/12, bytes idénticos y staging
  independiente por pista.
- `download::cover_art::tests`: aislamiento por proveedor/cuenta/edición/URL,
  identidad ambigua, URL canónica, fallos HTTP y vacíos, cancelación, TTL, LRU,
  presupuesto de bytes y payload sobredimensionado.

Reproducción local, desde la raíz y con el `GATE` del entorno:

```bash
"$GATE" nextest run --locked --test dlx05_cover_dedupe_test --no-capture
"$GATE" nextest run --locked -p syncify-tauri --lib cover_art --no-capture
```

## Límites

La evidencia es offline. No demuestra latencia de proveedores reales, GUI,
Windows ni descargas de punta a punta. Tampoco cambia selección de
identidad/edición, calidad, migraciones, IPC ni la resolución de portada animada.

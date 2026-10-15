Syncify v0.6.1 — fiabilidad de la cola de descargas

## Novedades

### 🧭 Cancelación de descargas en vuelo
Cancelar una descarga mientras la transferencia está en marcha la deja en `cancelled`.
Antes `mark_complete` reescribía la fila por ID sin comprobar el estado, así que el
elemento cancelado terminaba como `complete` en la cola y en la biblioteca.
La promoción a `complete` ahora exige que el worker siga siendo el dueño de la fila
como `downloading`: se comprueba el estado antes y se compara `rows_affected`
después, de modo que una cancelación o cualquier otra transición terminal que
llegue durante la descarga gana.

### 🔁 Rechazo de cuenta persistido
El rechazo de credenciales es un veredicto sobre la **cuenta**, no sobre el
intento. Cuando otra cuenta del mismo servicio completaba la pista, el rechazo de
la primera se perdía y la cuenta fallida volvía a colocarse delante en la siguiente
pista del mismo proveedor, repitiendo el fallo. Ahora la anotación se persiste
incluso cuando la descarga tuvo éxito, acotada a la cuenta y el servicio
intentados, sin reescribir una anotación previa.

### 📦 Resto del trabajo acumulado
Se integran las correcciones pre-beta de importación, selección de edición,
identidad de pista, letras y metadatos: las migraciones 0097-0100, el bloqueo de
fuente (`locked_track_source_id`), la separación de edición por ISRC y los tests
`url_import_atomic_edition_test` e `edition_selector`, entre otros.

## Lo que esta versión **no** afirma

- **No hay medición de tasa de éxito válido sobre cuentas reales.** Todas las
  verificaciones de esta versión son offline y con datos sintéticos. El porcentaje
  real de descargas completadas sigue sin ejercicio.
- No se relajó la política de calidad, ni la identidad de edición, ni el techo
  solicitado para subir ningún porcentaje.
- La selección de edición `ED-03`/`ED-04` permanece sin activarse de forma general.
- No se verificó GUI nativa, ni audio real de Tidal, ni la app instalada.

## Verificación del candidato

| Gate | Resultado |
|---|---|
| `cargo test --workspace --locked` | 2620 pasan, 0 fallan |
| `cargo check --workspace --all-targets --locked` | pasa |
| `cargo clippy --workspace --all-targets --locked -- -D warnings` | sin warnings |
| `cargo fmt --all -- --check` | pasa |
| `cargo test --test batch_50_audit_test -- --test-threads=1` | 2/2 |
| `npm run test:run` (UI) | 1041 pasan |
| `python -m unittest discover -s tests -p 'test_*.py'` | 281 pasan (2 saltados) |
| Coherencia documental y claims del README | 22/22 |

Tests nuevos en `src-tauri/tests/queue_state_transition_test.rs`, sobre migraciones
reales: cancelación en vuelo, promoción única de la fila propia y rechazo persistido
solo en la cuenta correcta sin tocar otras cuentas ni otros servicios.

Los tests se ejecutaron con `HOME`/`XDG`/`TMPDIR` aislados bajo `/home/alan`; ninguno
usa cuentas, base de datos ni música personales.

---

Syncify v0.6.1 is a download-reliability patch. Two queue-state defects are fixed:
an in-flight cancellation no longer ends up marked `complete`, and a credential
rejection is remembered even when a sibling account completes the same track. It
also carries the accumulated pre-beta import, edition-identity, lyrics and metadata
work, including migrations 0097-0100. There is still no measured valid-success rate
against real accounts: every check in this release is offline and synthetic.

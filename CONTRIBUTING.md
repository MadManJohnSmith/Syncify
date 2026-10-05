[English](#english) · [Español](#español)

# Contributing to Syncify

## English

Thanks for looking at Syncify. Issues, UI work, new lyrics or metadata
providers, another service, and documentation fixes are all welcome.

### Where to send the pull request

Fork the repository, create a focused branch, and open the PR against
**`main`**. That is the integration branch: `.github/workflows/ci.yml` triggers
its jobs on pushes to `main`, and releases are cut from it.

`main` is not the only branch upstream — `syncify-graphical` and the
`fix/*` topics also exist — but `syncify-graphical` stopped receiving work and
is not an integration target. A branch that no longer exists upstream cannot
receive your PR at all, which is why the READMEs name `main` and a gate
(`scripts/tests/test_docs_consistency.py`) fails if they ever name one that
has been deleted.

### Checks to run before you submit

CI runs all of these; running them locally saves a round trip.

| Area | Command |
|---|---|
| Rust types and build | `cargo check --all-targets --locked` |
| Rust tests | `cargo test --locked` |
| Rust lint | `cargo clippy --workspace --all-targets --locked -- -D warnings` |
| Rust format | `cargo fmt --all -- --check` |
| Frontend types | `cd ui && npx vue-tsc --noEmit` |
| Frontend tests | `cd ui && npm run test:run` |
| Python bridges | `python -m unittest discover -s scripts/tests -p 'test_*.py'` |

`cargo test --locked` also runs `batch_50_audit_test`, which CI executes
serialized on its own with `--test-threads=1`. If it only fails under
parallel load, reproduce it with
`cargo test --locked --test batch_50_audit_test -- --test-threads=1`.

### One gotcha worth knowing

On `push`, CI ignores changes confined to `**.md` and `docs/**`, so a
documentation-only commit pushed straight to `main` runs nothing. Pull
requests always run every job. Wait for the PR checks, not for the push.

### Documentation is gated, not just written

Two gates keep the front pages honest, and they run as part of the Python
suite:

- `scripts/tests/test_docs_consistency.py` — every file path a design doc
  cites must resolve in the tree, and the READMEs must name a branch that
  still exists.
- `scripts/tests/test_readme_claims.py` — the READMEs must not promise
  credential storage, cross-service sync, playlist migration, per-provider
  import coverage or delivery formats the code does not implement.

If you change what the app does, change what the docs claim in the same PR.
If a claim has to come back, the gate has to come with it.

Found a bug? [Open an issue](https://github.com/MadManJohnSmith/Syncify/issues)
with your OS and version, steps to reproduce, and the relevant log lines —
never personal credentials or tokens.

---

## Español

Gracias por mirar Syncify. Los issues, el trabajo de UI, nuevos proveedores
de letras o metadatos, otro servicio y las correcciones de documentación son
todos bienvenidos.

### Dónde enviar el pull request

Haz fork del repositorio, crea una rama enfocada y abre el PR contra
**`main`**. Esa es la rama de integración: `.github/workflows/ci.yml` dispara
sus jobs con los pushes a `main`, y de ella salen las releases.

`main` no es la única rama en el remoto — también existen `syncify-graphical`
y los topics `fix/*` — pero `syncify-graphical` dejó de recibir trabajo y no es
un destino de integración. Una rama que ya no existe en el remoto no puede
recibir tu PR, y por eso las README nombran `main` y un gate
(`scripts/tests/test_docs_consistency.py`) falla si alguna vez nombran una
que se haya borrado.

### Checks a correr antes de enviar

CI corre todos; correrlos en local ahorra una vuelta.

| Área | Comando |
|---|---|
| Tipos y build de Rust | `cargo check --all-targets --locked` |
| Tests de Rust | `cargo test --locked` |
| Lint de Rust | `cargo clippy --workspace --all-targets --locked -- -D warnings` |
| Formato de Rust | `cargo fmt --all -- --check` |
| Tipos del frontend | `cd ui && npx vue-tsc --noEmit` |
| Tests del frontend | `cd ui && npm run test:run` |
| Puentes Python | `python -m unittest discover -s scripts/tests -p 'test_*.py'` |

`cargo test --locked` también corre `batch_50_audit_test`, que CI ejecuta
serializado por separado con `--test-threads=1`. Si solo falla bajo carga
paralela, reprodúcelo con
`cargo test --locked --test batch_50_audit_test -- --test-threads=1`.

### Un detalle que conviene saber

En `push`, CI ignora los cambios limitados a `**.md` y `docs/**`, así que un
commit solo de documentación subido directo a `main` no corre nada. Los pull
requests siempre corren todos los jobs. Espera a los checks del PR, no al
push.

### La documentación está vigilada, no solo escrita

Dos gates mantienen honestas las portadas, y corren como parte de la suite
de Python:

- `scripts/tests/test_docs_consistency.py` — cada ruta que cita un documento
  de diseño debe existir en el árbol, y las README deben nombrar una rama que
  siga existiendo.
- `scripts/tests/test_readme_claims.py` — las README no pueden prometer
  almacenamiento de credenciales, sincronización entre servicios, migración
  de playlists, cobertura de importación por proveedor ni formatos de
  entrega que el código no implementa.

Si cambias lo que hace la app, cambia en el mismo PR lo que dicen los docs.
Si una promesa tiene que volver, el gate tiene que volver con ella.

¿Encontraste un bug? [Abre un issue](https://github.com/MadManJohnSmith/Syncify/issues)
con tu SO y versión, pasos para reproducir y las líneas de log relevantes —
nunca credenciales ni tokens personales.

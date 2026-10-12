//! CFGL-02/03/05 — guardian de las preferencias de incrustación.
//!
//! El worker dejó de llevar `embed_lyrics`/`embed_artwork` hardcodeados y ahora
//! los lee de `audio_processing_settings` (`src-tauri/src/worker.rs:1113-1123`,
//! `:1134`). El test unitario `worker.rs:2243` cubre esa lectura contra SQLite,
//! incluido el caso de fila ausente, pero NADA vigila que el valor leído siga
//! llegando a las dos ramas que construyen el `DownloadRequest`
//! (`:1345` y `:1392`). Ese es el cabo que falla hoy silenciosamente: si alguien
//! vuelve a escribir `embed_lyrics: true` en cualquiera de las dos ramas, el
//! comportamiento regresa a ignorar al usuario y la suite se queda verde.
//!
//! Tres piezas, de más conductual a más estructural:
//!
//! 1. `persisted_embed_preferences_round_trip_both_ways_on_the_real_schema` —
//!    sobre el esquema real migrado, llama al comando de lectura/escritura de
//!    ajustes (`src-tauri/src/commands/settings.rs:660-717`) y verifica que
//!    ambos flags cambian independientemente en ambos sentidos en la fila que
//!    consulta el worker. No congela el default de la fila ausente.
//! 2. `download_requests_carry_the_persisted_embed_flags_in_both_branches` —
//!    comprueba sobre el fuente que cada `DownloadRequest` de
//!    `process_download_internal` asigna los dos flags desde el binding que sale
//!    de `self.resolve_embed_preferences()`, no desde una constante.
//! 3. `guard_ignores_cosmetics_and_none_default_but_rejects_hardcoding` — el
//!    guardián no debe romperse con un refactor cosmético (shorthand, comentario
//!    dentro de una expresión, renombre del binding) pero SÍ con `true`/`false`
//!    reinsertados en cualquiera de las dos ramas. Los contraejemplos se
//!    construyen en memoria, sin modificar `worker.rs`.
//!
//! Por qué el punto 2 es una prueba sobre el fuente y no conductual: los flags
//! viajan dentro de `DownloadRequest` (`src-tauri/src/download/progress.rs:808-809`)
//! y **ningún descargador los consume hoy** — `crate::download::qobuz.rs` y
//! `crate::download::tidal.rs` deciden letras/portada por su lado. Probar un
//! artefacto descargado con `embed_lyrics: false` no observaría la preferencia.
//! Así que el contrato que sí está establecido hoy es "el `DownloadRequest` sale
//! de la BD", y eso es lo que se fija, sobre el fuente tokenizado (ignora
//! espacios, comentarios y literales) para que no lo rompa un refactor cosmético.
//!
//! Deliberadamente NO se persigue el default `(true, true)` de `resolve_embed_preferences`
//! cuando no hay fila: eso es política de producto (el esquema pone 1 en las dos
//! columnas, `migrations/0012_audio_processing_settings.sql:12-13`), no una
//! regresión.

use std::path::Path;

use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use std::str::FromStr;
use std::sync::Arc;
use syncify_tauri_lib::commands::settings::{
    get_audio_processing_settings, update_audio_processing_settings,
};
use syncify_tauri_lib::enrichment_worker::EnrichmentWorkerState;
use syncify_tauri_lib::services::ConcurrencyManager;
use syncify_tauri_lib::worker::DownloadWorkerState;
use syncify_tauri_lib::AppState;
use tauri::Manager;

/// Tokeniza Rust de forma suficiente para leer una firma y dos campos de struct:
/// palabras/identificadores, puntuación de un carácter, y saltar comentarios y
/// literales (entre ellos los `r#"..."#` y `r##"..."##`, abundantes en worker.rs).
fn tokens(source: &str) -> Vec<String> {
    let b = source.as_bytes();
    let mut out: Vec<String> = Vec::new();
    let mut i = 0usize;
    while i < b.len() {
        let c = b[i];
        if c.is_ascii_whitespace() {
            i += 1;
        } else if b[i..].starts_with(b"//") {
            i = b[i..]
                .iter()
                .position(|&x| x == b'\n')
                .map_or(b.len(), |n| i + n);
        } else if b[i..].starts_with(b"/*") {
            let mut depth = 1u32;
            i += 2;
            while i < b.len() && depth > 0 {
                if b[i..].starts_with(b"/*") {
                    depth += 1;
                    i += 2;
                } else if b[i..].starts_with(b"*/") {
                    depth -= 1;
                    i += 2;
                } else {
                    i += 1;
                }
            }
        } else if c == b'r' && matches!(b.get(i + 1), Some(b'#' | b'"')) {
            // Raw string: r, cero o más '#', comilla de apertura.
            let mut j = i + 1;
            while j < b.len() && b[j] == b'#' {
                j += 1;
            }
            let hashes = j - i - 1;
            if j < b.len() && b[j] == b'"' {
                i = j + 1;
                while let Some(rest) = b.get(i..) {
                    let Some(quote) = rest.iter().position(|&x| x == b'"') else {
                        i = b.len();
                        break;
                    };
                    let after = i + quote + 1;
                    if b.get(after..after + hashes)
                        .is_some_and(|suffix| suffix.iter().all(|&x| x == b'#'))
                    {
                        i = after + hashes;
                        break;
                    }
                    i = after;
                }
            } else {
                out.push("r".to_string());
                i += 1;
            }
        } else if c == b'"' {
            i += 1;
            while i < b.len() {
                match b[i] {
                    b'\\' => i = (i + 2).min(b.len()),
                    b'"' => {
                        i += 1;
                        break;
                    }
                    _ => i += 1,
                }
            }
        } else if c.is_ascii_alphabetic() || c == b'_' {
            let start = i;
            i += 1;
            while i < b.len() && (b[i].is_ascii_alphanumeric() || b[i] == b'_') {
                i += 1;
            }
            out.push(source[start..i].to_string());
        } else {
            out.push((c as char).to_string());
            i += 1;
        }
    }
    out
}

/// Índice del `}` que cierra el grupo abierto en `open` (que debe apuntar a `{`).
fn closing(depth_source: &[String], open: usize) -> Option<usize> {
    let mut depth = 0u32;
    for (i, token) in depth_source.iter().enumerate().skip(open) {
        match token.as_str() {
            "{" => depth += 1,
            "}" => {
                depth -= 1;
                if depth == 0 {
                    return Some(i);
                }
            }
            _ => {}
        }
    }
    None
}

/// Expresión (tokens) del campo `name` en una lista `a: x, b: y, ...` de nivel 0.
fn field_value<'a>(fields: &'a [String], name: &str) -> Option<&'a [String]> {
    let mut start = 0usize;
    let mut nesting = 0i32;
    for i in 0..=fields.len() {
        let at_boundary = i == fields.len() || (fields[i] == "," && nesting == 0);
        if !at_boundary && i < fields.len() {
            match fields[i].as_str() {
                "{" | "(" | "[" => nesting += 1,
                "}" | ")" | "]" => nesting -= 1,
                _ => {}
            }
            continue;
        }
        let entry = &fields[start..i];
        if entry.first().is_some_and(|s| s == name) {
            return if entry.len() == 1 {
                Some(entry) // campo de shorthand: `embed_lyrics,`
            } else if entry.get(1).is_some_and(|s| s == ":") {
                Some(&entry[2..])
            } else {
                None // path, no campo simple
            };
        }
        start = i + 1;
    }
    None
}

/// Revisa `worker.rs`: dentro de `process_download_internal`, los dos
/// `DownloadRequest` deben tomar `embed_lyrics`/`embed_artwork` del binding que
/// sale de `self.resolve_embed_preferences().await`.
fn check_worker(source: &str) -> Result<(), String> {
    let t = tokens(source);

    let method = t
        .windows(2)
        .position(|w| w[0] == "fn" && w[1] == "process_download_internal")
        .ok_or("no se encuentra el método worker `process_download_internal`")?;
    let open = t
        .iter()
        .skip(method + 2)
        .position(|s| s == "{")
        .map(|off| method + 2 + off)
        .ok_or("el cuerpo de `process_download_internal` no abre con `{`")?;
    let end = closing(&t, open).ok_or("cuerpo de `process_download_internal` desbalanceado")?;
    let body = &t[open + 1..end];

    // let (lyrics, artwork) = match self.resolve_embed_preferences().await { ... }
    let binding = body
        .windows(15)
        .find(|w| {
            w[0] == "let"
                && w[1] == "("
                && w[3] == ","
                && w[5] == ")"
                && w[6] == "="
                && w[7] == "match"
                && w[8] == "self"
                && w[9] == "."
                && w[10] == "resolve_embed_preferences"
                && w[11] == "("
                && w[12] == ")"
                && w[13] == "."
                && w[14] == "await"
        })
        .ok_or(
            "el worker debe enlazar ambos flags desde `self.resolve_embed_preferences().await`",
        )?;
    let lyrics_from_db = &binding[2];
    let artwork_from_db = &binding[4];

    let mut requests = 0usize;
    for (i, window) in body.windows(2).enumerate() {
        if window[0] != "DownloadRequest" || window[1] != "{" {
            continue;
        }
        let close = closing(body, i + 1).ok_or("literal `DownloadRequest` desbalanceado")?;
        let fields = &body[i + 2..close];
        for (field, expected) in [
            ("embed_lyrics", lyrics_from_db),
            ("embed_artwork", artwork_from_db),
        ] {
            let actual = field_value(fields, field).ok_or_else(|| {
                format!(
                    "el DownloadRequest #{} no tiene campo `{}`",
                    requests, field
                )
            })?;
            if actual != [expected.as_str()] {
                return Err(format!(
                    "el DownloadRequest #{} asigna `{}: {}`, pero debe ser el binding \
                     `{}` que sale de `audio_processing_settings` \
                     (worker.rs:1113-1123); una constante aquí = preferencia ignorada",
                    requests,
                    field,
                    actual.join(" "),
                    expected
                ));
            }
        }
        requests += 1;
    }

    if requests != 2 {
        return Err(format!(
            "se esperaban las dos ramas actuales del worker (`:1345` y `:1392`); se vieron {}",
            requests
        ));
    }
    Ok(())
}

#[test]
fn download_requests_carry_the_persisted_embed_flags_in_both_branches() {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("src/worker.rs");
    let source = std::fs::read_to_string(&path).expect("worker.rs debe existir");
    if let Err(reason) = check_worker(&source) {
        panic!("{}: {}", path.display(), reason);
    }
}

/// El guardián no debe depender de refactors cosméticos (comentario dentro de una
/// expresión, shorthand, otra palabra local) y sí debe romperse con
/// `embed_lyrics: true` o `embed_artwork: false` reinsertados en cualquiera de
/// las dos ramas. El default legítimo `(true, true)` sin fila no debe dispararlo.
#[test]
fn guard_ignores_cosmetics_and_none_default_but_rejects_hardcoding() {
    let fixture = r#"
fn resolve_embed_preferences(&self) -> Result<(bool, bool), String> {
    let row: Option<(i64, i64)> = sqlx::query_as(
        "SELECT embed_lyrics, embed_artwork FROM audio_processing_settings WHERE id = 1",
    )
    .fetch_optional(&self.db).await.map_err(|e| e.to_string())?;
    Ok(match row {
        Some((lyr, art)) => (lyr != 0, art != 0),
        // Política de producto: sin fila se embece igual que el esquema manda.
        None => (true, true),
    })
}

async fn process_download_internal(&self, queue_id: i64) {
    let (embed_lyrics, embed_artwork) = match self.resolve_embed_preferences().await {
        Ok(settings) => settings,
        Err(error) => return,
    };
    let one = crate::download::DownloadRequest {
        item_id: queue_id.to_string(),
        embed_lyrics, // shorthand
        embed_artwork /* comentario que no debe importar */: embed_artwork,
    };
    let two = crate::download::DownloadRequest {
        item_id: queue_id.to_string(),
        embed_lyrics: embed_lyrics,
        embed_artwork: embed_artwork,
    };
}
"#;

    assert!(
        check_worker(fixture).is_ok(),
        "el fixture con shorthand, comentarios y default None debe pasar: {:?}",
        check_worker(fixture)
    );

    // Regresión real 1: letras incrustadas a fuego en la primera rama.
    let case = fixture.replace(
        "embed_lyrics, // shorthand",
        "embed_lyrics: true, // shorthand",
    );
    assert!(
        check_worker(&case).is_err(),
        "embed_lyrics: true debe romper el guardián"
    );

    // Regresión real 2: portada desactivada a fuego en la segunda rama.
    let case = fixture.replace(
        "embed_artwork: embed_artwork,\n    };",
        "embed_artwork: false,\n    };",
    );
    assert!(
        check_worker(&case).is_err(),
        "embed_artwork: false debe romper el guardián"
    );

    // Cambio estructural contemplado: renombrar la variable local no debe romperlo
    // siempre que siga viniendo de la BD.
    let renamed = fixture
        .replace(
            "let (embed_lyrics, embed_artwork)",
            "let (letras_de_fila, embed_artwork)",
        )
        .replace(
            "embed_lyrics, // shorthand",
            "embed_lyrics: letras_de_fila, // explicit",
        )
        .replace(
            "embed_lyrics: embed_lyrics,",
            "embed_lyrics: letras_de_fila,",
        );
    assert!(
        check_worker(&renamed).is_ok(),
        "renombrar el binding local no debe romperlo: {:?}",
        check_worker(&renamed)
    );
}

// ---------------------------------------------------------------------------
// Conductual: la fila que escribe la ruta real de ajustes llega al worker en
// ambos sentidos, sobre el esquema de producción (las 88 migraciones), no una
// tabla improvisada.
// ---------------------------------------------------------------------------

/// Esquema real: BD en disco creada con `sqlx::migrate!` igual que las suites de
/// integración existentes, para que el guardián lea las columnas y el default
/// que de verdad tiene `audio_processing_settings` en producción.
async fn migrated_db(temp: &tempfile::TempDir) -> sqlx::SqlitePool {
    let db_path = temp.path().join("syncify_embed_guard.db");
    let db_url = format!("sqlite://{}?mode=rwc", db_path.to_string_lossy());
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect_with(
            SqliteConnectOptions::from_str(&db_url)
                .expect("url valida")
                .create_if_missing(true),
        )
        .await
        .expect("BD en disco");
    sqlx::migrate!("./migrations")
        .run(&pool)
        .await
        .expect("todas las migraciones deben aplicar");
    pool
}

/// El test unitario de `worker.rs:2243` cubre el lector privado del worker
/// contra SQLite en memoria; aquí usamos el esquema migrado y el comando Tauri
/// real para comprobar su escritura de ambos flags, incluidas combinaciones
/// asimétricas. La lectura final usa la misma consulta SQL que el worker.
/// No se hace público `resolve_embed_preferences` solo para probarlo; el guardián
/// estructural anterior protege la conexión hasta cada `DownloadRequest`.
/// No exigimos el default de la fila ausente: esa política puede cambiar.
#[tokio::test]
async fn persisted_embed_preferences_round_trip_both_ways_on_the_real_schema() {
    let temp = tempfile::tempdir().expect("tempdir");
    let pool = migrated_db(&temp).await;

    // Comando tauri real: get/update de audio_processing_settings
    // (src-tauri/src/commands/settings.rs:660-717). El worker lee esa misma fila.
    let app = tauri::test::mock_app();
    app.manage(AppState {
        db: pool,
        worker_state: DownloadWorkerState::new(1),
        enrichment_state: EnrichmentWorkerState::new(),
        concurrency_manager: Arc::new(ConcurrencyManager::new()),
    });
    let state = app.state::<AppState>();
    let saved = get_audio_processing_settings(state.clone())
        .await
        .expect("la fila sembrada debe leerse");

    // Sentido OFF: los dos flags apagan de forma independiente y persisten.
    let written = update_audio_processing_settings(
        state.clone(),
        syncify_tauri_lib::models::AudioProcessingSettings {
            embed_lyrics: false,
            embed_artwork: false,
            ..saved.clone()
        },
    )
    .await
    .expect("el UPDATE real debe aplicarse");
    assert!(
        !written.embed_lyrics && !written.embed_artwork,
        "el comando devuelve lo que escribió"
    );

    let (lyrics_off, artwork_off): (i64, i64) = sqlx::query_as(
        "SELECT embed_lyrics, embed_artwork FROM audio_processing_settings WHERE id = 1",
    )
    .fetch_one(&app.state::<AppState>().db)
    .await
    .expect("preferencias apagadas legibles");
    assert_eq!(
        (lyrics_off, artwork_off),
        (0, 0),
        "el usuario apaga ambas y la fila queda en 0/0, que es lo que el worker leerá"
    );

    // Sentido ON: volver a encender no se queda pegado en el estado apagado,
    // y cada flag se mueve por su cuenta (su único sentido legítimo).
    let asymmetric = update_audio_processing_settings(
        state.clone(),
        syncify_tauri_lib::models::AudioProcessingSettings {
            embed_lyrics: true,
            embed_artwork: false,
            ..saved.clone()
        },
    )
    .await
    .expect("el UPDATE real debe aplicarse");
    assert!(asymmetric.embed_lyrics, "letras encienden solas");
    assert!(
        !asymmetric.embed_artwork,
        "la portada sigue apagada: los flags no van atados"
    );

    let written_back = update_audio_processing_settings(
        state,
        syncify_tauri_lib::models::AudioProcessingSettings {
            embed_lyrics: true,
            embed_artwork: true,
            ..saved.clone()
        },
    )
    .await
    .expect("el UPDATE real debe aplicarse");
    let (lyrics_on, artwork_on): (i64, i64) = sqlx::query_as(
        "SELECT embed_lyrics, embed_artwork FROM audio_processing_settings WHERE id = 1",
    )
    .fetch_one(&app.state::<AppState>().db)
    .await
    .expect("preferencias encendidas legibles");
    assert_eq!(
        (lyrics_on, artwork_on),
        (1, 1),
        "volver a encender deja 1/1, que el worker traduce a (true, true)"
    );
    assert!(
        written_back.embed_lyrics && written_back.embed_artwork,
        "el comando devuelve el estado encendido"
    );
}

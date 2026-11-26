//! LY14 (#192) — sonda en vivo del endpoint nativo de letras de Tidal.
//!
//! Este fichero es la evidencia ejecutable del spike LY13/#182: con
//! credenciales reales del dueño, comprueba si Tidal sirve la letra de una pista
//! conocida, imprime la forma exacta de la respuesta y somete el payload al
//! parser y a la competición del motor. Sin letra, falla con el status y el
//! cuerpo exactos para que la conclusión negativa quede documentada.
//!
//! No corre en CI ni en las puertas locales: lleva el atributo `ignore` y las
//! credenciales fuera del árbol.
//!
//! ```bash
//! SYNCIFY_TIDAL_ACCESS_TOKEN='...' \
//! SYNCIFY_TIDAL_TRACK_ID=24167 \
//! SYNCIFY_TIDAL_COUNTRY_CODE=MX \
//! cargo test --locked --test tidal_native_lyrics_live_test -- --ignored --nocapture
//! ```
//!
//! Variables: `SYNCIFY_TIDAL_ACCESS_TOKEN` y `SYNCIFY_TIDAL_TRACK_ID` son
//! obligatorias; `SYNCIFY_TIDAL_SESSION_ID`, `SYNCIFY_TIDAL_COUNTRY_CODE`
//! (por defecto `US`) y `SYNCIFY_TIDAL_TRACK_TITLE` (por defecto `Song`) son
//! opcionales.

use syncify_lyrics_domain::{LyricsLineDomain, LyricsResolution, LyricsSyncType};
use syncify_tauri_lib::download::lyrics::{
    compete_with_native, parse_tidal_native_payload, LyricsEngineConfig,
};

/// Endpoint candidato del spike. La forma (ruta y entrega) queda **sin
/// confirmar** hasta que esta sonda devuelva una letra real.
fn tidal_native_lyrics_url(track_id: &str, country_code: &str) -> String {
    format!("https://api.tidal.com/v1/tracks/{track_id}/lyrics?countryCode={country_code}")
}

fn required_env(name: &str) -> String {
    std::env::var(name).unwrap_or_else(|_| {
        panic!(
            "sonda LY13/#182: falta {name}. Exporta credenciales reales de Tidal antes de correrla."
        )
    })
}

/// Resultado genérico contra el que compite el candidato nativo: una letra
/// sincronizada por línea que el motor genérico ya hubiera aceptado.
fn generic_line_synced_reference() -> LyricsResolution {
    LyricsResolution::new_resolved(
        "LRCLIB",
        "lrclib_synced",
        LyricsSyncType::LineSynced,
        Some("[00:10.00]I wish you could swim".to_string()),
        Some("I wish you could swim".to_string()),
        vec![LyricsLineDomain {
            start_time_ms: 10_000,
            words: "I wish you could swim".to_string(),
            end_time_ms: None,
        }],
        false,
        "lrclib.net",
    )
}

#[tokio::test]
#[ignore = "sonda en vivo: requiere credenciales reales de Tidal (LY13/#182)"]
async fn tidal_native_lyrics_endpoint_spike() {
    let token = required_env("SYNCIFY_TIDAL_ACCESS_TOKEN");
    let track_id = required_env("SYNCIFY_TIDAL_TRACK_ID");
    let country_code = std::env::var("SYNCIFY_TIDAL_COUNTRY_CODE").unwrap_or_else(|_| "US".into());
    let title = std::env::var("SYNCIFY_TIDAL_TRACK_TITLE").unwrap_or_else(|_| "Song".into());
    let url = tidal_native_lyrics_url(&track_id, &country_code);

    let client = reqwest::Client::new();
    let mut request = client
        .get(&url)
        .header("Authorization", format!("Bearer {token}"))
        .header(
            "User-Agent",
            "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36",
        );
    if let Ok(session_id) = std::env::var("SYNCIFY_TIDAL_SESSION_ID") {
        if !session_id.trim().is_empty() {
            request = request.header("X-Tidal-SessionId", session_id.trim());
        }
    }

    let response = request
        .send()
        .await
        .expect("la petición al endpoint nativo no debe fallar por transporte");
    let status = response.status();
    let body = response.text().await.unwrap_or_default();
    println!("[LY14] {url} -> {status}");
    println!(
        "[LY14] cuerpo (primeros 2000 caracteres):\n{}",
        body.chars().take(2000).collect::<String>()
    );

    assert!(
        status.is_success(),
        "conclusión negativa del spike: el endpoint respondió {status}. Cuerpo: {body}"
    );

    let json: serde_json::Value =
        serde_json::from_str(&body).expect("el endpoint nativo debe entregar JSON");
    let keys: Vec<&str> = json
        .as_object()
        .map(|object| object.keys().map(String::as_str).collect())
        .unwrap_or_default();
    println!("[LY14] claves del payload: {keys:?}");

    let native = parse_tidal_native_payload(&json).unwrap_or_else(|| {
        panic!("el payload nativo no produjo candidato: claves observadas {keys:?}")
    });
    println!(
        "[LY14] candidato nativo: provider={} sync={} líneas={} plain={}",
        native.provider,
        native.sync_type,
        native.lines.len(),
        native.plain_lyrics.is_some()
    );
    assert!(
        !native.lines.is_empty() || native.plain_lyrics.is_some(),
        "el criterio de éxito exige letra real"
    );

    let generic = generic_line_synced_reference();
    let config = LyricsEngineConfig::default();
    let winner = compete_with_native(&config, &title, generic, Some(native.clone()));
    println!(
        "[LY14] ganador de la competición: provider={} sync={:?}",
        winner.provider, winner.sync_type
    );

    if native.lines.is_empty() {
        assert_eq!(
            winner.provider, "LRCLIB",
            "una letra nativa plana no desbanca a la sincronizada del motor genérico"
        );
    } else if native.elrc_content.is_some() {
        assert_eq!(
            winner.provider, "Tidal",
            "una letra nativa con tiempos de palabra gana por Nivel de sincronía al genérico de línea"
        );
    } else {
        assert_eq!(
            winner.provider, "LRCLIB",
            "un empate de línea conserva la prioridad de la fuente genérica"
        );
    }
}

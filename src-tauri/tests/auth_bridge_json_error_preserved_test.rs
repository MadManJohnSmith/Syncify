//! Auditoría 37 y 39: el resultado del puente de autenticación se lee completo.
//!
//! `auth_bridge.py` cierra con código 1 en cuanto `success` es false, así que el
//! JSON que explica el fallo ya está en stdout cuando Rust decide que "el
//! proceso falló" y lo sustituye por un error genérico. Eso convertía un
//! mensaje accionable —"No se pudo abrir el navegador, abre esta URL…",
//! "MissingTidalCredentialsError"— en un timeout o un código de salida, y es la
//! razón por la que el login de Tidal se llevaba 120 s de silencio (R2).
//!
//! Estos tests fijan el contrato de `parse_auth_bridge_output`, que es donde
//! vive la decisión, sin depender de que el intérprete de Python tenga las
//! dependencias de los flujos interactivos instaladas.

use syncify_tauri_lib::commands::{parse_auth_bridge_output, AUTH_BRIDGE_TIMEOUT_SECS};

#[test]
fn json_error_survives_non_zero_exit_code() {
    // `json_response(False, error=...)` escribe esto y sale con 1.
    let stdout = r#"{"success": false, "error": "MissingTidalCredentialsError: TIDAL_CLIENT_ID is required"}"#;

    let result = parse_auth_bridge_output("tidal", stdout, "", false, Some(1));

    let auth_result = result.expect("el JSON de error debe deserializarse, no descartarse");
    assert!(!auth_result.success);
    assert_eq!(
        auth_result.error.as_deref(),
        Some("MissingTidalCredentialsError: TIDAL_CLIENT_ID is required")
    );
}

#[test]
fn verification_url_reaches_the_caller_on_failed_tidal_login() {
    // Exactamente el payload que produce `handle_tidal` cuando el escritorio no
    // abre el navegador (R2): el usuario necesita la URL para terminar a mano.
    let stdout = r#"{"success": false, "data": {"verification_url": "https://link.tidal.com/ABC123"}, "error": "No se pudo abrir el navegador automaticamente. Abre esta URL para conectar Tidal: https://link.tidal.com/ABC123"}"#;

    let auth_result = parse_auth_bridge_output("tidal", stdout, "", false, Some(1))
        .expect("el JSON de error debe deserializarse");

    assert!(!auth_result.success);
    let url = auth_result
        .data
        .as_ref()
        .and_then(|d| d.get("verification_url"))
        .and_then(|v| v.as_str())
        .expect("la URL de verificación debe viajar en data");
    assert_eq!(url, "https://link.tidal.com/ABC123");
}

#[test]
fn json_error_is_read_even_when_stderr_carries_the_diagnostic() {
    // Tidal anuncia la URL por stderr (stdout queda reservado al JSON).
    let stdout = r#"{"success": false, "error": "Authorization timed out"}"#;
    let stderr = "[Tidal Auth] Abre esta URL para continuar: https://link.tidal.com/ABC123\n";

    let auth_result =
        parse_auth_bridge_output("tidal", stdout, stderr, false, Some(1)).expect("JSON válido");

    assert!(!auth_result.success);
    assert_eq!(
        auth_result.error.as_deref(),
        Some("Authorization timed out")
    );
}

#[test]
fn stderr_backfills_a_missing_error_field() {
    let stdout = r#"{"success": false}"#;

    let auth_result =
        parse_auth_bridge_output("deezer", stdout, "Traceback: boom\n", false, Some(1))
            .expect("JSON válido");

    assert!(!auth_result.success);
    assert_eq!(auth_result.error.as_deref(), Some("Traceback: boom"));
}

#[test]
fn successful_exit_without_error_keeps_no_error() {
    let stdout = r#"{"success": true, "data": {"user_id": "7"}}"#;

    let auth_result =
        parse_auth_bridge_output("spotify", stdout, "", true, Some(0)).expect("JSON válido");

    assert!(auth_result.success);
    assert!(auth_result.error.is_none());
}

#[test]
fn debug_lines_before_the_json_are_still_tolerated() {
    // Comportamiento previo, conservado: librerías que escriben en stdout antes
    // del payload no pueden romper la lectura.
    let stdout = "warning: something\n{\"success\": true, \"data\": {\"ok\": true}}\n";

    let auth_result =
        parse_auth_bridge_output("qobuz", stdout, "", true, Some(0)).expect("JSON válido");

    assert!(auth_result.success);
}

#[test]
fn empty_stdout_still_falls_back_to_stderr_or_exit_code() {
    let err = parse_auth_bridge_output("soundcloud", "   ", "boom\n", false, Some(1))
        .expect_err("sin stdout no hay JSON que leer");
    assert!(err.contains("boom"), "{}", err);

    let err = parse_auth_bridge_output("soundcloud", "", "", false, Some(1))
        .expect_err("sin stdout no hay JSON que leer");
    assert!(err.contains("code"), "{}", err);
}

#[test]
fn unparsable_output_is_reported_with_stderr_context() {
    let err = parse_auth_bridge_output(
        "apple_music",
        "not json at all",
        "import failed\n",
        false,
        Some(1),
    )
    .expect_err("una salida que no es JSON debe fallar");
    assert!(err.contains("import failed"), "{}", err);
    assert!(err.contains("unparsable output"), "{}", err);
}

#[test]
fn timeout_covers_the_longest_interactive_browser_flow() {
    // Auditoría 37: los flujos de navegador del puente dan 300 s
    // (`scripts/services/tidal_auth.py`, `qobuz_auth.py`, `deezer_auth.py`).
    // Con un tope de 120 s Rust mataba el proceso a mitad del login.
    assert!(
        AUTH_BRIDGE_TIMEOUT_SECS > 300,
        "el tope de Rust ({}) debe superar los 300 s interactivos del puente",
        AUTH_BRIDGE_TIMEOUT_SECS
    );
}

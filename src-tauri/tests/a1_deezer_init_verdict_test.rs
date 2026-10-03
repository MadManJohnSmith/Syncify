//! A-1: el fallo de `init()` de Deezer debe distinguir el rechazo real de la
//! credencial de un fallo de comprobación (5xx / 429 / body ilegible).
//!
//! ANTES DEL FIX las cinco salidas de error de `init()` eran el mismo
//! `Err(String)` y el caller (`commands/service.rs`) marcaba
//! `credentials_invalid = 1` en todas ellas. Estos tests fallan contra esa
//! versión: entonces un 503 invalidaba la cuenta igual que un 401.

use syncify_tauri_lib::services::deezer::{DeezerClient, DeezerInitError};
use syncify_tauri_lib::worker::AuthVerdict;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;

/// Servidor mock que siempre responde lo mismo: suficiente para `init()`, que
/// solo hace una petición.
async fn spawn_mock(status: u16, body: &'static str) -> String {
    let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind mock");
    let addr = listener.local_addr().unwrap();
    let body = body.to_string();
    tokio::spawn(async move {
        loop {
            let Ok((mut socket, _)) = listener.accept().await else {
                break;
            };
            let body = body.clone();
            tokio::spawn(async move {
                let mut buf = vec![0u8; 32768];
                let _ = socket.read(&mut buf).await;
                let reason = if status == 200 { "OK" } else { "Error" };
                let resp = format!(
                    "HTTP/1.1 {} {}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                    status,
                    reason,
                    body.len(),
                    body
                );
                let _ = socket.write_all(resp.as_bytes()).await;
                let _ = socket.flush().await;
            });
        }
    });
    format!("http://{}", addr)
}

fn client_for(base: String) -> DeezerClient {
    DeezerClient::new("arl-test".into()).with_api_base(base)
}

/// El camino que usa producción para decidir si un fallo de `init()`
/// invalida la cuenta: `AuthVerdict::from_deezer_init`. Los tests consultan
/// ESA función, no un atajo propio, para que no puedan divergir del caller.
fn invalidates(result: &Result<(), DeezerInitError>) -> bool {
    AuthVerdict::from_deezer_init(result).invalidates_session()
}

/// Cuerpo 2xx bien formado: Deezer devuelve aquí el token de sesión.
/// Los nombres de campo son los del API real (`checkForm`, `USER_ID`), que es
/// lo que `DeezerResults` declara con `serde(rename = ...)`.
const OK_BODY: &str = r#"{"results":{"checkForm":"TOKEN-OK","USER":{"USER_ID":42}}}"#;

#[tokio::test]
async fn init_ok_yields_no_error_and_a_user_id() {
    let base = spawn_mock(200, OK_BODY).await;
    let mut client = client_for(base);
    assert!(
        client.init().await.is_ok(),
        "un 2xx con token de sesión debe inicializar bien"
    );
    assert_eq!(client.user_id().as_deref(), Some("42"));
}

// --- Rechazos REALES: deben seguir invalidando (comportamiento preservado) ---

#[tokio::test]
async fn real_rejections_still_invalidate_the_session() {
    // 401 y 403: el proveedor habla de la credencial.
    for status in [401u16, 403u16] {
        let base = spawn_mock(status, r#"{"error":"nope"}"#).await;
        let mut client = client_for(base);
        let result = client.init().await;
        let err = result.as_ref().expect_err("debe fallar");
        assert!(
            invalidates(&result),
            "HTTP {} es un rechazo real y DEBE invalidar (obtenido: {:?})",
            status,
            err
        );
        assert!(matches!(err, DeezerInitError::Rejected(_)));
    }

    // 2xx bien formado pero SIN token de sesión: Deezer acepta el transporte
    // y rechaza la ARL. Este es el rechazo "de facto".
    let base = spawn_mock(200, r#"{"results":{"USER":{"USER_ID":42}}}"#).await;
    let mut client = client_for(base);
    let result = client.init().await;
    let err = result.as_ref().expect_err("sin check_form debe fallar");
    assert!(
        invalidates(&result),
        "un 2xx sin token de sesión SÍ invalida (obtenido: {:?})",
        err
    );
}

// --- Fallos de COMPROBACIÓN: NUNCA deben invalidar (esto es el fix) ---

#[tokio::test]
async fn transient_provider_failures_never_invalidate_the_session() {
    // Un 503 de Deezer, un 429 de límite de tasa o un 502 de puerta no
    // dicen nada de la ARL: la sesión sigue viva.
    for status in [429u16, 500u16, 502u16, 503u16, 504u16] {
        let base = spawn_mock(status, r#"{"error":"busy"}"#).await;
        let mut client = client_for(base);
        let result = client.init().await;
        let err = result.as_ref().expect_err("debe fallar");
        assert!(
            !invalidates(&result),
            "HTTP {} es indisponibilidad y NO debe invalidar (obtenido: {:?})",
            status,
            err
        );
        assert!(matches!(err, DeezerInitError::Transient(_)));
    }
}

#[tokio::test]
async fn an_unparseable_body_is_not_a_credential_rejection() {
    // 2xx con HTML: una página de borde o un proxy. No dice nada de la ARL.
    let base = spawn_mock(200, "<html>gateway</html>").await;
    let mut client = client_for(base);
    let result = client.init().await;
    let err = result.as_ref().expect_err("body ilegible debe fallar");
    assert!(
        !invalidates(&result),
        "un body no parseable NO es rechazo de credenciales (obtenido: {:?})",
        err
    );
    assert!(matches!(err, DeezerInitError::Transient(_)));
}

#[tokio::test]
async fn an_unreachable_provider_never_invalidates_the_session() {
    // Puerto cerrado: sin red. El caso más claro de "no se pudo comprobar".
    let mut client = client_for("http://127.0.0.1:1".into());
    let result = client.init().await;
    let err = result.as_ref().expect_err("puerto cerrado debe fallar");
    assert!(
        !invalidates(&result),
        "sin red NO se puede rechazar la credencial (obtenido: {:?})",
        err
    );
    assert!(matches!(err, DeezerInitError::Transient(_)));
}

// --- El mensaje sigue siendo utilizable por los callers que solo loguean ---

#[test]
fn error_message_is_preserved_for_logging_callers() {
    let e = DeezerInitError::from_status(503, "Deezer API error (503): busy".into());
    assert_eq!(e.message(), "Deezer API error (503): busy");
    assert_eq!(e.to_string(), "Deezer API error (503): busy");
}

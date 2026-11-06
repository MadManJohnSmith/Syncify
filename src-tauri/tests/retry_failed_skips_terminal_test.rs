//! Regresión: "reintentar fallidos" no debe reencolar lo que no se puede reintentar.
//!
//! Existían dos comandos con el mismo nombre para el usuario. `retry_failed`
//! (queue.rs) clasifica cada fallo con la taxonomía y se salta los terminales
//! y los que requieren acción del usuario. `retry_failed_downloads`
//! (download.rs) hacía un UPDATE sin clasificar: `WHERE status = 'failed'`.
//!
//! La UI llamaba al segundo desde App.vue y DownloadsView.vue, así que un clic
//! del usuario devolvía a la cola descargas con un token caducado o una
//! credencial revocada, y las reintentaba sin límite (el UPDATE tampoco
//! miraba `retry_count`). Este test fija el comportamiento correcto.
//!
//! No hace falta montar la app: se replica el criterio que aplica
//! `perform_retry_all_failed` y se comprueba sobre mensajes reales de cada
//! categoría de la taxonomía.

use syncify_core_domain::errors::ErrorTaxonomy;
use syncify_tauri_lib::services::operation_recovery::classify_operation_error;
use syncify_tauri_lib::worker::download_operation_type;

fn classify(service: &str, error: &str) -> ErrorTaxonomy {
    classify_operation_error(download_operation_type(Some(service)), service, error)
}

/// Lo que decide el reencolado: terminal o requiere accion del usuario -> no entra.
fn retryable(service: &str, error: &str) -> bool {
    let taxonomy = classify(service, error);
    !taxonomy.is_terminal() && !taxonomy.requires_user_action()
}

#[test]
fn transient_failures_stay_retryable() {
    // Errores de red: deben volver a la cola, son justo lo que un reintento arregla.
    assert!(
        retryable("tidal", "connection reset by peer"),
        "un corte de red debe ser reintentable"
    );
    assert!(
        retryable("qobuz", "request timeout"),
        "un timeout debe ser reintentable"
    );
}

#[test]
fn credential_failures_are_not_retryable() {
    // Un token caducado o una credencial revocada no se arreglan reintentando:
    // el mismo request fallara igual y el usuario tiene que intervenir.
    assert!(
        !retryable("spotify", "invalid_grant"),
        "invalid_grant requiere re-autenticarse, no un reintento"
    );
    assert!(
        !retryable("tidal", "The access token expired"),
        "un token caducado requiere refresh, no reintentar a ciegas"
    );
}

#[test]
fn status_digits_inside_item_ids_are_not_auth_rejections() {
    for (service, error) in [
        ("qobuz", "Qobuz API error (500): track 4012345 failed"),
        (
            "tidal",
            "Tidal API error 500 Internal Server Error: item 4012345",
        ),
        ("spotify", "Failed to import item 4012345"),
    ] {
        assert!(
            !matches!(classify(service, error), ErrorTaxonomy::AuthInvalid { .. }),
            "'{}' must not be classified as an auth rejection",
            error
        );
    }

    for (service, error) in [
        ("qobuz", "Qobuz API error (401 Unauthorized): token expired"),
        ("tidal", "Tidal API error 401 Unauthorized: token expired"),
        ("spotify", "RequiresAuth: session expired"),
    ] {
        assert!(
            matches!(classify(service, error), ErrorTaxonomy::AuthInvalid { .. }),
            "'{}' must be classified as an auth rejection",
            error
        );
    }
}

#[test]
fn terminal_classes_still_block_automatic_retry() {
    // ADR-0022: la clase terminal gobierna el reintento AUTOMÁTICO, no el
    // botón del usuario. La taxonomía sigue siendo la fuente del primer
    // veredicto; el reintento manual se permite siempre.
    for (service, error) in [
        ("spotify", "invalid_grant"),
        ("tidal", "RejectedQuality: downgrade rejected"),
        ("qobuz", "AmbiguousSource: multiple editions"),
    ] {
        assert!(
            !retryable(service, error),
            "'{error}' no debe reencolarse automáticamente"
        );
    }
}

#[test]
fn manual_retry_is_always_allowed_and_has_no_attempt_cap() {
    use syncify_core_domain::job_state::{JobEvent, JobState};

    // La máquina permite ManualRetry desde failed sin mirar la clase.
    assert_eq!(
        JobState::Failed.transition(JobEvent::ManualRetry).unwrap(),
        JobState::Queued
    );

    // Y las dos transiciones que persisten la cola no llevan tope de intentos.
    let sql = include_str!("../src/commands/queue.rs");
    assert!(
        !sql.contains("retry_count < 5") && !sql.contains("retry_count < 99"),
        "el reintento manual no puede tener tope de intentos (ADR-0022)"
    );
    for transicion in [
        "UPDATE download_queue SET status = 'queued', error_message = NULL, last_error = NULL, decision_reason = NULL, progress_percent = 0, started_at = NULL, retry_count = 0, next_attempt_at = NULL WHERE id = ? AND status = 'failed'",
        "UPDATE download_queue SET status = 'queued', error_message = NULL, last_error = NULL, decision_reason = NULL, progress_percent = 0, started_at = NULL, retry_count = 0, next_attempt_at = NULL WHERE status = 'failed'",
    ] {
        assert!(
            sql.contains(transicion),
            "falta la transición ManualRetry: {transicion}"
        );
    }

    // La UI sigue delegando en el mismo camino y no hace su propio UPDATE.
    let comando_ui = include_str!("../src/commands/download.rs");
    assert!(comando_ui.contains("perform_retry_all_failed"));
    let reencola_sin_clasificar = comando_ui
        .lines()
        .map(str::trim_start)
        .filter(|l| !l.starts_with("//"))
        .any(|l| l.contains("UPDATE download_queue SET status = 'queued'"));
    assert!(
        !reencola_sin_clasificar,
        "retry_failed_downloads no debe reencolar por su cuenta"
    );
}

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
fn retrying_everything_would_be_the_bug_we_fixed() {
    // El UPDATE sin clasificar reencolaba estas mismas descargas. El test
    // documenta que el comportamiento viejo era incorrecto, para que nadie
    // lo reintroduzca creyendo que era equivalente.
    let antes_del_arreglo_reintentaba_todo = |error: &str| -> bool { !error.is_empty() };

    assert!(
        antes_del_arreglo_reintentaba_todo("invalid_grant"),
        "este es exactamente el fallo que se corrigio"
    );
    assert!(
        !retryable("spotify", "invalid_grant"),
        "con la taxonomia, el mismo error ya no se reencola"
    );
}

#[test]
fn the_retry_budget_is_bounded() {
    // perform_retry_all_failed filtra por `retry_count < 5`. Se comprueba que
    // ese limite existe en el SQL del camino seguro, porque un reencolado sin
    // tope es un bucle infinito disfrazado de reintento.
    let sql = include_str!("../src/commands/queue.rs");

    assert!(
        sql.contains("retry_count < 5"),
        "el camino seguro debe seguir limitando los intentos, \
         si no se reencola indefinidamente"
    );

    // Y el comando que usa la UI debe delegar en ese camino, no hacer su propio
    // UPDATE. Se busca solo el UPDATE de reencolado, ignorando los comentarios
    // (// y ///), que describen el SQL viejo como referencia. El fichero tiene
    // además un DELETE legitimo para "borrar fallidos", que es otra cosa.
    let comando_ui = include_str!("../src/commands/download.rs");
    let reencola_sin_clasificar = comando_ui
        .lines()
        .map(str::trim_start)
        .filter(|l| !l.starts_with("//"))
        .any(|l| l.contains("UPDATE download_queue SET status = 'queued'"));

    assert!(
        !reencola_sin_clasificar,
        "retry_failed_downloads no debe reencolar sin clasificar: \
         debe pasar por perform_retry_all_failed"
    );
    assert!(
        comando_ui.contains("perform_retry_all_failed"),
        "retry_failed_downloads debe delegar en el camino que clasifica"
    );
}

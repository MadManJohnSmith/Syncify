//! Regresión: distinguir preview/entitlement parcial de payload corrupto.
//!
//! Evidencia de campo (log `syncify-dev.log`, DB `download_queue`):
//!   - ~85 filas `error_message` = "Qobuz download failed: Downloaded audio failed
//!     bit-perfect FLAC magic verification", todas con `retry_count = 99`.
//!     El mensaje caía al `MalformedProviderPayload` por defecto (TERMINAL), así que
//!     el orquestador NUNCA intentaba el fallback a Tidal y el usuario no recibía
//!     ninguna explicación útil.
//!   - Los payloads que fallan miden exactamente ~1.200.588 B: 30 s a 320 kbps.
//!     Es el preview MP3 que Qobuz devuelve para `format_id=6` (FLAC) cuando la
//!     cuenta no tiene derecho a la versión completa.
//!
//! Tres clases distintas, tres veredictos distintos —este test los fija:
//!   1. `QobuzPreviewPayload`   → entitlement parcial, reintentable en otro
//!      proveedor/cuenta (`is_retryable`, `max_attempts() == 1`).
//!   2. `AudioValidationFailed` → payload corrupto (no es audio), TERMINAL: la
//!      misma URL devuelve los mismos bytes.
//!   3. `TemporaryNetworkFailure`/`RateLimited`/`Timeout` → fallo de red, reintentable.
//!
//! SIN app en marcha: se llama a `classify_operation_error` (pública) con los
//! mensajes reales de los productores, y se comprueba `is_terminal()`.

use syncify_core_domain::errors::ErrorTaxonomy;
use syncify_tauri_lib::services::operation_recovery::classify_operation_error;
use syncify_tauri_lib::worker::download_operation_type;

fn classify_qobuz(error: &str) -> ErrorTaxonomy {
    classify_operation_error(download_operation_type(Some("qobuz")), "qobuz", error)
}

fn classify_tidal(error: &str) -> ErrorTaxonomy {
    classify_operation_error(download_operation_type(Some("tidal")), "tidal", error)
}

/// Los mensajes EXACTOS que emiten los productores, copiados de los sitios de
/// error. Si uno cambia, este test falla y obliga a mantener el marcador.
mod producers {
    /// `download/qobuz.rs::validate_qobuz_audio_payload`, rama MP3.
    pub const PREVIEW_MP3_INSTEAD_OF_FLAC: &str = "QobuzPreviewPayload: Qobuz delivered MP3 instead of requested FLAC format_id=6: stream URL returned lossy audio, not a full lossless track";

    /// Misma función, rama "FLAC demasiado corto según STREAMINFO".
    pub const PREVIEW_SHORT_FLAC: &str = "QobuzPreviewPayload: Qobuz delivered short FLAC instead of requested FLAC format_id=6: FLAC STREAMINFO is shorter than the catalogued full track";

    /// Misma función, rama por defecto: los bytes no son audio en absoluto.
    pub const CORRUPT_NO_FLAC_MAGIC: &str =
        "InvalidAudioPayload: Qobuz FLAC format_id=6 stream has no FLAC magic";

    /// Ídem para el formato MP3 solicitado.
    pub const CORRUPT_NO_MP3_MAGIC: &str =
        "InvalidAudioPayload: Qobuz format_id=5 stream has no MP3 magic";

    /// Marcador histórico previo a la taxonomía (commit 3982dcf). Quedan filas
    /// en `download_queue` con este texto, así que debe clasificar igual.
    pub const CORRUPT_LEGACY_BIT_PERFECT: &str =
        "Qobuz download failed: Downloaded audio failed bit-perfect FLAC magic verification";

    /// Marcador de `download/tidal.rs` (verificación de magia FLAC/ISOBMFF).
    pub const CORRUPT_TIDAL: &str = "Downloaded audio failed FLAC/ISOBMFF magic verification";
}

#[test]
fn preview_payload_is_retryable_and_never_auth_or_malformed() {
    for msg in [
        producers::PREVIEW_MP3_INSTEAD_OF_FLAC,
        producers::PREVIEW_SHORT_FLAC,
        // El orquestador concatena el diagnóstico del fallback después del
        // fallo original (ver `handle_qobuz_fallback`); el marcador debe seguir
        // ganando para que el error original no quede enterrado.
        "QobuzPreviewPayload: Qobuz delivered MP3 instead of requested FLAC format_id=6: stream URL returned lossy audio, not a full lossless track; Tidal equivalent download failed: no confirmed equivalent Tidal edition found",
    ] {
        let t = classify_qobuz(msg);
        assert!(
            matches!(t, ErrorTaxonomy::QobuzPreviewPayload { .. }),
            "un preview debe clasificarse como QobuzPreviewPayload, fue {:?} para: {}",
            t,
            msg
        );
        assert!(
            t.is_retryable(),
            "el preview es entitlement parcial: alternar cuenta/proveedor sirve"
        );
        assert!(
            !t.is_terminal(),
            "QobuzPreviewPayload no es terminal (debe permitir el fallback)"
        );
        assert!(
            !t.requires_user_action(),
            "un preview de Qobuz no es un veredicto de credenciales"
        );
        assert_eq!(
            t.max_attempts(),
            1,
            "el presupuesto debe ser acotado: un reintento, no un bucle de transporte"
        );
    }
}

#[test]
fn preview_payload_does_not_say_fallback_is_a_stale_source_or_quality_rejection() {
    // El orquestador mira estos fragmentos para decidir "fuente obsoleta" o
    // "calidad rechazada". El preview NO es ninguna de las dos cosas.
    let t = classify_qobuz(producers::PREVIEW_MP3_INSTEAD_OF_FLAC);
    let msg = t.ui_message().to_lowercase();
    assert!(!msg.contains("rejectedquality"), "no es rechazo de calidad");
    assert!(!msg.contains("401"), "no es un fallo de credenciales");
    assert!(
        !msg.contains("malformed"),
        "no es JSON malformado del proveedor"
    );
}

#[test]
fn corrupt_payload_is_terminal_and_not_a_preview() {
    for msg in [
        producers::CORRUPT_NO_FLAC_MAGIC,
        producers::CORRUPT_NO_MP3_MAGIC,
        producers::CORRUPT_LEGACY_BIT_PERFECT,
    ] {
        let t = classify_qobuz(msg);
        assert!(
            matches!(t, ErrorTaxonomy::AudioValidationFailed { .. }),
            "bytes no-audio deben ser AudioValidationFailed, fue {:?} para: {}",
            t,
            msg
        );
        assert!(
            !t.is_retryable(),
            "reintentar la misma URL devuelve los mismos bytes corruptos"
        );
        assert!(t.is_terminal(), "payload corrupto es terminal");
        assert!(
            !t.requires_user_action(),
            "no requiere acción del usuario: no es un problema de credenciales"
        );
    }
}

#[test]
fn corrupt_payload_is_distinct_from_preview() {
    let preview = classify_qobuz(producers::PREVIEW_MP3_INSTEAD_OF_FLAC);
    let corrupt = classify_qobuz(producers::CORRUPT_NO_FLAC_MAGIC);

    assert!(
        matches!(preview, ErrorTaxonomy::QobuzPreviewPayload { .. }),
        "el preview es su propia clase"
    );
    assert!(
        matches!(corrupt, ErrorTaxonomy::AudioValidationFailed { .. }),
        "el payload corrupto es otra clase distinta"
    );
    assert!(
        preview.is_retryable() != corrupt.is_retryable(),
        "los dos veredictos no pueden coincidir: reintentable vs terminal"
    );
}

#[test]
fn tidal_corrupt_magic_is_classified_the_same_way() {
    let t = classify_tidal(producers::CORRUPT_TIDAL);
    assert!(
        matches!(t, ErrorTaxonomy::AudioValidationFailed { .. }),
        "Tidal debe clasificar igual, fue {:?}",
        t
    );
    assert!(t.is_terminal());
    assert!(
        !t.requires_user_action(),
        "no es un problema de sesión de Tidal"
    );
}

#[test]
fn network_failures_still_bypass_the_audio_verdict() {
    // Los fallos de transporte NUNCA deben reinterpretarse como veredicto de
    // audio: son reintentables. El escáner de corrupción no debe secuestrarlos.
    for msg in [
        "NetworkExhausted: Qobuz network stream exhausted retries: connection reset by peer",
        "request timeout",
        "Qobuz returned HTTP 503",
        "Qobuz returned HTTP 429",
    ] {
        let t = classify_qobuz(msg);
        assert!(
            t.is_retryable(),
            "{:?} debe seguir siendo reintentable (no AudioValidationFailed)",
            t
        );
        assert!(
            !matches!(t, ErrorTaxonomy::AudioValidationFailed { .. }),
            "un corte de red no es payload corrupto: {}",
            msg
        );
    }
}

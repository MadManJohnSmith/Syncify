//! Modelo puro de la política de composición: precedencia, fallback y corte del bucle.
//!
//! El orden de decisión es override explícito > preferencia persistida > sonda >
//! fallback compatible. Un intento acelerado que no llega a la UI deja un
//! marcador por clave de fingerprint; mientras siga ahí, `Automático` no vuelve
//! a intentarlo. Nada de este módulo lee el entorno, escribe ficheros ni crea el
//! webview: #54 conecta la decisión antes de construir Tauri, y aquí solo se
//! prueba que la política no se invierta ni repita un fallo de arranque.

use serde::{Deserialize, Serialize};

#[cfg(test)]
use super::report::Verdict;

/// Preferencia de composición solicitada.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum CompositionMode {
    /// Fuerza la ruta sin EGL que la app usa desde el workaround de Iris Xe.
    Compatible,
    /// Solo acelera con sonda positiva, fingerprint completo y sin fallo previo.
    Auto,
    /// Opt-in explícito: no consulta la sonda ni el marcador de fallo.
    Accelerated,
}

impl CompositionMode {
    /// Acepta los valores persistidos y de override; cualquier otra cosa es `None`
    /// para que el llamante conserve la preferencia almacenada.
    pub fn parse(value: &str) -> Option<Self> {
        match value.trim().to_ascii_lowercase().as_str() {
            "compatible" => Some(Self::Compatible),
            "auto" => Some(Self::Auto),
            "accelerated" => Some(Self::Accelerated),
            _ => None,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Compatible => "compatible",
            Self::Auto => "auto",
            Self::Accelerated => "accelerated",
        }
    }
}

/// Ruta de render efectiva que el arranque debe aplicar.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum RenderingRoute {
    Compatible,
    Accelerated,
}

impl RenderingRoute {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Compatible => "compatible",
            Self::Accelerated => "accelerated",
        }
    }
}

/// Resultado de la sonda de arranque, ya sin datos de evidencia.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ProbeOutcome {
    Supported,
    Unsupported,
    Inconclusive,
    Skipped,
}

impl ProbeOutcome {
    /// Traduce el veredicto de la sonda a los términos de la política. Vive
    /// bajo `cfg(test)` hasta que #54 conecte el resultado real: así el módulo
    /// de producción no arrastra un mapeo todavía sin llamante.
    #[cfg(test)]
    pub(crate) fn from_verdict(verdict: Verdict) -> Self {
        match verdict {
            Verdict::Supported => Self::Supported,
            Verdict::Unsupported => Self::Unsupported,
            Verdict::Inconclusive => Self::Inconclusive,
            Verdict::Skipped => Self::Skipped,
        }
    }
}

/// Marcador persistible de un arranque acelerado que no llegó a la UI.
///
/// Solo bloquea la clave de fingerprint que falló: una actualización de driver,
/// WebKitGTK, Mesa o el cambio de sesión produce otra clave y permite un intento
/// nuevo. Un arranque acelerado completo lo borra; un cierre de energía no lo
/// borra, que es el lado conservador. El marcador guarda la clave, nunca datos
/// del equipo.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct AttemptLedger {
    failed_fingerprint: Option<String>,
}

impl AttemptLedger {
    pub fn blocks_auto_acceleration(&self, fingerprint: &str) -> bool {
        self.failed_fingerprint.as_deref() == Some(fingerprint)
    }

    /// Idempotente: repetir el mismo fallo no acumula intentos ni estado.
    pub fn record_failure(&mut self, fingerprint: &str) {
        self.failed_fingerprint = Some(fingerprint.to_string());
    }

    pub fn record_success(&mut self) {
        self.failed_fingerprint = None;
    }

    pub fn failed_fingerprint(&self) -> Option<&str> {
        self.failed_fingerprint.as_deref()
    }
}

/// Entradas de la decisión. El override ya viene parseado: el nombre del
/// argumento o variable que lo expone pertenece a la recuperación de #54.
#[derive(Debug, Clone, Copy)]
pub struct PolicyInput<'a> {
    pub override_mode: Option<CompositionMode>,
    pub preference: CompositionMode,
    pub probe: ProbeOutcome,
    pub fingerprint_complete: bool,
    pub fingerprint: &'a str,
    pub ledger: &'a AttemptLedger,
}

/// Decisión registrable: modo solicitado, ruta efectiva y razón estable.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Decision {
    pub mode: CompositionMode,
    pub route: RenderingRoute,
    pub reason: &'static str,
}

fn auto_decision(input: &PolicyInput<'_>) -> Decision {
    if input.ledger.blocks_auto_acceleration(input.fingerprint) {
        return Decision {
            mode: CompositionMode::Auto,
            route: RenderingRoute::Compatible,
            reason: "previous-failure-fingerprint",
        };
    }
    if !input.fingerprint_complete {
        return Decision {
            mode: CompositionMode::Auto,
            route: RenderingRoute::Compatible,
            reason: "fingerprint-incomplete",
        };
    }
    match input.probe {
        ProbeOutcome::Supported => Decision {
            mode: CompositionMode::Auto,
            route: RenderingRoute::Accelerated,
            reason: "probe-supported",
        },
        ProbeOutcome::Unsupported => Decision {
            mode: CompositionMode::Auto,
            route: RenderingRoute::Compatible,
            reason: "probe-unsupported",
        },
        ProbeOutcome::Inconclusive => Decision {
            mode: CompositionMode::Auto,
            route: RenderingRoute::Compatible,
            reason: "probe-inconclusive",
        },
        ProbeOutcome::Skipped => Decision {
            mode: CompositionMode::Auto,
            route: RenderingRoute::Compatible,
            reason: "probe-skipped",
        },
    }
}

/// Única puerta de la política. Un override explícito manda sobre la preferencia;
/// la preferencia manda sobre la sonda; la sonda solo puede acelerar en `Auto`.
#[must_use]
pub fn decide(input: &PolicyInput<'_>) -> Decision {
    let overridden = input.override_mode.is_some();
    match input.override_mode.unwrap_or(input.preference) {
        CompositionMode::Compatible => Decision {
            mode: CompositionMode::Compatible,
            route: RenderingRoute::Compatible,
            reason: if overridden {
                "override-compatible"
            } else {
                "preference-compatible"
            },
        },
        CompositionMode::Accelerated => Decision {
            mode: CompositionMode::Accelerated,
            route: RenderingRoute::Accelerated,
            reason: if overridden {
                "override-accelerated"
            } else {
                "preference-accelerated"
            },
        },
        CompositionMode::Auto => auto_decision(input),
    }
}

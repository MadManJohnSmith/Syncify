//! Contrato tipado del veredicto y clasificación conservadora de la evidencia.

use serde::{Deserialize, Serialize};

pub(crate) const PROBE_CONTRACT_VERSION: u32 = 1;
pub(crate) const PROBE_FIXTURE_ID: &str = "wk02-red-marker-v1";
pub(crate) const RED_PIXELS_PRESENT: u64 = 300;
pub(crate) const CHANGED_PIXELS_PRESENT: u64 = 300;
pub(crate) const BLACK_MAX_CHANNEL: u8 = 8;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub(crate) enum Backend {
    X11,
    Wayland,
}

impl Backend {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::X11 => "x11",
            Self::Wayland => "wayland",
        }
    }

    pub(crate) fn parse(value: &str) -> Option<Self> {
        match value.trim().to_ascii_lowercase().as_str() {
            "x11" => Some(Self::X11),
            "wayland" => Some(Self::Wayland),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub(crate) enum Verdict {
    Supported,
    Unsupported,
    Inconclusive,
    Skipped,
}

impl Verdict {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::Supported => "supported",
            Self::Unsupported => "unsupported",
            Self::Inconclusive => "inconclusive",
            Self::Skipped => "skipped",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum FrameAssessment {
    ContentObserved,
    RenderedNotPresented,
    PaintObservedNoPixelProof,
    SuspectedBlack,
    CaptureUnavailable,
    NotPresented,
    Unknown,
}

impl FrameAssessment {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::ContentObserved => "content_observed",
            Self::RenderedNotPresented => "rendered_not_presented",
            Self::PaintObservedNoPixelProof => "paint_observed_no_pixel_proof",
            Self::SuspectedBlack => "suspected_black",
            Self::CaptureUnavailable => "capture_unavailable",
            Self::NotPresented => "not_presented",
            Self::Unknown => "unknown",
        }
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub(crate) struct PixelEvidence {
    pub(crate) available: bool,
    pub(crate) capture_supported: bool,
    pub(crate) width: u32,
    pub(crate) height: u32,
    pub(crate) red_px: u64,
    pub(crate) colored_px: u64,
    pub(crate) max_channel: u8,
    pub(crate) changed_px: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct ProbeEvidence {
    pub(crate) schema_version: u32,
    pub(crate) fixture_id: String,
    pub(crate) requested_backend: Backend,
    pub(crate) observed_backend: Option<Backend>,
    pub(crate) ready: bool,
    pub(crate) hidden: Option<bool>,
    pub(crate) viewable: bool,
    pub(crate) frames_delta: u64,
    pub(crate) paints_delta: u64,
    pub(crate) window: PixelEvidence,
    pub(crate) window_red_px_t1: u64,
    pub(crate) snapshot: PixelEvidence,
    pub(crate) acceleration_policy: String,
    pub(crate) runtime_errors: Vec<String>,
}

impl Default for ProbeEvidence {
    fn default() -> Self {
        Self {
            schema_version: PROBE_CONTRACT_VERSION,
            fixture_id: PROBE_FIXTURE_ID.into(),
            requested_backend: Backend::X11,
            observed_backend: None,
            ready: false,
            hidden: None,
            viewable: false,
            frames_delta: 0,
            paints_delta: 0,
            window: PixelEvidence::default(),
            window_red_px_t1: 0,
            snapshot: PixelEvidence::default(),
            acceleration_policy: "unknown".into(),
            runtime_errors: Vec::new(),
        }
    }
}

#[derive(Debug, Clone)]
pub(crate) struct Assessment {
    pub(crate) verdict: Verdict,
    pub(crate) frame: FrameAssessment,
    pub(crate) reason: &'static str,
}

/// Solo una captura X11 con marcador en ambos instantes y cambio entre frames
/// es evidencia positiva. Wayland conserva `after-paint` como corroboración,
/// nunca como prueba de píxel ni como negativo de presentación.
pub(crate) fn assess(evidence: &ProbeEvidence) -> Assessment {
    if evidence.schema_version != PROBE_CONTRACT_VERSION {
        return Assessment {
            verdict: Verdict::Inconclusive,
            frame: FrameAssessment::Unknown,
            reason: "probe-contract-mismatch",
        };
    }
    if evidence.fixture_id != PROBE_FIXTURE_ID {
        return Assessment {
            verdict: Verdict::Inconclusive,
            frame: FrameAssessment::Unknown,
            reason: "probe-fixture-mismatch",
        };
    }
    if evidence.observed_backend != Some(evidence.requested_backend) {
        return Assessment {
            verdict: Verdict::Inconclusive,
            frame: FrameAssessment::Unknown,
            reason: "backend-mismatch",
        };
    }
    if !evidence.acceleration_policy.eq_ignore_ascii_case("always") {
        return Assessment {
            verdict: Verdict::Inconclusive,
            frame: FrameAssessment::Unknown,
            reason: "acceleration-policy-not-always",
        };
    }
    if evidence.runtime_errors.iter().any(|error| {
        error.starts_with("web-process-terminated:")
            || error == "load-finished-without-ready"
            || error == "child-deadline"
    }) {
        return Assessment {
            verdict: Verdict::Inconclusive,
            frame: FrameAssessment::Unknown,
            reason: "probe-runtime-error",
        };
    }

    if evidence.requested_backend == Backend::Wayland {
        if !evidence.ready {
            return Assessment {
                verdict: Verdict::Inconclusive,
                frame: FrameAssessment::NotPresented,
                reason: "wayland-web-content-not-ready",
            };
        }
        if evidence.hidden == Some(true) || !evidence.viewable {
            return Assessment {
                verdict: Verdict::Inconclusive,
                frame: FrameAssessment::NotPresented,
                reason: "wayland-window-not-visible",
            };
        }
        if evidence.paints_delta == 0 {
            return Assessment {
                verdict: Verdict::Inconclusive,
                frame: if evidence.snapshot.available
                    && evidence.snapshot.red_px >= RED_PIXELS_PRESENT
                {
                    FrameAssessment::RenderedNotPresented
                } else {
                    FrameAssessment::NotPresented
                },
                reason: "wayland-no-gtk-presentation",
            };
        }
        if evidence.frames_delta == 0 {
            return Assessment {
                verdict: Verdict::Inconclusive,
                frame: FrameAssessment::NotPresented,
                reason: "wayland-web-frames-stalled",
            };
        }
        return if evidence.snapshot.available && evidence.snapshot.red_px >= RED_PIXELS_PRESENT {
            Assessment {
                verdict: Verdict::Inconclusive,
                frame: FrameAssessment::PaintObservedNoPixelProof,
                reason: "wayland-paint-without-pixel-proof",
            }
        } else {
            Assessment {
                verdict: Verdict::Inconclusive,
                frame: FrameAssessment::Unknown,
                reason: "wayland-snapshot-missing",
            }
        };
    }

    if !evidence.ready {
        return Assessment {
            verdict: Verdict::Unsupported,
            frame: FrameAssessment::NotPresented,
            reason: "web-content-not-ready",
        };
    }
    if evidence.hidden == Some(true) || !evidence.viewable {
        return Assessment {
            verdict: Verdict::Unsupported,
            frame: FrameAssessment::NotPresented,
            reason: "window-not-visible",
        };
    }
    if evidence.paints_delta == 0 {
        let rendered =
            evidence.snapshot.available && evidence.snapshot.red_px >= RED_PIXELS_PRESENT;
        return Assessment {
            verdict: Verdict::Unsupported,
            frame: if rendered {
                FrameAssessment::RenderedNotPresented
            } else {
                FrameAssessment::NotPresented
            },
            reason: "no-gtk-presentation",
        };
    }
    if evidence.frames_delta == 0 {
        return Assessment {
            verdict: Verdict::Unsupported,
            frame: FrameAssessment::NotPresented,
            reason: "web-frames-stalled",
        };
    }

    let window = &evidence.window;
    if !window.capture_supported || !window.available || window.width == 0 || window.height == 0 {
        return Assessment {
            verdict: Verdict::Inconclusive,
            frame: FrameAssessment::CaptureUnavailable,
            reason: "x11-window-capture-unavailable",
        };
    }
    let Some(changed) = window.changed_px else {
        return Assessment {
            verdict: Verdict::Inconclusive,
            frame: FrameAssessment::Unknown,
            reason: "x11-frame-change-missing",
        };
    };
    if window.red_px >= RED_PIXELS_PRESENT
        && evidence.window_red_px_t1 >= RED_PIXELS_PRESENT
        && changed >= CHANGED_PIXELS_PRESENT
    {
        return Assessment {
            verdict: Verdict::Supported,
            frame: FrameAssessment::ContentObserved,
            reason: "x11-visible-marker-and-change",
        };
    }
    if window.max_channel <= BLACK_MAX_CHANNEL && window.colored_px == 0 {
        return Assessment {
            verdict: Verdict::Inconclusive,
            frame: FrameAssessment::SuspectedBlack,
            reason: "x11-black-without-compatible-control",
        };
    }
    Assessment {
        verdict: Verdict::Inconclusive,
        frame: FrameAssessment::Unknown,
        reason: "x11-content-unrecognized",
    }
}

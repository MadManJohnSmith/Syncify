//! Reconciliación de la calidad física de una descarga.
//!
//! El fichero descargado es la fuente de verdad: si la inspección local puede
//! leerlo, sus valores de bit depth, sample rate, canales y bitrate sustituyen
//! a los que declaró el proveedor antes de evaluar la política de calidad. Sin
//! inspección, se conservan los valores del resultado y se etiqueta por
//! extensión.
//!
//! Vive fuera del orquestador para que los adaptadores de proveedor apliquen
//! exactamente la misma reconciliación que los flujos primarios.

use crate::download::progress::{DownloadRequest, DownloadResult};

/// Reconciles the downloaded file's physical audio metrics from disk (sample_rate, bit_depth, channels, bitrate)
/// and performs canonical quality policy evaluation against the physical reality.
pub fn reconcile_physical_audio_quality(res: &mut DownloadResult, request: &DownloadRequest) {
    let path = std::path::Path::new(&res.file_path);
    let phys_info = crate::download::audio_inspector::inspect_physical_audio_file(path);

    let (
        verified_bd,
        verified_sr,
        verified_fmt,
        verified_channels,
        verified_bitrate,
        quality_label,
    ) = if let Some(ref phys) = phys_info {
        (
            phys.bit_depth,
            phys.sample_rate,
            phys.format.clone(),
            Some(phys.channels),
            phys.bitrate,
            phys.quality_string(),
        )
    } else {
        let is_m4a = res.file_path.to_lowercase().ends_with(".m4a");
        let is_mp3 = res.file_path.to_lowercase().ends_with(".mp3");
        let fmt = if is_m4a {
            "AAC"
        } else if is_mp3 {
            "MP3"
        } else {
            "FLAC"
        };
        let label = if fmt == "FLAC" {
            format!(
                "FLAC {}-bit / {:.1}kHz",
                res.bit_depth,
                res.sample_rate as f64 / 1000.0
            )
        } else {
            format!("{} 320kbps", fmt)
        };
        (
            res.bit_depth,
            res.sample_rate,
            fmt.to_string(),
            res.channels.or(Some(2)),
            res.bitrate,
            label,
        )
    };

    res.bit_depth = verified_bd;
    res.sample_rate = verified_sr;
    if res.channels.is_none() {
        res.channels = verified_channels;
    }
    if res.bitrate.is_none() {
        res.bitrate = verified_bitrate;
    }

    let q_eval = syncify_core_domain::quality::QualityPolicy::evaluate_stream_resolution(
        &request.quality,
        &quality_label,
        &verified_fmt,
        verified_bd,
        verified_sr as f64,
        res.origin_service.as_deref().unwrap_or(&res.service),
        &res.service,
        request.strict_quality,
        request.allow_fallback,
    );
    res.quality_decision = Some(q_eval);
}

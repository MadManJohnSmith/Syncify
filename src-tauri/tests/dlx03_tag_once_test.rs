//! DLX-03 (#44): inventario offline de las relecturas de etiquetado y métricas.
//!
//! La línea base reproduce las llamadas que había en el árbol antes del cambio:
//! Tidal verificaba dos veces los tags tras escribirlos y el resultado final se
//! inspeccionaba para el gate, para calidad estricta, para el ledger y para las
//! métricas post-promoción. Las duraciones se agregan con la unión de intervalos
//! de DLX-01; los contadores son la evidencia determinista del trabajo repetido.

use syncify_flac_writer::{
    apply_and_verify_flac_tags_with_options, audit_flac_stage, verify_flac_tags, FlacMetadata,
    TagWriteOptions,
};
use syncify_tauri_lib::download::audio_inspector::inspect_physical_audio_file;
use syncify_tauri_lib::download::progress::{DownloadPhase, DownloadPhaseTracker};
use syncify_tauri_lib::download::timeline::TimelineArtifact;
use syncify_tauri_lib::download::verification::{verify_delivery, ExpectedDelivery};

const AUDIO: &[u8] = include_bytes!("fixtures/silence.flac");

fn metadata() -> FlacMetadata {
    // Un comentario de 8 MiB hace visible el coste de copiar/parsear los
    // VORBIS_COMMENT en cada relectura, sin depender de tiempos de red. La
    // portada es un PNG 1×1 real para que el writer no intente convertirla.
    const PNG: &[u8] = &[
        0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A, 0x00, 0x00, 0x00, 0x0D, b'I', b'H', b'D',
        b'R', 0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x01, 0x08, 0x06, 0x00, 0x00, 0x00, 0x1F,
        0x15, 0xC4, 0x89, 0x00, 0x00, 0x00, 0x0D, b'I', b'D', b'A', b'T', 0x78, 0xDA, 0x63, 0x64,
        0xF8, 0xCF, 0xC0, 0x00, 0x00, 0x01, 0x01, 0x01, 0x00, 0x18, 0xDD, 0x8D, 0xB0, 0x00, 0x00,
        0x00, 0x00, b'I', b'E', b'N', b'D', 0xAE, 0x42, 0x60, 0x82,
    ];
    FlacMetadata {
        title: "DLX-03".to_string(),
        artist: "Offline".to_string(),
        album: "Tag once".to_string(),
        comment: Some("x".repeat(8 * 1024 * 1024)),
        cover_data: Some(PNG.to_vec()),
        ..Default::default()
    }
}

fn copy_fixture(dir: &tempfile::TempDir, name: &str) -> std::path::PathBuf {
    let path = dir.path().join(name);
    std::fs::write(&path, AUDIO).expect("copia fixture FLAC");
    path
}

fn assert_union_report(artifact: &TimelineArtifact) {
    assert_eq!(
        artifact.union_ms + artifact.huecos_ms,
        artifact.total_pared_ms
    );
    assert!(artifact.etapas.iter().all(|p| p.end_ms >= p.start_ms));
}

#[test]
fn baseline_tidal_relee_y_verifica_los_tags_tres_veces() {
    let dir = tempfile::tempdir().expect("tmpdir");
    let path = copy_fixture(&dir, "tidal-baseline.flac");
    let mut tracker = DownloadPhaseTracker::new();

    tracker.start_phase(DownloadPhase::Tagging);
    let written =
        apply_and_verify_flac_tags_with_options(&path, &metadata(), &TagWriteOptions::default())
            .expect("primera verificación");
    assert!(written.tags_match);

    tracker.start_phase(DownloadPhase::Tagging);
    let duplicate = verify_flac_tags(&path, &metadata()).expect("segunda verificación");
    assert!(duplicate.tags_match);

    tracker.start_phase(DownloadPhase::Tagging);
    let audit = audit_flac_stage("post_tagging", &path).expect("auditoría de portada");
    assert_eq!(audit.picture_count, 1);

    let artifact = TimelineArtifact::from_timings(
        "dlx03-baseline-tidal",
        "tidal",
        &tracker.finish_completed(),
    );
    assert_union_report(&artifact);
    assert_eq!(artifact.etapas.len(), 4); // tres fases + Completed
    println!(
        "DLX-03 baseline Tidal: relecturas=3 (apply+verify=1, verify duplicado=1, auditoría=1)\n{}",
        artifact.report()
    );
}

#[test]
fn baseline_resultado_final_relee_metricas_y_tags_cinco_veces() {
    let dir = tempfile::tempdir().expect("tmpdir");
    let path = copy_fixture(&dir, "final-baseline.flac");
    apply_and_verify_flac_tags_with_options(&path, &metadata(), &TagWriteOptions::default())
        .expect("prepara entrega");
    let mut tracker = DownloadPhaseTracker::new();

    tracker.start_phase(DownloadPhase::Persisting);
    verify_delivery(&path, &ExpectedDelivery::default(), false, 0.0).expect("gate de entrega"); // inspección física + lectura de tags = 2

    tracker.start_phase(DownloadPhase::Persisting);
    inspect_physical_audio_file(&path).expect("calidad estricta"); // 1

    tracker.start_phase(DownloadPhase::Persisting);
    inspect_physical_audio_file(&path).expect("métricas del ledger"); // 1

    tracker.start_phase(DownloadPhase::Persisting);
    metaflac::Tag::read_from_path(&path).expect("métricas post-promoción"); // 1

    let artifact = TimelineArtifact::from_timings(
        "dlx03-baseline-final",
        "qobuz/tidal",
        &tracker.finish_completed(),
    );
    assert_union_report(&artifact);
    assert_eq!(artifact.etapas.len(), 5); // cuatro fases + Completed
    println!(
        "DLX-03 baseline resultado final: relecturas=5 (gate=2, estricta=1, ledger=1, post-promoción=1)\n{}",
        artifact.report()
    );
}

//! DLX-03 (#44): medición offline del trabajo de etiquetado y verificación.
//!
//! La línea base reproduce las llamadas del árbol anterior. La candidata usa los
//! veredictos y la instantánea física compartida. Los contadores son la evidencia
//! determinista; las duraciones se comparan con la unión de intervalos de DLX-01.

use syncify_flac_writer::{
    apply_and_verify_flac_tags_with_options, audit_flac_stage, verify_flac_tags, FlacMetadata,
    TagWriteOptions,
};
use syncify_tauri_lib::download::audio_inspector::inspect_physical_audio_file;
use syncify_tauri_lib::download::progress::{
    DownloadPhase, DownloadPhaseTracker, DownloadRequest, DownloadResult,
};
use syncify_tauri_lib::download::timeline::{compare_and_report, TimelineArtifact};
use syncify_tauri_lib::download::verification::{
    verify_delivery, verify_delivery_snapshot, ExpectedDelivery,
};
use syncify_tauri_lib::download::PhysicalAudioMetadata;
use syncify_tauri_lib::download::{DownloadOrchestrator, ProviderDelivery};

const AUDIO: &[u8] = include_bytes!("fixtures/silence.flac");

#[derive(Debug, PartialEq)]
struct Evidence {
    tags_match: bool,
    cover_present: bool,
    physical: Option<PhysicalAudioMetadata>,
}

struct Measurement {
    artifact: TimelineArtifact,
    reads: usize,
    evidence: Evidence,
}

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

fn prepare(path: &std::path::Path) {
    apply_and_verify_flac_tags_with_options(path, &metadata(), &TagWriteOptions::default())
        .expect("prepara entrega");
}

fn finish(
    tracker: &mut DownloadPhaseTracker,
    run: &str,
    service: &str,
    reads: usize,
    evidence: Evidence,
) -> Measurement {
    let artifact = TimelineArtifact::from_timings(run, service, &tracker.finish_completed());
    assert_eq!(
        artifact.union_ms + artifact.huecos_ms,
        artifact.total_pared_ms
    );
    assert!(artifact.etapas.iter().all(|p| p.end_ms >= p.start_ms));
    Measurement {
        artifact,
        reads,
        evidence,
    }
}

fn measure_tidal_baseline() -> Measurement {
    let dir = tempfile::tempdir().expect("tmpdir");
    let path = copy_fixture(&dir, "tidal-baseline.flac");
    let mut tracker = DownloadPhaseTracker::new();

    tracker.start_phase(DownloadPhase::Tagging);
    let written =
        apply_and_verify_flac_tags_with_options(&path, &metadata(), &TagWriteOptions::default())
            .expect("primera verificación");
    tracker.start_phase(DownloadPhase::Tagging);
    let duplicate = verify_flac_tags(&path, &metadata()).expect("segunda verificación");
    tracker.start_phase(DownloadPhase::Tagging);
    let audit = audit_flac_stage("post_tagging", &path).expect("auditoría de portada");

    assert_eq!(written.tags_match, duplicate.tags_match);
    assert_eq!(audit.picture_count, 1);
    finish(
        &mut tracker,
        "dlx03-baseline-tidal",
        "tidal",
        3,
        Evidence {
            tags_match: written.tags_match,
            cover_present: written.cover_present,
            physical: None,
        },
    )
}

fn measure_tidal_candidate() -> Measurement {
    let dir = tempfile::tempdir().expect("tmpdir");
    let path = copy_fixture(&dir, "tidal-candidate.flac");
    let mut tracker = DownloadPhaseTracker::new();

    tracker.start_phase(DownloadPhase::Tagging);
    let written =
        apply_and_verify_flac_tags_with_options(&path, &metadata(), &TagWriteOptions::default())
            .expect("única verificación de tags");
    tracker.start_phase(DownloadPhase::Tagging);
    let audit = audit_flac_stage("post_tagging", &path).expect("auditoría de portada");
    assert_eq!(audit.picture_count, 1);

    finish(
        &mut tracker,
        "dlx03-candidata-tidal",
        "tidal",
        2,
        Evidence {
            tags_match: written.tags_match,
            cover_present: written.cover_present,
            physical: None,
        },
    )
}

fn measure_final_baseline() -> Measurement {
    let dir = tempfile::tempdir().expect("tmpdir");
    let path = copy_fixture(&dir, "final-baseline.flac");
    prepare(&path);
    let mut tracker = DownloadPhaseTracker::new();

    tracker.start_phase(DownloadPhase::Promotion);
    metaflac::Tag::read_from_path(&path).expect("métricas post-promoción"); // 1
    tracker.start_phase(DownloadPhase::Persisting);
    let physical = inspect_physical_audio_file(&path).expect("reconciliación proveedor"); // 1
    tracker.start_phase(DownloadPhase::Persisting);
    inspect_physical_audio_file(&path).expect("gateway"); // 1
    tracker.start_phase(DownloadPhase::Persisting);
    inspect_physical_audio_file(&path).expect("reconciliación orquestador"); // 1
    tracker.start_phase(DownloadPhase::Persisting);
    let delivered =
        verify_delivery(&path, &ExpectedDelivery::default(), false, 0.0).expect("gate de entrega"); // física + tags = 2
    tracker.start_phase(DownloadPhase::Persisting);
    inspect_physical_audio_file(&path).expect("calidad estricta"); // 1
    tracker.start_phase(DownloadPhase::Persisting);
    inspect_physical_audio_file(&path).expect("métricas del ledger"); // 1

    finish(
        &mut tracker,
        "dlx03-baseline-final",
        "qobuz/tidal",
        8,
        Evidence {
            tags_match: delivered.title.is_some(),
            cover_present: delivered.has_valid_cover,
            physical: Some(physical),
        },
    )
}

fn measure_final_candidate() -> Measurement {
    let dir = tempfile::tempdir().expect("tmpdir");
    let path = copy_fixture(&dir, "final-candidate.flac");
    prepare(&path);
    let validated =
        syncify_core_domain::byte_validators::AudioByteValidator::parse_flac_streaminfo(AUDIO)
            .expect("STREAMINFO validado antes de etiquetar");
    let mut tracker = DownloadPhaseTracker::new();

    tracker.start_phase(DownloadPhase::Persisting);
    let physical = inspect_physical_audio_file(&path).expect("reconciliación proveedor"); // 1
    assert_eq!(physical.bit_depth, validated.bits_per_sample as i32);
    assert_eq!(physical.sample_rate, validated.sample_rate as i32);
    let mut result = DownloadResult {
        file_path: path.to_string_lossy().to_string(),
        service: "qobuz".to_string(),
        physical_audio: Some(physical.clone()),
        ..Default::default()
    };
    let delivery = ProviderDelivery::from_result(result.clone(), None);
    result = delivery.result;
    DownloadOrchestrator::reconcile_physical_audio_quality(
        &mut result,
        &DownloadRequest::default(),
    );
    tracker.start_phase(DownloadPhase::Persisting);
    let verified = verify_delivery_snapshot(
        &path,
        &ExpectedDelivery::default(),
        false,
        0.0,
        result.physical_audio.clone(),
    )
    .expect("gate con instantánea compartida"); // solo tags = 1
    assert_eq!(verified.parser_reads, 1);

    finish(
        &mut tracker,
        "dlx03-candidata-final",
        "qobuz/tidal",
        2,
        Evidence {
            tags_match: verified.delivered.title.is_some(),
            cover_present: verified.delivered.has_valid_cover,
            physical: verified.physical,
        },
    )
}

fn assert_same_evidence_and_union_comparison(
    baseline: &Measurement,
    candidate: &Measurement,
    expected_reads: (usize, usize),
) {
    assert_eq!((baseline.reads, candidate.reads), expected_reads);
    assert_eq!(baseline.evidence, candidate.evidence);
    let comparison = baseline.artifact.compare_runs(&candidate.artifact);
    assert_eq!(
        comparison.total_union_baseline_ms,
        baseline.artifact.union_ms
    );
    assert_eq!(
        comparison.total_union_candidata_ms,
        candidate.artifact.union_ms
    );
    let report = compare_and_report(&baseline.artifact, &candidate.artifact);
    assert!(report.contains("unión de intervalos, no sobre la suma"));
    println!(
        "DLX-03 lecturas {}→{} · evidencia idéntica\n{}",
        baseline.reads, candidate.reads, report
    );
}

#[test]
fn tidal_etiqueta_y_verifica_una_vez_sin_repetir_el_veredicto() {
    let baseline = measure_tidal_baseline();
    let candidate = measure_tidal_candidate();
    assert_same_evidence_and_union_comparison(&baseline, &candidate, (3, 2));
}

#[test]
fn resultado_final_reutiliza_una_instantanea_fisica() {
    let baseline = measure_final_baseline();
    let candidate = measure_final_candidate();
    assert_same_evidence_and_union_comparison(&baseline, &candidate, (8, 2));
}

#[test]
fn reconciliacion_gateway_y_orquestador_no_releen_el_fichero() {
    let dir = tempfile::tempdir().expect("tmpdir");
    let path = copy_fixture(&dir, "cached.flac");
    prepare(&path);
    let mut result = DownloadResult {
        file_path: path.to_string_lossy().to_string(),
        service: "qobuz".to_string(),
        bit_depth: 24,
        sample_rate: 96_000,
        ..Default::default()
    };
    DownloadOrchestrator::reconcile_physical_audio_quality(
        &mut result,
        &DownloadRequest::default(),
    );
    let cached = result.physical_audio.clone().expect("instantánea física");
    let validated =
        syncify_core_domain::byte_validators::AudioByteValidator::parse_flac_streaminfo(AUDIO)
            .expect("STREAMINFO del fixture");
    assert_eq!(cached.bit_depth, validated.bits_per_sample as i32);
    assert_eq!(cached.sample_rate, validated.sample_rate as i32);

    // Si cualquiera de estas capas volviera a abrir el fichero, fallaría o
    // perdería la evidencia. Al borrarlo demostramos que todas consumen la caché.
    std::fs::remove_file(&path).unwrap();
    DownloadOrchestrator::reconcile_physical_audio_quality(
        &mut result,
        &DownloadRequest::default(),
    );
    let delivery = ProviderDelivery::from_result(result.clone(), None);
    assert_eq!(delivery.bit_depth, validated.bits_per_sample as i32);
    assert_eq!(delivery.sample_rate, validated.sample_rate as i32);
    assert_eq!(
        DownloadOrchestrator::verify_post_download_quality_gate(&delivery.result),
        "lossless"
    );
}

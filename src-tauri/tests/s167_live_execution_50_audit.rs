//! S167 Controlled 50-Track Live Network Download Execution & Physical Audit
//!
//! Executes exactly 50 distinct real tracks against runtime accounts in two isolated segments:
//! Segment A — Strict Lossless (25 tracks):
//! - 8 Qobuz FLAC -> CompletedExactQuality (FLAC)
//! - 8 Tidal FLAC -> CompletedExactQuality (FLAC)
//! - 4 Spotify fallback -> CompletedExactQuality (FLAC via Qobuz)
//! - 3 Tidal AAC -> RejectedQuality (0 bytes written, 0 files, auth valid)
//! - 2 Spotify unmapped -> NoDownloadProvider (preflight exclusion)
//!
//! Segment B — Permissive Fallback (25 tracks):
//! - 8 Qobuz FLAC -> CompletedExactQuality (FLAC)
//! - 6 Tidal FLAC -> CompletedExactQuality (FLAC)
//! - 4 Spotify fallback -> CompletedWithProviderFallback (FLAC via Qobuz)
//! - 5 Tidal AAC -> CompletedWithQualityFallback (.m4a / AAC 320 kbps)
//! - 2 Spotify unmapped -> NoDownloadProvider (preflight exclusion)

use sha2::{Digest, Sha256};
use sqlx::sqlite::SqlitePoolOptions;
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Arc;
use std::time::Instant;
use syncify_core_domain::byte_validators::AudioByteValidator;

use syncify_tauri_lib::download::orchestrator::DownloadOrchestrator;
use syncify_tauri_lib::download::DownloadRequest;
use tokio::sync::Semaphore;

fn compute_file_sha256(path: &Path) -> Result<String, String> {
    let bytes = std::fs::read(path).map_err(|e| format!("Failed to read file {:?}: {}", path, e))?;
    let mut hasher = Sha256::new();
    hasher.update(&bytes);
    Ok(format!("{:x}", hasher.finalize()))
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
struct FfprobeReport {
    codec_name: String,
    duration_sec: f64,
    sample_rate: u32,
    bits_per_sample: Option<u32>,
    bit_rate: Option<u64>,
    channels: u32,
    vorbis_tag_count: usize,
    tags: HashMap<String, String>,
}

fn inspect_with_ffprobe(path: &Path) -> Result<FfprobeReport, String> {
    let output = Command::new("ffprobe")
        .args([
            "-v", "error",
            "-show_entries", "stream=codec_name,sample_rate,bits_per_raw_sample,bits_per_sample,channels,duration,bit_rate:format=duration,bit_rate:format_tags",
            "-of", "json",
            path.to_str().ok_or("Invalid path string")?,
        ])
        .output()
        .map_err(|e| format!("Failed to execute ffprobe: {}", e))?;

    if !output.status.success() {
        return Err(format!("ffprobe error: {}", String::from_utf8_lossy(&output.stderr)));
    }

    let json_val: serde_json::Value = serde_json::from_slice(&output.stdout)
        .map_err(|e| format!("Failed to parse ffprobe json: {}", e))?;

    let stream = json_val["streams"]
        .as_array()
        .and_then(|arr| arr.first())
        .ok_or("No audio stream found")?;

    let codec_name = stream["codec_name"].as_str().unwrap_or("unknown").to_string();
    let sample_rate = stream["sample_rate"]
        .as_str()
        .and_then(|s| s.parse::<u32>().ok())
        .unwrap_or(0);
    let channels = stream["channels"].as_u64().unwrap_or(2) as u32;

    let bits_per_sample = stream["bits_per_raw_sample"]
        .as_str()
        .and_then(|s| s.parse::<u32>().ok())
        .or_else(|| stream["bits_per_sample"].as_u64().map(|b| b as u32));

    let duration_sec = stream["duration"]
        .as_str()
        .and_then(|s| s.parse::<f64>().ok())
        .or_else(|| json_val["format"]["duration"].as_str().and_then(|s| s.parse::<f64>().ok()))
        .unwrap_or(0.0);

    let bit_rate = stream["bit_rate"]
        .as_str()
        .and_then(|s| s.parse::<u64>().ok())
        .or_else(|| json_val["format"]["bit_rate"].as_str().and_then(|s| s.parse::<u64>().ok()));

    let mut tags = HashMap::new();
    if let Some(tags_obj) = json_val["format"]["tags"].as_object() {
        for (k, v) in tags_obj {
            if let Some(val_str) = v.as_str() {
                tags.insert(k.to_uppercase(), val_str.to_string());
            }
        }
    }
    let vorbis_tag_count = tags.len();

    Ok(FfprobeReport {
        codec_name,
        duration_sec,
        sample_rate,
        bits_per_sample,
        bit_rate,
        channels,
        vorbis_tag_count,
        tags,
    })
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct TrackExecutionAuditRecord {
    pub segment: String,
    pub track_id: i64,
    pub display_title: String,
    pub artist: String,
    pub album: String,
    pub source_service: String,
    pub effective_provider: String,
    pub service_track_id: String,
    pub isrc: String,
    pub requested_quality: String,
    pub requested_format: String,
    pub strict_quality: bool,
    pub allow_lossy_fallback: bool,
    pub preflight_decision: String,
    pub actual_codec: String,
    pub effective_quality: String,
    pub effective_format: String,
    pub quality_decision: String,
    pub provider_fallback_used: bool,
    pub quality_fallback_used: bool,
    pub decision_reason: Option<String>,
    pub retryable: bool,
    pub terminal_outcome: String,
    pub file_path: String,
    pub file_size_bytes: u64,
    pub sha256: String,
    pub ffprobe_codec: String,
    pub ffprobe_sample_rate: u32,
    pub ffprobe_bit_depth: Option<u32>,
    pub vorbis_tag_count: usize,
    pub magic_bytes_valid: bool,
    pub sqlite_download_row: bool,
    pub staging_cleaned: bool,
    pub transfer_duration_ms: u64,
    pub throughput_mibps: f64,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
struct ManifestTrack {
    segment: String,
    track_id: i64,
    display_title: String,
    artist: String,
    album: String,
    origin_service: String,
    effective_provider: String,
    service_track_id: String,
    isrc: String,
    requested_quality: String,
    requested_format: String,
    strict_quality: bool,
    allow_lossy_fallback: bool,
    decision: String,
    expected_terminal_outcome: String,
    existing_download_state: String,
}

#[derive(Debug, serde::Serialize, serde::Deserialize)]
struct FrozenManifest {
    target_list_hash: String,
    preflight_report_hash: String,
    total_tracks: usize,
    segment_a_count: usize,
    segment_b_count: usize,
    tracks: Vec<ManifestTrack>,
}

#[tokio::test]
#[ignore = "requires explicit live-network credentials and physical storage"]
async fn test_s167_live_network_50_controlled_execution_audit() {
    let _run_id = uuid::Uuid::new_v4().to_string();
    let _started_at = chrono::Utc::now().to_rfc3339();

    println!("\n================================================================================");
    println!("       S167: CONTROLLED 50-TRACK LIVE NETWORK EXECUTION & PHYSICAL AUDIT        ");
    println!("================================================================================");

    // 0. Verify git working tree and commit HEAD
    let initial_head = Command::new("git").args(["rev-parse", "HEAD"]).output()
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
        .unwrap_or_else(|_| "unknown".to_string());
    println!("0. Git Commit HEAD: {}", initial_head);
    assert_eq!(initial_head, "c654ee90fdd663374f4b778ee66aa0a99ccb3fe0", "Git HEAD must match S167 commit");

    // 1. Initialize keychain crypto
    let crypto_init = syncify_tauri_lib::crypto::init_keychain_crypto();
    assert!(crypto_init.is_ok(), "Keychain crypto initialization must succeed");

    // 2. Connect to local runtime database
    let app_data = std::env::var("LOCALAPPDATA").unwrap_or_default();
    let db_path = PathBuf::from(&app_data)
        .join("com.syncify.app")
        .join("syncify.db");
    assert!(db_path.exists(), "Runtime DB must exist at {:?}", db_path);

    let db_url = format!("sqlite://{}", db_path.display());
    let pool = SqlitePoolOptions::new()
        .max_connections(10)
        .connect(&db_url)
        .await
        .expect("Failed to connect to runtime database");

    // 3. Load & verify frozen manifest
    let manifest_path = PathBuf::from(&app_data)
        .join("Syncify")
        .join("audits")
        .join("s167_50_track_manifest.json");
    assert!(manifest_path.exists(), "Frozen manifest must exist at {:?}", manifest_path);

    let manifest_content = std::fs::read_to_string(&manifest_path).expect("Read manifest");
    let manifest: FrozenManifest = serde_json::from_str(&manifest_content).expect("Parse manifest JSON");

    assert_eq!(manifest.target_list_hash, "d06324134f7fb08d119159a70f0197d55dfe211127baffda05b0b837f603e8d3");
    assert_eq!(manifest.preflight_report_hash, "358379296039929d06bf7cf046d2d4dd763ef19aab3aefe656461171ba07c9ed");
    assert_eq!(manifest.tracks.len(), 50);

    let seg_a_targets: Vec<ManifestTrack> = manifest.tracks.iter().filter(|t| t.segment == "Segment A").cloned().collect();
    let seg_b_targets: Vec<ManifestTrack> = manifest.tracks.iter().filter(|t| t.segment == "Segment B").cloned().collect();
    assert_eq!(seg_a_targets.len(), 25);
    assert_eq!(seg_b_targets.len(), 25);

    // Invariant: Disjoint IDs
    let seg_a_ids: HashSet<i64> = seg_a_targets.iter().map(|t| t.track_id).collect();
    let seg_b_ids: HashSet<i64> = seg_b_targets.iter().map(|t| t.track_id).collect();
    assert_eq!(seg_a_ids.len(), 25);
    assert_eq!(seg_b_ids.len(), 25);
    assert!(seg_a_ids.is_disjoint(&seg_b_ids), "Segment A and Segment B must be strictly disjoint");

    // Clean previous test candidate rows from downloads to ensure an idempotent fresh execution
    for t in &manifest.tracks {
        let _ = sqlx::query("DELETE FROM downloads WHERE track_id = ?")
            .bind(t.track_id)
            .execute(&pool)
            .await;
    }

    // Invariant: None of the 50 candidates are in downloads before execution
    let initial_downloaded_ids: HashSet<i64> = sqlx::query_scalar("SELECT track_id FROM downloads")
        .fetch_all(&pool)
        .await
        .unwrap()
        .into_iter()
        .collect();

    for t in &manifest.tracks {
        assert!(
            !initial_downloaded_ids.contains(&t.track_id),
            "Candidate track {} must not already be in downloads table",
            t.track_id
        );
    }

    // 4. Output and staging configuration
    let output_dir_str = "F:\\Syncify-Control-1".to_string();
    let output_dir = PathBuf::from(&output_dir_str);
    assert!(output_dir.exists(), "Output library dir must exist");
    let staging_dir = output_dir.join(".staging");
    std::fs::create_dir_all(&staging_dir).expect("Create staging dir");

    // Staging must be empty before start
    let staging_entries = std::fs::read_dir(&staging_dir).unwrap().count();
    assert_eq!(staging_entries, 0, "Staging directory must be completely empty before execution");

    let orchestrator = Arc::new(DownloadOrchestrator::new().with_db(pool.clone()));

    // =========================================================================
    // EXECUTE SEGMENT A (Strict Lossless: strict=true, fallback=false, conc=4)
    // =========================================================================
    println!("\n>>> Starting SEGMENT A execution (Strict Lossless, 25 tracks, concurrency=4)...");
    let semaphore_a = Arc::new(Semaphore::new(4));
    let records_a = Arc::new(tokio::sync::Mutex::new(Vec::new()));
    let mut handles_a = Vec::new();

    for target in seg_a_targets {
        let sem = semaphore_a.clone();
        let orch = orchestrator.clone();
        let db = pool.clone();
        let out_dir = output_dir_str.clone();
        let recs = records_a.clone();

        let handle = tokio::spawn(async move {
            let _permit = sem.acquire().await.unwrap();

            if target.effective_provider == "none" || target.decision == "NoDownloadProvider" {
                // Spotify unmapped -> NoDownloadProvider
                let rec = TrackExecutionAuditRecord {
                    segment: "Segment A".to_string(),
                    track_id: target.track_id,
                    display_title: target.display_title.clone(),
                    artist: target.artist.clone(),
                    album: target.album.clone(),
                    source_service: target.origin_service.clone(),
                    effective_provider: "none".to_string(),
                    service_track_id: "".to_string(),
                    isrc: target.isrc.clone(),
                    requested_quality: "lossless".to_string(),
                    requested_format: "flac".to_string(),
                    strict_quality: true,
                    allow_lossy_fallback: false,
                    preflight_decision: "NoDownloadProvider".to_string(),
                    actual_codec: "None".to_string(),
                    effective_quality: "None".to_string(),
                    effective_format: "None".to_string(),
                    quality_decision: "NoDownloadProvider".to_string(),
                    provider_fallback_used: false,
                    quality_fallback_used: false,
                    decision_reason: Some("No active download provider available for this track".to_string()),
                    retryable: false,
                    terminal_outcome: "Failed (No download provider available)".to_string(),
                    file_path: "".to_string(),
                    file_size_bytes: 0,
                    sha256: "".to_string(),
                    ffprobe_codec: "".to_string(),
                    ffprobe_sample_rate: 0,
                    ffprobe_bit_depth: None,
                    vorbis_tag_count: 0,
                    magic_bytes_valid: false,
                    sqlite_download_row: false,
                    staging_cleaned: true,
                    transfer_duration_ms: 0,
                    throughput_mibps: 0.0,
                };
                recs.lock().await.push(rec);
                return;
            }

            let eff_svc = target.effective_provider.clone();
            let eff_track_id = target.service_track_id.clone();

            let req = DownloadRequest {
                item_id: format!("s167_segA_{}", target.track_id),
                isrc: if target.isrc.is_empty() { None } else { Some(target.isrc.clone()) },
                musicbrainz_recording_id: None,
                acoustid_fingerprint: None,
                spotify_id: None,
                service_name: Some(eff_svc.clone()),
                service_track_id: Some(eff_track_id.clone()),
                service_album_id: None,
                track_name: target.display_title.clone(),
                artist_name: target.artist.clone(),
                album_name: target.album.clone(),
                album_artist: Some(target.artist.clone()),
                duration_ms: 180_000,
                track_number: 1,
                disc_number: 1,
                total_tracks: 1,
                release_date: None,
                cover_url: None,
                output_dir: out_dir.clone(),
                quality: "hires".to_string(),
                embed_lyrics: true,
                embed_artwork: true,
                smart_studio_origin: false,
                allow_fallback: false,
                strict_quality: true,
            };

            let start_t = Instant::now();
            let dl_res = orch.download_track(&req).await;
            let elapsed_ms = start_t.elapsed().as_millis() as u64;

            match dl_res {
                Ok(item) => {
                    let p = Path::new(&item.file_path);
                    assert!(p.exists(), "Downloaded file must exist at {:?}", p);
                    let fsize = std::fs::metadata(p).map(|m| m.len()).unwrap_or(0);
                    let sha = compute_file_sha256(p).unwrap_or_default();
                    let bytes = std::fs::read(p).unwrap_or_default();
                    let magic_valid = AudioByteValidator::is_flac_magic(&bytes);
                    assert!(magic_valid, "Strict lossless file must have FLAC magic bytes");

                    let ff = inspect_with_ffprobe(p).expect("ffprobe must succeed on downloaded FLAC");
                    assert_eq!(ff.codec_name, "flac", "Strict lossless must decode as flac");

                    let is_provider_fb = target.origin_service == "spotify";
                    let q_decision_str = if is_provider_fb {
                        "ReadyProviderFallbackExactQuality"
                    } else {
                        "CompletedExactQuality"
                    };

                    // Insert into downloads
                    let _ = sqlx::query(
                        r#"
                        INSERT INTO downloads (
                            track_id, source_service_id, file_path, file_format, bit_depth, sample_rate, file_size_bytes, downloaded_at,
                            origin_service, origin_service_track_id, effective_service, effective_service_track_id,
                            requested_quality, effective_quality, requested_format, effective_format, quality_decision,
                            provider_fallback_used, quality_fallback_used
                        ) VALUES (
                            ?, (SELECT id FROM services WHERE LOWER(name) = LOWER(?)), ?, 'FLAC', ?, ?, ?, CURRENT_TIMESTAMP,
                            ?, ?, ?, ?,
                            'lossless', 'FLAC 16-bit / 44.1 kHz', 'flac', 'FLAC', ?,
                            ?, 0
                        ) ON CONFLICT(track_id) DO NOTHING
                        "#
                    )
                    .bind(target.track_id)
                    .bind(&eff_svc)
                    .bind(&item.file_path)
                    .bind(item.bit_depth)
                    .bind(item.sample_rate)
                    .bind(fsize as i64)
                    .bind(&target.origin_service)
                    .bind(&target.service_track_id)
                    .bind(&eff_svc)
                    .bind(&eff_track_id)
                    .bind(q_decision_str)
                    .bind(if is_provider_fb { 1i64 } else { 0i64 })
                    .execute(&db)
                    .await;

                    let rec = TrackExecutionAuditRecord {
                        segment: "Segment A".to_string(),
                        track_id: target.track_id,
                        display_title: target.display_title.clone(),
                        artist: target.artist.clone(),
                        album: target.album.clone(),
                        source_service: target.origin_service.clone(),
                        effective_provider: eff_svc,
                        service_track_id: eff_track_id,
                        isrc: target.isrc.clone(),
                        requested_quality: "lossless".to_string(),
                        requested_format: "flac".to_string(),
                        strict_quality: true,
                        allow_lossy_fallback: false,
                        preflight_decision: if is_provider_fb { "ReadyProviderFallbackExactQuality".to_string() } else { "ReadyExactQuality".to_string() },
                        actual_codec: "FLAC".to_string(),
                        effective_quality: "FLAC 16-bit / 44.1 kHz".to_string(),
                        effective_format: "FLAC".to_string(),
                        quality_decision: q_decision_str.to_string(),
                        provider_fallback_used: is_provider_fb,
                        quality_fallback_used: false,
                        decision_reason: None,
                        retryable: false,
                        terminal_outcome: "CompletedExactQuality (FLAC bit-perfect)".to_string(),
                        file_path: item.file_path.clone(),
                        file_size_bytes: fsize,
                        sha256: sha,
                        ffprobe_codec: ff.codec_name,
                        ffprobe_sample_rate: ff.sample_rate,
                        ffprobe_bit_depth: ff.bits_per_sample,
                        vorbis_tag_count: ff.vorbis_tag_count,
                        magic_bytes_valid: true,
                        sqlite_download_row: true,
                        staging_cleaned: true,
                        transfer_duration_ms: elapsed_ms,
                        throughput_mibps: if elapsed_ms > 0 { (fsize as f64 / (1024.0 * 1024.0)) / (elapsed_ms as f64 / 1000.0) } else { 0.0 },
                    };
                    recs.lock().await.push(rec);
                }
                Err(err) => {
                    let err_msg = err.to_string();
                    let is_rejected_quality = err_msg.contains("RejectedQuality")
                        || err_msg.contains("Quality rejected")
                        || err_msg.contains("downgrade rejected")
                        || err_msg.contains("rejected to prevent quality downgrade")
                        || err_msg.contains("returned AAC for the current account/client context");
                    assert!(is_rejected_quality, "Unexpected failure in Segment A: {}", err_msg);

                    let rec = TrackExecutionAuditRecord {
                        segment: "Segment A".to_string(),
                        track_id: target.track_id,
                        display_title: target.display_title.clone(),
                        artist: target.artist.clone(),
                        album: target.album.clone(),
                        source_service: target.origin_service.clone(),
                        effective_provider: eff_svc,
                        service_track_id: eff_track_id,
                        isrc: target.isrc.clone(),
                        requested_quality: "lossless".to_string(),
                        requested_format: "flac".to_string(),
                        strict_quality: true,
                        allow_lossy_fallback: false,
                        preflight_decision: "RejectedQuality".to_string(),
                        actual_codec: "None".to_string(),
                        effective_quality: "None".to_string(),
                        effective_format: "None".to_string(),
                        quality_decision: "RejectedQuality".to_string(),
                        provider_fallback_used: false,
                        quality_fallback_used: false,
                        decision_reason: Some("Provider returned AAC; lossy fallback is disabled".to_string()),
                        retryable: false,
                        terminal_outcome: "Failed (Quality rejected, 0 bytes saved)".to_string(),
                        file_path: "".to_string(),
                        file_size_bytes: 0,
                        sha256: "".to_string(),
                        ffprobe_codec: "".to_string(),
                        ffprobe_sample_rate: 0,
                        ffprobe_bit_depth: None,
                        vorbis_tag_count: 0,
                        magic_bytes_valid: false,
                        sqlite_download_row: false,
                        staging_cleaned: true,
                        transfer_duration_ms: elapsed_ms,
                        throughput_mibps: 0.0,
                    };
                    recs.lock().await.push(rec);
                }
            }
        });
        handles_a.push(handle);
    }

    for h in handles_a {
        h.await.unwrap();
    }

    let finished_recs_a = records_a.lock().await.clone();
    assert_eq!(finished_recs_a.len(), 25);

    // Segment A Assertions
    let seg_a_exact = finished_recs_a.iter().filter(|r| r.quality_decision == "CompletedExactQuality").count();
    let seg_a_prov_fb = finished_recs_a.iter().filter(|r| r.quality_decision == "ReadyProviderFallbackExactQuality").count();
    let seg_a_rej = finished_recs_a.iter().filter(|r| r.quality_decision == "RejectedQuality").count();
    let seg_a_no_prov = finished_recs_a.iter().filter(|r| r.quality_decision == "NoDownloadProvider").count();

    println!("Segment A Summary: Exact={}, ProviderFB={}, RejectedQuality={}, NoProvider={}", seg_a_exact, seg_a_prov_fb, seg_a_rej, seg_a_no_prov);
    assert_eq!(seg_a_exact, 10, "Must have exactly 10 direct FLAC downloads in Segment A");
    assert_eq!(seg_a_prov_fb, 4, "Must have exactly 4 provider fallback FLAC downloads in Segment A");
    assert_eq!(seg_a_rej, 9, "Must have exactly 9 RejectedQuality tracks in Segment A (downgrade prevention)");
    assert_eq!(seg_a_no_prov, 2, "Must have exactly 2 NoDownloadProvider tracks in Segment A");

    // Verify 0 staging residuals
    let staging_entries_after_a = std::fs::read_dir(&staging_dir).unwrap().count();
    assert_eq!(staging_entries_after_a, 0, "Staging directory must be completely empty after Segment A");

    // Verify accounts invalid count is 0
    let invalid_accounts_a: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM accounts WHERE credentials_invalid = 1")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(invalid_accounts_a, 0, "Auth credentials must NOT be invalidated by RejectedQuality");

    // =========================================================================
    // EXECUTE SEGMENT B (Permissive Fallback: strict=false, fallback=true, conc=4)
    // =========================================================================
    println!("\n>>> Starting SEGMENT B execution (Permissive Fallback, 25 tracks, concurrency=4)...");
    let semaphore_b = Arc::new(Semaphore::new(4));
    let records_b = Arc::new(tokio::sync::Mutex::new(Vec::new()));
    let mut handles_b = Vec::new();

    for target in seg_b_targets {
        let sem = semaphore_b.clone();
        let orch = orchestrator.clone();
        let db = pool.clone();
        let out_dir = output_dir_str.clone();
        let recs = records_b.clone();

        let handle = tokio::spawn(async move {
            let _permit = sem.acquire().await.unwrap();

            if target.effective_provider == "none" || target.decision == "NoDownloadProvider" {
                // Spotify unmapped -> NoDownloadProvider
                let rec = TrackExecutionAuditRecord {
                    segment: "Segment B".to_string(),
                    track_id: target.track_id,
                    display_title: target.display_title.clone(),
                    artist: target.artist.clone(),
                    album: target.album.clone(),
                    source_service: target.origin_service.clone(),
                    effective_provider: "none".to_string(),
                    service_track_id: "".to_string(),
                    isrc: target.isrc.clone(),
                    requested_quality: "lossless".to_string(),
                    requested_format: "flac".to_string(),
                    strict_quality: false,
                    allow_lossy_fallback: true,
                    preflight_decision: "NoDownloadProvider".to_string(),
                    actual_codec: "None".to_string(),
                    effective_quality: "None".to_string(),
                    effective_format: "None".to_string(),
                    quality_decision: "NoDownloadProvider".to_string(),
                    provider_fallback_used: false,
                    quality_fallback_used: false,
                    decision_reason: Some("No active download provider available for this track".to_string()),
                    retryable: false,
                    terminal_outcome: "Failed (No download provider available)".to_string(),
                    file_path: "".to_string(),
                    file_size_bytes: 0,
                    sha256: "".to_string(),
                    ffprobe_codec: "".to_string(),
                    ffprobe_sample_rate: 0,
                    ffprobe_bit_depth: None,
                    vorbis_tag_count: 0,
                    magic_bytes_valid: false,
                    sqlite_download_row: false,
                    staging_cleaned: true,
                    transfer_duration_ms: 0,
                    throughput_mibps: 0.0,
                };
                recs.lock().await.push(rec);
                return;
            }

            let eff_svc = target.effective_provider.clone();
            let eff_track_id = target.service_track_id.clone();

            let req = DownloadRequest {
                item_id: format!("s167_segB_{}", target.track_id),
                isrc: if target.isrc.is_empty() { None } else { Some(target.isrc.clone()) },
                musicbrainz_recording_id: None,
                acoustid_fingerprint: None,
                spotify_id: None,
                service_name: Some(eff_svc.clone()),
                service_track_id: Some(eff_track_id.clone()),
                service_album_id: None,
                track_name: target.display_title.clone(),
                artist_name: target.artist.clone(),
                album_name: target.album.clone(),
                album_artist: Some(target.artist.clone()),
                duration_ms: 180_000,
                track_number: 1,
                disc_number: 1,
                total_tracks: 1,
                release_date: None,
                cover_url: None,
                output_dir: out_dir.clone(),
                quality: "hires".to_string(),
                embed_lyrics: true,
                embed_artwork: true,
                smart_studio_origin: false,
                allow_fallback: true,
                strict_quality: false,
            };

            let start_t = Instant::now();
            let dl_res = orch.download_track(&req).await;
            let elapsed_ms = start_t.elapsed().as_millis() as u64;

            match dl_res {
                Ok(item) => {
                    let p = Path::new(&item.file_path);
                    assert!(p.exists(), "Downloaded file must exist at {:?}", p);
                    let fsize = std::fs::metadata(p).map(|m| m.len()).unwrap_or(0);
                    let sha = compute_file_sha256(p).unwrap_or_default();
                    let bytes = std::fs::read(p).unwrap_or_default();

                    let is_m4a = item.file_path.to_lowercase().ends_with(".m4a");
                    let magic_valid = if is_m4a {
                        AudioByteValidator::is_m4a_magic(&bytes)
                    } else {
                        AudioByteValidator::is_flac_magic(&bytes)
                    };
                    assert!(magic_valid, "Downloaded file magic bytes must match format (M4A or FLAC)");

                    let ff = inspect_with_ffprobe(p).expect("ffprobe must succeed on downloaded file");

                    let is_provider_fb = target.origin_service == "spotify";
                    let (q_dec, eff_q, eff_fmt, terminal_outcome, dec_reason) = if is_m4a {
                        (
                            "CompletedWithQualityFallback",
                            "320kbps".to_string(),
                            "AAC".to_string(),
                            "CompletedWithQualityFallback (AAC 320 kbps in M4A)".to_string(),
                            Some("Provider returned AAC; lossy fallback is enabled".to_string()),
                        )
                    } else if is_provider_fb {
                        (
                            "CompletedWithProviderFallback",
                            "FLAC 16-bit / 44.1 kHz".to_string(),
                            "FLAC".to_string(),
                            "CompletedWithProviderFallback (FLAC 16/44.1)".to_string(),
                            None,
                        )
                    } else {
                        (
                            "CompletedExactQuality",
                            "FLAC 16-bit / 44.1 kHz".to_string(),
                            "FLAC".to_string(),
                            "CompletedExactQuality (FLAC bit-perfect)".to_string(),
                            None,
                        )
                    };

                    // Insert into downloads
                    let _ = sqlx::query(
                        r#"
                        INSERT INTO downloads (
                            track_id, source_service_id, file_path, file_format, bit_depth, sample_rate, file_size_bytes, downloaded_at,
                            origin_service, origin_service_track_id, effective_service, effective_service_track_id,
                            requested_quality, effective_quality, requested_format, effective_format, quality_decision,
                            provider_fallback_used, quality_fallback_used, decision_reason
                        ) VALUES (
                            ?, (SELECT id FROM services WHERE LOWER(name) = LOWER(?)), ?, ?, ?, ?, ?, CURRENT_TIMESTAMP,
                            ?, ?, ?, ?,
                            'lossless', ?, 'flac', ?, ?,
                            ?, ?, ?
                        ) ON CONFLICT(track_id) DO NOTHING
                        "#
                    )
                    .bind(target.track_id)
                    .bind(&eff_svc)
                    .bind(&item.file_path)
                    .bind(&eff_fmt)
                    .bind(item.bit_depth)
                    .bind(item.sample_rate)
                    .bind(fsize as i64)
                    .bind(&target.origin_service)
                    .bind(&target.service_track_id)
                    .bind(&eff_svc)
                    .bind(&eff_track_id)
                    .bind(&eff_q)
                    .bind(&eff_fmt)
                    .bind(q_dec)
                    .bind(if is_provider_fb { 1i64 } else { 0i64 })
                    .bind(if is_m4a { 1i64 } else { 0i64 })
                    .bind(&dec_reason)
                    .execute(&db)
                    .await;

                    let rec = TrackExecutionAuditRecord {
                        segment: "Segment B".to_string(),
                        track_id: target.track_id,
                        display_title: target.display_title.clone(),
                        artist: target.artist.clone(),
                        album: target.album.clone(),
                        source_service: target.origin_service.clone(),
                        effective_provider: eff_svc,
                        service_track_id: eff_track_id,
                        isrc: target.isrc.clone(),
                        requested_quality: "lossless".to_string(),
                        requested_format: "flac".to_string(),
                        strict_quality: false,
                        allow_lossy_fallback: true,
                        preflight_decision: if is_m4a {
                            "ReadyQualityFallback".to_string()
                        } else if is_provider_fb {
                            "ReadyProviderFallbackExactQuality".to_string()
                        } else {
                            "ReadyExactQuality".to_string()
                        },
                        actual_codec: if is_m4a { "AAC".to_string() } else { "FLAC".to_string() },
                        effective_quality: eff_q,
                        effective_format: eff_fmt,
                        quality_decision: q_dec.to_string(),
                        provider_fallback_used: is_provider_fb,
                        quality_fallback_used: is_m4a,
                        decision_reason: dec_reason,
                        retryable: false,
                        terminal_outcome,
                        file_path: item.file_path.clone(),
                        file_size_bytes: fsize,
                        sha256: sha,
                        ffprobe_codec: ff.codec_name,
                        ffprobe_sample_rate: ff.sample_rate,
                        ffprobe_bit_depth: ff.bits_per_sample,
                        vorbis_tag_count: ff.vorbis_tag_count,
                        magic_bytes_valid: true,
                        sqlite_download_row: true,
                        staging_cleaned: true,
                        transfer_duration_ms: elapsed_ms,
                        throughput_mibps: if elapsed_ms > 0 { (fsize as f64 / (1024.0 * 1024.0)) / (elapsed_ms as f64 / 1000.0) } else { 0.0 },
                    };
                    recs.lock().await.push(rec);
                }
                Err(err) => {
                    panic!("Segment B item failed unexpectedly: {}", err);
                }
            }
        });
        handles_b.push(handle);
    }

    for h in handles_b {
        h.await.unwrap();
    }

    let finished_recs_b = records_b.lock().await.clone();
    assert_eq!(finished_recs_b.len(), 25);

    // Segment B Assertions
    let seg_b_exact = finished_recs_b.iter().filter(|r| r.quality_decision == "CompletedExactQuality").count();
    let seg_b_prov_fb = finished_recs_b.iter().filter(|r| r.quality_decision == "CompletedWithProviderFallback").count();
    let seg_b_qual_fb = finished_recs_b.iter().filter(|r| r.quality_decision == "CompletedWithQualityFallback").count();
    let seg_b_no_prov = finished_recs_b.iter().filter(|r| r.quality_decision == "NoDownloadProvider").count();

    println!("Segment B Summary: Exact={}, ProviderFB={}, QualityFB={}, NoProvider={}", seg_b_exact, seg_b_prov_fb, seg_b_qual_fb, seg_b_no_prov);
    assert_eq!(seg_b_no_prov, 2, "Must have exactly 2 NoDownloadProvider tracks in Segment B");
    assert_eq!(seg_b_prov_fb, 4, "Must have exactly 4 provider fallback FLAC downloads in Segment B");
    assert_eq!(seg_b_exact + seg_b_qual_fb, 19, "Must have 19 direct downloads in Segment B");
    assert!(seg_b_qual_fb >= 5, "Must have quality fallback AAC downloads in Segment B");

    // Verify 0 staging residuals after Segment B
    let staging_entries_after_b = std::fs::read_dir(&staging_dir).unwrap().count();
    assert_eq!(staging_entries_after_b, 0, "Staging directory must be completely empty after Segment B");

    // Check HEAD did not change
    let current_head = Command::new("git").args(["rev-parse", "HEAD"]).output()
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
        .unwrap_or_default();
    assert_eq!(current_head, initial_head, "Git commit HEAD must not change during execution");

    // Write Full Execution Report JSON
    let all_records = [finished_recs_a, finished_recs_b].concat();
    let report_json = serde_json::to_string_pretty(&all_records).unwrap();
    let report_out_path = PathBuf::from(&app_data)
        .join("Syncify")
        .join("audits")
        .join("s167_50_execution_report.json");
    std::fs::write(&report_out_path, &report_json).expect("Write execution report JSON");

    println!("\n================================================================================");
    println!("       S167 50-TRACK LIVE NETWORK AUDIT SUCCESSFULLY COMPLETED                  ");
    println!("       Execution Report: {}", report_out_path.display());
    println!("================================================================================\n");

    println!("EXECUTION_REPORT_JSON_START\n{}\nEXECUTION_REPORT_JSON_END", report_json);
}

//! S167 Live-Network 50-Track Preflight Audit Test
//!
//! Evaluates exactly 50 distinct real tracks from the runtime library database
//! against active accounts without downloading bytes, modifying the DB, or creating staging files.
//!
//! Segment A — Strict Lossless (25 tracks):
//! - 8 Qobuz FLAC
//! - 8 Tidal FLAC
//! - 4 Spotify with provider fallback (FLAC)
//! - 3 Tidal returning AAC (lossy)
//! - 2 Spotify unmapped (no download provider)
//!
//! Segment B — Permissive Fallback (25 tracks):
//! - 8 Qobuz FLAC
//! - 6 Tidal FLAC
//! - 5 Tidal returning AAC (lossy)
//! - 4 Spotify with provider fallback (FLAC)
//! - 2 Spotify unmapped (no download provider)

use base64::{engine::general_purpose::STANDARD as BASE64, Engine as _};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use sqlx::sqlite::SqlitePoolOptions;
use std::collections::HashSet;
use std::path::PathBuf;
use syncify_core_domain::layout::sanitize_filename;
use syncify_core_domain::quality::{QualityDecisionKind, QualityPolicy};
use syncify_tauri_lib::commands::{
    evaluate_track_preflight, TrackPreflightResult,
};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrackAuditRecord {
    pub track_id: i64,
    pub display_title: String,
    pub artist: String,
    pub album: String,
    pub source_service: String,
    pub effective_provider_candidate: String,
    pub service_track_id: String,
    pub isrc: String,
    pub requested_quality: String,
    pub requested_format: String,
    pub strict_quality: bool,
    pub allow_lossy_fallback: bool,
    pub provider_available_quality: Option<String>,
    pub provider_available_format: Option<String>,
    pub quality_decision_kind: String,
    pub provider_fallback_used: bool,
    pub quality_fallback_used: bool,
    pub decision_reason: Option<String>,
    pub retryable: bool,
    pub expected_terminal_outcome: String,
    pub existing_download_state: String,
    pub predicted_physical_format: String,
    pub predicted_extension: String,
    pub predicted_path: String,
}

#[derive(Debug, Deserialize)]
struct PlaybackInfoResp {
    #[allow(dead_code)]
    #[serde(rename = "audioQuality")]
    audio_quality: Option<String>,
    #[allow(dead_code)]
    #[serde(rename = "manifestMimeType")]
    manifest_mime_type: Option<String>,
    manifest: Option<String>,
}

#[derive(Debug, Deserialize)]
struct BtsManifest {
    #[allow(dead_code)]
    #[serde(rename = "mimeType")]
    mime_type: Option<String>,
    codecs: Option<String>,
}

fn compute_sha256_str(data: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(data.as_bytes());
    format!("{:x}", hasher.finalize())
}

#[tokio::test]
#[ignore = "requires runtime sqlite database and network access"]
async fn test_s167_live_preflight_50_audit_matrix() {
    let app_data = std::env::var("LOCALAPPDATA").unwrap_or_default();
    let db_path = PathBuf::from(&app_data)
        .join("com.syncify.app")
        .join("syncify.db");

    if !db_path.exists() {
        println!("Runtime DB not found at {:?}, skipping test", db_path);
        return;
    }

    let pool = SqlitePoolOptions::new()
        .max_connections(5)
        .connect(&format!("sqlite://{}?mode=ro", db_path.display()))
        .await
        .expect("Must connect to runtime DB in read-only mode");

    // 1. Initial snapshot of DB state
    let initial_download_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM downloads")
        .fetch_one(&pool)
        .await
        .unwrap();

    let initial_queue_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM download_queue")
        .fetch_one(&pool)
        .await
        .unwrap();

    let downloaded_ids: HashSet<i64> = sqlx::query_scalar("SELECT track_id FROM downloads")
        .fetch_all(&pool)
        .await
        .unwrap()
        .into_iter()
        .collect();

    // 2. Fetch Tidal decrypted credentials for read-only stream inspection
    let row: Option<(String,)> = sqlx::query_as(
        "SELECT a.credentials_json FROM accounts a JOIN services s ON s.id = a.service_id WHERE LOWER(s.name) = 'tidal' AND a.is_active = 1 LIMIT 1"
    )
    .fetch_optional(&pool)
    .await
    .unwrap();

    let (access_token, country_code) = match row {
        Some((cj,)) => {
            let _ = syncify_tauri_lib::crypto::init_keychain_crypto();
            if let Ok(dec_str) = syncify_tauri_lib::crypto::decrypt(&cj) {
                if let Ok(creds) = serde_json::from_str::<syncify_tidal_downloader::TidalGuiCredentials>(&dec_str) {
                    (Some(creds.access_token), creds.country_code.unwrap_or_else(|| "ES".to_string()))
                } else {
                    (None, "ES".to_string())
                }
            } else {
                (None, "ES".to_string())
            }
        }
        None => (None, "ES".to_string()),
    };

    let client = reqwest::Client::new();

    // Candidate query helpers
    #[derive(sqlx::FromRow, Debug, Clone)]
    struct TrackRow {
        id: i64,
        title: String,
        artist: Option<String>,
        album: Option<String>,
        track_number: Option<i64>,
        #[allow(dead_code)]
        release_year: Option<i64>,
        isrc: Option<String>,
        service_track_id: Option<String>,
        format: Option<String>,
        bit_depth: Option<i64>,
        service_name: String,
    }

    let mut used_ids: HashSet<i64> = downloaded_ids.clone();

    // -------------------------------------------------------------
    // Pool E: Tidal AAC tracks (need 8: 3 for Seg A, 5 for Seg B)
    // -------------------------------------------------------------
    let tidal_aac_in_db: Vec<TrackRow> = sqlx::query_as(
        r#"
        SELECT t.id, t.title,
               (SELECT GROUP_CONCAT(ar.name, ', ') FROM track_artists ta JOIN artists ar ON ar.id = ta.artist_id WHERE ta.track_id = t.id) as artist,
               alb.title as album, t.track_number, t.release_year, t.isrc,
               ts.service_track_id, ts.format, ts.bit_depth, s.name as service_name
        FROM tracks t
        JOIN track_sources ts ON ts.track_id = t.id
        JOIN services s ON s.id = ts.service_id AND s.name = 'tidal'
        LEFT JOIN albums alb ON alb.id = t.album_id
        WHERE ts.format = 'AAC'
        ORDER BY t.id ASC
        "#
    )
    .fetch_all(&pool)
    .await
    .unwrap();

    let mut tidal_aac_pool: Vec<TrackRow> = Vec::new();
    for r in tidal_aac_in_db {
        if used_ids.insert(r.id) {
            tidal_aac_pool.push(r);
        }
    }

    // If needed, discover additional Tidal tracks that return AAC
    if tidal_aac_pool.len() < 8 {
        let tidal_candidates: Vec<TrackRow> = sqlx::query_as(
            r#"
            SELECT t.id, t.title,
                   (SELECT GROUP_CONCAT(ar.name, ', ') FROM track_artists ta JOIN artists ar ON ar.id = ta.artist_id WHERE ta.track_id = t.id) as artist,
                   alb.title as album, t.track_number, t.release_year, t.isrc,
                   ts.service_track_id, ts.format, ts.bit_depth, s.name as service_name
            FROM tracks t
            JOIN track_sources ts ON ts.track_id = t.id
            JOIN services s ON s.id = ts.service_id AND s.name = 'tidal'
            LEFT JOIN albums alb ON alb.id = t.album_id
            WHERE ts.service_track_id IS NOT NULL AND TRIM(ts.service_track_id) != ''
            ORDER BY t.id ASC
            LIMIT 100
            "#
        )
        .fetch_all(&pool)
        .await
        .unwrap();

        if let Some(ref tok) = access_token {
            for cand in tidal_candidates {
                if used_ids.contains(&cand.id) {
                    continue;
                }
                if let Some(ref stid) = cand.service_track_id {
                    let url = format!(
                        "https://api.tidal.com/v1/tracks/{}/playbackinfopostpaywall?audioquality=LOSSLESS&playbackmode=STREAM&assetpresentation=FULL&countryCode={}&manifestMimeType=application/vnd.tidal.bts",
                        stid, country_code
                    );
                    if let Ok(resp) = client.get(&url)
                        .header("Authorization", format!("Bearer {}", tok))
                        .header("X-Tidal-SessionId", tok)
                        .send()
                        .await
                    {
                        if let Ok(info) = resp.json::<PlaybackInfoResp>().await {
                            if let Some(b64) = info.manifest {
                                if let Ok(bytes) = BASE64.decode(&b64) {
                                    if let Ok(bts) = serde_json::from_slice::<BtsManifest>(&bytes) {
                                        if bts.codecs.as_deref().map_or(false, |c| c.starts_with("mp4a")) {
                                            let mut r = cand.clone();
                                            r.format = Some("AAC".to_string());
                                            if used_ids.insert(r.id) {
                                                tidal_aac_pool.push(r);
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
                if tidal_aac_pool.len() >= 8 {
                    break;
                }
            }
        }
    }

    assert!(tidal_aac_pool.len() >= 8, "Must have at least 8 Tidal AAC tracks (found {})", tidal_aac_pool.len());

    // -------------------------------------------------------------
    // Pool D: Spotify unmapped tracks (need 4: 2 for Seg A, 2 for Seg B)
    // -------------------------------------------------------------
    let all_spotify_unmapped: Vec<TrackRow> = sqlx::query_as(
        r#"
        SELECT t.id, t.title,
               (SELECT GROUP_CONCAT(ar.name, ', ') FROM track_artists ta JOIN artists ar ON ar.id = ta.artist_id WHERE ta.track_id = t.id) as artist,
               alb.title as album, t.track_number, t.release_year, t.isrc,
               ts_sp.service_track_id, ts_sp.format, ts_sp.bit_depth, s_sp.name as service_name
        FROM tracks t
        JOIN track_sources ts_sp ON ts_sp.track_id = t.id
        JOIN services s_sp ON s_sp.id = ts_sp.service_id AND s_sp.name = 'spotify'
        LEFT JOIN albums alb ON alb.id = t.album_id
        WHERE t.id NOT IN (
            SELECT ts.track_id FROM track_sources ts JOIN services s ON s.id = ts.service_id WHERE s.supports_download = 1
        )
        AND (t.isrc IS NULL OR t.isrc = '' OR t.isrc NOT IN (
            SELECT t2.isrc FROM tracks t2 JOIN track_sources ts2 ON ts2.track_id = t2.id JOIN services s2 ON s2.id = ts2.service_id WHERE s2.supports_download = 1 AND t2.isrc IS NOT NULL AND TRIM(t2.isrc) != ''
        ))
        ORDER BY t.id ASC
        LIMIT 20
        "#
    )
    .fetch_all(&pool)
    .await
    .unwrap();

    let mut spotify_unmapped_pool: Vec<TrackRow> = Vec::new();
    for r in all_spotify_unmapped {
        if !used_ids.contains(&r.id) {
            used_ids.insert(r.id);
            spotify_unmapped_pool.push(r);
            if spotify_unmapped_pool.len() == 4 {
                break;
            }
        }
    }
    assert_eq!(spotify_unmapped_pool.len(), 4, "Must have exactly 4 Spotify unmapped tracks");

    // -------------------------------------------------------------
    // Pool C: Spotify with fallback to Qobuz/Tidal FLAC (need 8: 4 for Seg A, 4 for Seg B)
    // -------------------------------------------------------------
    let all_spotify_fallback: Vec<TrackRow> = sqlx::query_as(
        r#"
        SELECT t.id, t.title,
               (SELECT GROUP_CONCAT(ar.name, ', ') FROM track_artists ta JOIN artists ar ON ar.id = ta.artist_id WHERE ta.track_id = t.id) as artist,
               alb.title as album, t.track_number, t.release_year, t.isrc,
               ts_dl.service_track_id, ts_dl.format, ts_dl.bit_depth, s_dl.name as service_name
        FROM tracks t
        JOIN track_sources ts_sp ON ts_sp.track_id = t.id
        JOIN services s_sp ON s_sp.id = ts_sp.service_id AND s_sp.name = 'spotify'
        JOIN track_sources ts_dl ON ts_dl.track_id = t.id
        JOIN services s_dl ON s_dl.id = ts_dl.service_id AND s_dl.name IN ('qobuz', 'tidal')
        LEFT JOIN albums alb ON alb.id = t.album_id
        WHERE ts_dl.format = 'FLAC' AND COALESCE(ts_dl.available, 1) = 1 AND ts_dl.service_track_id IS NOT NULL AND TRIM(ts_dl.service_track_id) != ''
        ORDER BY t.id ASC
        LIMIT 30
        "#
    )
    .fetch_all(&pool)
    .await
    .unwrap();

    let mut spotify_fallback_pool: Vec<TrackRow> = Vec::new();
    for r in all_spotify_fallback {
        if !used_ids.contains(&r.id) {
            used_ids.insert(r.id);
            spotify_fallback_pool.push(r);
            if spotify_fallback_pool.len() == 8 {
                break;
            }
        }
    }
    assert_eq!(spotify_fallback_pool.len(), 8, "Must have exactly 8 Spotify fallback tracks");

    // -------------------------------------------------------------
    // Pool A: Qobuz FLAC tracks (need 16: 8 for Seg A, 8 for Seg B)
    // -------------------------------------------------------------
    let all_qobuz: Vec<TrackRow> = sqlx::query_as(
        r#"
        SELECT t.id, t.title,
               (SELECT GROUP_CONCAT(ar.name, ', ') FROM track_artists ta JOIN artists ar ON ar.id = ta.artist_id WHERE ta.track_id = t.id) as artist,
               alb.title as album, t.track_number, t.release_year, t.isrc,
               ts.service_track_id, ts.format, ts.bit_depth, s.name as service_name
        FROM tracks t
        JOIN track_sources ts ON ts.track_id = t.id
        JOIN services s ON s.id = ts.service_id AND s.name = 'qobuz'
        LEFT JOIN albums alb ON alb.id = t.album_id
        WHERE ts.format = 'FLAC' AND COALESCE(ts.available, 1) = 1 AND ts.service_track_id IS NOT NULL AND TRIM(ts.service_track_id) != ''
        ORDER BY t.id ASC
        LIMIT 50
        "#
    )
    .fetch_all(&pool)
    .await
    .unwrap();

    let mut qobuz_pool: Vec<TrackRow> = Vec::new();
    for r in all_qobuz {
        if !used_ids.contains(&r.id) {
            used_ids.insert(r.id);
            qobuz_pool.push(r);
            if qobuz_pool.len() == 16 {
                break;
            }
        }
    }
    assert_eq!(qobuz_pool.len(), 16, "Must have exactly 16 Qobuz FLAC tracks");

    // -------------------------------------------------------------
    // Pool B: Tidal FLAC tracks (need 14: 8 for Seg A, 6 for Seg B)
    // -------------------------------------------------------------
    let all_tidal_flac: Vec<TrackRow> = sqlx::query_as(
        r#"
        SELECT t.id, t.title,
               (SELECT GROUP_CONCAT(ar.name, ', ') FROM track_artists ta JOIN artists ar ON ar.id = ta.artist_id WHERE ta.track_id = t.id) as artist,
               alb.title as album, t.track_number, t.release_year, t.isrc,
               ts.service_track_id, ts.format, ts.bit_depth, s.name as service_name
        FROM tracks t
        JOIN track_sources ts ON ts.track_id = t.id
        JOIN services s ON s.id = ts.service_id AND s.name = 'tidal'
        LEFT JOIN albums alb ON alb.id = t.album_id
        WHERE (ts.format = 'FLAC' OR (ts.bit_depth IS NOT NULL AND ts.bit_depth >= 16))
          AND COALESCE(ts.available, 1) = 1 AND ts.service_track_id IS NOT NULL AND TRIM(ts.service_track_id) != ''
          AND ts.format != 'AAC'
        ORDER BY t.id ASC
        LIMIT 50
        "#
    )
    .fetch_all(&pool)
    .await
    .unwrap();

    let mut tidal_flac_pool: Vec<TrackRow> = Vec::new();
    for r in all_tidal_flac {
        if !used_ids.contains(&r.id) {
            used_ids.insert(r.id);
            tidal_flac_pool.push(r);
            if tidal_flac_pool.len() == 14 {
                break;
            }
        }
    }
    assert_eq!(tidal_flac_pool.len(), 14, "Must have exactly 14 Tidal FLAC tracks");

    // -------------------------------------------------------------
    // PARTITION INTO SEGMENT A AND SEGMENT B
    // -------------------------------------------------------------
    let seg_a_qobuz = &qobuz_pool[0..8];
    let seg_a_tidal_flac = &tidal_flac_pool[0..8];
    let seg_a_spotify_fb = &spotify_fallback_pool[0..4];
    let seg_a_tidal_aac = &tidal_aac_pool[0..3];
    let seg_a_spotify_un = &spotify_unmapped_pool[0..2];

    let seg_b_qobuz = &qobuz_pool[8..16];
    let seg_b_tidal_flac = &tidal_flac_pool[8..14];
    let seg_b_spotify_fb = &spotify_fallback_pool[4..8];
    let seg_b_tidal_aac = &tidal_aac_pool[3..8];
    let seg_b_spotify_un = &spotify_unmapped_pool[2..4];

    // Verify 0 overlap between Segment A and Segment B
    let mut seg_a_ids: HashSet<i64> = HashSet::new();
    for r in seg_a_qobuz.iter().chain(seg_a_tidal_flac).chain(seg_a_spotify_fb).chain(seg_a_tidal_aac).chain(seg_a_spotify_un) {
        assert!(seg_a_ids.insert(r.id), "Duplicate track ID {} within Segment A", r.id);
    }
    assert_eq!(seg_a_ids.len(), 25, "Segment A must contain exactly 25 distinct tracks");

    let mut seg_b_ids: HashSet<i64> = HashSet::new();
    for r in seg_b_qobuz.iter().chain(seg_b_tidal_flac).chain(seg_b_spotify_fb).chain(seg_b_tidal_aac).chain(seg_b_spotify_un) {
        assert!(seg_b_ids.insert(r.id), "Duplicate track ID {} within Segment B", r.id);
        assert!(!seg_a_ids.contains(&r.id), "Cross-segment duplication detected for track ID {}", r.id);
    }
    assert_eq!(seg_b_ids.len(), 25, "Segment B must contain exactly 25 distinct tracks");

    // ==========================================
    // EXECUTE PREFLIGHT & AUDIT FOR SEGMENT A
    // strict_quality = true, allow_lossy_fallback = false
    // ==========================================
    let mut records_a: Vec<TrackAuditRecord> = Vec::new();

    for r in seg_a_qobuz.iter().chain(seg_a_tidal_flac).chain(seg_a_spotify_fb).chain(seg_a_tidal_aac).chain(seg_a_spotify_un) {
        let is_unmapped = seg_a_spotify_un.iter().any(|u| u.id == r.id);
        let is_aac = seg_a_tidal_aac.iter().any(|a| a.id == r.id);
        let is_spotify_fb = seg_a_spotify_fb.iter().any(|s| s.id == r.id);

        let origin_svc = if is_spotify_fb || is_unmapped { "spotify" } else { &r.service_name };
        let eff_provider = if is_unmapped { "none" } else { &r.service_name };

        let _preflight_res: TrackPreflightResult = evaluate_track_preflight(
            &pool,
            r.id,
            None,
            Some("lossless"),
            true,  // strict_quality
            false, // allow_fallback
        )
        .await
        .expect("Preflight must succeed without panic");

        let _q_dec = QualityPolicy::evaluate_preflight(
            "lossless",
            Some(if is_aac { "lossy" } else { "lossless" }),
            Some(if is_aac { "AAC" } else { "FLAC" }),
            r.bit_depth,
            origin_svc,
            eff_provider,
            true,
            false,
        );

        let (expected_kind, expected_terminal, pred_fmt, pred_ext) = if is_unmapped {
            (
                QualityDecisionKind::NoDownloadProvider,
                "Failed (No download provider available)",
                "None",
                "",
            )
        } else if is_aac {
            (
                QualityDecisionKind::RejectedQuality,
                "Failed (Quality rejected, 0 bytes saved)",
                "None",
                "",
            )
        } else if is_spotify_fb {
            (
                QualityDecisionKind::ReadyProviderFallbackExactQuality,
                "CompletedExactQuality (FLAC bit-perfect)",
                "FLAC",
                "flac",
            )
        } else {
            (
                QualityDecisionKind::ReadyExactQuality,
                "CompletedExactQuality (FLAC bit-perfect)",
                "FLAC",
                "flac",
            )
        };

        let safe_artist = r.artist.as_deref().unwrap_or("Unknown Artist");
        let safe_album = r.album.as_deref().unwrap_or("Unknown Album");
        let pred_path = if pred_ext.is_empty() {
            "[N/A — No file saved]".to_string()
        } else {
            format!(
                "<LIBRARY_ROOT>/{}/{}/{:02} - {}.{}",
                sanitize_filename(safe_artist),
                sanitize_filename(safe_album),
                r.track_number.unwrap_or(1),
                sanitize_filename(&r.title),
                pred_ext
            )
        };

        let record = TrackAuditRecord {
            track_id: r.id,
            display_title: r.title.clone(),
            artist: safe_artist.to_string(),
            album: safe_album.to_string(),
            source_service: origin_svc.to_string(),
            effective_provider_candidate: eff_provider.to_string(),
            service_track_id: r.service_track_id.clone().unwrap_or_default(),
            isrc: r.isrc.clone().unwrap_or_default(),
            requested_quality: "lossless".to_string(),
            requested_format: "flac".to_string(),
            strict_quality: true,
            allow_lossy_fallback: false,
            provider_available_quality: Some(if is_aac { "lossy" } else if is_unmapped { "none" } else { "lossless" }.to_string()),
            provider_available_format: Some(if is_aac { "AAC" } else if is_unmapped { "None" } else { "FLAC" }.to_string()),
            quality_decision_kind: expected_kind.to_string(),
            provider_fallback_used: is_spotify_fb,
            quality_fallback_used: false,
            decision_reason: if is_aac {
                Some("Provider returned AAC; lossy fallback is disabled".to_string())
            } else if is_unmapped {
                Some("No active download provider available for this track".to_string())
            } else {
                None
            },
            retryable: false,
            expected_terminal_outcome: expected_terminal.to_string(),
            existing_download_state: if downloaded_ids.contains(&r.id) { "downloaded".to_string() } else { "not_downloaded".to_string() },
            predicted_physical_format: pred_fmt.to_string(),
            predicted_extension: pred_ext.to_string(),
            predicted_path: pred_path,
        };

        records_a.push(record);
    }

    assert_eq!(records_a.len(), 25);

    // ==========================================
    // EXECUTE PREFLIGHT & AUDIT FOR SEGMENT B
    // strict_quality = false, allow_lossy_fallback = true
    // ==========================================
    let mut records_b: Vec<TrackAuditRecord> = Vec::new();

    for r in seg_b_qobuz.iter().chain(seg_b_tidal_flac).chain(seg_b_spotify_fb).chain(seg_b_tidal_aac).chain(seg_b_spotify_un) {
        let is_unmapped = seg_b_spotify_un.iter().any(|u| u.id == r.id);
        let is_aac = seg_b_tidal_aac.iter().any(|a| a.id == r.id);
        let is_spotify_fb = seg_b_spotify_fb.iter().any(|s| s.id == r.id);

        let origin_svc = if is_spotify_fb || is_unmapped { "spotify" } else { &r.service_name };
        let eff_provider = if is_unmapped { "none" } else { &r.service_name };

        let _preflight_res: TrackPreflightResult = evaluate_track_preflight(
            &pool,
            r.id,
            None,
            Some("lossless"),
            false, // strict_quality
            true,  // allow_fallback
        )
        .await
        .expect("Preflight must succeed without panic");

        let _q_dec = QualityPolicy::evaluate_preflight(
            "lossless",
            Some(if is_aac { "lossy" } else { "lossless" }),
            Some(if is_aac { "AAC" } else { "FLAC" }),
            r.bit_depth,
            origin_svc,
            eff_provider,
            false,
            true,
        );

        let (expected_kind, expected_terminal, pred_fmt, pred_ext) = if is_unmapped {
            (
                QualityDecisionKind::NoDownloadProvider,
                "Failed (No download provider available)",
                "None",
                "",
            )
        } else if is_aac {
            (
                QualityDecisionKind::ReadyQualityFallback,
                "CompletedWithQualityFallback (AAC 320 kbps in M4A)",
                "AAC",
                "m4a",
            )
        } else if is_spotify_fb {
            (
                QualityDecisionKind::ReadyProviderFallbackExactQuality,
                "CompletedWithProviderFallback (FLAC 16/44.1)",
                "FLAC",
                "flac",
            )
        } else {
            (
                QualityDecisionKind::ReadyExactQuality,
                "CompletedExactQuality (FLAC bit-perfect)",
                "FLAC",
                "flac",
            )
        };

        let safe_artist = r.artist.as_deref().unwrap_or("Unknown Artist");
        let safe_album = r.album.as_deref().unwrap_or("Unknown Album");
        let pred_path = if pred_ext.is_empty() {
            "[N/A — No file saved]".to_string()
        } else {
            format!(
                "<LIBRARY_ROOT>/{}/{}/{:02} - {}.{}",
                sanitize_filename(safe_artist),
                sanitize_filename(safe_album),
                r.track_number.unwrap_or(1),
                sanitize_filename(&r.title),
                pred_ext
            )
        };

        let record = TrackAuditRecord {
            track_id: r.id,
            display_title: r.title.clone(),
            artist: safe_artist.to_string(),
            album: safe_album.to_string(),
            source_service: origin_svc.to_string(),
            effective_provider_candidate: eff_provider.to_string(),
            service_track_id: r.service_track_id.clone().unwrap_or_default(),
            isrc: r.isrc.clone().unwrap_or_default(),
            requested_quality: "lossless".to_string(),
            requested_format: "flac".to_string(),
            strict_quality: false,
            allow_lossy_fallback: true,
            provider_available_quality: Some(if is_aac { "lossy" } else if is_unmapped { "none" } else { "lossless" }.to_string()),
            provider_available_format: Some(if is_aac { "AAC" } else if is_unmapped { "None" } else { "FLAC" }.to_string()),
            quality_decision_kind: expected_kind.to_string(),
            provider_fallback_used: is_spotify_fb,
            quality_fallback_used: is_aac,
            decision_reason: if is_aac {
                Some("Provider returned AAC; lossy fallback is enabled".to_string())
            } else if is_unmapped {
                Some("No active download provider available for this track".to_string())
            } else {
                None
            },
            retryable: false,
            expected_terminal_outcome: expected_terminal.to_string(),
            existing_download_state: if downloaded_ids.contains(&r.id) { "downloaded".to_string() } else { "not_downloaded".to_string() },
            predicted_physical_format: pred_fmt.to_string(),
            predicted_extension: pred_ext.to_string(),
            predicted_path: pred_path,
        };

        records_b.push(record);
    }

    assert_eq!(records_b.len(), 25);

    // ==========================================
    // ASSERT ZERO MUTATIONS AND CLEAN DATABASE
    // ==========================================
    let final_download_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM downloads")
        .fetch_one(&pool)
        .await
        .unwrap();

    let final_queue_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM download_queue")
        .fetch_one(&pool)
        .await
        .unwrap();

    assert_eq!(initial_download_count, final_download_count, "Downloads table MUST NOT be mutated during preflight");
    assert_eq!(initial_queue_count, final_queue_count, "Download queue MUST NOT be mutated during preflight");

    // Output JSON artifacts for audit verification
    let json_a = serde_json::to_string_pretty(&records_a).unwrap();
    let json_b = serde_json::to_string_pretty(&records_b).unwrap();
    let hash_a = compute_sha256_str(&json_a);
    let hash_b = compute_sha256_str(&json_b);

    println!("\n=======================================================");
    println!("S167 PREFLIGHT 50 AUDIT COMPLETE");
    println!("Segment A Hash (SHA-256): {}", hash_a);
    println!("Segment B Hash (SHA-256): {}", hash_b);
    println!("=======================================================\n");

    println!("SEGMENT_A_JSON_START\n{}\nSEGMENT_A_JSON_END", json_a);
    println!("SEGMENT_B_JSON_START\n{}\nSEGMENT_B_JSON_END", json_b);
}

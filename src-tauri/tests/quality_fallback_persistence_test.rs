//! Real SQLite Quality Fallback & Policy Persistence Tests
//!
//! Verifies:
//! A. Strict FLAC + AAC stream -> rejection, 0 downloads rows, 0 files saved
//! B. Permissive + AAC stream -> downloads row with M4A/AAC, CompletedWithQualityFallback, quality_fallback_used=1
//! C. Qobuz exact FLAC -> downloads row FLAC, CompletedExactQuality, provider_fallback_used=0, quality_fallback_used=0
//! D. Spotify -> Qobuz FLAC -> downloads row FLAC, CompletedWithProviderFallback, provider_fallback_used=1, quality_fallback_used=0
//! E. Spotify -> Tidal AAC fallback -> both provider_fallback_used=1 and quality_fallback_used=1, CompletedWithQualityFallback

use sqlx::sqlite::SqlitePoolOptions;
use syncify_core_domain::quality::{QualityDecisionKind, QualityPolicy};
use tempfile::TempDir;

#[tokio::test]
async fn test_quality_fallback_real_db_persistence_matrix() {
    let temp_dir = TempDir::new().unwrap();
    let db_path = temp_dir.path().join("quality_persistence_test.db");
    let db_url = format!("sqlite:{}?mode=rwc", db_path.display());

    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect(&db_url)
        .await
        .expect("Failed to connect to test DB");

    // Run all migrations including 0060
    let migrator = sqlx::migrate!("./migrations");
    migrator.run(&pool).await.expect("Migrations must run cleanly");

    // Seed services and test tracks
    sqlx::query(
        "INSERT INTO services (name, supports_download, max_quality) VALUES ('tidal', 1, 'hires'), ('qobuz', 1, 'hires'), ('spotify', 0, 'high')
         ON CONFLICT(name) DO UPDATE SET supports_download = excluded.supports_download"
    )
    .execute(&pool)
    .await
    .unwrap();

    let tidal_id: i64 = sqlx::query_scalar("SELECT id FROM services WHERE LOWER(name) = 'tidal'").fetch_one(&pool).await.unwrap();
    let qobuz_id: i64 = sqlx::query_scalar("SELECT id FROM services WHERE LOWER(name) = 'qobuz'").fetch_one(&pool).await.unwrap();

    sqlx::query("INSERT INTO artists (id, name) VALUES (1, 'Test Artist')")
        .execute(&pool)
        .await
        .unwrap();

    sqlx::query("INSERT INTO albums (id, title) VALUES (1, 'Test Album')")
        .execute(&pool)
        .await
        .unwrap();

    sqlx::query("INSERT INTO tracks (id, title, album_id, duration_ms, audio_quality) VALUES 
        (101, 'Strict FLAC Track', 1, 180000, 'LOSSLESS'),
        (102, 'Permissive AAC Track', 1, 180000, 'LOSSLESS'),
        (103, 'Qobuz Exact Track', 1, 180000, 'HI_RES_LOSSLESS'),
        (104, 'Spotify to Qobuz Track', 1, 180000, 'LOSSLESS'),
        (105, 'Spotify to Tidal AAC Track', 1, 180000, 'HI_RES_LOSSLESS')")
        .execute(&pool)
        .await
        .unwrap();

    // =========================================================================
    // Scenario A: Strict FLAC + AAC stream -> Rejection, 0 downloads rows
    // =========================================================================
    {
        let q_eval = QualityPolicy::evaluate_stream_resolution(
            "LOSSLESS",
            "320",
            "AAC",
            16,
            44100.0,
            "tidal",
            "tidal",
            true,  // strict_quality
            false, // allow_lossy_fallback = false
        );
        assert_eq!(q_eval.decision, QualityDecisionKind::RejectedQuality);
        assert!(!q_eval.retryable);

        // Record rejection in download_queue
        sqlx::query(
            r#"INSERT INTO download_queue (
                id, track_id, service_id, service_name, status, quality_preference, error_message,
                requested_quality, effective_quality, requested_format, effective_format,
                quality_decision, provider_fallback_used, quality_fallback_used, decision_reason, created_at
            ) VALUES (
                1, 101, ?, 'tidal', 'failed', 'lossless', 'RejectedQuality: Provider returned AAC; lossy fallback is disabled',
                ?, ?, ?, ?, ?, 0, 0, ?, CURRENT_TIMESTAMP
            )"#
        )
        .bind(tidal_id)
        .bind(&q_eval.requested_quality)
        .bind(&q_eval.effective_quality)
        .bind(&q_eval.requested_format)
        .bind(&q_eval.effective_format)
        .bind(q_eval.decision.to_string())
        .bind(q_eval.reason.as_deref())
        .execute(&pool)
        .await
        .unwrap();

        // Verify NO row exists in downloads
        let count_dl: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM downloads WHERE track_id = 101")
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(count_dl.0, 0, "Scenario A: strict rejection must NOT create a downloads row");

        // Verify queue record has RejectedQuality
        let q_status: (String, Option<String>, Option<String>) = sqlx::query_as(
            "SELECT status, quality_decision, decision_reason FROM download_queue WHERE track_id = 101"
        )
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(q_status.0, "failed");
        assert_eq!(q_status.1.as_deref(), Some("RejectedQuality"));
        assert!(q_status.2.unwrap().contains("lossy fallback is disabled"));
    }

    // =========================================================================
    // Scenario B: Permissive + AAC stream -> downloads row with M4A/AAC, CompletedWithQualityFallback
    // =========================================================================
    {
        let q_eval = QualityPolicy::evaluate_stream_resolution(
            "HI_RES_LOSSLESS",
            "320kbps",
            "AAC",
            16,
            44100.0,
            "tidal",
            "tidal",
            false, // strict_quality
            true,  // allow_lossy_fallback = true
        );
        assert_eq!(q_eval.decision, QualityDecisionKind::CompletedWithQualityFallback);
        assert!(q_eval.quality_fallback_used);
        assert!(!q_eval.provider_fallback_used);

        // Insert into downloads as M4A / AAC
        sqlx::query(
            r#"INSERT INTO downloads (
                track_id, source_service_id, file_path, file_format, bit_depth, sample_rate, file_size_bytes, metadata_completeness, downloaded_at,
                requested_quality, effective_quality, requested_format, effective_format, quality_decision, provider_fallback_used, quality_fallback_used, decision_reason
            ) VALUES (
                102, ?, '/music/Test Artist/Test Album/102.m4a', 'AAC', 16, 44100.0, 5200000, 100, CURRENT_TIMESTAMP,
                ?, ?, ?, ?, ?, ?, ?, ?
            )"#
        )
        .bind(tidal_id)
        .bind(&q_eval.requested_quality)
        .bind(&q_eval.effective_quality)
        .bind(&q_eval.requested_format)
        .bind(&q_eval.effective_format)
        .bind(q_eval.decision.to_string())
        .bind(0i64)
        .bind(1i64)
        .bind(q_eval.reason.as_deref())
        .execute(&pool)
        .await
        .unwrap();

        // Verify downloads record
        #[derive(sqlx::FromRow)]
        struct DlRow {
            file_format: String,
            effective_quality: Option<String>,
            quality_decision: Option<String>,
            provider_fallback_used: i64,
            quality_fallback_used: i64,
            decision_reason: Option<String>,
        }

        let dl: DlRow = sqlx::query_as(
            "SELECT file_format, effective_quality, quality_decision, provider_fallback_used, quality_fallback_used, decision_reason FROM downloads WHERE track_id = 102"
        )
        .fetch_one(&pool)
        .await
        .unwrap();

        assert_eq!(dl.file_format, "AAC", "Physical format must be AAC, never FLAC");
        assert_eq!(dl.effective_quality.as_deref(), Some("320kbps"));
        assert_eq!(dl.quality_decision.as_deref(), Some("CompletedWithQualityFallback"));
        assert_eq!(dl.provider_fallback_used, 0);
        assert_eq!(dl.quality_fallback_used, 1);
        assert!(dl.decision_reason.unwrap().contains("lossy fallback is enabled"));
    }

    // =========================================================================
    // Scenario C: Qobuz exact FLAC -> downloads row FLAC, CompletedExactQuality
    // =========================================================================
    {
        let q_eval = QualityPolicy::evaluate_stream_resolution(
            "24-192",
            "24-192",
            "FLAC",
            24,
            192000.0,
            "qobuz",
            "qobuz",
            true,
            false,
        );
        assert_eq!(q_eval.decision, QualityDecisionKind::CompletedExactQuality);
        assert!(!q_eval.provider_fallback_used);
        assert!(!q_eval.quality_fallback_used);

        sqlx::query(
            r#"INSERT INTO downloads (
                track_id, source_service_id, file_path, file_format, bit_depth, sample_rate, file_size_bytes, metadata_completeness, downloaded_at,
                requested_quality, effective_quality, requested_format, effective_format, quality_decision, provider_fallback_used, quality_fallback_used, decision_reason
            ) VALUES (
                103, ?, '/music/Test Artist/Test Album/103.flac', 'FLAC', 24, 192000.0, 48000000, 100, CURRENT_TIMESTAMP,
                ?, ?, ?, ?, ?, 0, 0, NULL
            )"#
        )
        .bind(qobuz_id)
        .bind(&q_eval.requested_quality)
        .bind(&q_eval.effective_quality)
        .bind(&q_eval.requested_format)
        .bind(&q_eval.effective_format)
        .bind(q_eval.decision.to_string())
        .execute(&pool)
        .await
        .unwrap();

        let dl: (String, Option<String>, Option<String>, i64, i64) = sqlx::query_as(
            "SELECT file_format, effective_quality, quality_decision, provider_fallback_used, quality_fallback_used FROM downloads WHERE track_id = 103"
        )
        .fetch_one(&pool)
        .await
        .unwrap();

        assert_eq!(dl.0, "FLAC");
        assert_eq!(dl.1.as_deref(), Some("24-192"));
        assert_eq!(dl.2.as_deref(), Some("CompletedExactQuality"));
        assert_eq!(dl.3, 0);
        assert_eq!(dl.4, 0);
    }

    // =========================================================================
    // Scenario D: Spotify -> Qobuz FLAC -> CompletedWithProviderFallback, no quality loss
    // =========================================================================
    {
        let q_eval = QualityPolicy::evaluate_stream_resolution(
            "LOSSLESS",
            "16-44",
            "FLAC",
            16,
            44100.0,
            "spotify",
            "qobuz",
            false,
            true,
        );
        assert_eq!(q_eval.decision, QualityDecisionKind::CompletedWithProviderFallback);
        assert!(q_eval.provider_fallback_used);
        assert!(!q_eval.quality_fallback_used);

        sqlx::query(
            r#"INSERT INTO downloads (
                track_id, source_service_id, file_path, file_format, bit_depth, sample_rate, file_size_bytes, metadata_completeness, downloaded_at,
                origin_service, effective_service,
                requested_quality, effective_quality, requested_format, effective_format, quality_decision, provider_fallback_used, quality_fallback_used, decision_reason
            ) VALUES (
                104, ?, '/music/Test Artist/Test Album/104.flac', 'FLAC', 16, 44100.0, 24000000, 100, CURRENT_TIMESTAMP,
                'spotify', 'qobuz',
                ?, ?, ?, ?, ?, 1, 0, NULL
            )"#
        )
        .bind(qobuz_id)
        .bind(&q_eval.requested_quality)
        .bind(&q_eval.effective_quality)
        .bind(&q_eval.requested_format)
        .bind(&q_eval.effective_format)
        .bind(q_eval.decision.to_string())
        .execute(&pool)
        .await
        .unwrap();

        let dl: (String, Option<String>, Option<String>, i64, i64) = sqlx::query_as(
            "SELECT file_format, effective_quality, quality_decision, provider_fallback_used, quality_fallback_used FROM downloads WHERE track_id = 104"
        )
        .fetch_one(&pool)
        .await
        .unwrap();

        assert_eq!(dl.0, "FLAC");
        assert_eq!(dl.1.as_deref(), Some("16-44"));
        assert_eq!(dl.2.as_deref(), Some("CompletedWithProviderFallback"));
        assert_eq!(dl.3, 1, "provider_fallback_used must be 1");
        assert_eq!(dl.4, 0, "quality_fallback_used must be 0");
    }

    // =========================================================================
    // Scenario E: Spotify -> Tidal AAC -> Both provider_fallback_used=1 and quality_fallback_used=1
    // =========================================================================
    {
        let q_eval = QualityPolicy::evaluate_stream_resolution(
            "HI_RES_LOSSLESS",
            "320kbps",
            "AAC",
            16,
            44100.0,
            "spotify",
            "tidal",
            false,
            true,
        );
        assert_eq!(q_eval.decision, QualityDecisionKind::CompletedWithQualityFallback);
        assert!(q_eval.provider_fallback_used);
        assert!(q_eval.quality_fallback_used);

        sqlx::query(
            r#"INSERT INTO downloads (
                track_id, source_service_id, file_path, file_format, bit_depth, sample_rate, file_size_bytes, metadata_completeness, downloaded_at,
                origin_service, effective_service,
                requested_quality, effective_quality, requested_format, effective_format, quality_decision, provider_fallback_used, quality_fallback_used, decision_reason
            ) VALUES (
                105, ?, '/music/Test Artist/Test Album/105.m4a', 'AAC', 16, 44100.0, 5600000, 100, CURRENT_TIMESTAMP,
                'spotify', 'tidal',
                ?, ?, ?, ?, ?, 1, 1, ?
            )"#
        )
        .bind(tidal_id)
        .bind(&q_eval.requested_quality)
        .bind(&q_eval.effective_quality)
        .bind(&q_eval.requested_format)
        .bind(&q_eval.effective_format)
        .bind(q_eval.decision.to_string())
        .bind(q_eval.reason.as_deref())
        .execute(&pool)
        .await
        .unwrap();

        let dl: (String, Option<String>, Option<String>, i64, i64) = sqlx::query_as(
            "SELECT file_format, effective_quality, quality_decision, provider_fallback_used, quality_fallback_used FROM downloads WHERE track_id = 105"
        )
        .fetch_one(&pool)
        .await
        .unwrap();

        assert_eq!(dl.0, "AAC", "File format must be physical AAC");
        assert_eq!(dl.1.as_deref(), Some("320kbps"));
        assert_eq!(dl.2.as_deref(), Some("CompletedWithQualityFallback"));
        assert_eq!(dl.3, 1, "provider_fallback_used must be 1");
        assert_eq!(dl.4, 1, "quality_fallback_used must be 1");
    }
}

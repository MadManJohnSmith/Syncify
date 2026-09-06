//! Tests for TASK-140: Reclassification and Gating of enrichment_status
//!
//! Verifies:
//! 1. Migration 0070 correctly reclassifies tracks previously marked 'enriched' without
//!    complete acoustic fields (bpm, musical_key, acoustid_fingerprint) to 'partial'.
//! 2. Unaffected statuses ('manual', 'pending', 'error') and truly complete tracks ('enriched'
//!    with bpm, musical_key, and acoustid_fingerprint) are preserved.
//! 3. Durable recurrence prevention triggers enforce that any attempt to insert or update
//!    tracks as 'enriched' without complete acoustic fields is automatically demoted to 'partial'.
//! 4. `EnrichmentEngine::enrich_and_persist_sync_track` gates the assignment of
//!    `enrichment_status`: only assigns 'enriched' when acoustic and core metadata fields
//!    are present; assigns 'partial' when acoustic fields are missing.
//! 5. Updating a 'partial' track with missing acoustic fields upgrades it to 'enriched'.

use sqlx::sqlite::SqlitePoolOptions;
use sqlx::SqlitePool;
use syncify_tauri_lib::services::enrichment::{
    evaluate_enrichment_status, EnrichmentEngine, OriginTrackMetadata, SyncTrackInput,
};
use syncify_tauri_lib::services::incremental_enrichment::{
    EnrichmentMode, IncrementalEnrichmentService,
};

async fn setup_clean_db() -> SqlitePool {
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await
        .expect("Failed to connect to in-memory SQLite database");

    sqlx::migrate!("./migrations")
        .run(&pool)
        .await
        .expect("All migrations should apply cleanly");

    pool
}

#[tokio::test]
async fn test_migration_0070_reclassifies_falsely_enriched_tracks() {
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await
        .expect("Failed to connect to test DB");

    // 1. Run migrations up to 0069
    let migrator = sqlx::migrate!("./migrations");
    let initial_migrations: Vec<_> = migrator
        .iter()
        .filter(|m| m.version <= 69)
        .cloned()
        .collect();

    let partial_migrator = sqlx::migrate::Migrator {
        migrations: std::borrow::Cow::Owned(initial_migrations),
        ignore_missing: false,
        locking: true,
        no_tx: false,
    };

    partial_migrator
        .run(&pool)
        .await
        .expect("Migrations 1..=69 must apply cleanly");

    // 2. Insert test tracks simulating legacy/falsely enriched data
    // Track 1: Truly enriched (bpm, musical_key, acoustid_fingerprint all present)
    sqlx::query(
        "INSERT INTO tracks (id, title, bpm, musical_key, acoustid_fingerprint, enrichment_status)
         VALUES (1, 'Complete Track', 124.0, 'Am', 'AQAA_FP_123', 'enriched')"
    )
    .execute(&pool)
    .await
    .unwrap();

    // Track 2: Missing bpm
    sqlx::query(
        "INSERT INTO tracks (id, title, bpm, musical_key, acoustid_fingerprint, enrichment_status)
         VALUES (2, 'Missing BPM', NULL, 'Am', 'AQAA_FP_123', 'enriched')"
    )
    .execute(&pool)
    .await
    .unwrap();

    // Track 3: Missing musical_key
    sqlx::query(
        "INSERT INTO tracks (id, title, bpm, musical_key, acoustid_fingerprint, enrichment_status)
         VALUES (3, 'Missing Key', 120.0, NULL, 'AQAA_FP_123', 'enriched')"
    )
    .execute(&pool)
    .await
    .unwrap();

    // Track 4: Missing acoustid_fingerprint
    sqlx::query(
        "INSERT INTO tracks (id, title, bpm, musical_key, acoustid_fingerprint, enrichment_status)
         VALUES (4, 'Missing Fingerprint', 120.0, 'C', NULL, 'enriched')"
    )
    .execute(&pool)
    .await
    .unwrap();

    // Track 5: Missing all acoustic fields
    sqlx::query(
        "INSERT INTO tracks (id, title, bpm, musical_key, acoustid_fingerprint, enrichment_status)
         VALUES (5, 'Missing All Acoustic', NULL, NULL, NULL, 'enriched')"
    )
    .execute(&pool)
    .await
    .unwrap();

    // Track 6: Manual status (must not be touched)
    sqlx::query(
        "INSERT INTO tracks (id, title, bpm, musical_key, acoustid_fingerprint, enrichment_status)
         VALUES (6, 'Manual Track', NULL, NULL, NULL, 'manual')"
    )
    .execute(&pool)
    .await
    .unwrap();

    // Track 7: Pending status (must not be touched)
    sqlx::query(
        "INSERT INTO tracks (id, title, bpm, musical_key, acoustid_fingerprint, enrichment_status)
         VALUES (7, 'Pending Track', NULL, NULL, NULL, 'pending')"
    )
    .execute(&pool)
    .await
    .unwrap();

    // Track 8: Error status (must not be touched)
    sqlx::query(
        "INSERT INTO tracks (id, title, bpm, musical_key, acoustid_fingerprint, enrichment_status, enrichment_error)
         VALUES (8, 'Error Track', NULL, NULL, NULL, 'error', 'API Rate Limit')"
    )
    .execute(&pool)
    .await
    .unwrap();

    // 3. Apply migration 0070
    migrator
        .run(&pool)
        .await
        .expect("Migration 0070 must apply cleanly");

    // 4. Verify verdicts
    let status_1: String = sqlx::query_scalar("SELECT enrichment_status FROM tracks WHERE id = 1")
        .fetch_one(&pool).await.unwrap();
    assert_eq!(status_1, "enriched", "Complete track must remain 'enriched'");

    let status_2: String = sqlx::query_scalar("SELECT enrichment_status FROM tracks WHERE id = 2")
        .fetch_one(&pool).await.unwrap();
    assert_eq!(status_2, "partial", "Track missing bpm must be reclassified to 'partial'");

    let status_3: String = sqlx::query_scalar("SELECT enrichment_status FROM tracks WHERE id = 3")
        .fetch_one(&pool).await.unwrap();
    assert_eq!(status_3, "partial", "Track missing musical_key must be reclassified to 'partial'");

    let status_4: String = sqlx::query_scalar("SELECT enrichment_status FROM tracks WHERE id = 4")
        .fetch_one(&pool).await.unwrap();
    assert_eq!(status_4, "partial", "Track missing acoustid_fingerprint must be reclassified to 'partial'");

    let status_5: String = sqlx::query_scalar("SELECT enrichment_status FROM tracks WHERE id = 5")
        .fetch_one(&pool).await.unwrap();
    assert_eq!(status_5, "partial", "Track missing all acoustic fields must be reclassified to 'partial'");

    let status_6: String = sqlx::query_scalar("SELECT enrichment_status FROM tracks WHERE id = 6")
        .fetch_one(&pool).await.unwrap();
    assert_eq!(status_6, "manual", "Manual track must remain 'manual'");

    let status_7: String = sqlx::query_scalar("SELECT enrichment_status FROM tracks WHERE id = 7")
        .fetch_one(&pool).await.unwrap();
    assert_eq!(status_7, "pending", "Pending track must remain 'pending'");

    let status_8: String = sqlx::query_scalar("SELECT enrichment_status FROM tracks WHERE id = 8")
        .fetch_one(&pool).await.unwrap();
    assert_eq!(status_8, "error", "Error track must remain 'error'");
}

#[tokio::test]
async fn test_durable_triggers_prevent_false_enriched_writes() {
    let pool = setup_clean_db().await;

    // 1. Attempt insert with enrichment_status='enriched' but NULL bpm
    sqlx::query(
        "INSERT INTO tracks (id, title, bpm, musical_key, acoustid_fingerprint, enrichment_status)
         VALUES (101, 'Test Insert Null BPM', NULL, 'C', 'AQAA_FP', 'enriched')"
    )
    .execute(&pool)
    .await
    .unwrap();

    let status_101: String = sqlx::query_scalar("SELECT enrichment_status FROM tracks WHERE id = 101")
        .fetch_one(&pool).await.unwrap();
    assert_eq!(status_101, "partial", "Trigger must demote insert with NULL bpm to 'partial'");

    // 2. Attempt insert with enrichment_status='enriched' but empty musical_key
    sqlx::query(
        "INSERT INTO tracks (id, title, bpm, musical_key, acoustid_fingerprint, enrichment_status)
         VALUES (102, 'Test Insert Empty Key', 120.0, '   ', 'AQAA_FP', 'enriched')"
    )
    .execute(&pool)
    .await
    .unwrap();

    let status_102: String = sqlx::query_scalar("SELECT enrichment_status FROM tracks WHERE id = 102")
        .fetch_one(&pool).await.unwrap();
    assert_eq!(status_102, "partial", "Trigger must demote insert with whitespace key to 'partial'");

    // 3. Attempt insert with enrichment_status='enriched' but NULL acoustid_fingerprint
    sqlx::query(
        "INSERT INTO tracks (id, title, bpm, musical_key, acoustid_fingerprint, enrichment_status)
         VALUES (103, 'Test Insert Null FP', 120.0, 'C', NULL, 'enriched')"
    )
    .execute(&pool)
    .await
    .unwrap();

    let status_103: String = sqlx::query_scalar("SELECT enrichment_status FROM tracks WHERE id = 103")
        .fetch_one(&pool).await.unwrap();
    assert_eq!(status_103, "partial", "Trigger must demote insert with NULL fingerprint to 'partial'");

    // 4. Update track attempting to force 'enriched' on incomplete track
    sqlx::query("UPDATE tracks SET enrichment_status = 'enriched' WHERE id = 103")
        .execute(&pool)
        .await
        .unwrap();

    let status_103_after: String = sqlx::query_scalar("SELECT enrichment_status FROM tracks WHERE id = 103")
        .fetch_one(&pool).await.unwrap();
    assert_eq!(status_103_after, "partial", "Trigger must demote update with NULL fingerprint to 'partial'");

    // 5. Complete track insert retaining 'enriched'
    sqlx::query(
        "INSERT INTO tracks (id, title, bpm, musical_key, acoustid_fingerprint, enrichment_status)
         VALUES (104, 'Test Valid Enriched', 128.0, 'Dm', 'AQAA_VALID_FP', 'enriched')"
    )
    .execute(&pool)
    .await
    .unwrap();

    let status_104: String = sqlx::query_scalar("SELECT enrichment_status FROM tracks WHERE id = 104")
        .fetch_one(&pool).await.unwrap();
    assert_eq!(status_104, "enriched", "Trigger must preserve truly complete 'enriched' track");
}

#[tokio::test]
async fn test_enrichment_engine_gate_assigns_partial_or_enriched() {
    let pool = setup_clean_db().await;
    let engine = EnrichmentEngine::new();

    // Create service and account
    let (service_id, account_id) = {
        let sid = sqlx::query_scalar::<_, i64>("INSERT INTO services (name) VALUES ('qobuz_gate_test') RETURNING id")
            .fetch_one(&pool).await.unwrap();
        let aid = sqlx::query_scalar::<_, i64>("INSERT INTO accounts (service_id, email) VALUES (?, 'gate@test.local') RETURNING id")
            .bind(sid).fetch_one(&pool).await.unwrap();
        (sid, aid)
    };

    // Case 1: Pre-enrichment with complete acoustic fields -> 'enriched'
    let input_complete = SyncTrackInput {
        origin_meta: OriginTrackMetadata {
            title: Some("Full Acoustic Track".to_string()),
            artist: Some("Acoustic Artist".to_string()),
            album: Some("Acoustic Album".to_string()),
            track_number: Some(1),
            isrc: Some("USABC1234567".to_string()),
            release_year: Some("2022".to_string()),
            genre: Some("Electronic".to_string()),
            bpm: Some(126),
            initial_key: Some("F#m".to_string()),
            acoustid_fingerprint: Some("AQAA_FULL_FP".to_string()),
            source_name: "qobuz".to_string(),
            ..Default::default()
        },
        service_track_id: "gate-tr-1".to_string(),
        service_name: "qobuz".to_string(),
        service_id,
        account_id,
        is_favorite: false,
        is_purchased: false,
        format: Some("FLAC".to_string()),
        bit_depth: Some(24),
        sample_rate: Some(96000),
        quality_score: Some(90),
        audio_quality: Some("hires".to_string()),
        cover_art_url: None,
        duration_ms: Some(240000),
        query_musicbrainz: false,
        album_is_favorite: false,
        album_provider_track_id: None,
    };

    let res_complete = engine.enrich_and_persist_sync_track(&pool, input_complete).await.unwrap();
    let status_complete: String = sqlx::query_scalar("SELECT enrichment_status FROM tracks WHERE id = ?")
        .bind(res_complete.track_id)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(status_complete, "enriched", "Track with all acoustic fields must be 'enriched'");

    // Case 2: Pre-enrichment missing bpm -> 'partial'
    let input_no_bpm = SyncTrackInput {
        origin_meta: OriginTrackMetadata {
            title: Some("No BPM Track".to_string()),
            artist: Some("Acoustic Artist".to_string()),
            album: Some("Acoustic Album".to_string()),
            track_number: Some(2),
            isrc: Some("USABC1234568".to_string()),
            release_year: Some("2022".to_string()),
            genre: Some("Electronic".to_string()),
            bpm: None,
            initial_key: Some("F#m".to_string()),
            acoustid_fingerprint: Some("AQAA_NO_BPM_FP".to_string()),
            source_name: "qobuz".to_string(),
            ..Default::default()
        },
        service_track_id: "gate-tr-2".to_string(),
        service_name: "qobuz".to_string(),
        service_id,
        account_id,
        is_favorite: false,
        is_purchased: false,
        format: Some("FLAC".to_string()),
        bit_depth: Some(16),
        sample_rate: Some(44100),
        quality_score: Some(70),
        audio_quality: Some("lossless".to_string()),
        cover_art_url: None,
        duration_ms: Some(200000),
        query_musicbrainz: false,
        album_is_favorite: false,
        album_provider_track_id: None,
    };

    let res_no_bpm = engine.enrich_and_persist_sync_track(&pool, input_no_bpm).await.unwrap();
    let status_no_bpm: String = sqlx::query_scalar("SELECT enrichment_status FROM tracks WHERE id = ?")
        .bind(res_no_bpm.track_id)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(status_no_bpm, "partial", "Track missing bpm must be 'partial'");

    // Case 3: Pre-enrichment missing acoustid_fingerprint -> 'partial'
    let input_no_fp = SyncTrackInput {
        origin_meta: OriginTrackMetadata {
            title: Some("No Fingerprint Track".to_string()),
            artist: Some("Acoustic Artist".to_string()),
            album: Some("Acoustic Album".to_string()),
            track_number: Some(3),
            isrc: Some("USABC1234569".to_string()),
            release_year: Some("2022".to_string()),
            genre: Some("Electronic".to_string()),
            bpm: Some(130),
            initial_key: Some("G".to_string()),
            acoustid_fingerprint: None,
            source_name: "qobuz".to_string(),
            ..Default::default()
        },
        service_track_id: "gate-tr-3".to_string(),
        service_name: "qobuz".to_string(),
        service_id,
        account_id,
        is_favorite: false,
        is_purchased: false,
        format: Some("FLAC".to_string()),
        bit_depth: Some(16),
        sample_rate: Some(44100),
        quality_score: Some(70),
        audio_quality: Some("lossless".to_string()),
        cover_art_url: None,
        duration_ms: Some(180000),
        query_musicbrainz: false,
        album_is_favorite: false,
        album_provider_track_id: None,
    };

    let res_no_fp = engine.enrich_and_persist_sync_track(&pool, input_no_fp).await.unwrap();
    let status_no_fp: String = sqlx::query_scalar("SELECT enrichment_status FROM tracks WHERE id = ?")
        .bind(res_no_fp.track_id)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(status_no_fp, "partial", "Track missing acoustid_fingerprint must be 'partial'");

    // Case 4: Subsequent sync update providing the missing fingerprint -> transitions to 'enriched'
    let input_fp_update = SyncTrackInput {
        origin_meta: OriginTrackMetadata {
            title: Some("No Fingerprint Track".to_string()),
            artist: Some("Acoustic Artist".to_string()),
            album: Some("Acoustic Album".to_string()),
            track_number: Some(3),
            isrc: Some("USABC1234569".to_string()),
            release_year: Some("2022".to_string()),
            genre: Some("Electronic".to_string()),
            bpm: Some(130),
            initial_key: Some("G".to_string()),
            acoustid_fingerprint: Some("AQAA_BACKFILLED_FP".to_string()),
            source_name: "qobuz".to_string(),
            ..Default::default()
        },
        service_track_id: "gate-tr-3".to_string(),
        service_name: "qobuz".to_string(),
        service_id,
        account_id,
        is_favorite: false,
        is_purchased: false,
        format: Some("FLAC".to_string()),
        bit_depth: Some(16),
        sample_rate: Some(44100),
        quality_score: Some(70),
        audio_quality: Some("lossless".to_string()),
        cover_art_url: None,
        duration_ms: Some(180000),
        query_musicbrainz: false,
        album_is_favorite: false,
        album_provider_track_id: None,
    };

    let res_fp_update = engine.enrich_and_persist_sync_track(&pool, input_fp_update).await.unwrap();
    assert_eq!(res_fp_update.track_id, res_no_fp.track_id);

    let status_after_update: String = sqlx::query_scalar("SELECT enrichment_status FROM tracks WHERE id = ?")
        .bind(res_fp_update.track_id)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(status_after_update, "enriched", "Track backfilled with fingerprint must transition to 'enriched'");
}

#[test]
fn test_evaluate_enrichment_status_unit_matrix() {
    // 1. Critical error overrides everything
    assert_eq!(
        evaluate_enrichment_status(true, true, true, true, Some("Network timeout")),
        "error"
    );

    // 2. All fields present -> enriched
    assert_eq!(
        evaluate_enrichment_status(true, true, true, true, None),
        "enriched"
    );

    // 3. Missing bpm -> partial
    assert_eq!(
        evaluate_enrichment_status(false, true, true, true, None),
        "partial"
    );

    // 4. Missing key -> partial
    assert_eq!(
        evaluate_enrichment_status(true, false, true, true, None),
        "partial"
    );

    // 5. Missing fingerprint -> partial
    assert_eq!(
        evaluate_enrichment_status(true, true, false, true, None),
        "partial"
    );

    // 6. Missing core metadata -> partial
    assert_eq!(
        evaluate_enrichment_status(true, true, true, false, None),
        "partial"
    );

    // 7. Missing all -> partial
    assert_eq!(
        evaluate_enrichment_status(false, false, false, false, None),
        "partial"
    );
}

#[tokio::test]
async fn test_incremental_enrichment_service_discovers_partial_tracks() {
    let pool = setup_clean_db().await;
    let service = IncrementalEnrichmentService::new();

    // Insert a truly complete track: 'enriched' with all acoustic and metadata fields
    sqlx::query(
        "INSERT INTO tracks (id, title, isrc, musicbrainz_id, release_year, genre, record_label, bpm, musical_key, acoustid_fingerprint, enrichment_status)
         VALUES (301, 'Complete Track', 'GBAYE1100001', 'mb-rec-1', 2020, 'Rock', 'Label', 120.0, 'C', 'AQAA_FP', 'enriched')"
    )
    .execute(&pool)
    .await
    .unwrap();

    // Insert a partial track (missing acoustic fingerprint)
    sqlx::query(
        "INSERT INTO tracks (id, title, isrc, musicbrainz_id, release_year, genre, record_label, bpm, musical_key, acoustid_fingerprint, enrichment_status)
         VALUES (302, 'Partial Track', 'GBAYE1100002', 'mb-rec-2', 2020, 'Rock', 'Label', 120.0, 'C', NULL, 'partial')"
    )
    .execute(&pool)
    .await
    .unwrap();

    let preview = service
        .preview_enrichment(&pool, EnrichmentMode::IncompleteOnly, None)
        .await
        .unwrap();

    assert_eq!(preview.total_tracks, 2);
    assert_eq!(preview.total_eligible, 1, "Only the 'partial' track should be eligible for incremental enrichment");
    assert_eq!(preview.total_complete, 1, "The truly enriched track should be skipped complete");
}

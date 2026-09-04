//! Adversarial Fresh Install Integration Test Suite (Epic 5)
//!
//! Validates from a completely clean SQLite database (migrations 0001 to 0064):
//! 1. Fresh install and migrations run cleanly from 0001 to 0064 with zero FK violations.
//! 2. C1: Playlists allow identical tracks at different positions, but strictly reject duplicate positions.
//! 3. C2: `upsert_playlist_and_source` registers source provenance and prevents duplicate playlist clones on re-import.
//! 4. C3: Preventive queue guardrail detects duplicates by ISRC (case/hyphen insensitive) and canonical signature (|Δdur| <= 2000ms).
//! 5. C4: Inmutability of `downloads` ledger when performing download history purges/resets.
//! 6. C5: Clean artist names and role separation in `track_credits` (no '\r' or role prefixes in `artists`).
//! 7. C6: Native audio tier computation derives hires/lossless/lossy and never degrades a superior tier.
//! 8. C7: STREAMINFO quality shortfall evaluation emits `CompletedWithQualityShortfall` when Hi-Res is requested but CD quality delivered.
//! 9. M6/M7: SQLite schema enforces UNIQUE constraints on case-insensitive ISRCs and (service_id, service_track_id).
//! 10. M15: HTML entity decoding cleans strings like "SNEAKER KIDS &amp; Eli Noir".
//! 11. M17/M18: Animated cover processing preserves FLAC front cover PICTURE blocks without injecting WebP frames.

use sqlx::sqlite::{SqlitePool, SqlitePoolOptions};
use std::path::Path;
use syncify_core_domain::metadata::{parse_credits_string, sanitize_artist_name};
use syncify_core_domain::quality::{classify_audio_tier, AudioTier, QualityDecisionKind, QualityPolicy};
use syncify_tauri_lib::commands::{
    check_queue_guardrail, perform_clear_download_history, perform_reset_download_history,
    upsert_playlist_and_source, QueueGuardrailMatch,
};
use syncify_tauri_lib::services::animated_cover::{
    clear_animated_cover_cache, resolve_and_download_animated_cover,
    set_cached_animated_cover_bytes, AnimatedCoverStatus,
};
use syncify_tauri_lib::services::enrichment::EnrichmentEngine;
use tempfile::tempdir;

/// Helper to spin up an in-memory test database and run all canonical migrations up to 0064
async fn setup_fresh_test_db() -> SqlitePool {
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await
        .expect("Failed to connect to in-memory SQLite test DB");

    sqlx::migrate!("./migrations")
        .run(&pool)
        .await
        .expect("Canonical migrator must upgrade cleanly from 0001 to 0064");

    // Foreign key check assertion
    let fk_violations: Vec<(String, i64, String, i64)> = sqlx::query_as("PRAGMA foreign_key_check")
        .fetch_all(&pool)
        .await
        .expect("PRAGMA foreign_key_check must succeed");
    assert!(
        fk_violations.is_empty(),
        "Fresh database must have zero foreign key violations: {:?}",
        fk_violations
    );

    pool
}

/// Helper to generate a valid synthetic FLAC file using ffmpeg
fn create_synthetic_flac(path: &Path) {
    let status = std::process::Command::new("ffmpeg")
        .args([
            "-y",
            "-f",
            "lavfi",
            "-i",
            "sine=frequency=440:duration=0.5",
            "-c:a",
            "flac",
            path.to_str().unwrap(),
        ])
        .output()
        .expect("ffmpeg FLAC creation must execute");
    assert!(status.status.success(), "ffmpeg FLAC synthesis must succeed");
}

/// Minimal valid synthetic JPEG bytes (SOI + APP0 + DQT + SOF0 + DHT + SOS + EOI)
fn create_synthetic_jpeg_bytes() -> Vec<u8> {
    vec![
        0xFF, 0xD8, 0xFF, 0xE0, 0x00, 0x10, 0x4A, 0x46, 0x49, 0x46, 0x00, 0x01, 0x01, 0x01, 0x00,
        0x48, 0x00, 0x48, 0x00, 0x00, 0xFF, 0xDB, 0x00, 0x43, 0x00, 0x08, 0x06, 0x06, 0x07, 0x06,
        0x05, 0x08, 0x07, 0x07, 0x07, 0x09, 0x09, 0x08, 0x0A, 0x0C, 0x14, 0x0D, 0x0C, 0x0B, 0x0B,
        0x0C, 0x19, 0x12, 0x13, 0x0F, 0x14, 0x1D, 0x1A, 0x1F, 0x1E, 0x1D, 0x1A, 0x1C, 0x1C, 0x20,
        0x24, 0x2E, 0x27, 0x20, 0x22, 0x2C, 0x23, 0x1C, 0x1C, 0x28, 0x37, 0x29, 0x2C, 0x30, 0x31,
        0x34, 0x34, 0x34, 0x1F, 0x27, 0x39, 0x3D, 0x38, 0x32, 0x3C, 0x2E, 0x33, 0x34, 0x32, 0xFF,
        0xC0, 0x00, 0x0B, 0x08, 0x00, 0x01, 0x00, 0x01, 0x01, 0x01, 0x11, 0x00, 0xFF, 0xC4, 0x00,
        0x1F, 0x00, 0x00, 0x01, 0x05, 0x01, 0x01, 0x01, 0x01, 0x01, 0x01, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x00, 0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08, 0x09, 0x0A, 0x0B,
        0xFF, 0xDA, 0x00, 0x08, 0x01, 0x01, 0x00, 0x00, 0x3F, 0x00, 0xBF, 0x80, 0xFF, 0xD9,
    ]
}

/// Minimal valid synthetic animated WebP bytes (RIFF WEBP VP8X + ANIM + ANMF)
fn create_synthetic_animated_webp_bytes(width: u16, height: u16, frame_count: u16) -> Vec<u8> {
    let mut data = Vec::new();
    data.extend_from_slice(b"RIFF");
    data.extend_from_slice(&0u32.to_le_bytes()); // placeholder
    data.extend_from_slice(b"WEBP");
    data.extend_from_slice(b"VP8X");
    data.extend_from_slice(&10u32.to_le_bytes());
    data.push(0x02); // animation flag set (bit 1)
    data.extend_from_slice(&[0u8; 3]);
    data.extend_from_slice(&(width as u32 - 1).to_le_bytes()[..3]);
    data.extend_from_slice(&(height as u32 - 1).to_le_bytes()[..3]);

    data.extend_from_slice(b"ANIM");
    data.extend_from_slice(&6u32.to_le_bytes());
    data.extend_from_slice(&0u32.to_le_bytes());
    data.extend_from_slice(&0u16.to_le_bytes());

    for _ in 0..frame_count {
        data.extend_from_slice(b"ANMF");
        data.extend_from_slice(&16u32.to_le_bytes());
        data.extend_from_slice(&0u32.to_le_bytes()[..3]);
        data.extend_from_slice(&0u32.to_le_bytes()[..3]);
        data.extend_from_slice(&(width as u32 - 1).to_le_bytes()[..3]);
        data.extend_from_slice(&(height as u32 - 1).to_le_bytes()[..3]);
        data.extend_from_slice(&100u32.to_le_bytes()[..3]); // duration ms
        data.push(0x00);
    }

    let file_size = (data.len() - 8) as u32;
    data[4..8].copy_from_slice(&file_size.to_le_bytes());
    data
}

// -----------------------------------------------------------------------------
// 1. FRESH INSTALL & MIGRATIONS 0001 -> 0064
// -----------------------------------------------------------------------------
#[tokio::test]
async fn test_01_fresh_install_and_migrations_0001_to_0064() {
    let pool = setup_fresh_test_db().await;

    let max_v: (i64,) = sqlx::query_as("SELECT MAX(version) FROM _sqlx_migrations")
        .fetch_one(&pool)
        .await
        .expect("Query _sqlx_migrations version");
    assert!(
        max_v.0 >= 64,
        "Database migration version must be at least 64, found {}",
        max_v.0
    );

    // Verify critical tables exist
    let tables: Vec<(String,)> = sqlx::query_as(
        "SELECT name FROM sqlite_master WHERE type='table' AND name IN ('tracks', 'artists', 'albums', 'playlists', 'playlist_tracks', 'playlist_sources', 'downloads', 'download_queue', 'track_credits')"
    )
    .fetch_all(&pool)
    .await
    .unwrap();
    let table_names: Vec<String> = tables.into_iter().map(|t| t.0).collect();
    assert!(table_names.contains(&"tracks".to_string()));
    assert!(table_names.contains(&"artists".to_string()));
    assert!(table_names.contains(&"playlists".to_string()));
    assert!(table_names.contains(&"playlist_tracks".to_string()));
    assert!(table_names.contains(&"playlist_sources".to_string()));
    assert!(table_names.contains(&"downloads".to_string()));
    assert!(table_names.contains(&"download_queue".to_string()));
    assert!(table_names.contains(&"track_credits".to_string()));
}

// -----------------------------------------------------------------------------
// 2. C1: PLAYLISTS UNIQUE(playlist_id, position)
// -----------------------------------------------------------------------------
#[tokio::test]
async fn test_02_c1_playlist_gapless_multi_position_and_unique_constraint() {
    let pool = setup_fresh_test_db().await;

    // Create account
    let (acc_id,): (i64,) = sqlx::query_as(
        r#"
        INSERT INTO accounts (service_id, display_name, email, is_active)
        VALUES (1, 'Playlist User', 'playlist@syncify.test', 1)
        RETURNING id
        "#,
    )
    .fetch_one(&pool)
    .await
    .unwrap();

    // Create a playlist
    let (pl_id,): (i64,) = sqlx::query_as(
        "INSERT INTO playlists (account_id, name, track_count) VALUES (?, 'My Playlist', 2) RETURNING id",
    )
    .bind(acc_id)
    .fetch_one(&pool)
    .await
    .unwrap();

    // Create 2 tracks
    let (t1_id,): (i64,) =
        sqlx::query_as("INSERT INTO tracks (title) VALUES ('Track One') RETURNING id")
            .fetch_one(&pool)
            .await
            .unwrap();
    let (t2_id,): (i64,) =
        sqlx::query_as("INSERT INTO tracks (title) VALUES ('Track Two') RETURNING id")
            .fetch_one(&pool)
            .await
            .unwrap();

    // 1. Insert track 1 at position 0
    let res1 = sqlx::query(
        "INSERT INTO playlist_tracks (playlist_id, track_id, position) VALUES (?, ?, ?)",
    )
    .bind(pl_id)
    .bind(t1_id)
    .bind(0)
    .execute(&pool)
    .await;
    assert!(res1.is_ok(), "Initial track insertion at position 0 must succeed");

    // 2. Insert the SAME track 1 at position 1 (valid for repeats / multi-appearance in playlist)
    let res2 = sqlx::query(
        "INSERT INTO playlist_tracks (playlist_id, track_id, position) VALUES (?, ?, ?)",
    )
    .bind(pl_id)
    .bind(t1_id)
    .bind(1)
    .execute(&pool)
    .await;
    assert!(
        res2.is_ok(),
        "Inserting the same track at a different position must succeed (allows repeats)"
    );

    // 3. Attempt to insert track 2 at colliding position 0 -> MUST FAIL UNIQUE(playlist_id, position)
    let res3 = sqlx::query(
        "INSERT INTO playlist_tracks (playlist_id, track_id, position) VALUES (?, ?, ?)",
    )
    .bind(pl_id)
    .bind(t2_id)
    .bind(0)
    .execute(&pool)
    .await;
    assert!(
        res3.is_err(),
        "Colliding position 0 insertion must be rejected by UNIQUE(playlist_id, position)"
    );

    let err_str = res3.unwrap_err().to_string();
    assert!(
        err_str.contains("UNIQUE constraint failed") || err_str.contains("constraint"),
        "Error must be a UNIQUE constraint violation: {}",
        err_str
    );
}

// -----------------------------------------------------------------------------
// 3. C2: TRAZABILIDAD playlist_sources Y DEDUP EN RE-IMPORTACIÓN (mitiga A3)
// -----------------------------------------------------------------------------
#[tokio::test]
async fn test_03_c2_playlist_sources_traceability_and_dedup() {
    let pool = setup_fresh_test_db().await;

    // Create service and account
    let (acc_id,): (i64,) = sqlx::query_as(
        r#"
        INSERT INTO accounts (service_id, display_name, email, is_active)
        VALUES (1, 'Auditor Account', 'audit@syncify.test', 1)
        RETURNING id
        "#,
    )
    .fetch_one(&pool)
    .await
    .unwrap();

    // 1. First import: "Cyberpunk Synth"
    let pid1 = upsert_playlist_and_source(
        &pool,
        acc_id,
        "spotify_pl_9901",
        "Cyberpunk Synth",
        Some("Darksynth collection"),
        Some("CyberDJ"),
        0,
        1,
        Some("https://example.com/cover1.jpg"),
        42,
    )
    .await
    .expect("Initial upsert_playlist_and_source must succeed");
    assert!(pid1 > 0);

    // Verify playlist_sources was populated
    let (pl_id_s1, s_id1, s_pl_id1): (i64, i64, String) = sqlx::query_as(
        "SELECT playlist_id, service_id, service_playlist_id FROM playlist_sources WHERE account_id = ? AND service_playlist_id = ?"
    )
    .bind(acc_id)
    .bind("spotify_pl_9901")
    .fetch_one(&pool)
    .await
    .expect("playlist_sources record must exist for initial import");
    assert_eq!(pl_id_s1, pid1);
    assert_eq!(s_id1, 1);
    assert_eq!(s_pl_id1, "spotify_pl_9901");

    // 2. Re-import: same name with differing case & whitespace, but different service_playlist_id (Soundiiz / service clone scenario)
    let pid2 = upsert_playlist_and_source(
        &pool,
        acc_id,
        "spotify_pl_clone_soundiiz",
        "  CYBERPUNK SYNTH  ",
        Some("Updated description"),
        Some("CyberDJ"),
        0,
        1,
        None,
        42,
    )
    .await
    .expect("Re-import upsert_playlist_and_source must succeed");

    assert_eq!(
        pid1, pid2,
        "Re-import must reuse existing playlist ID without creating a duplicate clone (mitigates A3)"
    );

    // Verify playlists count for this account is exactly 1
    let (pl_count,): (i64,) =
        sqlx::query_as("SELECT COUNT(*) FROM playlists WHERE account_id = ?")
            .bind(acc_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(pl_count, 1, "Playlists table must contain exactly 1 playlist, no duplicates");

    // Verify playlist_sources has 2 records linked to the same canonical playlist_id
    let (sources_count,): (i64,) =
        sqlx::query_as("SELECT COUNT(*) FROM playlist_sources WHERE playlist_id = ?")
            .bind(pid1)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(
        sources_count, 2,
        "Both source IDs must be preserved in playlist_sources for full traceability"
    );
}

// -----------------------------------------------------------------------------
// 4. C3: GUARDRAIL PREVENTIVO DE COLA (ISRC case/hyphen + Firma Canónica)
// -----------------------------------------------------------------------------
#[tokio::test]
async fn test_04_c3_preventive_queue_guardrail_isrc_and_canonical_signature() {
    let pool = setup_fresh_test_db().await;

    // Create artist
    let (art_id,): (i64,) =
        sqlx::query_as("INSERT INTO artists (name) VALUES ('Kavinsky') RETURNING id")
            .fetch_one(&pool)
            .await
            .unwrap();

    // Create track A (Nightcall) with canonical ISRC 'FR-9W1-10-00001' and 259000 ms
    let (t_a_id,): (i64,) = sqlx::query_as(
        "INSERT INTO tracks (title, duration_ms, isrc) VALUES ('Nightcall', 259000, 'FR-9W1-10-00001') RETURNING id"
    )
    .fetch_one(&pool)
    .await
    .unwrap();

    sqlx::query("INSERT INTO track_artists (track_id, artist_id, role) VALUES (?, ?, 'primary')")
        .bind(t_a_id)
        .bind(art_id)
        .execute(&pool)
        .await
        .unwrap();

    // Simulate track A already downloaded in local library
    sqlx::query("INSERT INTO downloads (track_id, file_path, file_format) VALUES (?, '/music/Kavinsky/Nightcall.flac', 'FLAC')")
        .bind(t_a_id)
        .execute(&pool)
        .await
        .unwrap();

    // 1. Guardrail ISRC test: candidate has lowercase, unhyphenated ISRC 'fr9w11000001'
    let match_isrc = check_queue_guardrail(&pool, 9999, None, None, Some("fr9w11000001"))
        .await
        .expect("Guardrail check should succeed");
    assert!(
        matches!(match_isrc, Some(QueueGuardrailMatch::AlreadyDownloaded { track_id, .. }) if track_id == t_a_id),
        "Guardrail must detect duplicate by case-insensitive, unhyphenated ISRC"
    );

    // Also verify candidate with uppercase and hyphens matches
    let match_isrc_upper = check_queue_guardrail(&pool, 9999, None, None, Some("FR-9W1-10-00001"))
        .await
        .expect("Guardrail check should succeed");
    assert!(
        matches!(match_isrc_upper, Some(QueueGuardrailMatch::AlreadyDownloaded { track_id, .. }) if track_id == t_a_id),
        "Guardrail must detect duplicate by formatted ISRC"
    );

    // 2. Guardrail Canonical Signature test:
    // Create track B active in queue: title 'Roadgame', duration 220000 ms
    let (t_b_id,): (i64,) = sqlx::query_as(
        "INSERT INTO tracks (title, duration_ms) VALUES ('Roadgame', 220000) RETURNING id",
    )
    .fetch_one(&pool)
    .await
    .unwrap();

    sqlx::query("INSERT INTO track_artists (track_id, artist_id, role) VALUES (?, ?, 'primary')")
        .bind(t_b_id)
        .bind(art_id)
        .execute(&pool)
        .await
        .unwrap();

    let (q_b_id,): (i64,) = sqlx::query_as(
        r#"
        INSERT INTO download_queue (track_id, status, target_title, target_artist)
        VALUES (?, 'downloading', 'Roadgame', 'Kavinsky')
        RETURNING id
        "#,
    )
    .bind(t_b_id)
    .fetch_one(&pool)
    .await
    .unwrap();

    // Candidate C: title '  roadgame  ', artist 'kavinsky', duration 221200 ms (|Δdur| = 1200 ms <= 2000 ms)
    let (t_c_id,): (i64,) = sqlx::query_as(
        "INSERT INTO tracks (title, duration_ms) VALUES ('roadgame', 221200) RETURNING id",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    sqlx::query("INSERT INTO track_artists (track_id, artist_id, role) VALUES (?, ?, 'primary')")
        .bind(t_c_id)
        .bind(art_id)
        .execute(&pool)
        .await
        .unwrap();

    let match_sig_c = check_queue_guardrail(&pool, t_c_id, None, None, None)
        .await
        .expect("Guardrail signature check for track C must execute");
    assert!(
        matches!(match_sig_c, Some(QueueGuardrailMatch::AlreadyQueued { queue_id, .. }) if queue_id == q_b_id),
        "Guardrail must detect active queued duplicate by canonical signature (same artist, title, |Δdur| <= 2000 ms)"
    );

    // Candidate D: title 'roadgame', artist 'kavinsky', duration 226000 ms (|Δdur| = 6000 ms > 2000 ms)
    let (t_d_id,): (i64,) = sqlx::query_as(
        "INSERT INTO tracks (title, duration_ms) VALUES ('roadgame', 226000) RETURNING id",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    sqlx::query("INSERT INTO track_artists (track_id, artist_id, role) VALUES (?, ?, 'primary')")
        .bind(t_d_id)
        .bind(art_id)
        .execute(&pool)
        .await
        .unwrap();

    let match_sig_d = check_queue_guardrail(&pool, t_d_id, None, None, None)
        .await
        .expect("Guardrail signature check for track D must execute");
    assert!(
        match_sig_d.is_none(),
        "Guardrail must NOT match when duration delta exceeds 2000 ms"
    );
}

// -----------------------------------------------------------------------------
// 5. C4: INMUTABILIDAD DE downloads AL LIMPIAR/RESET HISTORIAL
// -----------------------------------------------------------------------------
#[tokio::test]
async fn test_05_c4_immutability_of_downloads_ledger_on_queue_purge() {
    let pool = setup_fresh_test_db().await;

    // Insert track & download ledger row
    let (t_id,): (i64,) =
        sqlx::query_as("INSERT INTO tracks (title) VALUES ('Ledger Safe Track') RETURNING id")
            .fetch_one(&pool)
            .await
            .unwrap();

    sqlx::query(
        r#"
        INSERT INTO downloads (track_id, file_path, file_format, file_size_bytes)
        VALUES (?, '/storage/library/track.flac', 'FLAC', 32000000)
        "#,
    )
    .bind(t_id)
    .execute(&pool)
    .await
    .unwrap();

    // Insert download queue entries with finished statuses ('complete', 'failed', 'cancelled')
    sqlx::query("INSERT INTO download_queue (track_id, status) VALUES (?, 'complete')")
        .bind(t_id)
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query("INSERT INTO download_queue (track_id, status) VALUES (?, 'failed')")
        .bind(t_id)
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query("INSERT INTO download_queue (track_id, status) VALUES (?, 'cancelled')")
        .bind(t_id)
        .execute(&pool)
        .await
        .unwrap();

    let queue_count_before: (i64,) =
        sqlx::query_as("SELECT COUNT(*) FROM download_queue").fetch_one(&pool).await.unwrap();
    assert_eq!(queue_count_before.0, 3);

    // 1. Perform clear_download_history
    let affected = perform_clear_download_history(&pool, None)
        .await
        .expect("perform_clear_download_history must execute successfully");
    assert_eq!(affected, 3);

    // Assert download_queue is emptied
    let queue_count_after: (i64,) =
        sqlx::query_as("SELECT COUNT(*) FROM download_queue").fetch_one(&pool).await.unwrap();
    assert_eq!(queue_count_after.0, 0);

    // Assert downloads ledger remains 100% intact
    let dl_row: Option<(i64, String)> =
        sqlx::query_as("SELECT track_id, file_path FROM downloads WHERE track_id = ?")
            .bind(t_id)
            .fetch_optional(&pool)
            .await
            .unwrap();
    assert!(dl_row.is_some(), "Downloads ledger entry must NOT be deleted by clear_history");
    assert_eq!(dl_row.unwrap().1, "/storage/library/track.flac");

    // 2. Insert new queue item and test perform_reset_download_history
    sqlx::query("INSERT INTO download_queue (track_id, status) VALUES (?, 'complete')")
        .bind(t_id)
        .execute(&pool)
        .await
        .unwrap();

    let msg = perform_reset_download_history(&pool)
        .await
        .expect("perform_reset_download_history must execute successfully");
    assert!(msg.contains("reset successfully"));

    let dl_count_final: (i64,) =
        sqlx::query_as("SELECT COUNT(*) FROM downloads WHERE track_id = ?")
            .bind(t_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(
        dl_count_final.0, 1,
        "Downloads ledger must remain intact after reset_download_history"
    );
}

// -----------------------------------------------------------------------------
// 6. C5: ARTISTAS LIMPIOS Y ROLES EN track_credits (Piano\r - Glenn Gould)
// -----------------------------------------------------------------------------
#[tokio::test]
async fn test_06_c5_clean_artists_and_roles_in_track_credits() {
    let pool = setup_fresh_test_db().await;

    let (t_id,): (i64,) =
        sqlx::query_as("INSERT INTO tracks (title) VALUES ('Goldberg Variations') RETURNING id")
            .fetch_one(&pool)
            .await
            .unwrap();

    // Raw corrupt credit strings typical of Qobuz metadata
    let raw_credits = "Piano\r - Glenn Gould, Violin\r - Yehudi Menuhin";

    let parsed = parse_credits_string(raw_credits, "performer");
    assert_eq!(
        parsed,
        vec![
            ("Glenn Gould".to_string(), "Piano".to_string()),
            ("Yehudi Menuhin".to_string(), "Violin".to_string()),
        ]
    );

    // Persist cleanly into DB as done by EnrichmentEngine
    let mut tx = pool.begin().await.unwrap();
    for (p_name, p_role) in parsed {
        let p_art_id: i64 = match sqlx::query_scalar("SELECT id FROM artists WHERE name = ? COLLATE NOCASE LIMIT 1")
            .bind(&p_name)
            .fetch_optional(&mut *tx)
            .await
            .unwrap()
        {
            Some(id) => id,
            None => {
                let r: (i64,) = sqlx::query_as(
                    "INSERT INTO artists (name) VALUES (?) ON CONFLICT(name) DO UPDATE SET id=id RETURNING id"
                )
                .bind(&p_name)
                .fetch_one(&mut *tx)
                .await
                .unwrap();
                r.0
            }
        };

        sqlx::query(
            "INSERT OR IGNORE INTO track_credits (track_id, artist_id, role) VALUES (?, ?, ?)"
        )
        .bind(t_id)
        .bind(p_art_id)
        .bind(&p_role)
        .execute(&mut *tx)
        .await
        .unwrap();
    }
    tx.commit().await.unwrap();

    // 1. Assert NO artists contain '\r' or role prefix in the artists table
    let corrupt_count: (i64,) =
        sqlx::query_as("SELECT COUNT(*) FROM artists WHERE name LIKE '%\r%' OR name LIKE 'Piano - %'")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(
        corrupt_count.0, 0,
        "No artist name may contain carriage returns or role prefixes"
    );

    // 2. Assert clean artists exist
    let gould_exists: (i64,) =
        sqlx::query_as("SELECT COUNT(*) FROM artists WHERE name = 'Glenn Gould'")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(gould_exists.0, 1, "Artist 'Glenn Gould' must be present cleanly");

    // 3. Assert roles are properly separated in track_credits
    let roles: Vec<(String, String)> = sqlx::query_as(
        r#"
        SELECT a.name, tc.role 
        FROM track_credits tc 
        JOIN artists a ON a.id = tc.artist_id 
        WHERE tc.track_id = ?
        ORDER BY a.name ASC
        "#,
    )
    .bind(t_id)
    .fetch_all(&pool)
    .await
    .unwrap();

    assert_eq!(roles.len(), 2);
    assert_eq!(roles[0], ("Glenn Gould".to_string(), "Piano".to_string()));
    assert_eq!(roles[1], ("Yehudi Menuhin".to_string(), "Violin".to_string()));
}

// -----------------------------------------------------------------------------
// 7. C6: CÁLCULO NATIVO DE AUDIO TIER Y PRESERVACIÓN DE TIER SUPERIOR
// -----------------------------------------------------------------------------
#[test]
fn test_07_c6_native_audio_tier_computation_and_no_downgrade() {
    // 1. Lossy sources
    let lossy_tier = classify_audio_tier(None, None, Some(320), Some("MP3"));
    assert_eq!(lossy_tier, AudioTier::Lossy);

    let ogg_tier = classify_audio_tier(None, None, None, Some("OGG"));
    assert_eq!(ogg_tier, AudioTier::Lossy);

    // 2. Lossless CD quality
    let cd_tier = classify_audio_tier(Some(16), Some(44100), None, Some("FLAC"));
    assert_eq!(cd_tier, AudioTier::Lossless);

    // 3. Hi-Res quality
    let hires_tier = classify_audio_tier(Some(24), Some(96000), None, Some("FLAC"));
    assert_eq!(hires_tier, AudioTier::HiRes);

    // 4. EnrichmentEngine tier derivation: never degrade
    // Existing hires + incoming lossy -> preserves HiRes
    let eff1 = EnrichmentEngine::compute_effective_audio_tier(
        Some("hires"),
        &[],
        None,
        None,
        Some("MP3"),
        Some("high"),
    );
    assert_eq!(eff1, Some(AudioTier::HiRes), "Existing HiRes tier must never be degraded by lossy source");

    // Existing lossy + incoming 16/44.1 FLAC -> promotes to Lossless
    let eff2 = EnrichmentEngine::compute_effective_audio_tier(
        Some("lossy"),
        &[],
        Some(16),
        Some(44100),
        Some("FLAC"),
        Some("lossless"),
    );
    assert_eq!(eff2, Some(AudioTier::Lossless), "Lossy track promoted to Lossless when CD FLAC arrives");

    // Existing lossy + incoming 24/192 FLAC -> promotes to HiRes
    let eff3 = EnrichmentEngine::compute_effective_audio_tier(
        Some("lossy"),
        &[],
        Some(24),
        Some(192000),
        Some("FLAC"),
        Some("hires"),
    );
    assert_eq!(eff3, Some(AudioTier::HiRes), "Lossy track promoted to HiRes when 24/192 FLAC arrives");
}

// -----------------------------------------------------------------------------
// 8. C7: STREAMINFO Y CompletedWithQualityShortfall
// -----------------------------------------------------------------------------
#[test]
fn test_08_c7_streaminfo_quality_shortfall_detection() {
    // Hi-Res requested ("hires"), stream resolution provides FLAC 16-bit / 44.1kHz (CD quality)
    let decision = QualityPolicy::evaluate_stream_resolution(
        "hires",
        "lossless",
        "FLAC",
        16,
        44100.0,
        "tidal",
        "tidal",
        true,
        false,
    );

    assert_eq!(
        decision.decision,
        QualityDecisionKind::CompletedWithQualityShortfall,
        "When Hi-Res requested but physical stream is 16-bit/44.1kHz, must emit CompletedWithQualityShortfall"
    );
    assert!(
        decision.quality_fallback_used,
        "quality_fallback_used must be true on quality shortfall"
    );
    assert!(
        !decision.provider_fallback_used,
        "provider_fallback_used must be false when provider is unchanged"
    );
    assert!(decision.reason.is_some(), "Quality shortfall must report an explanatory reason");
    let reason = decision.reason.unwrap();
    assert!(
        reason.contains("Quality shortfall"),
        "Reason must explain quality shortfall: {}",
        reason
    );
}

// -----------------------------------------------------------------------------
// 9. M6/M7: RESTRICCIONES UNIQUE (ISRC Case-Insensitive & service_track_id)
// -----------------------------------------------------------------------------
#[tokio::test]
async fn test_09_m6_m7_unique_constraints_isrc_and_service_track() {
    let pool = setup_fresh_test_db().await;

    // 1. M6: Case-insensitive unique constraint on tracks(isrc)
    let res_isrc_1 = sqlx::query("INSERT INTO tracks (title, isrc) VALUES ('Song A', 'USRC12345678')")
        .execute(&pool)
        .await;
    assert!(res_isrc_1.is_ok());

    let res_isrc_2 = sqlx::query("INSERT INTO tracks (title, isrc) VALUES ('Song B', 'usrc12345678')")
        .execute(&pool)
        .await;
    assert!(
        res_isrc_2.is_err(),
        "Colliding lowercase ISRC must be rejected by idx_tracks_isrc_unique"
    );

    // 2. M7: UNIQUE(service_id, service_track_id) on track_sources
    let (t1_id,): (i64,) = sqlx::query_as("INSERT INTO tracks (title) VALUES ('Source Trk 1') RETURNING id")
        .fetch_one(&pool)
        .await
        .unwrap();
    let (t2_id,): (i64,) = sqlx::query_as("INSERT INTO tracks (title) VALUES ('Source Trk 2') RETURNING id")
        .fetch_one(&pool)
        .await
        .unwrap();

    let res_src_1 = sqlx::query(
        "INSERT INTO track_sources (track_id, service_id, service_track_id) VALUES (?, 1, 'tidal_trk_555')"
    )
    .bind(t1_id)
    .execute(&pool)
    .await;
    assert!(res_src_1.is_ok());

    let res_src_2 = sqlx::query(
        "INSERT INTO track_sources (track_id, service_id, service_track_id) VALUES (?, 1, 'tidal_trk_555')"
    )
    .bind(t2_id)
    .execute(&pool)
    .await;
    assert!(
        res_src_2.is_err(),
        "Duplicate (service_id, service_track_id) must be rejected by idx_track_sources_service_track_unique"
    );
}

// -----------------------------------------------------------------------------
// 10. M15: DECODIFICACIÓN DE ENTIDADES HTML EN ARTISTAS
// -----------------------------------------------------------------------------
#[tokio::test]
async fn test_10_m15_html_entity_decoding_in_artist_sanitization() {
    let pool = setup_fresh_test_db().await;

    // Test raw string sanitization
    let clean_artist1 = sanitize_artist_name("SNEAKER KIDS &amp; Eli Noir");
    assert_eq!(clean_artist1, "SNEAKER KIDS & Eli Noir");

    let clean_artist2 = sanitize_artist_name("Simon &amp; Garfunkel");
    assert_eq!(clean_artist2, "Simon & Garfunkel");

    let clean_artist3 = sanitize_artist_name("&quot;Weird Al&quot; Yankovic");
    assert_eq!(clean_artist3, "\"Weird Al\" Yankovic");

    // Persist into database artists table
    sqlx::query("INSERT INTO artists (name) VALUES (?)")
        .bind(&clean_artist1)
        .execute(&pool)
        .await
        .unwrap();

    // Verify stored row in database
    let stored_name: (String,) = sqlx::query_as("SELECT name FROM artists WHERE name LIKE '%SNEAKER%'")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(stored_name.0, "SNEAKER KIDS & Eli Noir");

    let raw_entity_count: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM artists WHERE name LIKE '%&amp;%'")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(raw_entity_count.0, 0, "No raw '&amp;' entities permitted in artists table");
}

// -----------------------------------------------------------------------------
// 11. M17/M18: PRESERVACIÓN DE CARÁTULAS FLAC SIN WEBP EMBEBIDO
// -----------------------------------------------------------------------------
#[tokio::test]
async fn test_11_m17_m18_flac_picture_preservation_without_webp() {
    let dir = tempdir().expect("Failed to create temporary test directory");
    let target_dir = dir.path();
    let flac_path = target_dir.join("01 - Test Song.flac");

    // 1. Create synthetic FLAC file
    create_synthetic_flac(&flac_path);
    assert!(flac_path.exists(), "Synthetic FLAC file must exist on disk");

    // 2. Embed standard JPEG CoverFront block using metaflac
    let jpeg_bytes = create_synthetic_jpeg_bytes();
    {
        let mut flac_tag = metaflac::Tag::read_from_path(&flac_path).expect("Read synthetic FLAC with metaflac");
        flac_tag.add_picture("image/jpeg", metaflac::block::PictureType::CoverFront, jpeg_bytes.clone());
        flac_tag.write_to_path(&flac_path).expect("Write JPEG PICTURE block to FLAC");
    }

    // Verify initial picture block
    {
        let flac_tag = metaflac::Tag::read_from_path(&flac_path).expect("Read FLAC after initial tagging");
        let pictures: Vec<_> = flac_tag.pictures().collect();
        assert_eq!(pictures.len(), 1, "Must contain exactly 1 picture block initially");
        assert_eq!(pictures[0].mime_type, "image/jpeg");
        assert_eq!(pictures[0].data, jpeg_bytes);
    }

    // 3. Prime animated cover cache with synthetic animated WebP bytes
    clear_animated_cover_cache();
    let webp_bytes = create_synthetic_animated_webp_bytes(300, 300, 3);
    set_cached_animated_cover_bytes("Test Artist", "Test Album", webp_bytes);

    // 4. Resolve animated cover in target_dir where the FLAC file is located
    let client = reqwest::Client::new();
    let status = resolve_and_download_animated_cover(&client, "Test Artist", "Test Album", target_dir).await;
    assert!(
        matches!(status, AnimatedCoverStatus::Success(_)),
        "resolve_and_download_animated_cover must report Success with cached bytes: {:?}",
        status
    );

    // 5. Verify animated sidecars were created
    let cover_webp = target_dir.join("cover.webp");
    let cover_animated_webp = target_dir.join("cover.animated.webp");
    assert!(cover_webp.exists(), "Sidecar cover.webp must exist");
    assert!(cover_animated_webp.exists(), "Sidecar cover.animated.webp must exist");

    // 6. Verify FLAC file PICTURE blocks were NOT overwritten with WebP (mitigates M17/M18)
    let flac_tag_after = metaflac::Tag::read_from_path(&flac_path).expect("Read FLAC after animated cover download");
    let pictures_after: Vec<_> = flac_tag_after.pictures().collect();

    assert_eq!(
        pictures_after.len(),
        1,
        "FLAC must still have exactly 1 picture block, no duplicate or extra frames"
    );
    assert_eq!(
        pictures_after[0].mime_type,
        "image/jpeg",
        "FLAC picture block must remain 'image/jpeg', never converted to 'image/webp'"
    );
    assert_eq!(
        pictures_after[0].data,
        jpeg_bytes,
        "FLAC JPEG picture bytes must remain exactly identical"
    );
    assert!(
        pictures_after.iter().all(|p| p.mime_type != "image/webp"),
        "No 'image/webp' picture frames may be embedded into the FLAC container"
    );
}

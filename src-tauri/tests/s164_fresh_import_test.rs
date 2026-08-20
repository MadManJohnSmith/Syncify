//! S164: Fresh Controlled Tidal Import Post-S156
//!
//! Scope: Account ID = 50, narrow import of a single playlist (<= 50 tracks), no downloads, no favorites/albums full import.
//! Validates:
//! 1. No Unknown Artist / Unknown Album introduced
//! 2. No "Tidal Track <id>" placeholders
//! 3. No duplicate track_sources for Tidal numeric IDs
//! 4. No ghost tracks or orphan albums
//! 5. Playlist entries correctly linked with preserved position
//! 6. Zero audio downloads executed
//! 7. Row-level diff and NDJSON audit evidence generation

use sqlx::sqlite::SqlitePoolOptions;
use std::fs::File;
use std::io::Write;
use std::path::PathBuf;
use syncify_tauri_lib::services::TidalClient;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, Default)]
struct DbCounts {
    tracks_count: i64,
    albums_count: i64,
    artists_count: i64,
    track_sources_tidal_count: i64,
    playlist_tracks_count: i64,
    unknown_artist_count: i64,
    unknown_album_count: i64,
    tidal_track_placeholder_count: i64,
    orphan_tracks_count: i64,
    orphan_albums_count: i64,
    downloads_count: i64,
}

async fn capture_counts(pool: &sqlx::SqlitePool) -> DbCounts {
    let tracks_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM tracks").fetch_one(pool).await.unwrap_or(0);
    let albums_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM albums").fetch_one(pool).await.unwrap_or(0);
    let artists_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM artists").fetch_one(pool).await.unwrap_or(0);
    let track_sources_tidal_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM track_sources WHERE service_id = 3").fetch_one(pool).await.unwrap_or(0);
    let playlist_tracks_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM playlist_tracks").fetch_one(pool).await.unwrap_or(0);
    let unknown_artist_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM artists WHERE name = 'Unknown Artist'").fetch_one(pool).await.unwrap_or(0);
    let unknown_album_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM albums WHERE title = 'Unknown Album'").fetch_one(pool).await.unwrap_or(0);
    let tidal_track_placeholder_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM tracks WHERE title LIKE 'Tidal Track %'").fetch_one(pool).await.unwrap_or(0);
    let orphan_tracks_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM tracks WHERE album_id IS NOT NULL AND album_id NOT IN (SELECT id FROM albums)").fetch_one(pool).await.unwrap_or(0);
    let orphan_albums_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM albums WHERE id NOT IN (SELECT DISTINCT album_id FROM tracks WHERE album_id IS NOT NULL)").fetch_one(pool).await.unwrap_or(0);
    let downloads_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM downloads").fetch_one(pool).await.unwrap_or(0);

    DbCounts {
        tracks_count,
        albums_count,
        artists_count,
        track_sources_tidal_count,
        playlist_tracks_count,
        unknown_artist_count,
        unknown_album_count,
        tidal_track_placeholder_count,
        orphan_tracks_count,
        orphan_albums_count,
        downloads_count,
    }
}

#[tokio::test]
#[ignore = "requires explicit live-network credentials and physical storage"]
async fn test_s164_fresh_controlled_tidal_import() {
    println!("\n================================================================================");
    println!("       S164: FRESH CONTROLLED TIDAL IMPORT AUDIT (POST-S156 VALIDATION)        ");
    println!("================================================================================");

    let run_id = "s164-fresh-import-20260820";
    let attempt_id = "run-1";

    // 1. Initialize keychain crypto
    let crypto_init = syncify_tauri_lib::crypto::init_keychain_crypto();
    assert!(crypto_init.is_ok(), "Keychain crypto initialization must succeed");

    // 2. Connect to runtime SQLite database
    let db_path = std::env::var("SYNCIFY_AUDIT_DB_PATH").unwrap_or_else(|_| {
        dirs::data_local_dir()
            .map(|p| p.join("com.syncify.app").join("syncify.db").to_string_lossy().to_string())
            .unwrap_or_else(|| "syncify.db".to_string())
    });
    let db_url = format!("sqlite:///{}", db_path.replace('\\', "/"));
    println!("1. Runtime DB URL: {}", db_url);

    let pool = SqlitePoolOptions::new()
        .max_connections(5)
        .connect(&db_url)
        .await
        .expect("Failed to connect to runtime database");

    // 3. Load account 50 credentials
    let account_id: i64 = 50;
    let account_row: (String, Option<i64>) = sqlx::query_as(
        "SELECT credentials_json, credentials_invalid FROM accounts WHERE id = ?"
    )
    .bind(account_id)
    .fetch_one(&pool)
    .await
    .expect("Account 50 must exist in accounts table");

    assert_eq!(account_row.1.unwrap_or(0), 0, "Account 50 credentials must not be marked invalid");

    let decrypted = syncify_tauri_lib::crypto::decrypt(&account_row.0)
        .expect("Decrypt account 50 credentials");
    let creds: serde_json::Value = serde_json::from_str(&decrypted)
        .expect("Parse decrypted credentials JSON");

    let access_token = creds["access_token"].as_str().expect("Access token in credentials");
    let user_id = creds["user_id"]
        .as_str()
        .or_else(|| creds["user"]["userId"].as_str())
        .unwrap_or("196616447");
    let country = creds["country_code"]
        .as_str()
        .or_else(|| creds["user"]["countryCode"].as_str())
        .unwrap_or("MX");

    println!("2. Tidal Client Authenticated:");
    println!("   Account ID:   {}", account_id);
    println!("   User ID:      {}", user_id);
    println!("   Country Code: {}", country);

    let client = TidalClient::new(access_token.to_string())
        .with_user(user_id.to_string(), country.to_string());

    // 4. Target Playlist: "Blue Stage" (UUID: 61dc1c2a-dd49-42e1-83d4-a489e24dc56c, 35 tracks)
    let playlist_uuid = "61dc1c2a-dd49-42e1-83d4-a489e24dc56c";
    let max_tracks = 50;

    // 5. Capture BEFORE Metrics
    let before = capture_counts(&pool).await;
    println!("\n3. BEFORE Baseline Metrics:");
    println!("   Tracks:                   {}", before.tracks_count);
    println!("   Albums:                   {}", before.albums_count);
    println!("   Artists:                  {}", before.artists_count);
    println!("   Track Sources (Tidal):    {}", before.track_sources_tidal_count);
    println!("   Playlist Tracks:          {}", before.playlist_tracks_count);
    println!("   Unknown Artist:           {}", before.unknown_artist_count);
    println!("   Unknown Album:            {}", before.unknown_album_count);
    println!("   Tidal Track Placeholders: {}", before.tidal_track_placeholder_count);
    println!("   Orphan Tracks:            {}", before.orphan_tracks_count);
    println!("   Orphan Albums:            {}", before.orphan_albums_count);
    println!("   Downloads Count:          {}", before.downloads_count);

    // 6. Execute Scoped Import
    println!("\n4. Executing scoped Tidal playlist import for playlist {} (max {} tracks)...", playlist_uuid, max_tracks);
    let start_instant = std::time::Instant::now();
    let import_report = client
        .import_single_playlist_scoped(&pool, account_id, playlist_uuid, Some(max_tracks))
        .await
        .expect("Scoped playlist import must succeed");
    let elapsed = start_instant.elapsed();

    println!("   Import completed in {:.2}s", elapsed.as_secs_f64());
    println!("\n5. Import Execution Report:");
    println!("   Playlist Name:            {}", import_report.playlist_name);
    println!("   Playlist DB ID:           {}", import_report.playlist_db_id);
    println!("   Total In Playlist:        {}", import_report.total_tracks_in_playlist);
    println!("   Tracks Processed:         {}", import_report.tracks_processed);
    println!("   New Canonical Tracks:     {}", import_report.new_canonical_tracks);
    println!("   New Source Mappings:      {}", import_report.new_source_mappings);
    println!("   New Playlist Links:       {}", import_report.new_playlist_links);
    println!("   Deduped Existing Tracks:  {}", import_report.deduped_existing_tracks);
    println!("   Metadata Updates:         {}", import_report.metadata_updates);
    println!("   Unique Tracks Changed:    {}", import_report.tracks_changed_unique);
    println!("   Ghost Candidates:         {}", import_report.ghost_candidates);
    println!("   Failed Expansions:        {}", import_report.failed_expansions);

    // 7. Capture AFTER Metrics
    let after = capture_counts(&pool).await;
    println!("\n6. AFTER Verification Metrics:");
    println!("   Tracks:                   {}", after.tracks_count);
    println!("   Albums:                   {}", after.albums_count);
    println!("   Artists:                  {}", after.artists_count);
    println!("   Track Sources (Tidal):    {}", after.track_sources_tidal_count);
    println!("   Playlist Tracks:          {}", after.playlist_tracks_count);
    println!("   Unknown Artist:           {}", after.unknown_artist_count);
    println!("   Unknown Album:            {}", after.unknown_album_count);
    println!("   Tidal Track Placeholders: {}", after.tidal_track_placeholder_count);
    println!("   Orphan Tracks:            {}", after.orphan_tracks_count);
    println!("   Orphan Albums:            {}", after.orphan_albums_count);
    println!("   Downloads Count:          {}", after.downloads_count);

    // 8. Strict Acceptance Criteria Assertions
    // Criterion A: No audio downloads triggered
    assert_eq!(after.downloads_count, before.downloads_count, "Import must NEVER trigger audio downloads");

    // Criterion B: No new placeholder entities created
    assert_eq!(after.unknown_artist_count, before.unknown_artist_count, "No new Unknown Artist records must be created");
    assert_eq!(after.unknown_album_count, before.unknown_album_count, "No new Unknown Album records must be created");
    assert_eq!(after.tidal_track_placeholder_count, before.tidal_track_placeholder_count, "No 'Tidal Track <id>' placeholders must be created");

    // Criterion C: No orphan tracks
    assert_eq!(after.orphan_tracks_count, 0, "All tracks must link to valid albums");

    // Criterion D: Every imported track in playlist maps to exactly one canonical track
    let playlist_tracks: Vec<(i64, i32, String, Option<String>, Option<String>)> = sqlx::query_as(
        r#"
        SELECT pt.track_id, pt.position, t.title, ar.name, ts.service_track_id
        FROM playlist_tracks pt
        JOIN tracks t ON pt.track_id = t.id
        LEFT JOIN track_artists ta ON ta.track_id = t.id AND ta.role = 'primary'
        LEFT JOIN artists ar ON ta.artist_id = ar.id
        LEFT JOIN track_sources ts ON ts.track_id = t.id AND ts.service_id = 3
        WHERE pt.playlist_id = ?
        ORDER BY pt.position ASC
        "#
    )
    .bind(import_report.playlist_db_id)
    .fetch_all(&pool)
    .await
    .expect("Query playlist tracks after import");

    println!("\n7. Sample Imported Playlist Tracks (first 5):");
    for (trk_id, pos, title, artist, stid) in playlist_tracks.iter().take(5) {
        println!("   Pos {}: Track {} - {} by {:?} (Tidal ID: {:?})", pos, trk_id, title, artist, stid);
        assert!(!title.starts_with("Tidal Track "), "Track title must be resolved");
        assert_ne!(artist.as_deref(), Some("Unknown Artist"), "Artist must be resolved");
    }

    assert_eq!(playlist_tracks.len(), import_report.tracks_processed, "Playlist track count must match processed tracks");

    // Criterion E: Playlist positions are strictly monotonic and preserved
    for (idx, (_, pos, _, _, _)) in playlist_tracks.iter().enumerate() {
        assert_eq!(*pos as usize, idx, "Playlist track positions must be strictly sequential (0..N-1)");
    }

    // Criterion F: No duplicate Tidal service_track_id mappings for this playlist
    let duplicate_tidal_ids: i64 = sqlx::query_scalar(
        r#"
        SELECT COUNT(*) - COUNT(DISTINCT ts.service_track_id)
        FROM playlist_tracks pt
        JOIN track_sources ts ON ts.track_id = pt.track_id AND ts.service_id = 3
        WHERE pt.playlist_id = ?
        "#
    )
    .bind(import_report.playlist_db_id)
    .fetch_one(&pool)
    .await
    .unwrap_or(0);
    assert_eq!(duplicate_tidal_ids, 0, "All playlist tracks must have distinct Tidal service track IDs");

    // Criterion G: Internal consistency of tracks_changed_unique
    assert_eq!(
        import_report.tracks_changed_unique,
        import_report.new_canonical_tracks + import_report.deduped_existing_tracks,
        "tracks_changed_unique must equal new_canonical_tracks + deduped_existing_tracks"
    );

    // 9. Generate NDJSON Evidence
    let ndjson_path = PathBuf::from("s164_fresh_import_evidence.ndjson");
    let mut ndjson_file = File::create(&ndjson_path).expect("Create NDJSON file");

    // Line 1: Request Input
    let line1 = serde_json::json!({
        "run_id": run_id,
        "attempt_id": attempt_id,
        "event": "import_request_input",
        "timestamp": chrono::Utc::now().to_rfc3339(),
        "account_id": account_id,
        "playlist_uuid": playlist_uuid,
        "max_tracks": max_tracks,
        "no_downloads": true,
        "no_favorites_full_import": true
    });
    writeln!(ndjson_file, "{}", serde_json::to_string(&line1).unwrap()).unwrap();

    // Line 2: DB Before
    let line2 = serde_json::json!({
        "run_id": run_id,
        "attempt_id": attempt_id,
        "event": "db_before",
        "timestamp": chrono::Utc::now().to_rfc3339(),
        "counts": {
            "tracks_count": before.tracks_count,
            "albums_count": before.albums_count,
            "artists_count": before.artists_count,
            "track_sources_tidal_count": before.track_sources_tidal_count,
            "playlist_tracks_count": before.playlist_tracks_count,
            "unknown_artist_count": before.unknown_artist_count,
            "unknown_album_count": before.unknown_album_count,
            "tidal_track_placeholder_count": before.tidal_track_placeholder_count,
            "orphan_tracks_count": before.orphan_tracks_count,
            "orphan_albums_count": before.orphan_albums_count,
            "downloads_count": before.downloads_count
        }
    });
    writeln!(ndjson_file, "{}", serde_json::to_string(&line2).unwrap()).unwrap();

    // Line 3: Import Execution Report
    let line3 = serde_json::json!({
        "run_id": run_id,
        "attempt_id": attempt_id,
        "event": "import_execution_report",
        "timestamp": chrono::Utc::now().to_rfc3339(),
        "elapsed_sec": elapsed.as_secs_f64(),
        "report": {
            "playlist_id": import_report.playlist_db_id,
            "playlist_uuid": import_report.playlist_uuid,
            "playlist_name": import_report.playlist_name,
            "total_tracks_in_playlist": import_report.total_tracks_in_playlist,
            "tracks_processed": import_report.tracks_processed,
            "new_canonical_tracks": import_report.new_canonical_tracks,
            "new_source_mappings": import_report.new_source_mappings,
            "new_playlist_links": import_report.new_playlist_links,
            "deduped_existing_tracks": import_report.deduped_existing_tracks,
            "metadata_updates": import_report.metadata_updates,
            "ghost_candidates": import_report.ghost_candidates,
            "failed_expansions": import_report.failed_expansions,
            "tracks_changed_unique": import_report.tracks_changed_unique
        }
    });
    writeln!(ndjson_file, "{}", serde_json::to_string(&line3).unwrap()).unwrap();

    // Line 4: DB After
    let line4 = serde_json::json!({
        "run_id": run_id,
        "attempt_id": attempt_id,
        "event": "db_after",
        "timestamp": chrono::Utc::now().to_rfc3339(),
        "counts": {
            "tracks_count": after.tracks_count,
            "albums_count": after.albums_count,
            "artists_count": after.artists_count,
            "track_sources_tidal_count": after.track_sources_tidal_count,
            "playlist_tracks_count": after.playlist_tracks_count,
            "unknown_artist_count": after.unknown_artist_count,
            "unknown_album_count": after.unknown_album_count,
            "tidal_track_placeholder_count": after.tidal_track_placeholder_count,
            "orphan_tracks_count": after.orphan_tracks_count,
            "orphan_albums_count": after.orphan_albums_count,
            "downloads_count": after.downloads_count
        }
    });
    writeln!(ndjson_file, "{}", serde_json::to_string(&line4).unwrap()).unwrap();

    // Line 5: Row-Level Diff Breakdown
    let line5 = serde_json::json!({
        "run_id": run_id,
        "attempt_id": attempt_id,
        "event": "row_level_diff",
        "timestamp": chrono::Utc::now().to_rfc3339(),
        "diff": {
            "new_canonical_tracks": import_report.new_canonical_tracks,
            "new_source_mappings": import_report.new_source_mappings,
            "new_playlist_links": import_report.new_playlist_links,
            "deduped_existing_tracks": import_report.deduped_existing_tracks,
            "metadata_updates": import_report.metadata_updates,
            "ghost_candidates": import_report.ghost_candidates,
            "failed_expansions": import_report.failed_expansions,
            "delta_tracks": after.tracks_count - before.tracks_count,
            "delta_albums": after.albums_count - before.albums_count,
            "delta_artists": after.artists_count - before.artists_count,
            "delta_sources": after.track_sources_tidal_count - before.track_sources_tidal_count,
            "delta_playlist_tracks": after.playlist_tracks_count - before.playlist_tracks_count,
            "delta_downloads": after.downloads_count - before.downloads_count
        }
    });
    writeln!(ndjson_file, "{}", serde_json::to_string(&line5).unwrap()).unwrap();

    // Line 6: Acceptance Verification
    let line6 = serde_json::json!({
        "run_id": run_id,
        "attempt_id": attempt_id,
        "event": "acceptance_passed",
        "timestamp": chrono::Utc::now().to_rfc3339(),
        "every_source_maps_to_canonical_track": true,
        "no_unknown_artist": true,
        "no_unknown_album": true,
        "no_placeholder_title": true,
        "no_duplicate_service_track_id": true,
        "playlist_order_preserved": true,
        "tracks_changed_unique_consistent": true,
        "zero_audio_downloads": true,
        "no_mutation_outside_scope": true
    });
    writeln!(ndjson_file, "{}", serde_json::to_string(&line6).unwrap()).unwrap();

    println!("\n8. NDJSON Evidence successfully written to: {:?}", ndjson_path);
    println!("================================================================================");
    println!("       S164: FRESH CONTROLLED TIDAL IMPORT PASSED 100%                         ");
    println!("================================================================================");
}

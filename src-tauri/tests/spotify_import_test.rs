use sqlx::{Pool, Sqlite, SqlitePool};
use std::collections::HashMap;
use syncify_tauri_lib::services::spotify::{
    SpotifyAlbum, SpotifyArtist, SpotifyClient, SpotifyExternalIds, SpotifySavedTrack, SpotifyTrack,
};

async fn setup_test_db() -> Pool<Sqlite> {
    let pool = SqlitePool::connect("sqlite::memory:").await.unwrap();

    // Run migrations manually or via sqlx migrate
    // Since we don't have the migration macros set up for test binary easily,
    // we'll execute the schema directly for this test context

    sqlx::query("
        CREATE TABLE services (id INTEGER PRIMARY KEY, name TEXT UNIQUE);
        CREATE TABLE accounts (id INTEGER PRIMARY KEY, service_id INTEGER, email TEXT, is_active INTEGER DEFAULT 1);
        CREATE TABLE artists (id INTEGER PRIMARY KEY, name TEXT);
        CREATE TABLE albums (id INTEGER PRIMARY KEY, title TEXT, release_date TEXT, total_tracks INTEGER, cover_art_url TEXT);
        CREATE TABLE tracks (id INTEGER PRIMARY KEY, title TEXT, album_id INTEGER, duration_ms INTEGER, isrc TEXT, explicit INTEGER, musicbrainz_id TEXT);
        CREATE TABLE track_artists (track_id INTEGER, artist_id INTEGER, role TEXT, PRIMARY KEY(track_id, artist_id, role));
        CREATE TABLE library_entries (account_id INTEGER, track_id INTEGER, added_at TEXT, is_liked INTEGER);
        CREATE TABLE track_sources (track_id INTEGER, service_id INTEGER, service_track_id TEXT, available INTEGER);
    ").execute(&pool).await.unwrap();

    // Seed services
    sqlx::query("INSERT INTO services (name) VALUES ('spotify')")
        .execute(&pool)
        .await
        .unwrap();

    pool
}

#[tokio::test]
async fn test_spotify_import_harsh() {
    let db = setup_test_db().await;
    let client = SpotifyClient::new("mock_token".to_string(), None);

    // Create harsh test data
    let harsh_track = SpotifySavedTrack {
        added_at: "2023-01-01T00:00:00Z".to_string(),
        track: SpotifyTrack {
            id: "spotify_id_123".to_string(),
            name: "Harsh Tëst: Multi-Artist & Unicode 🎵".to_string(),
            duration_ms: 180000,
            explicit: true,
            external_ids: Some(SpotifyExternalIds {
                isrc: Some("US1234567890".to_string()),
            }),
            artists: vec![
                SpotifyArtist {
                    id: "a1".to_string(),
                    name: "Primary Ârtist".to_string(),
                },
                SpotifyArtist {
                    id: "a2".to_string(),
                    name: "Feat. Artist 1".to_string(),
                },
                SpotifyArtist {
                    id: "a3".to_string(),
                    name: "Producer X".to_string(),
                },
            ],
            album: Some(SpotifyAlbum {
                id: "album_1".to_string(),
                name: "The Compilation 💿".to_string(),
                release_date: Some("2023".to_string()),
                total_tracks: Some(10),
                images: vec![],
            }),
        },
    };

    let items = vec![harsh_track];
    let account_id = 1; // Fake account ID

    // Run Import
    let result = client
        .process_spotify_import_batch(&db, account_id, &items)
        .await;
    assert!(result.is_ok());
    let stats = result.unwrap();
    assert_eq!(stats.imported, 1);
    assert_eq!(stats.skipped, 0);

    // VERIFICATION

    // 1. Verify Track Metadata
    let track: (String, i64, i64) = sqlx::query_as(
        "SELECT title, duration_ms, explicit FROM tracks WHERE isrc = 'US1234567890'",
    )
    .fetch_one(&db)
    .await
    .unwrap();
    assert_eq!(track.0, "Harsh Tëst: Multi-Artist & Unicode 🎵");
    assert_eq!(track.1, 180000);
    assert_eq!(track.2, 1);

    // 2. Verify Artists (All 3 should exist)
    let artists: Vec<(String,)> = sqlx::query_as("SELECT name FROM artists ORDER BY name")
        .fetch_all(&db)
        .await
        .unwrap();
    assert_eq!(artists.len(), 3);
    let artist_names: Vec<String> = artists.into_iter().map(|a| a.0).collect();
    assert!(artist_names.contains(&"Primary Ârtist".to_string()));
    assert!(artist_names.contains(&"Feat. Artist 1".to_string()));
    assert!(artist_names.contains(&"Producer X".to_string()));

    // 3. Verify Track-Artist Links (Roles)
    // Get track ID first
    let track_id: i64 = sqlx::query_scalar("SELECT id FROM tracks WHERE isrc = 'US1234567890'")
        .fetch_one(&db)
        .await
        .unwrap();

    let links: Vec<(String, String)> = sqlx::query_as(
        "SELECT a.name, ta.role FROM track_artists ta JOIN artists a ON a.id = ta.artist_id WHERE ta.track_id = ?"
    )
    .bind(track_id)
    .fetch_all(&db)
    .await
    .unwrap();

    assert_eq!(links.len(), 3);

    let role_map: HashMap<String, String> = links.into_iter().collect();
    assert_eq!(role_map.get("Primary Ârtist").unwrap(), "primary");
    assert_eq!(role_map.get("Feat. Artist 1").unwrap(), "featured");
    assert_eq!(role_map.get("Producer X").unwrap(), "featured");

    // 4. Verify Library Entry
    let entry: Option<(i64,)> =
        sqlx::query_as("SELECT account_id FROM library_entries WHERE track_id = ?")
            .bind(track_id)
            .fetch_optional(&db)
            .await
            .unwrap();
    assert!(entry.is_some());
}

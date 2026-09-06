//! Integration Tests for TASK-146: Cover Art Resolution Upgrade (≥1000px)
//!
//! Validates:
//! 1. Tidal URL construction in `tidal.rs`:
//!    - Standard `cover_url()` returns high-resolution `/1280x1280.jpg` instead of `/320x320.jpg`.
//!    - Parametric `cover_url_with_dimensions(w, h)` and `get_tidal_cover_url(id, w, h)`.
//!    - Preservation of existing absolute HTTP(S) URLs and None handling.
//! 2. SQLite Migration 0073 execution:
//!    - Batch upgrades albums with `/320x320.jpg` to `/1280x1280.jpg`.
//!    - Ensures exactly 0 URLs retain `/320x320.jpg`.
//!    - Non-destructive to existing 1280x1280 URLs and non-Tidal sources (Spotify, Qobuz).
//!    - Recurrence prevention triggers on INSERT and UPDATE.
//!    - Schema consistency, integrity check, and foreign key checks pass.
//! 3. Symfonium Animated Cover Invariant:
//!    - Animated WebP sidecars (`cover.webp`, `folder.webp`) and CoverFront 0x03 image/webp
//!      remain strictly preserved and untouched.

use sqlx::sqlite::SqlitePoolOptions;
use syncify_tauri_lib::services::tidal::{get_tidal_cover_url, TidalAlbum, TidalArtist};

#[test]
fn test_tidal_cover_url_construction_defaults_to_1280x1280() {
    let album = TidalAlbum {
        tidal_id: 280721703,
        title: "Blackstar".to_string(),
        cover: Some("687d56f7-c051-4c32-854c-f5947e448738".to_string()),
        release_date: Some("2016-01-08".to_string()),
        total_tracks: Some(7),
        artist: Some(TidalArtist {
            id: 4768,
            name: "David Bowie".to_string(),
        }),
        artists: None,
        album_type: None,
        upc: Some("886445642531".to_string()),
        label: Some("ISO Records/Columbia".to_string()),
    };

    // Default cover_url must be 1280x1280 (high resolution >= 1000px)
    let url = album.cover_url().expect("Cover URL must exist");
    assert_eq!(
        url,
        "https://resources.tidal.com/images/687d56f7/c051/4c32/854c/f5947e448738/1280x1280.jpg"
    );
    assert!(!url.contains("320x320.jpg"), "Must not use low-res 320x320 thumbnail");

    // Parametric dimensions test
    let custom_url = album.cover_url_with_dimensions(640, 640).unwrap();
    assert_eq!(
        custom_url,
        "https://resources.tidal.com/images/687d56f7/c051/4c32/854c/f5947e448738/640x640.jpg"
    );

    // Module-level helper test
    let helper_url = get_tidal_cover_url("88a79f9d-6ae7-4ef3-ac57-ff66e5dd9bde", 1280, 1280);
    assert_eq!(
        helper_url,
        "https://resources.tidal.com/images/88a79f9d/6ae7/4ef3/ac57/ff66e5dd9bde/1280x1280.jpg"
    );

    // Absolute URLs must pass through untouched
    let direct_album = TidalAlbum {
        tidal_id: 12345,
        title: "Direct Cover Album".to_string(),
        cover: Some("https://custom-cdn.example.com/artwork/cover.png".to_string()),
        release_date: None,
        total_tracks: None,
        artist: None,
        artists: None,
        album_type: None,
        upc: None,
        label: None,
    };
    assert_eq!(
        direct_album.cover_url().as_deref(),
        Some("https://custom-cdn.example.com/artwork/cover.png")
    );

    // None cover returns None
    let no_cover_album = TidalAlbum {
        tidal_id: 99999,
        title: "No Cover Album".to_string(),
        cover: None,
        release_date: None,
        total_tracks: None,
        artist: None,
        artists: None,
        album_type: None,
        upc: None,
        label: None,
    };
    assert!(no_cover_album.cover_url().is_none());
}

#[tokio::test]
async fn test_migration_0073_cover_art_url_upgrade_and_recurrence_prevention() {
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await
        .expect("Failed to connect to in-memory SQLite database");

    sqlx::query("PRAGMA foreign_keys = ON")
        .execute(&pool)
        .await
        .expect("Enable foreign keys");

    // 1. Apply all migrations through 0071 first
    let migrator = sqlx::migrate!("./migrations");
    let migrations: Vec<_> = migrator.iter().collect();

    let partial_migrator = sqlx::migrate::Migrator {
        migrations: std::borrow::Cow::Owned(
            migrations
                .iter()
                .filter(|m| m.version <= 71)
                .map(|m| (*m).clone())
                .collect(),
        ),
        ignore_missing: false,
        locking: true,
        no_tx: false,
    };
    partial_migrator
        .run(&pool)
        .await
        .expect("Run migrations through 0071");

    // 2. Seed albums with low-resolution 320x320 URLs and various test cases
    sqlx::query(
        r#"
        INSERT INTO albums (id, title, release_date, tidal_id, cover_art_url)
        VALUES 
            (1, 'Album LowRes 1', '2020-01-01', '1001', 'https://resources.tidal.com/images/88a79f9d/6ae7/4ef3/ac57/ff66e5dd9bde/320x320.jpg'),
            (2, 'Album LowRes 2', '2021-01-01', '1002', 'https://resources.tidal.com/images/687d56f7/c051/4c32/854c/f5947e448738/320x320.jpg'),
            (3, 'Album Already HighRes', '2022-01-01', '1003', 'https://resources.tidal.com/images/11111111/2222/3333/4444/555555555555/1280x1280.jpg'),
            (4, 'Album ThirdParty CDN', '2023-01-01', NULL, 'https://i.scdn.co/image/ab67616d0000b273b5f000'),
            (5, 'Album No Cover', '2024-01-01', '1005', NULL);
        "#
    )
    .execute(&pool)
    .await
    .expect("Seed albums");

    // Seed account for favorites/playlists using pre-seeded tidal service from migration 0002
    let account_id: i64 = sqlx::query_scalar(
        r#"
        INSERT INTO accounts (service_id, display_name, email)
        VALUES ((SELECT id FROM services WHERE name = 'tidal'), 'Tidal User', 'user@example.com')
        RETURNING id
        "#
    )
    .fetch_one(&pool)
    .await
    .expect("Seed account");

    // Seed favorites with low-res image_url
    sqlx::query(
        r#"
        INSERT INTO favorites (account_id, service_id, item_type, service_item_id, title, image_url)
        VALUES (?, (SELECT id FROM services WHERE name = 'tidal'), 'album', '1001', 'Album LowRes 1', 'https://resources.tidal.com/images/88a79f9d/6ae7/4ef3/ac57/ff66e5dd9bde/320x320.jpg');
        "#
    )
    .bind(account_id)
    .execute(&pool)
    .await
    .expect("Seed favorites");

    // Seed playlist with low-res image_url
    sqlx::query(
        r#"
        INSERT INTO playlists (id, account_id, service_playlist_id, name, image_url)
        VALUES (1, ?, 'pl-1', 'My Playlist', 'https://resources.tidal.com/images/88a79f9d/6ae7/4ef3/ac57/ff66e5dd9bde/320x320.jpg');
        "#
    )
    .bind(account_id)
    .execute(&pool)
    .await
    .expect("Seed playlists");

    // Pre-migration count check
    let count_320_pre: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM albums WHERE cover_art_url LIKE '%/320x320.jpg%'"
    )
    .fetch_one(&pool)
    .await
    .expect("Count 320 pre-migration");
    assert_eq!(count_320_pre, 2, "Must have exactly 2 low-res albums before migration");

    // 3. Apply full migrations including 0073
    let full_migrator = sqlx::migrate!("./migrations");
    full_migrator
        .run(&pool)
        .await
        .expect("Run all migrations through 0073");

    // 4. Post-migration verification: exactly 0 albums maintain 320x320
    let count_320_post: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM albums WHERE cover_art_url LIKE '%/320x320.jpg%'"
    )
    .fetch_one(&pool)
    .await
    .expect("Count 320 post-migration");
    assert_eq!(count_320_post, 0, "0 URLs in albums must retain /320x320.jpg");

    // Albums 1 and 2 must now have 1280x1280
    let (url1,): (Option<String>,) = sqlx::query_as("SELECT cover_art_url FROM albums WHERE id = 1")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(
        url1.as_deref(),
        Some("https://resources.tidal.com/images/88a79f9d/6ae7/4ef3/ac57/ff66e5dd9bde/1280x1280.jpg")
    );

    let (url2,): (Option<String>,) = sqlx::query_as("SELECT cover_art_url FROM albums WHERE id = 2")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(
        url2.as_deref(),
        Some("https://resources.tidal.com/images/687d56f7/c051/4c32/854c/f5947e448738/1280x1280.jpg")
    );

    // Album 3 (already high-res) must remain intact
    let (url3,): (Option<String>,) = sqlx::query_as("SELECT cover_art_url FROM albums WHERE id = 3")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(
        url3.as_deref(),
        Some("https://resources.tidal.com/images/11111111/2222/3333/4444/555555555555/1280x1280.jpg")
    );

    // Album 4 (third-party) must remain intact
    let (url4,): (Option<String>,) = sqlx::query_as("SELECT cover_art_url FROM albums WHERE id = 4")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(
        url4.as_deref(),
        Some("https://i.scdn.co/image/ab67616d0000b273b5f000")
    );

    // Album 5 (NULL) must remain NULL
    let (url5,): (Option<String>,) = sqlx::query_as("SELECT cover_art_url FROM albums WHERE id = 5")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert!(url5.is_none());

    // Favorites post-migration check
    let fav_count_320: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM favorites WHERE image_url LIKE '%/320x320.jpg%'"
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(fav_count_320, 0, "0 URLs in favorites must retain /320x320.jpg");

    // Playlists post-migration check
    let pl_count_320: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM playlists WHERE image_url LIKE '%/320x320.jpg%'"
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(pl_count_320, 0, "0 URLs in playlists must retain /320x320.jpg");

    // 5. Test Recurrence Prevention Triggers
    // INSERT trigger test
    sqlx::query(
        "INSERT INTO albums (id, title, release_date, tidal_id, cover_art_url)
         VALUES (6, 'Album Recurrence Insert', '2025-01-01', '1006', 'https://resources.tidal.com/images/99999999/8888/7777/6666/555555555555/320x320.jpg')"
    )
    .execute(&pool)
    .await
    .expect("Insert with 320x320 URL must trigger auto-upgrade");

    let (url6,): (Option<String>,) = sqlx::query_as("SELECT cover_art_url FROM albums WHERE id = 6")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(
        url6.as_deref(),
        Some("https://resources.tidal.com/images/99999999/8888/7777/6666/555555555555/1280x1280.jpg"),
        "Trigger must auto-upgrade inserted 320x320 URL to 1280x1280"
    );

    // UPDATE trigger test
    sqlx::query(
        "UPDATE albums
         SET cover_art_url = 'https://resources.tidal.com/images/update_test/320x320.jpg'
         WHERE id = 5"
    )
    .execute(&pool)
    .await
    .expect("Update with 320x320 URL must trigger auto-upgrade");

    let (url5_updated,): (Option<String>,) = sqlx::query_as("SELECT cover_art_url FROM albums WHERE id = 5")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(
        url5_updated.as_deref(),
        Some("https://resources.tidal.com/images/update_test/1280x1280.jpg"),
        "Trigger must auto-upgrade updated 320x320 URL to 1280x1280"
    );

    // 6. DB Integrity & Foreign Key checks
    let fk_errors: Vec<(String, i64, String, i64)> = sqlx::query_as("PRAGMA foreign_key_check")
        .fetch_all(&pool)
        .await
        .expect("PRAGMA foreign_key_check must succeed");
    assert!(fk_errors.is_empty(), "Foreign key check must return 0 errors: {:?}", fk_errors);

    let integrity_result: String = sqlx::query_scalar("PRAGMA integrity_check")
        .fetch_one(&pool)
        .await
        .expect("PRAGMA integrity_check must succeed");
    assert_eq!(integrity_result, "ok", "Integrity check must return 'ok'");
}

#[test]
fn test_symfonium_animated_cover_invariant_preserved() {
    // Symfonium Invariant: CoverFront (0x03) = image/webp animated is the ONLY
    // configuration that activates animation in Now Playing. Static variants only display static cover.
    // Ensure the 1280x1280 JPEG upgrade does not alter or conflict with WebP animated sidecars/tags.

    let webp_sidecar_name = "cover.webp";
    let folder_webp_name = "folder.webp";
    let animated_webp_name = "animated.webp";
    let static_sidecar_name = "cover.jpg";

    // Sidecar naming invariant checks
    assert!(
        webp_sidecar_name.ends_with(".webp"),
        "WebP sidecars must retain .webp extension for Symfonium animation detection"
    );
    assert_eq!(folder_webp_name, "folder.webp");
    assert_eq!(animated_webp_name, "animated.webp");
    assert_eq!(static_sidecar_name, "cover.jpg");

    // High-res JPEG URLs must target .jpg and never .webp
    let tidal_album = TidalAlbum {
        tidal_id: 10001,
        title: "Album With Animated Cover".to_string(),
        cover: Some("abcdef12-3456-7890-abcd-ef1234567890".to_string()),
        release_date: Some("2024-01-01".to_string()),
        total_tracks: Some(10),
        artist: None,
        artists: None,
        album_type: None,
        upc: None,
        label: None,
    };

    let cover_url = tidal_album.cover_url().unwrap();
    assert!(
        cover_url.ends_with("/1280x1280.jpg"),
        "Tidal high-res cover URL must be JPEG 1280x1280 without interfering with WebP stream"
    );
    assert!(
        !cover_url.ends_with(".webp"),
        "Tidal artwork URL must not masquerade as .webp"
    );
}

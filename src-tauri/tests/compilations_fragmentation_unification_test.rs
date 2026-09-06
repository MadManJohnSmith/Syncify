//! TASK-136: Integration and Regression Test Suite for Compilations Fragmentation Unification
//!
//! Validates:
//! 1. Import Gate compilation deduplication:
//!    - Multiple tracks of a compilation with different artists collapse into a single
//!      album record under canonical "Various Artists" with `is_compilation = 1`.
//! 2. Homonymous mono-artist albums isolation:
//!    - Legitimate mono-artist albums with identical titles (e.g. Queen "Greatest Hits" vs
//!      The Cure "Greatest Hits") are NEVER merged and preserve `is_compilation = 0`.
//! 3. Service struct compilation detection:
//!    - `SpotifyAlbum` and `TidalAlbum` correctly identify compilation releases via
//!      `album_type == "compilation"`, multiple artists, or Various Artists variants.
//! 4. Python maintenance script (`scripts/unify_fragmented_compilations.py`):
//!    - Migrates fragmented tracks from losing album stubs into the canonical winner.
//!    - Recompacts `track_number` sequentially per disc.
//!    - Purges empty losing album stubs cleanly.
//!    - Verifies zero `PRAGMA foreign_key_check` violations and `PRAGMA integrity_check = ok`.

use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use std::process::Command;
use syncify_core_domain::metadata::CANONICAL_VARIOUS_ARTISTS;
use syncify_tauri_lib::import_cache::{get_or_create_canonical_various_artists, ImportCache};
use syncify_tauri_lib::services::spotify::{SpotifyAlbum, SpotifyArtist};
use syncify_tauri_lib::services::tidal::{TidalAlbum, TidalArtist};

#[tokio::test]
async fn test_import_cache_compilation_deduplication() {
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await
        .expect("Failed to connect to in-memory SQLite database");

    sqlx::query("PRAGMA foreign_keys = ON;")
        .execute(&pool)
        .await
        .expect("Enable foreign keys");

    // Apply all canonical migrations
    let migrator = sqlx::migrate!("./migrations");
    migrator
        .run(&pool)
        .await
        .expect("All migrations must apply cleanly");

    let mut cache = ImportCache::new();

    // 1. Create two individual artists that are part of the same compilation
    let artist_a = cache
        .get_or_create_artist(&pool, "Royksopp")
        .await
        .expect("Create Royksopp");
    let artist_b = cache
        .get_or_create_artist(&pool, "Moby")
        .await
        .expect("Create Moby");

    assert_ne!(artist_a, artist_b, "Artists must be distinct");

    // 2. Import first track of compilation 'Late Night Tales'
    let album_title = "Late Night Tales";
    let key_a = format!("{}:{}", artist_a, album_title);
    let album_id_1 = cache
        .get_or_create_album_with_compilation(
            &pool,
            &key_a,
            album_title,
            artist_a,
            Some("2002-01-01"),
            None,
            true, // Marked as compilation
        )
        .await
        .expect("Create compilation album track 1");

    // Verify album properties
    let (is_comp, title): (i64, String) =
        sqlx::query_as("SELECT is_compilation, title FROM albums WHERE id = ?")
            .bind(album_id_1)
            .fetch_one(&pool)
            .await
            .expect("Fetch album row");
    assert_eq!(is_comp, 1, "Album must be marked as compilation");
    assert_eq!(title, album_title);

    // Verify album artist is canonical Various Artists
    let va_artist_name: String = sqlx::query_scalar(
        "SELECT ar.name FROM album_artists aa
         JOIN artists ar ON ar.id = aa.artist_id
         WHERE aa.album_id = ? AND aa.is_primary = 1",
    )
    .bind(album_id_1)
    .fetch_one(&pool)
    .await
    .expect("Fetch primary album artist");
    assert_eq!(
        va_artist_name, CANONICAL_VARIOUS_ARTISTS,
        "Primary album artist must be Various Artists"
    );

    // 3. Import second track of compilation 'Late Night Tales' with different artist
    let key_b = format!("{}:{}", artist_b, album_title);
    let album_id_2 = cache
        .get_or_create_album_with_compilation(
            &pool,
            &key_b,
            album_title,
            artist_b,
            Some("2002-01-01"),
            None,
            true, // Also marked as compilation
        )
        .await
        .expect("Create compilation album track 2");

    assert_eq!(
        album_id_1, album_id_2,
        "Compilation tracks with different artists must collapse into the exact same album ID"
    );

    // Verify only ONE album exists with this title
    let count: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM albums WHERE LOWER(title) = LOWER(?)")
            .bind(album_title)
            .fetch_one(&pool)
            .await
            .expect("Count albums");
    assert_eq!(count, 1, "Only one album record must exist for this compilation");
}

#[tokio::test]
async fn test_mono_artist_homonymous_albums_not_merged() {
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await
        .expect("Failed to connect to in-memory SQLite database");

    sqlx::query("PRAGMA foreign_keys = ON;")
        .execute(&pool)
        .await
        .expect("Enable foreign keys");

    let migrator = sqlx::migrate!("./migrations");
    migrator
        .run(&pool)
        .await
        .expect("All migrations must apply cleanly");

    let mut cache = ImportCache::new();

    let queen_id = cache
        .get_or_create_artist(&pool, "Queen")
        .await
        .expect("Create Queen");
    let cure_id = cache
        .get_or_create_artist(&pool, "The Cure")
        .await
        .expect("Create The Cure");

    // Album 1: Queen - 'Greatest Hits'
    let album_title = "Greatest Hits";
    let key_queen = format!("{}:{}", queen_id, album_title);
    let queen_album_id = cache
        .get_or_create_album_with_compilation(
            &pool,
            &key_queen,
            album_title,
            queen_id,
            Some("1981-10-26"),
            None,
            false, // Mono-artist release
        )
        .await
        .expect("Create Queen Greatest Hits");

    // Album 2: The Cure - 'Greatest Hits'
    let key_cure = format!("{}:{}", cure_id, album_title);
    let cure_album_id = cache
        .get_or_create_album_with_compilation(
            &pool,
            &key_cure,
            album_title,
            cure_id,
            Some("2001-11-13"),
            None,
            false, // Mono-artist release
        )
        .await
        .expect("Create Cure Greatest Hits");

    // Verify albums remain distinct!
    assert_ne!(
        queen_album_id, cure_album_id,
        "Homonymous mono-artist albums must NEVER merge into the same album ID"
    );

    // Verify Queen album is linked to Queen and is_compilation = 0
    let (queen_is_comp, queen_artist): (i64, String) = sqlx::query_as(
        "SELECT a.is_compilation, ar.name FROM albums a
         JOIN album_artists aa ON aa.album_id = a.id AND aa.is_primary = 1
         JOIN artists ar ON ar.id = aa.artist_id
         WHERE a.id = ?",
    )
    .bind(queen_album_id)
    .fetch_one(&pool)
    .await
    .expect("Query Queen album");
    assert_eq!(queen_is_comp, 0, "Queen album must not be compilation");
    assert_eq!(queen_artist, "Queen");

    // Verify Cure album is linked to The Cure and is_compilation = 0
    let (cure_is_comp, cure_artist): (i64, String) = sqlx::query_as(
        "SELECT a.is_compilation, ar.name FROM albums a
         JOIN album_artists aa ON aa.album_id = a.id AND aa.is_primary = 1
         JOIN artists ar ON ar.id = aa.artist_id
         WHERE a.id = ?",
    )
    .bind(cure_album_id)
    .fetch_one(&pool)
    .await
    .expect("Query Cure album");
    assert_eq!(cure_is_comp, 0, "The Cure album must not be compilation");
    assert_eq!(cure_artist, "The Cure");
}

#[test]
fn test_spotify_and_tidal_compilation_detection() {
    // 1. SpotifyAlbum: compilation via album_type
    let mut spotify_comp = SpotifyAlbum::default();
    spotify_comp.name = "Rock Now".to_string();
    spotify_comp.album_type = Some("compilation".to_string());
    assert!(spotify_comp.is_compilation());

    // 2. SpotifyAlbum: compilation via multiple artists
    let mut spotify_multi = SpotifyAlbum::default();
    spotify_multi.name = "50 najlepszych polskich piosenek".to_string();
    spotify_multi.artists = vec![
        SpotifyArtist {
            id: "1".to_string(),
            name: "Artist 1".to_string(),
        },
        SpotifyArtist {
            id: "2".to_string(),
            name: "Artist 2".to_string(),
        },
    ];
    assert!(spotify_multi.is_compilation());

    // 3. SpotifyAlbum: compilation via Various Artists
    let mut spotify_va = SpotifyAlbum::default();
    spotify_va.artists = vec![SpotifyArtist {
        id: "1".to_string(),
        name: "Various Artists".to_string(),
    }];
    assert!(spotify_va.is_compilation());

    // 4. SpotifyAlbum: standard mono-artist
    let mut spotify_mono = SpotifyAlbum::default();
    spotify_mono.album_type = Some("album".to_string());
    spotify_mono.artists = vec![SpotifyArtist {
        id: "1".to_string(),
        name: "Queen".to_string(),
    }];
    assert!(!spotify_mono.is_compilation());

    // 5. TidalAlbum: compilation via album_type
    let tidal_comp = TidalAlbum {
        tidal_id: 100,
        title: "Top Hits".to_string(),
        cover: None,
        release_date: None,
        total_tracks: Some(20),
        artist: None,
        artists: None,
        album_type: Some("COMPILATION".to_string()),
        upc: None,
        label: None,
    };
    assert!(tidal_comp.is_compilation());

    // 6. TidalAlbum: compilation via Various Artists
    let tidal_va = TidalAlbum {
        tidal_id: 101,
        title: "Summer 2025".to_string(),
        cover: None,
        release_date: None,
        total_tracks: Some(15),
        artist: Some(TidalArtist {
            id: 999,
            name: "Various Artists".to_string(),
        }),
        artists: None,
        album_type: None,
        upc: None,
        label: None,
    };
    assert!(tidal_va.is_compilation());

    // 7. TidalAlbum: mono-artist
    let tidal_mono = TidalAlbum {
        tidal_id: 102,
        title: "A Night at the Opera".to_string(),
        cover: None,
        release_date: None,
        total_tracks: Some(12),
        artist: Some(TidalArtist {
            id: 42,
            name: "Queen".to_string(),
        }),
        artists: None,
        album_type: Some("ALBUM".to_string()),
        upc: None,
        label: None,
    };
    assert!(!tidal_mono.is_compilation());
}

#[tokio::test]
async fn test_python_unification_script_full_flow() {
    let temp_dir = tempfile::tempdir().expect("Create tempdir");
    let db_path = temp_dir.path().join("syncify_test_unify.db");
    let db_path_str = db_path.to_str().expect("DB path to str").to_string();

    let connect_opts = SqliteConnectOptions::new()
        .filename(&db_path)
        .create_if_missing(true);

    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect_with(connect_opts)
        .await
        .expect("Connect to temp file SQLite DB");

    sqlx::query("PRAGMA foreign_keys = ON;")
        .execute(&pool)
        .await
        .expect("Enable foreign keys");

    let migrator = sqlx::migrate!("./migrations");
    migrator
        .run(&pool)
        .await
        .expect("Apply migrations to temp DB");

    // Ensure Various Artists exists
    let va_id = get_or_create_canonical_various_artists(&pool)
        .await
        .expect("Ensure Various Artists");

    // Insert 3 artists
    let art1: i64 = sqlx::query_scalar("INSERT INTO artists (name) VALUES ('Maanam') RETURNING id")
        .fetch_one(&pool)
        .await
        .unwrap();
    let art2: i64 =
        sqlx::query_scalar("INSERT INTO artists (name) VALUES ('Perfect') RETURNING id")
            .fetch_one(&pool)
            .await
            .unwrap();
    let art3: i64 = sqlx::query_scalar("INSERT INTO artists (name) VALUES ('Lady Pank') RETURNING id")
        .fetch_one(&pool)
        .await
        .unwrap();

    // Insert fragmented compilation '50 najlepszych polskich piosenek'
    let alb_title = "50 najlepszych polskich piosenek";
    let alb1: i64 = sqlx::query_scalar(
        "INSERT INTO albums (title, total_tracks, is_compilation) VALUES (?, 1, 0) RETURNING id",
    )
    .bind(alb_title)
    .fetch_one(&pool)
    .await
    .unwrap();
    sqlx::query("INSERT INTO album_artists (album_id, artist_id, is_primary) VALUES (?, ?, 1)")
        .bind(alb1)
        .bind(art1)
        .execute(&pool)
        .await
        .unwrap();
    let trk1: i64 = sqlx::query_scalar(
        "INSERT INTO tracks (title, album_id, track_number) VALUES ('Cyklady na Cykladach', ?, 1) RETURNING id",
    )
    .bind(alb1)
    .fetch_one(&pool)
    .await
    .unwrap();
    sqlx::query("INSERT INTO track_artists (track_id, artist_id, role) VALUES (?, ?, 'primary')")
        .bind(trk1)
        .bind(art1)
        .execute(&pool)
        .await
        .unwrap();

    let alb2: i64 = sqlx::query_scalar(
        "INSERT INTO albums (title, total_tracks, is_compilation) VALUES (?, 1, 0) RETURNING id",
    )
    .bind(alb_title)
    .fetch_one(&pool)
    .await
    .unwrap();
    sqlx::query("INSERT INTO album_artists (album_id, artist_id, is_primary) VALUES (?, ?, 1)")
        .bind(alb2)
        .bind(art2)
        .execute(&pool)
        .await
        .unwrap();
    let trk2: i64 = sqlx::query_scalar(
        "INSERT INTO tracks (title, album_id, track_number) VALUES ('Autobiografia', ?, 1) RETURNING id",
    )
    .bind(alb2)
    .fetch_one(&pool)
    .await
    .unwrap();
    sqlx::query("INSERT INTO track_artists (track_id, artist_id, role) VALUES (?, ?, 'primary')")
        .bind(trk2)
        .bind(art2)
        .execute(&pool)
        .await
        .unwrap();

    let alb3: i64 = sqlx::query_scalar(
        "INSERT INTO albums (title, total_tracks, is_compilation) VALUES (?, 1, 0) RETURNING id",
    )
    .bind(alb_title)
    .fetch_one(&pool)
    .await
    .unwrap();
    sqlx::query("INSERT INTO album_artists (album_id, artist_id, is_primary) VALUES (?, ?, 1)")
        .bind(alb3)
        .bind(art3)
        .execute(&pool)
        .await
        .unwrap();
    let trk3: i64 = sqlx::query_scalar(
        "INSERT INTO tracks (title, album_id, track_number) VALUES ('Mniej niz zero', ?, 1) RETURNING id",
    )
    .bind(alb3)
    .fetch_one(&pool)
    .await
    .unwrap();
    sqlx::query("INSERT INTO track_artists (track_id, artist_id, role) VALUES (?, ?, 'primary')")
        .bind(trk3)
        .bind(art3)
        .execute(&pool)
        .await
        .unwrap();

    // Insert legitimate mono-artist homonymous albums: Queen 'Greatest Hits' & The Cure 'Greatest Hits'
    let queen_art: i64 =
        sqlx::query_scalar("INSERT INTO artists (name) VALUES ('Queen Mono') RETURNING id")
            .fetch_one(&pool)
            .await
            .unwrap();
    let cure_art: i64 =
        sqlx::query_scalar("INSERT INTO artists (name) VALUES ('Cure Mono') RETURNING id")
            .fetch_one(&pool)
            .await
            .unwrap();

    let queen_alb: i64 = sqlx::query_scalar(
        "INSERT INTO albums (title, total_tracks, is_compilation) VALUES ('Greatest Hits', 5, 0) RETURNING id",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    sqlx::query("INSERT INTO album_artists (album_id, artist_id, is_primary) VALUES (?, ?, 1)")
        .bind(queen_alb)
        .bind(queen_art)
        .execute(&pool)
        .await
        .unwrap();
    for i in 1..=5 {
        let tid: i64 = sqlx::query_scalar(
            "INSERT INTO tracks (title, album_id, track_number) VALUES (?, ?, ?) RETURNING id",
        )
        .bind(format!("Queen Hit {}", i))
        .bind(queen_alb)
        .bind(i)
        .fetch_one(&pool)
        .await
        .unwrap();
        sqlx::query("INSERT INTO track_artists (track_id, artist_id, role) VALUES (?, ?, 'primary')")
            .bind(tid)
            .bind(queen_art)
            .execute(&pool)
            .await
            .unwrap();
    }

    let cure_alb: i64 = sqlx::query_scalar(
        "INSERT INTO albums (title, total_tracks, is_compilation) VALUES ('Greatest Hits', 5, 0) RETURNING id",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    sqlx::query("INSERT INTO album_artists (album_id, artist_id, is_primary) VALUES (?, ?, 1)")
        .bind(cure_alb)
        .bind(cure_art)
        .execute(&pool)
        .await
        .unwrap();
    for i in 1..=5 {
        let tid: i64 = sqlx::query_scalar(
            "INSERT INTO tracks (title, album_id, track_number) VALUES (?, ?, ?) RETURNING id",
        )
        .bind(format!("Cure Hit {}", i))
        .bind(cure_alb)
        .bind(i)
        .fetch_one(&pool)
        .await
        .unwrap();
        sqlx::query("INSERT INTO track_artists (track_id, artist_id, role) VALUES (?, ?, 'primary')")
            .bind(tid)
            .bind(cure_art)
            .execute(&pool)
            .await
            .unwrap();
    }

    // Close pool so python script can write cleanly
    drop(pool);

    let script_path = if std::path::Path::new("scripts/unify_fragmented_compilations.py").exists() {
        "scripts/unify_fragmented_compilations.py".to_string()
    } else if std::path::Path::new("../scripts/unify_fragmented_compilations.py").exists() {
        "../scripts/unify_fragmented_compilations.py".to_string()
    } else {
        panic!("unify_fragmented_compilations.py not found");
    };

    // Execute python script
    let status = Command::new("python3")
        .arg(&script_path)
        .arg("--db-path")
        .arg(&db_path_str)
        .arg("--backup-dir")
        .arg(temp_dir.path().to_str().unwrap())
        .status()
        .expect("Execute unify_fragmented_compilations.py");
    assert!(status.success(), "Script execution must exit successfully");

    // Reopen DB and verify assertions
    let post_opts = SqliteConnectOptions::new().filename(&db_path);
    let pool_post = SqlitePoolOptions::new()
        .max_connections(1)
        .connect_with(post_opts)
        .await
        .expect("Reconnect post script");

    // 1. Only 1 album row exists for '50 najlepszych polskich piosenek'
    let remaining_comp_albums: Vec<(i64, i64, i64)> = sqlx::query_as(
        "SELECT id, is_compilation, total_tracks FROM albums WHERE title = ?",
    )
    .bind(alb_title)
    .fetch_all(&pool_post)
    .await
    .expect("Query comp albums");
    assert_eq!(
        remaining_comp_albums.len(),
        1,
        "Fragmented albums must be collapsed to exactly 1 winner album"
    );
    let (winner_id, winner_is_comp, winner_total) = remaining_comp_albums[0];
    assert_eq!(winner_is_comp, 1, "Winner album must be marked is_compilation = 1");
    assert_eq!(winner_total, 3, "Winner album must have total_tracks = 3");

    // 2. Winner album is linked to Various Artists
    let winner_primary_art: (i64, String) = sqlx::query_as(
        "SELECT ar.id, ar.name FROM album_artists aa
         JOIN artists ar ON ar.id = aa.artist_id
         WHERE aa.album_id = ? AND aa.is_primary = 1",
    )
    .bind(winner_id)
    .fetch_one(&pool_post)
    .await
    .expect("Fetch winner primary artist");
    assert_eq!(winner_primary_art.0, va_id);
    assert_eq!(winner_primary_art.1, CANONICAL_VARIOUS_ARTISTS);

    // 3. All 3 tracks are on winner with sequential track numbers
    let comp_tracks: Vec<(i64, i64)> = sqlx::query_as(
        "SELECT id, track_number FROM tracks WHERE album_id = ? ORDER BY track_number ASC",
    )
    .bind(winner_id)
    .fetch_all(&pool_post)
    .await
    .expect("Fetch comp tracks");
    assert_eq!(comp_tracks.len(), 3, "All 3 tracks must point to winner");
    assert_eq!(comp_tracks[0].1, 1);
    assert_eq!(comp_tracks[1].1, 2);
    assert_eq!(comp_tracks[2].1, 3);

    // 4. Loser album stubs are deleted
    let loser_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM albums WHERE id IN (?, ?)")
        .bind(alb2)
        .bind(alb3)
        .fetch_one(&pool_post)
        .await
        .unwrap();
    assert_eq!(loser_count, 0, "Loser albums must be purged");

    // 5. Queen and Cure mono-artist albums are untouched
    let queen_post: (i64, i64) = sqlx::query_as(
        "SELECT is_compilation, (SELECT COUNT(*) FROM tracks WHERE album_id = ?) FROM albums WHERE id = ?",
    )
    .bind(queen_alb)
    .bind(queen_alb)
    .fetch_one(&pool_post)
    .await
    .unwrap();
    assert_eq!(queen_post.0, 0, "Queen album remains mono-artist is_compilation = 0");
    assert_eq!(queen_post.1, 5, "Queen album retains all 5 tracks");

    let cure_post: (i64, i64) = sqlx::query_as(
        "SELECT is_compilation, (SELECT COUNT(*) FROM tracks WHERE album_id = ?) FROM albums WHERE id = ?",
    )
    .bind(cure_alb)
    .bind(cure_alb)
    .fetch_one(&pool_post)
    .await
    .unwrap();
    assert_eq!(cure_post.0, 0, "Cure album remains mono-artist is_compilation = 0");
    assert_eq!(cure_post.1, 5, "Cure album retains all 5 tracks");

    // 6. PRAGMA foreign_key_check is 0
    let fk_violations: Vec<(String, i64, String, i64)> =
        sqlx::query_as("PRAGMA foreign_key_check")
            .fetch_all(&pool_post)
            .await
            .unwrap();
    assert!(
        fk_violations.is_empty(),
        "Must have 0 foreign key violations, found: {:?}",
        fk_violations
    );

    // 7. PRAGMA integrity_check is ok
    let integrity: (String,) = sqlx::query_as("PRAGMA integrity_check")
        .fetch_one(&pool_post)
        .await
        .unwrap();
    assert_eq!(integrity.0, "ok", "Database integrity must be ok");
}

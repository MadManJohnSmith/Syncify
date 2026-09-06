//! TASK-38: Integration tests for playlist tracks query from SQLite
//!
//! Verifies:
//! 1. Can query tracks for a playlist by numeric `playlist_id`.
//! 2. Returned tracks are ordered by `position ASC` within the playlist.
//! 3. `position`, `track_number`, title, artist, and album are correctly decoded.
//! 4. Pagination via offset and limit works as expected.
//! 5. Querying non-existent playlists returns an empty list without error.
//! 6. Playlist isolation: tracks in one playlist are not returned for another playlist.

use sqlx::sqlite::SqlitePoolOptions;
use syncify_tauri_lib::commands::fetch_local_playlist_tracks_page;

async fn setup_test_db() -> sqlx::Pool<sqlx::Sqlite> {
    let pool = SqlitePoolOptions::new()
        .connect("sqlite::memory:")
        .await
        .expect("Failed to connect to in-memory test DB");

    sqlx::query(
        r#"
        CREATE TABLE services (
            id INTEGER PRIMARY KEY,
            name TEXT NOT NULL UNIQUE
        );
        CREATE TABLE accounts (
            id INTEGER PRIMARY KEY,
            service_id INTEGER REFERENCES services(id)
        );
        CREATE TABLE artists (
            id INTEGER PRIMARY KEY,
            name TEXT NOT NULL
        );
        CREATE TABLE albums (
            id INTEGER PRIMARY KEY,
            title TEXT NOT NULL,
            cover_art_url TEXT
        );
        CREATE TABLE tracks (
            id INTEGER PRIMARY KEY,
            title TEXT NOT NULL,
            album_id INTEGER REFERENCES albums(id),
            duration_ms INTEGER,
            track_number INTEGER,
            disc_number INTEGER DEFAULT 1,
            isrc TEXT,
            genre TEXT,
            bpm REAL,
            musical_key TEXT,
            release_year INTEGER,
            explicit INTEGER DEFAULT 0,
            musicbrainz_id TEXT,
            is_favorite INTEGER NOT NULL DEFAULT 0,
            favorite_at TEXT,
            display_title TEXT,
            source_title TEXT,
            file_disambiguator TEXT
        );
        CREATE TABLE track_artists (
            track_id INTEGER REFERENCES tracks(id),
            artist_id INTEGER REFERENCES artists(id),
            role TEXT DEFAULT 'primary',
            PRIMARY KEY (track_id, artist_id, role)
        );
        CREATE TABLE track_sources (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            track_id INTEGER REFERENCES tracks(id),
            service_id INTEGER REFERENCES services(id),
            service_track_id TEXT NOT NULL,
            format TEXT,
            availability_status TEXT NOT NULL DEFAULT 'unknown_unchecked',
            UNIQUE(track_id, service_id)
        );
        CREATE TABLE library_entries (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            account_id INTEGER REFERENCES accounts(id),
            track_id INTEGER REFERENCES tracks(id),
            UNIQUE(account_id, track_id)
        );
        CREATE TABLE playlists (
            id INTEGER PRIMARY KEY,
            name TEXT NOT NULL
        );
        CREATE TABLE playlist_tracks (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            playlist_id INTEGER REFERENCES playlists(id),
            track_id INTEGER REFERENCES tracks(id),
            position INTEGER NOT NULL,
            UNIQUE(playlist_id, track_id)
        );
        CREATE TABLE downloads (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            track_id INTEGER UNIQUE REFERENCES tracks(id),
            source_service_id INTEGER REFERENCES services(id),
            file_path TEXT NOT NULL,
            file_format TEXT,
            effective_service TEXT,
            file_disambiguator TEXT
        );
        CREATE TABLE download_queue (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            track_id INTEGER REFERENCES tracks(id),
            status TEXT DEFAULT 'queued'
        );
        CREATE TABLE lyrics (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            track_id INTEGER UNIQUE REFERENCES tracks(id),
            content TEXT,
            sync_level TEXT DEFAULT 'none'
        );
        "#,
    )
    .execute(&pool)
    .await
    .expect("Schema creation must succeed");

    // Insert baseline service & artist
    sqlx::query("INSERT INTO services (id, name) VALUES (1, 'qobuz'), (2, 'tidal'), (3, 'spotify');")
        .execute(&pool)
        .await
        .unwrap();

    sqlx::query("INSERT INTO artists (id, name) VALUES (1, 'Pink Floyd'), (2, 'Led Zeppelin');")
        .execute(&pool)
        .await
        .unwrap();

    sqlx::query("INSERT INTO albums (id, title, cover_art_url) VALUES (1, 'The Dark Side of the Moon', 'https://art.example/dsotm.jpg');")
        .execute(&pool)
        .await
        .unwrap();

    pool
}

#[tokio::test]
async fn test_get_playlist_tracks_orders_by_position() {
    let db = setup_test_db().await;

    // Create 3 tracks
    sqlx::query(
        r#"
        INSERT INTO tracks (id, title, album_id, duration_ms, track_number)
        VALUES 
            (10, 'Time', 1, 413000, 4),
            (20, 'Money', 1, 382000, 6),
            (30, 'Us and Them', 1, 462000, 7);
        "#,
    )
    .execute(&db)
    .await
    .unwrap();

    sqlx::query(
        r#"
        INSERT INTO track_artists (track_id, artist_id, role)
        VALUES (10, 1, 'primary'), (20, 1, 'primary'), (30, 1, 'primary');
        "#,
    )
    .execute(&db)
    .await
    .unwrap();

    // Create playlist 1
    sqlx::query("INSERT INTO playlists (id, name) VALUES (1, 'Prog Rock Essentials');")
        .execute(&db)
        .await
        .unwrap();

    // Link tracks to playlist 1 with deliberate positions:
    // Track 30 at position 1
    // Track 10 at position 2
    // Track 20 at position 3
    sqlx::query(
        r#"
        INSERT INTO playlist_tracks (playlist_id, track_id, position)
        VALUES 
            (1, 30, 1),
            (1, 10, 2),
            (1, 20, 3);
        "#,
    )
    .execute(&db)
    .await
    .unwrap();

    let tracks = fetch_local_playlist_tracks_page(&db, 1, 0, 100)
        .await
        .expect("Fetch playlist tracks must succeed");

    assert_eq!(tracks.len(), 3);

    // Verify ordering by position (1, 2, 3)
    assert_eq!(tracks[0].id, 30);
    assert_eq!(tracks[0].title, "Us and Them");
    assert_eq!(tracks[0].artist_name.as_deref(), Some("Pink Floyd"));
    assert_eq!(tracks[0].album_name.as_deref(), Some("The Dark Side of the Moon"));
    assert_eq!(tracks[0].track_number, Some(7));
    assert_eq!(tracks[0].position, Some(1));

    assert_eq!(tracks[1].id, 10);
    assert_eq!(tracks[1].title, "Time");
    assert_eq!(tracks[1].track_number, Some(4));
    assert_eq!(tracks[1].position, Some(2));

    assert_eq!(tracks[2].id, 20);
    assert_eq!(tracks[2].title, "Money");
    assert_eq!(tracks[2].track_number, Some(6));
    assert_eq!(tracks[2].position, Some(3));
}

#[tokio::test]
async fn test_get_playlist_tracks_pagination() {
    let db = setup_test_db().await;

    sqlx::query(
        r#"
        INSERT INTO tracks (id, title, album_id, duration_ms, track_number)
        VALUES 
            (1, 'Track One', 1, 100000, 1),
            (2, 'Track Two', 1, 120000, 2),
            (3, 'Track Three', 1, 140000, 3),
            (4, 'Track Four', 1, 160000, 4);
        "#,
    )
    .execute(&db)
    .await
    .unwrap();

    sqlx::query("INSERT INTO playlists (id, name) VALUES (10, 'Paged Playlist');")
        .execute(&db)
        .await
        .unwrap();

    sqlx::query(
        r#"
        INSERT INTO playlist_tracks (playlist_id, track_id, position)
        VALUES (10, 1, 1), (10, 2, 2), (10, 3, 3), (10, 4, 4);
        "#,
    )
    .execute(&db)
    .await
    .unwrap();

    // Page 1: limit 2, offset 0 -> Tracks 1 and 2
    let page1 = fetch_local_playlist_tracks_page(&db, 10, 0, 2)
        .await
        .expect("Page 1 must succeed");
    assert_eq!(page1.len(), 2);
    assert_eq!(page1[0].id, 1);
    assert_eq!(page1[0].position, Some(1));
    assert_eq!(page1[1].id, 2);
    assert_eq!(page1[1].position, Some(2));

    // Page 2: limit 2, offset 2 -> Tracks 3 and 4
    let page2 = fetch_local_playlist_tracks_page(&db, 10, 2, 2)
        .await
        .expect("Page 2 must succeed");
    assert_eq!(page2.len(), 2);
    assert_eq!(page2[0].id, 3);
    assert_eq!(page2[0].position, Some(3));
    assert_eq!(page2[1].id, 4);
    assert_eq!(page2[1].position, Some(4));

    // Page 3: offset beyond total -> Empty
    let page3 = fetch_local_playlist_tracks_page(&db, 10, 4, 2)
        .await
        .expect("Page 3 must succeed");
    assert_eq!(page3.len(), 0);
}

#[tokio::test]
async fn test_get_playlist_tracks_playlist_isolation_and_empty() {
    let db = setup_test_db().await;

    sqlx::query("INSERT INTO tracks (id, title, album_id) VALUES (5, 'Song 5', 1);")
        .execute(&db)
        .await
        .unwrap();

    sqlx::query("INSERT INTO playlists (id, name) VALUES (100, 'PL 100'), (200, 'PL 200');")
        .execute(&db)
        .await
        .unwrap();

    sqlx::query("INSERT INTO playlist_tracks (playlist_id, track_id, position) VALUES (100, 5, 1);")
        .execute(&db)
        .await
        .unwrap();

    // Query PL 100 -> returns Track 5
    let tracks100 = fetch_local_playlist_tracks_page(&db, 100, 0, 50).await.unwrap();
    assert_eq!(tracks100.len(), 1);
    assert_eq!(tracks100[0].id, 5);

    // Query PL 200 -> empty
    let tracks200 = fetch_local_playlist_tracks_page(&db, 200, 0, 50).await.unwrap();
    assert_eq!(tracks200.len(), 0);

    // Query non-existent PL 999 -> empty
    let tracks999 = fetch_local_playlist_tracks_page(&db, 999, 0, 50).await.unwrap();
    assert_eq!(tracks999.len(), 0);
}

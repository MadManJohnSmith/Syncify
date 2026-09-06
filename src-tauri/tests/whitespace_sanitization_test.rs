//! Tests for TASK-134: Whitespace Sanitization in Albums and Track Titles
//!
//! Validates:
//! 1. `sanitize_album_title` and `sanitize_track_title` string sanitization rules:
//!    - Leading and trailing whitespace stripping
//!    - Internal consecutive whitespace collapsing (spaces, tabs, newlines, carriage returns)
//!    - HTML entity decoding and mojibake resolution
//! 2. SQLite Migration 0068 execution:
//!    - In-memory database migration with dirty album and track records
//!    - Deduplication of colliding albums sharing the same artist
//!    - Reassignment of tracks and artist links to canonical winner albums
//!    - Verification of database triggers ensuring future inserts are sanitized
//!    - Foreign key and integrity checks pass with 0 errors

use sqlx::sqlite::SqlitePoolOptions;
use sqlx::Row;
use syncify_core_domain::metadata::{sanitize_album_title, sanitize_track_title};

#[test]
fn test_sanitize_album_and_track_titles() {
    // 1. Album titles
    assert_eq!(sanitize_album_title("Neon Golden "), "Neon Golden");
    assert_eq!(sanitize_album_title("   Neon Golden"), "Neon Golden");
    assert_eq!(sanitize_album_title("Neon   Golden"), "Neon Golden");
    assert_eq!(
        sanitize_album_title("The   Dark  \r\n Side \t  of the   Moon  "),
        "The Dark Side of the Moon"
    );
    assert_eq!(sanitize_album_title("   "), "");
    assert_eq!(sanitize_album_title("Tom &amp; Jerry"), "Tom & Jerry");

    // 2. Track titles
    assert_eq!(
        sanitize_track_title("Sept pièces lyriques op. 47,  No. 3 : Mélodie"),
        "Sept pièces lyriques op. 47, No. 3 : Mélodie"
    );
    assert_eq!(
        sanitize_track_title("  Track  \t  Title  \r\n  (Remix)   "),
        "Track Title (Remix)"
    );
    assert_eq!(sanitize_track_title("   "), "");
    assert_eq!(
        sanitize_track_title("Rock &amp; Roll  Music"),
        "Rock & Roll Music"
    );
}

#[tokio::test]
async fn test_sqlite_whitespace_sanitization_and_deduplication() {
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await
        .expect("Failed to connect to in-memory SQLite database");

    // Enable foreign keys
    sqlx::query("PRAGMA foreign_keys = ON")
        .execute(&pool)
        .await
        .expect("Enable foreign keys");

    // 1. Apply all migrations through 0067 first to set up pre-migration dirty state
    let migrator = sqlx::migrate!("./migrations");
    let migrations: Vec<_> = migrator.iter().collect();

    // Migrate up to 0067
    let partial_migrator = sqlx::migrate::Migrator {
        migrations: std::borrow::Cow::Owned(
            migrations
                .iter()
                .filter(|m| m.version <= 67)
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
        .expect("Run migrations 0001..=0067");

    // 2. Seed an artist
    let artist_id: i64 = sqlx::query_scalar(
        "INSERT INTO artists (name) VALUES ('The Notwist') RETURNING id",
    )
    .fetch_one(&pool)
    .await
    .expect("Insert artist");

    // 3. Seed colliding albums:
    // Winner: 'Neon Golden' (clean, with tidal_id)
    let winner_album_id: i64 = sqlx::query_scalar(
        "INSERT INTO albums (title, release_date, tidal_id) VALUES ('Neon Golden', '2002-01-01', 'tidal_alb_1') RETURNING id",
    )
    .fetch_one(&pool)
    .await
    .expect("Insert winner album");

    sqlx::query("INSERT INTO album_artists (album_id, artist_id, is_primary) VALUES (?, ?, 1)")
        .bind(winner_album_id)
        .bind(artist_id)
        .execute(&pool)
        .await
        .expect("Link winner album artist");

    // Loser: 'Neon Golden ' (trailing space, with cover_art_url)
    let loser_album_id: i64 = sqlx::query_scalar(
        "INSERT INTO albums (title, cover_art_url) VALUES ('Neon Golden ', 'https://example.com/cover.jpg') RETURNING id",
    )
    .fetch_one(&pool)
    .await
    .expect("Insert loser album");

    sqlx::query("INSERT INTO album_artists (album_id, artist_id, is_primary) VALUES (?, ?, 1)")
        .bind(loser_album_id)
        .bind(artist_id)
        .execute(&pool)
        .await
        .expect("Link loser album artist");

    // 4. Seed tracks:
    // Track 1 under winner album
    let _track1_id: i64 = sqlx::query_scalar(
        "INSERT INTO tracks (title, album_id, duration_ms, isrc) VALUES ('One With the Freaks', ?, 218000, 'USNOTWIST001') RETURNING id",
    )
    .bind(winner_album_id)
    .fetch_one(&pool)
    .await
    .expect("Insert track 1");

    // Track 2 under loser album with internal double space in title
    let track2_id: i64 = sqlx::query_scalar(
        "INSERT INTO tracks (title, album_id, duration_ms, isrc) VALUES ('Sept pièces lyriques op. 47,  No. 3 : Mélodie', ?, 195000, 'USNOTWIST002') RETURNING id",
    )
    .bind(loser_album_id)
    .fetch_one(&pool)
    .await
    .expect("Insert track 2");

    // Track 3 with trailing whitespace
    let track3_id: i64 = sqlx::query_scalar(
        "INSERT INTO tracks (title, duration_ms, isrc) VALUES ('Trailing Track Title  ', 180000, 'USNOTWIST003') RETURNING id",
    )
    .fetch_one(&pool)
    .await
    .expect("Insert track 3");

    // 5. Now run complete migrator to apply 0068
    migrator
        .run(&pool)
        .await
        .expect("Run all migrations including 0068");

    // 6. Assertions after migration 0068:
    // (a) Loser album should be deleted/merged
    let loser_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM albums WHERE id = ?")
        .bind(loser_album_id)
        .fetch_one(&pool)
        .await
        .expect("Query loser count");
    assert_eq!(loser_count, 0, "Loser album must be merged and deleted");

    // (b) Winner album title is sanitized and metadata merged
    let (winner_title, cover_url, tidal_id): (String, Option<String>, Option<String>) = sqlx::query_as(
        "SELECT title, cover_art_url, tidal_id FROM albums WHERE id = ?",
    )
    .bind(winner_album_id)
    .fetch_one(&pool)
    .await
    .expect("Query winner album");

    assert_eq!(winner_title, "Neon Golden");
    assert_eq!(cover_url.as_deref(), Some("https://example.com/cover.jpg"));
    assert_eq!(tidal_id.as_deref(), Some("tidal_alb_1"));

    // (c) Track 2's album_id was reassigned to winner_album_id
    let (t2_title, t2_album_id): (String, Option<i64>) =
        sqlx::query_as("SELECT title, album_id FROM tracks WHERE id = ?")
            .bind(track2_id)
            .fetch_one(&pool)
            .await
            .expect("Query track 2");

    assert_eq!(t2_album_id, Some(winner_album_id));
    assert_eq!(t2_title, "Sept pièces lyriques op. 47, No. 3 : Mélodie");

    // (d) Track 3's title was trimmed
    let t3_title: String = sqlx::query_scalar("SELECT title FROM tracks WHERE id = ?")
        .bind(track3_id)
        .fetch_one(&pool)
        .await
        .expect("Query track 3");
    assert_eq!(t3_title, "Trailing Track Title");

    // (e) Verify database triggers sanitize future inserts
    let test_album_id: i64 = sqlx::query_scalar(
        "INSERT INTO albums (title) VALUES ('  Future Album With Spaces  ') RETURNING id",
    )
    .fetch_one(&pool)
    .await
    .expect("Insert future album");

    let test_album_title: String =
        sqlx::query_scalar("SELECT title FROM albums WHERE id = ?")
            .bind(test_album_id)
            .fetch_one(&pool)
            .await
            .expect("Query future album title");
    assert_eq!(
        test_album_title, "Future Album With Spaces",
        "Trigger must sanitize leading/trailing spaces on album insert"
    );

    let test_track_id: i64 = sqlx::query_scalar(
        "INSERT INTO tracks (title, isrc) VALUES ('  Future Track With Spaces  ', 'USFUTURE00001') RETURNING id",
    )
    .fetch_one(&pool)
    .await
    .expect("Insert future track");

    let test_track_title: String =
        sqlx::query_scalar("SELECT title FROM tracks WHERE id = ?")
            .bind(test_track_id)
            .fetch_one(&pool)
            .await
            .expect("Query future track title");
    assert_eq!(
        test_track_title, "Future Track With Spaces",
        "Trigger must sanitize leading/trailing spaces on track insert"
    );

    // (f) Verify zero foreign key violations and clean integrity check
    let fk_rows = sqlx::query("PRAGMA foreign_key_check")
        .fetch_all(&pool)
        .await
        .expect("PRAGMA foreign_key_check");
    assert!(
        fk_rows.is_empty(),
        "PRAGMA foreign_key_check must return 0 violations, got {}",
        fk_rows.len()
    );

    let integrity_row = sqlx::query("PRAGMA integrity_check")
        .fetch_one(&pool)
        .await
        .expect("PRAGMA integrity_check");
    let integrity_status: String = integrity_row.get(0);
    assert_eq!(
        integrity_status, "ok",
        "Database integrity check must return 'ok'"
    );
}

//! Tests for TASK-144: Mojibake and Residual Control Characters in Catalog (track 4037, album 4918)
//!
//! Validates:
//! 1. `clean_mojibake`, `sanitize_album_title`, and `sanitize_track_title` string sanitization rules:
//!    - Mojibake sequences `Â¿` -> `¿`, `Â¡` -> `¡`, `àº` -> `ú`, `Àº` -> `Ú`, `Êº` -> `”`
//!    - Stripping of embedded control characters `\r`, `\n`, `\t` and residual empty parentheticals `(\n\t)`
//! 2. SQLite Migration 0071 execution:
//!    - In-memory database migration with dirty album 4918 and track 4037 records
//!    - Point repair of track 4037 (`¿Y Tú Qué Has Hecho?`) and album 4918 (`Attack Decay Sustain Release`)
//!    - General catalog sanitization for residual mojibake and embedded controls
//!    - Recurrence prevention triggers on albums and tracks
//!    - Foreign key check and SQLite integrity check pass with 0 errors

use sqlx::sqlite::SqlitePoolOptions;
use syncify_core_domain::metadata::{clean_mojibake, sanitize_album_title, sanitize_track_title};

#[test]
fn test_clean_mojibake_and_sanitization() {
    // 1. Direct mojibake sequences
    assert_eq!(
        clean_mojibake("Â¿Y Tàº Qué Has Hecho?"),
        "¿Y Tú Qué Has Hecho?"
    );
    assert_eq!(clean_mojibake("Â¡Hola Mundo!"), "¡Hola Mundo!");
    assert_eq!(clean_mojibake("ÊºQuoted TitleÊº"), "”Quoted Title”");
    assert_eq!(clean_mojibake("Àºltimo aviso"), "Último aviso");
    assert_eq!(clean_mojibake("Standard Track Title"), "Standard Track Title");

    // 2. Track title sanitization with controls and mojibake
    assert_eq!(
        sanitize_track_title("Â¿Y Tàº Qué Has Hecho?"),
        "¿Y Tú Qué Has Hecho?"
    );
    assert_eq!(
        sanitize_track_title("Track Title (\n\t)"),
        "Track Title"
    );
    assert_eq!(
        sanitize_track_title("  \r\n\t Â¡Viva  la  Vida! \t "),
        "¡Viva la Vida!"
    );

    // 3. Album title sanitization with embedded controls and mojibake
    assert_eq!(
        sanitize_album_title("Attack Decay Sustain Release (\n\t)"),
        "Attack Decay Sustain Release"
    );
    assert_eq!(
        sanitize_album_title("Attack Decay Sustain Release \r\n\t"),
        "Attack Decay Sustain Release"
    );
    assert_eq!(
        sanitize_album_title("  ÊºGreatest HitsÊº  "),
        "”Greatest Hits”"
    );
    assert_eq!(
        sanitize_album_title("Album (\r\n\t) (Deluxe)"),
        "Album (Deluxe)"
    );
}

#[tokio::test]
async fn test_sqlite_migration_0071_repair_and_triggers() {
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

    // 1. Apply all migrations through 0070 first to set up pre-migration dirty state
    let migrator = sqlx::migrate!("./migrations");
    let migrations: Vec<_> = migrator.iter().collect();

    let partial_migrator = sqlx::migrate::Migrator {
        migrations: std::borrow::Cow::Owned(
            migrations
                .iter()
                .filter(|m| m.version <= 70)
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
        .expect("Run migrations 0001..=0070");

    // 2. Seed dirty records:
    // Artist for Buena Vista Social Club
    let artist_bvsc: i64 = sqlx::query_scalar(
        "INSERT INTO artists (name) VALUES ('Buena Vista Social Club') RETURNING id",
    )
    .fetch_one(&pool)
    .await
    .expect("Insert BVSC artist");

    // Album 4918 with embedded controls: 'Attack Decay Sustain Release (\n\t)'
    sqlx::query(
        "INSERT INTO albums (id, title, release_date) VALUES (4918, 'Attack Decay Sustain Release (\n\t)', '2007-01-01')",
    )
    .execute(&pool)
    .await
    .expect("Insert dirty album 4918");

    // Track 4037 with mojibake: 'Â¿Y Tàº Qué Has Hecho?'
    sqlx::query(
        "INSERT INTO tracks (id, title, album_id, duration_ms, isrc) VALUES (4037, 'Â¿Y Tàº Qué Has Hecho?', 4918, 198000, 'US1234567890')",
    )
    .execute(&pool)
    .await
    .expect("Insert dirty track 4037");

    sqlx::query("INSERT INTO track_artists (track_id, artist_id, role) VALUES (4037, ?, 'primary')")
        .bind(artist_bvsc)
        .execute(&pool)
        .await
        .expect("Link track artist");

    // Additional dirty track and album to verify general sanitization
    let dirty_album_id: i64 = sqlx::query_scalar(
        "INSERT INTO albums (title) VALUES ('Â¿Otro àºlbum corrupto? (\n\t)') RETURNING id",
    )
    .fetch_one(&pool)
    .await
    .expect("Insert additional dirty album");

    sqlx::query(
        "INSERT INTO tracks (title, album_id, duration_ms, isrc) VALUES ('Â¡Canciàºn con ÊºmojibakeÊº!', ?, 210000, 'US1234567891')",
    )
    .bind(dirty_album_id)
    .execute(&pool)
    .await
    .expect("Insert additional dirty track");

    // 3. Now apply migration 0071
    let full_migrator = sqlx::migrate!("./migrations");
    full_migrator
        .run(&pool)
        .await
        .expect("Run migration 0071");

    // 4. Verify Track 4037 was repaired
    let track_4037_title: String = sqlx::query_scalar("SELECT title FROM tracks WHERE id = 4037")
        .fetch_one(&pool)
        .await
        .expect("Fetch track 4037");
    assert_eq!(
        track_4037_title, "¿Y Tú Qué Has Hecho?",
        "Track 4037 title must be repaired to ¿Y Tú Qué Has Hecho?"
    );

    // 5. Verify Album 4918 was repaired
    let album_4918_title: String = sqlx::query_scalar("SELECT title FROM albums WHERE id = 4918")
        .fetch_one(&pool)
        .await
        .expect("Fetch album 4918");
    assert_eq!(
        album_4918_title, "Attack Decay Sustain Release",
        "Album 4918 title must have control characters and residual parentheticals cleaned"
    );

    // 6. Verify additional dirty album and track were sanitized
    let dirty_album_title: String =
        sqlx::query_scalar("SELECT title FROM albums WHERE id = ?")
            .bind(dirty_album_id)
            .fetch_one(&pool)
            .await
            .expect("Fetch dirty album");
    assert_eq!(
        dirty_album_title, "¿Otro úlbum corrupto?",
        "General album sanitization must clean Â¿, àº, and (\\n\\t)"
    );

    let dirty_track_title: String =
        sqlx::query_scalar("SELECT title FROM tracks WHERE album_id = ?")
            .bind(dirty_album_id)
            .fetch_one(&pool)
            .await
            .expect("Fetch dirty track");
    assert_eq!(
        dirty_track_title, "¡Canciún con ”mojibake”!",
        "General track sanitization must clean Â¡, àº, and Êº"
    );

    // 7. Verify zero residual mojibake or control characters in all tracks and albums
    let residual_track_controls: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM tracks WHERE title LIKE '%' || char(10) || '%' OR title LIKE '%' || char(13) || '%' OR title LIKE '%' || char(9) || '%' OR title LIKE '%Â¿%' OR title LIKE '%Â¡%' OR title LIKE '%àº%' OR title LIKE '%Àº%' OR title LIKE '%Êº%'"
    )
    .fetch_one(&pool)
    .await
    .expect("Count residual track corruptions");
    assert_eq!(residual_track_controls, 0, "Zero tracks should contain residual mojibake or control chars");

    let residual_album_controls: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM albums WHERE title LIKE '%' || char(10) || '%' OR title LIKE '%' || char(13) || '%' OR title LIKE '%' || char(9) || '%' OR title LIKE '%Â¿%' OR title LIKE '%Â¡%' OR title LIKE '%àº%' OR title LIKE '%Àº%' OR title LIKE '%Êº%'"
    )
    .fetch_one(&pool)
    .await
    .expect("Count residual album corruptions");
    assert_eq!(residual_album_controls, 0, "Zero albums should contain residual mojibake or control chars");

    // 8. Test recurrence prevention triggers:
    // A) Insert new album with mojibake and controls
    let new_album_id: i64 = sqlx::query_scalar(
        "INSERT INTO albums (title) VALUES ('Â¿Nuevo àºlbum? (\n\t)') RETURNING id",
    )
    .fetch_one(&pool)
    .await
    .expect("Insert new album via trigger");

    let new_album_title: String =
        sqlx::query_scalar("SELECT title FROM albums WHERE id = ?")
            .bind(new_album_id)
            .fetch_one(&pool)
            .await
            .expect("Fetch new album");
    assert_eq!(
        new_album_title, "¿Nuevo úlbum?",
        "Trigger trg_albums_clean_mojibake_controls_ins must sanitize newly inserted album"
    );

    // B) Insert new track with mojibake
    let new_track_id: i64 = sqlx::query_scalar(
        "INSERT INTO tracks (title, album_id, isrc) VALUES ('Â¿Pista àºnica con ÊºquotesÊº?', ?, 'US9999999999') RETURNING id",
    )
    .bind(new_album_id)
    .fetch_one(&pool)
    .await
    .expect("Insert new track via trigger");

    let new_track_title: String =
        sqlx::query_scalar("SELECT title FROM tracks WHERE id = ?")
            .bind(new_track_id)
            .fetch_one(&pool)
            .await
            .expect("Fetch new track");
    assert_eq!(
        new_track_title, "¿Pista única con ”quotes”?",
        "Trigger trg_tracks_clean_mojibake_controls_ins must sanitize newly inserted track"
    );

    // C) Update track with controls and mojibake
    sqlx::query("UPDATE tracks SET title = 'Â¡Actualizaciàºn exitosa! \r\n' WHERE id = ?")
        .bind(new_track_id)
        .execute(&pool)
        .await
        .expect("Update track via trigger");

    let updated_track_title: String =
        sqlx::query_scalar("SELECT title FROM tracks WHERE id = ?")
            .bind(new_track_id)
            .fetch_one(&pool)
            .await
            .expect("Fetch updated track");
    assert_eq!(
        updated_track_title, "¡Actualizaciún exitosa!",
        "Trigger trg_tracks_clean_mojibake_controls_upd must sanitize updated track"
    );

    // 9. Integrity checks
    let fk_errors: Vec<(String, i64, String, i64)> =
        sqlx::query_as("PRAGMA foreign_key_check")
            .fetch_all(&pool)
            .await
            .expect("PRAGMA foreign_key_check");
    assert!(
        fk_errors.is_empty(),
        "Foreign key check must return 0 violations: {:?}",
        fk_errors
    );

    let integrity_row: (String,) = sqlx::query_as("PRAGMA integrity_check")
        .fetch_one(&pool)
        .await
        .expect("PRAGMA integrity_check");
    assert_eq!(
        integrity_row.0, "ok",
        "SQLite integrity check must return 'ok'"
    );
}

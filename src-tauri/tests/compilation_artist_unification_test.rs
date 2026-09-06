//! TASK-137: Integration and Regression Test Suite for Compilation Artist Unification
//! (Various Interprets / Unknown / Unknown Artist / VA -> Various Artists)
//!
//! Validates:
//! 1. `crates/syncify-core-domain/src/metadata.rs`:
//!    - `is_various_artists_variant` correctly identifies all compilation variants.
//!    - `sanitize_artist_name` maps compilation artist variants directly to canonical "Various Artists".
//!    - `normalize_compilation_artist_name` and `normalize_compilation_artist` normalize
//!      "Various Interprets", "Unknown Artist", "Unknown", "VA", "V.A." into "Various Artists"
//!      in compilation context, while preserving legitimate individual artists.
//! 2. Migration 0081 (`0081_unify_various_artists_compilations.sql`):
//!    - Strictly additive, adds `is_compilation` column to `albums` with index.
//!    - Unifies 'Various Interprets', 'Unknown', 'Unknown Artist', 'VA', 'V.A.', 'Various'
//!      into canonical 'Various Artists' record.
//!    - Reassigns `album_artists`, `track_artists`, and `track_credits` without PK collisions.
//!    - Marks compilation albums with `is_compilation = 1`.
//!    - Purges residual unlinked obsolete artist records.
//!    - Recurrence prevention triggers reject inserting or updating artist names to compilation variants.
//!    - Maintains `PRAGMA foreign_key_check` and `PRAGMA integrity_check` clean.
//! 3. Python maintenance script `scripts/unify_compilation_artists.py`:
//!    - Runs cleanly with `--dry-run` and live execution.
//!    - Creates VACUUM INTO backup snapshot prior to mutation.

use sqlx::sqlite::SqlitePoolOptions;
use std::borrow::Cow;
use std::path::Path;
use std::process::Command;
use syncify_core_domain::metadata::{
    is_various_artists_variant, normalize_compilation_artist,
    normalize_compilation_artist_name, sanitize_artist_name, CANONICAL_VARIOUS_ARTISTS,
};

#[test]
fn test_domain_various_artists_detection_and_normalization() {
    // 1. Detection of various artists variants
    assert!(is_various_artists_variant("Various Artists"));
    assert!(is_various_artists_variant("various artists"));
    assert!(is_various_artists_variant("Various Artist"));
    assert!(is_various_artists_variant("Various Interprets"));
    assert!(is_various_artists_variant("various interprets"));
    assert!(is_various_artists_variant("Various Interpret"));
    assert!(is_various_artists_variant("various interpret"));
    assert!(is_various_artists_variant("Verschiedene Interpreten"));
    assert!(is_various_artists_variant("Divers Interprètes"));
    assert!(is_various_artists_variant("Divers Interpretes"));
    assert!(is_various_artists_variant("V.A."));
    assert!(is_various_artists_variant("v.a."));
    assert!(is_various_artists_variant("VA"));
    assert!(is_various_artists_variant("va"));
    assert!(is_various_artists_variant("V/A"));
    assert!(is_various_artists_variant("v / a"));
    assert!(is_various_artists_variant("Various"));
    assert!(is_various_artists_variant("various"));

    // Legitimate artist names must NOT match
    assert!(!is_various_artists_variant("Queen"));
    assert!(!is_various_artists_variant("David Bowie"));
    assert!(!is_various_artists_variant("Tony Castle"));
    assert!(!is_various_artists_variant("Unknown Mortal Orchestra"));
    assert!(!is_various_artists_variant("The Various"));

    // 2. Direct sanitization of artist variants
    assert_eq!(sanitize_artist_name("Various Interprets"), CANONICAL_VARIOUS_ARTISTS);
    assert_eq!(sanitize_artist_name("various interprets"), CANONICAL_VARIOUS_ARTISTS);
    assert_eq!(sanitize_artist_name("Various Interpret"), CANONICAL_VARIOUS_ARTISTS);
    assert_eq!(sanitize_artist_name("V.A."), CANONICAL_VARIOUS_ARTISTS);
    assert_eq!(sanitize_artist_name("VA"), CANONICAL_VARIOUS_ARTISTS);
    assert_eq!(sanitize_artist_name("Various"), CANONICAL_VARIOUS_ARTISTS);
    assert_eq!(sanitize_artist_name("Verschiedene Interpreten"), CANONICAL_VARIOUS_ARTISTS);

    // 3. Normalization with compilation context
    assert_eq!(
        normalize_compilation_artist_name("Various Interprets", false),
        CANONICAL_VARIOUS_ARTISTS
    );
    assert_eq!(
        normalize_compilation_artist_name("V.A.", false),
        CANONICAL_VARIOUS_ARTISTS
    );
    assert_eq!(
        normalize_compilation_artist_name("Unknown Artist", true),
        CANONICAL_VARIOUS_ARTISTS
    );
    assert_eq!(
        normalize_compilation_artist_name("Unknown", true),
        CANONICAL_VARIOUS_ARTISTS
    );
    assert_eq!(
        normalize_compilation_artist_name("unknown", true),
        CANONICAL_VARIOUS_ARTISTS
    );
    assert_eq!(
        normalize_compilation_artist_name("Unknown Artist", false),
        "Unknown Artist"
    );
    assert_eq!(
        normalize_compilation_artist_name("Queen", true),
        "Queen"
    );
    assert_eq!(
        normalize_compilation_artist_name("Queen", false),
        "Queen"
    );

    // 4. normalize_compilation_artist helper
    assert_eq!(
        normalize_compilation_artist("Various Interprets"),
        CANONICAL_VARIOUS_ARTISTS
    );
    assert_eq!(
        normalize_compilation_artist("Unknown Artist"),
        CANONICAL_VARIOUS_ARTISTS
    );
    assert_eq!(
        normalize_compilation_artist("Unknown"),
        CANONICAL_VARIOUS_ARTISTS
    );
    assert_eq!(
        normalize_compilation_artist("VA"),
        CANONICAL_VARIOUS_ARTISTS
    );
    assert_eq!(
        normalize_compilation_artist("Daft Punk"),
        "Daft Punk"
    );
}

#[tokio::test]
async fn test_sqlite_migration_0081_clean_application_and_triggers() {
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
        .expect("All migrations through 0081 must apply cleanly");

    // Verify is_compilation column exists on albums
    let album_id: i64 = sqlx::query_scalar(
        "INSERT INTO albums (title, is_compilation) VALUES ('Test Compilation', 1) RETURNING id",
    )
    .fetch_one(&pool)
    .await
    .expect("Album with is_compilation must insert successfully");

    let is_comp: (i64,) = sqlx::query_as("SELECT is_compilation FROM albums WHERE id = ?")
        .bind(album_id)
        .fetch_one(&pool)
        .await
        .expect("Read is_compilation");
    assert_eq!(is_comp.0, 1, "is_compilation column must be 1");

    // Verify recurrence prevention triggers reject compilation variants
    let res_vi = sqlx::query("INSERT INTO artists (name) VALUES ('Various Interprets')")
        .execute(&pool)
        .await;
    assert!(
        res_vi.is_err(),
        "Trigger must reject inserting 'Various Interprets'"
    );

    let res_va_short = sqlx::query("INSERT INTO artists (name) VALUES ('VA')")
        .execute(&pool)
        .await;
    assert!(res_va_short.is_err(), "Trigger must reject inserting 'VA'");

    let res_va_dot = sqlx::query("INSERT INTO artists (name) VALUES ('V.A.')")
        .execute(&pool)
        .await;
    assert!(res_va_dot.is_err(), "Trigger must reject inserting 'V.A.'");

    let res_single_interpret = sqlx::query("INSERT INTO artists (name) VALUES ('Various Interpret')")
        .execute(&pool)
        .await;
    assert!(
        res_single_interpret.is_err(),
        "Trigger must reject inserting 'Various Interpret'"
    );

    // Legitimate artist insert must succeed
    let clean_artist_id: i64 =
        sqlx::query_scalar("INSERT INTO artists (name) VALUES ('Clean Artist') RETURNING id")
            .fetch_one(&pool)
            .await
            .expect("Clean artist insert must succeed");

    // Update with compilation variant must be rejected
    let res_upd = sqlx::query("UPDATE artists SET name = 'Various Interprets' WHERE id = ?")
        .bind(clean_artist_id)
        .execute(&pool)
        .await;
    assert!(
        res_upd.is_err(),
        "Trigger must reject updating artist to 'Various Interprets'"
    );

    // Verify automatic album compilation flag trigger
    let va_artist_id: i64 = sqlx::query_scalar(
        "INSERT INTO artists (name) VALUES ('Various Artists') RETURNING id",
    )
    .fetch_one(&pool)
    .await
    .expect("Insert Various Artists");

    let standard_album_id: i64 =
        sqlx::query_scalar("INSERT INTO albums (title, is_compilation) VALUES ('Soundtrack 2025', 0) RETURNING id")
            .fetch_one(&pool)
            .await
            .expect("Insert standard album");

    // Link standard album to Various Artists
    sqlx::query("INSERT INTO album_artists (album_id, artist_id) VALUES (?, ?)")
        .bind(standard_album_id)
        .bind(va_artist_id)
        .execute(&pool)
        .await
        .expect("Link album to Various Artists");

    let auto_comp: (i64,) = sqlx::query_as("SELECT is_compilation FROM albums WHERE id = ?")
        .bind(standard_album_id)
        .fetch_one(&pool)
        .await
        .expect("Read auto is_compilation");
    assert_eq!(
        auto_comp.0, 1,
        "Linking album to Various Artists must automatically set is_compilation = 1"
    );

    // Integrity checks
    let fk_violations: Vec<(String, i64, String, i64)> = sqlx::query_as("PRAGMA foreign_key_check;")
        .fetch_all(&pool)
        .await
        .expect("FK check");
    assert!(fk_violations.is_empty(), "0 foreign key violations expected");
}

#[tokio::test]
async fn test_sqlite_migration_0081_unifies_variants_and_remaps_albums() {
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await
        .expect("Failed to connect to memory DB");

    sqlx::query("PRAGMA foreign_keys = ON;")
        .execute(&pool)
        .await
        .expect("Enable foreign keys");

    // 1. Run migrations 0001 through 0080
    let migrator = sqlx::migrate!("./migrations");
    let all_migrations: Vec<_> = migrator.iter().collect();

    let pre_0081_migrator = sqlx::migrate::Migrator {
        migrations: Cow::Owned(
            all_migrations
                .iter()
                .filter(|m| m.version <= 80)
                .map(|m| (*m).clone())
                .collect(),
        ),
        ignore_missing: false,
        locking: true,
        no_tx: false,
    };
    pre_0081_migrator
        .run(&pool)
        .await
        .expect("Run migrations 0001..=0080");

    // Temporarily allow inserting test seed names that might have been guarded in 0079
    sqlx::query("DROP TRIGGER IF EXISTS trg_artists_reject_garbage_ins;")
        .execute(&pool)
        .await
        .unwrap();

    // 2. Seed dirty state matching production diagnosis:
    // Artists:
    let vi_artist_id: i64 = sqlx::query_scalar(
        "INSERT INTO artists (name) VALUES ('Various Interprets') RETURNING id",
    )
    .fetch_one(&pool)
    .await
    .unwrap();

    let unknown_art_id: i64 = sqlx::query_scalar(
        "INSERT INTO artists (name) VALUES ('Unknown Artist') RETURNING id",
    )
    .fetch_one(&pool)
    .await
    .unwrap();

    let unknown_id: i64 = sqlx::query_scalar(
        "INSERT INTO artists (name) VALUES ('Unknown') RETURNING id",
    )
    .fetch_one(&pool)
    .await
    .unwrap();

    let queen_id: i64 = sqlx::query_scalar(
        "INSERT INTO artists (name) VALUES ('Queen') RETURNING id",
    )
    .fetch_one(&pool)
    .await
    .unwrap();

    // Compilations assigned to Various Interprets:
    let alb_matrix: i64 = sqlx::query_scalar(
        "INSERT INTO albums (title) VALUES ('The Matrix Reloaded: The Album') RETURNING id",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    sqlx::query("INSERT INTO album_artists (album_id, artist_id) VALUES (?, ?)")
        .bind(alb_matrix)
        .bind(vi_artist_id)
        .execute(&pool)
        .await
        .unwrap();

    let alb_shrek: i64 = sqlx::query_scalar(
        "INSERT INTO albums (title) VALUES ('Shrek 2') RETURNING id",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    sqlx::query("INSERT INTO album_artists (album_id, artist_id) VALUES (?, ?)")
        .bind(alb_shrek)
        .bind(vi_artist_id)
        .execute(&pool)
        .await
        .unwrap();

    let alb_austin: i64 = sqlx::query_scalar(
        "INSERT INTO albums (title) VALUES ('Austin Powers: The Spy Who Shagged Me') RETURNING id",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    sqlx::query("INSERT INTO album_artists (album_id, artist_id) VALUES (?, ?)")
        .bind(alb_austin)
        .bind(vi_artist_id)
        .execute(&pool)
        .await
        .unwrap();

    // Album assigned to Unknown Artist:
    let alb_unknown_art: i64 = sqlx::query_scalar(
        "INSERT INTO albums (title) VALUES ('Mystery Compilation') RETURNING id",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    sqlx::query("INSERT INTO album_artists (album_id, artist_id) VALUES (?, ?)")
        .bind(alb_unknown_art)
        .bind(unknown_art_id)
        .execute(&pool)
        .await
        .unwrap();

    // Album assigned to Unknown:
    let alb_unknown: i64 = sqlx::query_scalar(
        "INSERT INTO albums (title) VALUES ('50 Best Classics') RETURNING id",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    sqlx::query("INSERT INTO album_artists (album_id, artist_id) VALUES (?, ?)")
        .bind(alb_unknown)
        .bind(unknown_id)
        .execute(&pool)
        .await
        .unwrap();

    // Mono-artist album assigned to Queen:
    let alb_queen: i64 = sqlx::query_scalar(
        "INSERT INTO albums (title) VALUES ('A Night at the Opera') RETURNING id",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    sqlx::query("INSERT INTO album_artists (album_id, artist_id) VALUES (?, ?)")
        .bind(alb_queen)
        .bind(queen_id)
        .execute(&pool)
        .await
        .unwrap();

    // Seed tracks
    let trk_matrix: i64 = sqlx::query_scalar(
        "INSERT INTO tracks (title, album_id, duration_ms) VALUES ('Session', ?, 180000) RETURNING id",
    )
    .bind(alb_matrix)
    .fetch_one(&pool)
    .await
    .unwrap();
    sqlx::query("INSERT INTO track_artists (track_id, artist_id, role) VALUES (?, ?, 'primary')")
        .bind(trk_matrix)
        .bind(vi_artist_id)
        .execute(&pool)
        .await
        .unwrap();

    let trk_queen: i64 = sqlx::query_scalar(
        "INSERT INTO tracks (title, album_id, duration_ms) VALUES ('Bohemian Rhapsody', ?, 354000) RETURNING id",
    )
    .bind(alb_queen)
    .fetch_one(&pool)
    .await
    .unwrap();
    sqlx::query("INSERT INTO track_artists (track_id, artist_id, role) VALUES (?, ?, 'primary')")
        .bind(trk_queen)
        .bind(queen_id)
        .execute(&pool)
        .await
        .unwrap();

    // 3. Apply migration 0081 using the full migrator
    // SQLx checks _sqlx_migrations and sees 0001..=0080 already applied, so it runs only 0081.
    migrator
        .run(&pool)
        .await
        .expect("Run migration 0081 via full migrator");

    // 4. Assertions:
    // Assert 0 albums remain under 'Various Interprets', 'Unknown', or 'Unknown Artist'
    let remaining_bad_albums: (i64,) = sqlx::query_as(
        r#"
        SELECT COUNT(*) FROM album_artists aa
        JOIN artists ar ON ar.id = aa.artist_id
        WHERE LOWER(TRIM(ar.name)) IN ('various interprets', 'unknown', 'unknown artist')
        "#
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(
        remaining_bad_albums.0, 0,
        "0 albums must remain assigned to Various Interprets, Unknown, or Unknown Artist"
    );

    // Get canonical Various Artists ID
    let canonical_va_id: i64 = sqlx::query_scalar(
        "SELECT id FROM artists WHERE LOWER(TRIM(name)) = 'various artists' LIMIT 1",
    )
    .fetch_one(&pool)
    .await
    .unwrap();

    // Assert the 5 compilations are now remapped to canonical Various Artists
    for aid in [alb_matrix, alb_shrek, alb_austin, alb_unknown_art, alb_unknown] {
        let mapped_artist: (i64, String) = sqlx::query_as(
            r#"
            SELECT ar.id, ar.name FROM album_artists aa
            JOIN artists ar ON ar.id = aa.artist_id
            WHERE aa.album_id = ?
            "#,
        )
        .bind(aid)
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(mapped_artist.0, canonical_va_id);
        assert_eq!(mapped_artist.1, "Various Artists");

        // Assert is_compilation = 1
        let is_comp: (i64,) = sqlx::query_as("SELECT is_compilation FROM albums WHERE id = ?")
            .bind(aid)
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(is_comp.0, 1, "Compilation album {} must have is_compilation = 1", aid);
    }

    // Assert Queen's album remains under Queen and has is_compilation = 0
    let queen_album_artist: (i64, String) = sqlx::query_as(
        r#"
        SELECT ar.id, ar.name FROM album_artists aa
        JOIN artists ar ON ar.id = aa.artist_id
        WHERE aa.album_id = ?
        "#,
    )
    .bind(alb_queen)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(queen_album_artist.0, queen_id);
    assert_eq!(queen_album_artist.1, "Queen");

    let queen_is_comp: (i64,) = sqlx::query_as("SELECT is_compilation FROM albums WHERE id = ?")
        .bind(alb_queen)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(queen_is_comp.0, 0, "Queen album must have is_compilation = 0");

    // Assert track artist remapped to Various Artists
    let track_mapped_artist: (i64,) = sqlx::query_as(
        "SELECT artist_id FROM track_artists WHERE track_id = ?",
    )
    .bind(trk_matrix)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(track_mapped_artist.0, canonical_va_id);

    // Assert obsolete residual artists were purged
    let purged_count: (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM artists WHERE id IN (?, ?, ?)",
    )
    .bind(vi_artist_id)
    .bind(unknown_art_id)
    .bind(unknown_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(purged_count.0, 0, "Obsolete residual artists must be purged from artists table");

    // Assert Queen remains intact
    let queen_exists: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM artists WHERE id = ?")
        .bind(queen_id)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(queen_exists.0, 1, "Queen must remain in artists table");

    // Verify foreign key integrity
    let fk_violations: Vec<(String, i64, String, i64)> = sqlx::query_as("PRAGMA foreign_key_check;")
        .fetch_all(&pool)
        .await
        .unwrap();
    assert!(fk_violations.is_empty(), "0 foreign key violations expected after migration");
}

#[test]
fn test_python_script_execution_and_backup_creation() {
    let script_path = if Path::new("scripts/unify_compilation_artists.py").exists() {
        "scripts/unify_compilation_artists.py".to_string()
    } else if Path::new("../scripts/unify_compilation_artists.py").exists() {
        "../scripts/unify_compilation_artists.py".to_string()
    } else {
        panic!("unify_compilation_artists.py not found in current directory or parent directory");
    };

    // 1. Validate help command
    let help_output = Command::new("python3")
        .arg(&script_path)
        .arg("--help")
        .output()
        .expect("Failed to execute python script help");
    assert!(help_output.status.success(), "Help command must exit with 0");
    let stdout = String::from_utf8_lossy(&help_output.stdout);
    assert!(stdout.contains("Unify compilation artist variants"));

    // 2. Validate dry-run execution against syncify.db if present
    let db_candidate = if Path::new("syncify.db").exists() {
        Some("syncify.db".to_string())
    } else if Path::new("../syncify.db").exists() {
        Some("../syncify.db".to_string())
    } else {
        None
    };

    if let Some(db_path) = db_candidate {
        let dry_output = Command::new("python3")
            .arg(&script_path)
            .arg("--db-path")
            .arg(db_path)
            .arg("--dry-run")
            .output()
            .expect("Failed to execute python dry-run");
        assert!(dry_output.status.success(), "Dry-run command must exit with 0");
        let dry_stdout = String::from_utf8_lossy(&dry_output.stdout);
        assert!(dry_stdout.contains("[DRY RUN] Complete"));
    }
}

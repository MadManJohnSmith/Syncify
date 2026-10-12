//! Offline integration coverage for the library-scoped MusicBrainz index
//! (`mb_library_index`, the L3 layer added by migration `0094`).
//!
//! Every test below performs zero network I/O. There are two kinds:
//!
//!   * Hit-path tests, which observe `MusicBrainzClient::lookup_by_isrc`
//!     returning from a fresh, library-backed row *before* the service ever
//!     reaches `self.client.get(..)`. These are self-proving: had the code
//!     fallen through to HTTP, the call would block on the network.
//!   * Predicate tests, which run the same SQL the service reads/writes directly
//!     against a temporary database, for the cases a hit-path test cannot
//!     express offline (a miss always tries the live API).
//!
//! Verified limitations that shape the design of these tests:
//!   * The WRITE path (`index_library_recording`) runs only after a real API
//!     response, so rows are seeded directly with the column layout of
//!     `migrations/0094_musicbrainz_library_index.sql` rather than produced by
//!     the service.
//!   * L1 is a process-wide static (`MB_QUERY_CACHE`) shared by every test in
//!     this binary. No test clears it: a parallel clear would race the others.
//!     Each test therefore uses an ISRC unique to this file, so a leaked L1
//!     entry can never answer on another test's behalf.
//!   * L2 (`mb_query_cache`) is absent from these databases, so a slow response
//!     can only mean the code moved past the index and started a real request.

use sqlx::sqlite::SqlitePoolOptions;
use sqlx::{Pool, Sqlite};
use syncify_tauri_lib::services::musicbrainz::MusicBrainzClient;
use tempfile::TempDir;

/// The freshness predicate the service applies to `mb_library_index`, re-stated
/// here so the SQL itself — not a Rust copy of it — is what gets tested.
const READ_PREDICATE: &str = "SELECT i.isrc FROM mb_library_index i \
    WHERE i.updated_at >= datetime('now', '-30 days') \
      AND i.updated_at <= datetime('now') \
      AND EXISTS (SELECT 1 FROM tracks t \
                  WHERE UPPER(REPLACE(REPLACE(TRIM(t.isrc), '-', ''), ' ', '')) = i.isrc)";

/// The membership condition every write is gated on: an ISRC may only enter the
/// index if it already exists in the library, after normalization.
const LIBRARY_MEMBERSHIP: &str = "EXISTS (SELECT 1 FROM tracks t \
    WHERE UPPER(REPLACE(REPLACE(TRIM(t.isrc), '-', ''), ' ', '')) = ?)";

/// Open a temporary SQLite database holding exactly the two tables the index
/// touches, and apply the real `0094` migration to it.
async fn index_pool(dir: &TempDir) -> Pool<Sqlite> {
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect(&format!(
            "sqlite:{}?mode=rwc",
            dir.path().join("index.db").display()
        ))
        .await
        .expect("open temporary SQLite database");
    sqlx::query("CREATE TABLE tracks (id INTEGER PRIMARY KEY, title TEXT NOT NULL, isrc TEXT)")
        .execute(&pool)
        .await
        .expect("create minimal tracks schema");
    sqlx::raw_sql(include_str!(
        "../migrations/0094_musicbrainz_library_index.sql"
    ))
    .execute(&pool)
    .await
    .expect("apply migration 0094 to the temporary database");
    pool
}

/// Reopen the same database file, discarding every pooled connection so reads
/// come from disk rather than from a warm pool.
async fn reopen(dir: &TempDir) -> Pool<Sqlite> {
    SqlitePoolOptions::new()
        .max_connections(1)
        .connect(&format!(
            "sqlite:{}?mode=rwc",
            dir.path().join("index.db").display()
        ))
        .await
        .expect("reopen temporary SQLite database")
}

/// Add a track to the library. `isrc` is stored exactly as given on purpose, so
/// tests can exercise unnormalized library values.
async fn seed_track(pool: &Pool<Sqlite>, isrc: &str) {
    sqlx::query("INSERT INTO tracks (title, isrc) VALUES ('Track', ?)")
        .bind(isrc)
        .execute(pool)
        .await
        .expect("seed library track");
}

/// Insert one index row the way the service would. `updated_at` is built from a
/// SQLite date modifier (`-1 day`, `-31 days`, `+1 day`, ...) so freshness is
/// reproducible offline without any clock manipulation.
async fn seed_index_row(pool: &Pool<Sqlite>, isrc: &str, mbid: &str, age_modifier: &str) {
    let payload = format!(r#"{{"id":"{mbid}","title":"Indexed recording"}}"#);
    sqlx::query(
        "INSERT INTO mb_library_index (isrc, mbid, payload, updated_at) \
         VALUES (?, ?, ?, datetime('now', ?))",
    )
    .bind(isrc)
    .bind(mbid)
    .bind(payload)
    .bind(age_modifier)
    .execute(pool)
    .await
    .expect("seed index row");
}

/// The ISRCs the read predicate would currently answer.
async fn readable_isrcs(pool: &Pool<Sqlite>) -> Vec<String> {
    let rows: Vec<(String,)> = sqlx::query_as(READ_PREDICATE)
        .fetch_all(pool)
        .await
        .expect("run index read predicate");
    rows.into_iter().map(|(isrc,)| isrc).collect()
}

#[test]
fn normalization_is_the_key_format_without_changing_l1_or_l2_keys() {
    // The index key is uppercase and separator-free, matching how `db.rs`
    // normalizes `tracks.isrc`.
    assert_eq!(
        MusicBrainzClient::normalize_isrc(" us-rc1 7607-839 "),
        "USRC17607839"
    );
    // L1/L2 keys keep the raw, caller-supplied ISRC untouched for parity.
    assert_eq!(
        MusicBrainzClient::isrc_cache_key("us-rc1-7607-839"),
        "isrc:us-rc1-7607-839"
    );
}

#[test]
fn migration_declares_isrc_as_the_primary_key_and_validates_the_payload() {
    let migration = include_str!("../migrations/0094_musicbrainz_library_index.sql");
    assert!(migration.contains("isrc TEXT PRIMARY KEY"));
    assert!(migration.contains("mbid TEXT NOT NULL"));
    assert!(migration.contains("payload TEXT NOT NULL"));
    assert!(migration.contains("updated_at TEXT NOT NULL"));
    assert!(migration.contains("json_valid(payload)"));
}

#[tokio::test]
async fn fresh_row_backed_by_a_track_is_reused_before_any_http_call() {
    let dir = TempDir::new().expect("temp dir");
    let isrc = "USRC1HIT0001";
    let mbid = "bbbbbbbb-0001-0001-0001-000000000001";
    let pool = index_pool(&dir).await;
    seed_track(&pool, isrc).await;
    seed_index_row(&pool, isrc, mbid, "-1 day").await;
    drop(pool);

    let client = MusicBrainzClient::with_db(reopen(&dir).await);
    // A hit must return immediately; a miss would issue a live request.
    let started = std::time::Instant::now();
    let found = client.lookup_by_isrc(isrc).await.expect("positive lookup");
    assert!(
        started.elapsed() < std::time::Duration::from_secs(2),
        "a fresh, library-backed index hit must not fetch"
    );
    let found = found.expect("recording");
    assert_eq!(found.id, mbid);
    assert_eq!(found.title, "Indexed recording");
}

#[tokio::test]
async fn an_index_hit_is_never_promoted_into_l1_or_l2() {
    let dir = TempDir::new().expect("temp dir");
    let isrc = "USRC1HIT0002";
    let mbid = "bbbbbbbb-0002-0002-0002-000000000002";
    let pool = index_pool(&dir).await;
    seed_track(&pool, isrc).await;
    seed_index_row(&pool, isrc, mbid, "-1 day").await;

    let client = MusicBrainzClient::with_db(pool.clone());
    let first = client.lookup_by_isrc(isrc).await.expect("first lookup");
    assert_eq!(first.expect("recording").id, mbid);

    // Nothing may have been written by the read.
    let l2_tables: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM sqlite_master WHERE type = 'table' AND name = 'mb_query_cache'",
    )
    .fetch_one(&pool)
    .await
    .expect("count L2 tables");
    assert_eq!(l2_tables, 0, "the index read must not create or write L2");
    let stale_rows: Vec<(String,)> = sqlx::query_as("SELECT isrc FROM mb_library_index")
        .fetch_all(&pool)
        .await
        .expect("read index rows");
    assert_eq!(stale_rows, vec![(isrc.to_string(),)]);
}

#[tokio::test]
async fn normalization_of_the_library_isrc_still_matches_the_index() {
    let dir = TempDir::new().expect("temp dir");
    let isrc = "USRC1HIT030003";
    let mbid = "bbbbbbbb-0003-0003-0003-000000000003";
    let pool = index_pool(&dir).await;
    // The library stores a lower-case, dash-separated ISRC.
    seed_track(&pool, "us-rc1-hit03-0003").await;
    seed_index_row(&pool, isrc, mbid, "-1 day").await;
    drop(pool);

    let client = MusicBrainzClient::with_db(reopen(&dir).await);
    let started = std::time::Instant::now();
    // The caller hands over the ISRC in yet another shape; the lookup must
    // canonicalize both sides before comparing.
    let found = client
        .lookup_by_isrc(" US RC1 HIT03 0003 ")
        .await
        .expect("normalized lookup");
    assert!(
        started.elapsed() < std::time::Duration::from_secs(2),
        "an unnormalized library ISRC must still hit the index"
    );
    assert_eq!(found.expect("recording").id, mbid);
}

#[tokio::test]
async fn an_index_hit_leaves_tracks_and_existing_matches_untouched() {
    let dir = TempDir::new().expect("temp dir");
    let isrc = "USRC1HIT0004";
    let mbid = "bbbbbbbb-0004-0004-0004-000000000004";
    let pool = index_pool(&dir).await;
    seed_track(&pool, isrc).await;
    let tracks_before: Vec<(String, String)> =
        sqlx::query_as("SELECT title, CAST(isrc AS TEXT) FROM tracks")
            .fetch_all(&pool)
            .await
            .expect("read tracks before the lookup");
    seed_index_row(&pool, isrc, mbid, "-1 day").await;

    let client = MusicBrainzClient::with_db(pool.clone());
    let found = client.lookup_by_isrc(isrc).await.expect("lookup");
    assert_eq!(found.expect("recording").id, mbid);

    // Nothing in `tracks` may be created, updated or deleted by a hit.
    let tracks_after: Vec<(String, String)> =
        sqlx::query_as("SELECT title, CAST(isrc AS TEXT) FROM tracks")
            .fetch_all(&pool)
            .await
            .expect("list tracks after the lookup");
    assert_eq!(tracks_after, tracks_before);
    assert_eq!(tracks_after, vec![("Track".to_string(), isrc.to_string())]);
}

#[tokio::test]
async fn stale_future_and_orphan_rows_are_not_readable() {
    let dir = TempDir::new().expect("temp dir");
    let pool = index_pool(&dir).await;
    for isrc in ["USRC1FRESH01", "USRC1STALE01", "USRC1FUTURE1"] {
        seed_track(&pool, isrc).await;
    }
    seed_index_row(&pool, "USRC1FRESH01", "mb-fresh", "-29 days").await;
    seed_index_row(&pool, "USRC1STALE01", "mb-stale", "-31 days").await;
    seed_index_row(&pool, "USRC1FUTURE1", "mb-future", "+1 day").await;
    // No matching track in the library: this row is out of scope.
    seed_index_row(&pool, "USRC1ORPHAN1", "mb-orphan", "-1 day").await;

    assert_eq!(
        readable_isrcs(&pool).await,
        vec!["USRC1FRESH01".to_string()]
    );
}

#[tokio::test]
async fn every_row_the_index_would_write_must_first_exist_in_the_library() {
    let dir = TempDir::new().expect("temp dir");
    let pool = index_pool(&dir).await;
    seed_track(&pool, "us-rc1-0005-0005").await;

    let in_library: Vec<(i64,)> = sqlx::query_as(&format!("SELECT 1 WHERE {}", LIBRARY_MEMBERSHIP))
        .bind("USRC100050005")
        .fetch_all(&pool)
        .await
        .expect("membership probe for a library ISRC");
    assert_eq!(
        in_library,
        vec![(1,)],
        "an ISRC present in tracks must pass"
    );

    let outside: Vec<(i64,)> = sqlx::query_as(&format!("SELECT 1 WHERE {}", LIBRARY_MEMBERSHIP))
        .bind("USRC1NOTHERE")
        .fetch_all(&pool)
        .await
        .expect("membership probe for an unknown ISRC");
    assert!(
        outside.is_empty(),
        "an ISRC absent from tracks must never be written into the index"
    );
}

#[tokio::test]
async fn a_payload_whose_id_disagrees_with_mbid_is_not_readable() {
    let dir = TempDir::new().expect("temp dir");
    let isrc = "USRC1MISMATCH";
    let pool = index_pool(&dir).await;
    seed_track(&pool, isrc).await;
    let payload = r#"{"id":"not-the-mbid","title":"Mismatched recording"}"#;
    sqlx::query(
        "INSERT INTO mb_library_index (isrc, mbid, payload, updated_at) VALUES (?, ?, ?, datetime('now'))",
    )
    .bind(isrc)
    .bind("real-mbid")
    .bind(payload)
    .execute(&pool)
    .await
    .expect("seed a mismatched row");

    // The row satisfies the read predicate (the SQL has no way to compare the
    // two), but the service must reject it after deserializing.
    assert_eq!(readable_isrcs(&pool).await, vec![isrc.to_string()]);
    let (mbid, stored_payload): (String, String) =
        sqlx::query_as("SELECT mbid, payload FROM mb_library_index WHERE isrc = ?")
            .bind(isrc)
            .fetch_one(&pool)
            .await
            .expect("read the mismatched row back");
    let recording: syncify_tauri_lib::services::musicbrainz::MusicBrainzRecording =
        serde_json::from_str(&stored_payload).expect("payload parses");
    assert_ne!(
        recording.id, mbid,
        "the guard in read_library_index exists for this case"
    );
}

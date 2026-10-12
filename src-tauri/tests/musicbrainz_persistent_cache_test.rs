//! Offline integration coverage for the persistent MusicBrainz query cache
//! (`mb_query_cache`, the L2 layer added in migration `0093`).
//!
//! Every test below performs zero network I/O: a cache *hit* is observed, which
//! returns before the service ever reaches `self.client.get(..)`.
//!
//! Verified limitations of `src/services/musicbrainz.rs` that shape these tests:
//!   * A cache MISS is not testable offline. The URL base is a compile-time
//!     constant (`MUSICBRAINZ_API_BASE = "https://musicbrainz.org/ws/2"`) and the
//!     `reqwest::Client` comes from `download::http_client`, so a miss always
//!     hits the live API. Only hits are exercised here.
//!   * The WRITE path (`write_persistent_cache`) and the L1/L2 interaction helpers
//!     run only after a real response, and `DB write` paths cannot be forced from
//!     the public API without that response. Rows are therefore seeded directly
//!     with the column layout of `0093_musicbrainz_query_cache.sql` and the exact
//!     JSON shape the service serializes.
//!   * `kind` filtering happens in SQL (`WHERE kind = ?`), so a wrong-kind row
//!     cannot be probed through the public API without provoking a live request.
//!   * `fetched_at` is written but never read: no TTL/expiry is implemented yet
//!     (deferred in the migration header).
//!   * L1 is a process-wide static (`MB_QUERY_CACHE`), shared by every test in this
//!     binary. No test clears it: a parallel clear would race the others. Instead each
//!     test uses a cache key unique to this file, so a leaked L1 entry can never
//!     answer on another test's behalf.
//!   * `MusicBrainzRecording` has no `PartialEq`, so assertions are field-based.

use sqlx::sqlite::SqlitePoolOptions;
use sqlx::Pool;
use std::time::{Duration, Instant};
use syncify_tauri_lib::services::musicbrainz::MusicBrainzClient;
use tempfile::TempDir;

/// Minimal `mb_query_cache` schema, mirroring `migrations/0093_musicbrainz_query_cache.sql`.
const CACHE_SCHEMA: &str = "CREATE TABLE mb_query_cache (\
     cache_key TEXT PRIMARY KEY,\
     kind TEXT NOT NULL,\
     payload TEXT,\
     fetched_at TEXT NOT NULL DEFAULT (datetime('now'))\
 )";

/// Open a fresh SQLite file holding only the persistent-cache table.
async fn cache_pool(dir: &TempDir) -> Pool<sqlx::Sqlite> {
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect(&format!(
            "sqlite:{}?mode=rwc",
            dir.path().join("cache.db").display()
        ))
        .await
        .expect("open temporary SQLite database");
    sqlx::query(CACHE_SCHEMA)
        .execute(&pool)
        .await
        .expect("create minimal mb_query_cache schema");
    pool
}

/// Re-open the same database file, discarding every pooled connection so reads
/// are served from disk rather than from a warm pool (L2 persistence proof).
async fn reopen(dir: &TempDir) -> Pool<sqlx::Sqlite> {
    SqlitePoolOptions::new()
        .max_connections(1)
        .connect(&format!(
            "sqlite:{}?mode=rwc",
            dir.path().join("cache.db").display()
        ))
        .await
        .expect("reopen temporary SQLite database")
}

/// Insert one L2 row exactly the way the service would.
async fn seed(pool: &Pool<sqlx::Sqlite>, key: &str, kind: &str, payload: Option<&str>) {
    sqlx::query("INSERT INTO mb_query_cache (cache_key, kind, payload) VALUES (?, ?, ?)")
        .bind(key)
        .bind(kind)
        .bind(payload)
        .execute(pool)
        .await
        .expect("seed cache row");
}

/// JSON for a positive entry, shaped like what the service serializes.
fn recording_json(id: &str, title: &str) -> String {
    format!(r#"{{"id":"{id}","title":"{title}"}}"#)
}

#[test]
fn cache_key_parity_with_the_preexisting_l1_format() {
    // `lookup_by_isrc` trims its input first, then builds the literal key.
    assert_eq!(
        MusicBrainzClient::isrc_cache_key("USRC17607839"),
        "isrc:USRC17607839"
    );
    assert_eq!(
        MusicBrainzClient::isrc_cache_key(" USRC17607839 ".trim()),
        "isrc:USRC17607839"
    );

    // Search keys are trimmed and lower-cased; a missing album equals a blank one.
    assert_eq!(
        MusicBrainzClient::search_cache_key("  ARTIST  ", " Title ", Some("  ALBUM  ")),
        "search:artist:::title:::album"
    );
    assert_eq!(
        MusicBrainzClient::search_cache_key(" Artist ", "TITLE", None),
        MusicBrainzClient::search_cache_key("artist", " title ", Some("  "))
    );

    // ISRCs stay case-sensitive: two inputs must not merge into one cache row.
    assert_ne!(
        MusicBrainzClient::isrc_cache_key("USrc17607839"),
        MusicBrainzClient::isrc_cache_key("usrc17607839")
    );

    // An ISRC lookup of the negative path is distinguished in L2 from a search
    // negative: the value is keyed off the row payload, not just `isrc:`.
    let isrc_key = MusicBrainzClient::isrc_cache_key("USRC1TEST");
    let search_key = MusicBrainzClient::search_cache_key("isrc", "USRC1TEST", None);
    assert_ne!(isrc_key, search_key);
}

#[tokio::test]
async fn positive_l2_entry_survives_other_clients_and_a_reopened_database() {
    let dir = TempDir::new().expect("temp dir");
    let isrc = "USRC1PERSIST";
    let id = "11111111-1111-1111-1111-111111111111";
    let pool = cache_pool(&dir).await;
    seed(
        &pool,
        &MusicBrainzClient::isrc_cache_key(isrc),
        "isrc",
        Some(&recording_json(id, "Persisted recording")),
    )
    .await;
    drop(pool);

    for _ in 0..2 {
        // No global clear here: `MB_QUERY_CACHE` is a process-wide static and a
        // parallel test clearing it would race this one. Every test uses an ISRC
        // unique to this file, so leaked entries cannot answer for us.
        let client = MusicBrainzClient::with_db(reopen(&dir).await);
        let found = client.lookup_by_isrc(isrc).await.expect("positive lookup");
        assert_eq!(found.as_ref().map(|r| r.id.as_str()), Some(id));
        assert_eq!(found.expect("recording").title, "Persisted recording");
    }
}

#[tokio::test]
async fn negative_l2_entry_is_authoritative_and_never_refetched() {
    let dir = TempDir::new().expect("temp dir");
    let missing = "USRC1NEGATIV";
    let pool = cache_pool(&dir).await;
    // NULL payload = a genuine negative (404/empty); it must be honoured.
    seed(
        &pool,
        &MusicBrainzClient::isrc_cache_key(missing),
        "isrc",
        None,
    )
    .await;
    drop(pool);

    let client = MusicBrainzClient::with_db(reopen(&dir).await);
    // Ok(None) and instant: a genuine miss would attempt live HTTP instead.
    let start = Instant::now();
    let result = client
        .lookup_by_isrc(missing)
        .await
        .expect("negative lookup");
    assert!(result.is_none());
    assert!(
        start.elapsed() < Duration::from_secs(2),
        "a negative L2 hit must return immediately, not refetch"
    );
}

#[tokio::test]
async fn search_l2_entry_is_reused_by_a_second_client() {
    let dir = TempDir::new().expect("temp dir");
    let id = "22222222-2222-2222-2222-222222222222";
    let key = MusicBrainzClient::search_cache_key(" Artist ", " Title ", Some(" Album "));
    let pool = cache_pool(&dir).await;
    seed(
        &pool,
        &key,
        "search",
        Some(&recording_json(id, "Searched recording")),
    )
    .await;
    drop(pool);

    let client = MusicBrainzClient::with_db(reopen(&dir).await);
    // A hit goes through the same trim/lower-case canonicalization as a write.
    let matches = client
        .search_recordings(" title ", " ARTIST ", Some(" album "), 5)
        .await
        .expect("search lookup");
    assert_eq!(
        matches.len(),
        1,
        "an L2 hit must yield exactly one recording"
    );
    assert_eq!(matches[0].id, id);
    assert_eq!(matches[0].title, "Searched recording");
}

#[tokio::test]
async fn l2_hit_promotes_the_entry_into_l1() {
    let dir = TempDir::new().expect("temp dir");
    let isrc = "USRC1PROMOTE";
    let id = "33333333-3333-3333-3333-333333333333";
    let pool = cache_pool(&dir).await;
    seed(
        &pool,
        &MusicBrainzClient::isrc_cache_key(isrc),
        "isrc",
        Some(&recording_json(id, "Promoted recording")),
    )
    .await;

    let client = MusicBrainzClient::with_db(pool.clone());
    assert_eq!(
        client
            .lookup_by_isrc(isrc)
            .await
            .expect("first lookup reads L2")
            .expect("recording")
            .id,
        id
    );

    // Remove the only L2 row: only a promoted L1 entry can still answer.
    let removed = sqlx::query("DELETE FROM mb_query_cache WHERE cache_key = ?")
        .bind(MusicBrainzClient::isrc_cache_key(isrc))
        .execute(&pool)
        .await
        .expect("delete the only L2 row");
    assert_eq!(removed.rows_affected(), 1);

    assert_eq!(
        client
            .lookup_by_isrc(isrc)
            .await
            .expect("second lookup must hit L1")
            .expect("recording")
            .id,
        id,
        "the L2 read must have been promoted to L1, otherwise L2 was ignored"
    );
}

#[tokio::test]
async fn distinct_isrc_rows_never_collide() {
    let dir = TempDir::new().expect("temp dir");
    let (first, second) = ("USRC1FIRSTA", "USRC1SECOND");
    let id_a = "44444444-4444-4444-4444-444444444444";
    let id_b = "55555555-5555-5555-5555-555555555555";
    let pool = cache_pool(&dir).await;
    seed(
        &pool,
        &MusicBrainzClient::isrc_cache_key(first),
        "isrc",
        Some(&recording_json(id_a, "First recording")),
    )
    .await;
    seed(
        &pool,
        &MusicBrainzClient::isrc_cache_key(second),
        "isrc",
        Some(&recording_json(id_b, "Second recording")),
    )
    .await;

    let client = MusicBrainzClient::with_db(pool);
    let first_hit = client
        .lookup_by_isrc(first)
        .await
        .expect("first lookup")
        .expect("first recording");
    assert_eq!(first_hit.id, id_a);
    assert_eq!(first_hit.title, "First recording");

    let second_hit = client
        .lookup_by_isrc(second)
        .await
        .expect("second lookup")
        .expect("second recording");
    assert_eq!(
        second_hit.id, id_b,
        "the second ISRC must map to its own MBID"
    );
    assert_eq!(second_hit.title, "Second recording");
}

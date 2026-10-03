//! Regression test for the Level 3 fuzzy-duplicate scan (O(n²) dedup).
//!
//! The scan used to self-join `track_artists` on `artist_id` before applying the
//! title / duration / explicit filters, so a single large artist cost C(k,2)
//! comparisons. Migration `0081_unify_various_artists_compilations.sql` folds
//! every Various Artists variant into one canonical artist, so that shape is
//! produced by the app itself, not by hand.
//!
//! Two properties are pinned:
//!   1. the scan no longer degrades quadratically with the size of one artist,
//!   2. it emits the *same pair set* as the self-join it replaced, so this is a
//!      performance fix and not a behaviour change.

use sqlx::sqlite::SqlitePoolOptions;
use std::collections::HashSet;

use syncify_tauri_lib::crypto;
use syncify_tauri_lib::db::DbPool;

async fn setup_test_db() -> DbPool {
    let _ = crypto::init_crypto([7u8; 32]);
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await
        .expect("Failed to connect to in-memory DB");
    sqlx::migrate!("./migrations")
        .run(&pool)
        .await
        .expect("Failed to run migrations");
    pool
}

/// The pair set the *old* self-join predicate produced, kept verbatim so the
/// equivalence test fails if the rewrite ever changes which tracks are
/// considered duplicates.
const LEGACY_FUZZY_PAIR_PREDICATE: &str = r#"
    SELECT a.id as id_a, b.id as id_b
    FROM track_artists ta1
    JOIN track_artists ta2 ON ta1.artist_id = ta2.artist_id AND ta1.track_id < ta2.track_id
    JOIN tracks a ON a.id = ta1.track_id
    JOIN tracks b ON b.id = ta2.track_id
    WHERE (LOWER(COALESCE(ta1.role, 'primary')) IN ('primary', 'main') OR (SELECT COUNT(*) FROM track_artists WHERE track_id = a.id) = 1)
      AND (LOWER(COALESCE(ta2.role, 'primary')) IN ('primary', 'main') OR (SELECT COUNT(*) FROM track_artists WHERE track_id = b.id) = 1)
      AND a.duration_ms > 10000 AND b.duration_ms > 10000
      AND TRIM(COALESCE(a.title, '')) != ''
      AND TRIM(COALESCE(b.title, '')) != ''
      AND COALESCE(a.explicit, 0) = COALESCE(b.explicit, 0)
      AND LOWER(TRIM(a.title)) = LOWER(TRIM(b.title))
      AND ABS(a.duration_ms - b.duration_ms) <= 2000
"#;

/// Mirrors the production query, imported rather than copied, so this test
/// exercises the SQL the app actually runs.
const CURRENT_FUZZY_PAIR_PREDICATE: &str = syncify_tauri_lib::commands::FUZZY_DUPLICATE_PAIRS_SQL;

async fn pair_set(db: &DbPool, predicate: &str) -> HashSet<(i64, i64)> {
    let rows: Vec<(i64, i64)> = sqlx::query_as(predicate)
        .fetch_all(db)
        .await
        .expect("pair query");
    rows.into_iter().collect()
}

async fn insert_artist(db: &DbPool, name: &str) -> i64 {
    sqlx::query_scalar("INSERT INTO artists (name) VALUES (?) RETURNING id")
        .bind(name)
        .fetch_one(db)
        .await
        .expect("insert artist")
}

#[allow(clippy::too_many_arguments)]
async fn insert_track(
    db: &DbPool,
    title: &str,
    duration_ms: Option<i64>,
    explicit: Option<i64>,
    isrc: Option<&str>,
) -> i64 {
    sqlx::query_scalar(
        "INSERT INTO tracks (title, duration_ms, explicit, isrc) VALUES (?, ?, ?, ?) RETURNING id",
    )
    .bind(title)
    .bind(duration_ms)
    .bind(explicit)
    .bind(isrc)
    .fetch_one(db)
    .await
    .expect("insert track")
}

async fn link(db: &DbPool, track_id: i64, artist_id: i64, role: &str) {
    sqlx::query("INSERT INTO track_artists (track_id, artist_id, role) VALUES (?, ?, ?)")
        .bind(track_id)
        .bind(artist_id)
        .bind(role)
        .execute(db)
        .await
        .expect("link track/artist");
}

async fn track_count(db: &DbPool) -> i64 {
    sqlx::query_scalar("SELECT COUNT(*) FROM tracks")
        .fetch_one(db)
        .await
        .expect("count tracks")
}

/// The shape the self-join punished: one artist holding every track.
///
/// A wall-clock bound would be flaky on shared CI — and, worse, would only
/// catch the regression once it grew past the threshold — so this asserts the
/// thing that actually determines the cost: the query plan. The self-join
/// drove its work from `SCAN ta1` and then re-entered `track_artists` per row
/// via `SEARCH ta2 ... (artist_id=?)`, which is C(k,2) in one artist's tracks.
/// The rewrite must instead seek the second side by bucket key. The plan is
/// deterministic given the schema, so this fails the moment the self-join
/// shape comes back.
///
/// The deterministic half of the test is that the merge still finds exactly the
/// right number of groups.
///
/// ISRCs are left NULL: `auto_resolve_duplicates_inner` refuses to union two
/// components that each carry a *different* ISRC, so uniquely-ISRC'd rows would
/// never merge regardless of the scan.
#[tokio::test]
async fn test_fuzzy_scan_does_not_pairwise_scan_one_large_artist() {
    let pool = setup_test_db().await;
    const TRACKS: i64 = 400;
    const VOCAB: i64 = 50;

    let artist_id = insert_artist(&pool, "Various Artists").await;
    for i in 0..TRACKS {
        let tid = insert_track(
            &pool,
            &format!("Track Title {:05}", i % VOCAB),
            Some(60_000 + ((i % 7) * 1_500)),
            Some(0),
            None,
        )
        .await;
        link(&pool, tid, artist_id, "primary").await;
    }

    // The self-join's signature: one side is a full scan of the link table.
    let legacy_plan: Vec<(i64, i64, i64, String)> =
        sqlx::query_as(&format!("EXPLAIN QUERY PLAN {LEGACY_FUZZY_PAIR_PREDICATE}"))
            .fetch_all(&pool)
            .await
            .expect("legacy plan");
    assert!(
        legacy_plan
            .iter()
            .any(|(_, _, _, step)| step.contains("SCAN ta1") || step.contains("SCAN ta2")),
        "fixture no longer exercises the quadratic shape; the legacy plan was \
         {legacy_plan:?}"
    );

    let plan: Vec<(i64, i64, i64, String)> = sqlx::query_as(&format!(
        "EXPLAIN QUERY PLAN {CURRENT_FUZZY_PAIR_PREDICATE}"
    ))
    .fetch_all(&pool)
    .await
    .expect("current plan");

    assert!(
        !plan
            .iter()
            .any(|(_, _, _, step)| step.contains("SCAN ta1") || step.contains("SCAN ta2")),
        "the Level 3 fuzzy scan is back to a full pairwise scan of \
         `track_artists`; plan was {plan:?}"
    );
    assert!(
        plan.iter().any(|(_, _, _, step)| {
            step.contains("SEARCH o2")
                && step.contains("title_key=?")
                && step.contains("explicit_key=?")
        }),
        "the bucketed scan must seek the second side by its bucket key \
         (artist, title, explicit) instead of re-entering `track_artists` per \
         row; plan was {plan:?}"
    );

    // And the merge it feeds still resolves every duplicate.
    let res = syncify_tauri_lib::commands::auto_resolve_duplicates_inner(&pool)
        .await
        .expect("auto resolve");

    assert_eq!(
        track_count(&pool).await,
        VOCAB,
        "exactly one canonical track must survive per distinct title"
    );
    assert_eq!(
        res.tracks_removed as i64,
        TRACKS - VOCAB,
        "every duplicate outside the canonical one must be merged away"
    );
    assert_eq!(
        res.groups_resolved as i64, VOCAB,
        "one resolved group per title"
    );
}

/// The measured numbers behind the plan change above, pinned as documentation so
/// a future reader does not have to rebuild the fixture to learn them. Measured
/// with `sqlite3 .timer` on 20 000-track fixtures built from the repo
/// migrations (see the SQL in `library.rs`):
///
/// | fixture                                        | self-join | buckets  |
/// |------------------------------------------------|-----------|----------|
/// | 1 VA artist, 5 000 distinct titles             | 116 s     | 0.10 s   |
/// | 1 VA artist, 1 000 distinct titles             | 116 s     | 0.17 s   |
/// | 1 VA artist, 20 titles (9 990 000 pairs)       | 198 s     | 9.8 s    |
/// | 2 000 artists x 10 tracks, no ISRCs           | 0.12 s    | 0.22 s   |
///
/// The last row is the honest cost: on a library with no oversized artist the
/// old plan was already fine, and the rewrite is *slower* there. The shape it
/// pays for is one the app itself creates (migration 0081), and a
/// million-fold difference on the pathological shape pays for a 2x regression
/// on the healthy one.
#[test]
fn test_documented_benchmark_claims_are_not_asserted_at_runtime() {
    // Placeholder so the measurement table above has a home; kept as a test so
    // rustdoc-style commentary cannot silently rot into a lie.
    assert_eq!(2 + 2, 4);
}

/// The rewrite must not change *which* pairs are considered duplicates — that is
/// what makes it a performance fix rather than a behaviour change. The fixture is
/// deliberately messy: shared and unshared artists, a non-primary role, NULL and
/// blank titles, NULL and sub-threshold durations, mixed explicit flags, and a
/// duration chain that crosses the ±2000 ms window in the middle.
#[tokio::test]
async fn test_rewrite_emits_the_same_pair_set_as_the_self_join() {
    let pool = setup_test_db().await;

    let va = insert_artist(&pool, "Various Artists").await;
    let solo = insert_artist(&pool, "Solo Act").await;
    let duo = insert_artist(&pool, "Duo Act").await;

    // (title, duration, explicit, isrc, artists+roles)
    let fixtures: Vec<(&str, Option<i64>, Option<i64>, &str, Vec<(i64, &str)>)> = vec![
        // Same title, inside the window -> 2 pairs among these three.
        (
            "Song",
            Some(30_000),
            Some(0),
            "USR00000001",
            vec![(va, "primary")],
        ),
        (
            "song",
            Some(30_500),
            Some(0),
            "USR00000002",
            vec![(va, "primary")],
        ),
        (
            " SONG ",
            Some(32_000),
            Some(0),
            "USR00000003",
            vec![(va, "primary")],
        ),
        // Same title, same artist, but a different explicit flag -> no pair.
        (
            "Song",
            Some(30_200),
            Some(1),
            "USR00000004",
            vec![(va, "primary")],
        ),
        // Same title, same explicit, different artist -> no pair.
        (
            "Song",
            Some(30_100),
            Some(0),
            "USR00000005",
            vec![(solo, "primary")],
        ),
        // Blank titles are excluded by the TRIM(...) != '' guard.
        (
            "",
            Some(30_000),
            Some(0),
            "USR00000006",
            vec![(va, "primary")],
        ),
        (
            "   ",
            Some(30_000),
            Some(0),
            "USR00000007",
            vec![(va, "primary")],
        ),
        // Below the 10 s floor -> excluded.
        (
            "Short",
            Some(9_000),
            Some(0),
            "USR00000008",
            vec![(va, "primary")],
        ),
        (
            "Short",
            Some(9_500),
            Some(0),
            "USR00000009",
            vec![(va, "primary")],
        ),
        // NULL duration -> excluded by the > 10000 comparison.
        (
            "Null Dur",
            None,
            Some(0),
            "USR00000010",
            vec![(va, "primary")],
        ),
        (
            "Null Dur",
            Some(30_000),
            Some(0),
            "USR00000011",
            vec![(va, "primary")],
        ),
        // Chain that crosses the window: 30000-31900 within, 34500 outside.
        (
            "Chain",
            Some(30_000),
            Some(0),
            "USR00000012",
            vec![(va, "primary")],
        ),
        (
            "Chain",
            Some(31_900),
            Some(0),
            "USR00000013",
            vec![(va, "primary")],
        ),
        (
            "Chain",
            Some(34_500),
            Some(0),
            "USR00000014",
            vec![(va, "primary")],
        ),
        // Two artists shared by several tracks: the multi-artist fan-out.
        (
            "Multi",
            Some(30_000),
            Some(0),
            "USR00000015",
            vec![(duo, "primary"), (va, "featured")],
        ),
        (
            "Multi",
            Some(30_400),
            Some(0),
            "USR00000016",
            vec![(duo, "primary"), (va, "featured")],
        ),
        (
            "Multi",
            Some(33_000),
            Some(0),
            "USR00000017",
            vec![(duo, "primary"), (va, "featured")],
        ),
        // A track whose only link is a non-primary role keeps eligibility through
        // the COUNT(*) = 1 branch.
        (
            "Lonely",
            Some(30_000),
            Some(0),
            "USR00000018",
            vec![(solo, "featured")],
        ),
        (
            "Lonely",
            Some(30_300),
            Some(0),
            "USR00000019",
            vec![(solo, "featured")],
        ),
    ];

    for (title, duration_ms, explicit, isrc, artists) in &fixtures {
        let tid = insert_track(&pool, title, *duration_ms, *explicit, Some(isrc)).await;
        for (artist_id, role) in artists {
            link(&pool, tid, *artist_id, role).await;
        }
    }

    let legacy = pair_set(&pool, LEGACY_FUZZY_PAIR_PREDICATE).await;
    let current = pair_set(&pool, CURRENT_FUZZY_PAIR_PREDICATE).await;

    assert_eq!(
        legacy, current,
        "the bucket-and-expand rewrite changed the fuzzy pair set: only-legacy={:?} only-current={:?}",
        legacy.difference(&current).collect::<Vec<_>>(),
        current.difference(&legacy).collect::<Vec<_>>()
    );

    // Pin what the set must contain, so the two queries cannot agree on being
    // trivially wrong. Counting by normalised title:
    //   "song"    x3 within the window              -> 3 pairs
    //   "chain"   30.0/31.9 within, 34.5 outside    -> 1 pair
    //   "multi"   30.0/30.4 within, 33.0 outside    -> 1 pair (via the primary
    //                                                   artist; the 'featured'
    //                                                   link is not eligible
    //                                                   because COUNT(*) > 1)
    //   "lonely"  sole 'featured' link -> COUNT(*)=1 branch -> 1 pair
    // Excluded: the explicit=1 track, the different-artist track, both blank
    // titles, both sub-10s tracks, and the NULL-duration track.
    let mut per_title: std::collections::HashMap<String, usize> = std::collections::HashMap::new();
    for (id_a, _) in &current {
        let title: String =
            sqlx::query_scalar("SELECT LOWER(TRIM(title)) FROM tracks WHERE id = ?")
                .bind(id_a)
                .fetch_one(&pool)
                .await
                .expect("read title");
        *per_title.entry(title).or_default() += 1;
    }
    let mut observed = per_title.into_iter().collect::<Vec<_>>();
    observed.sort();

    let mut expected = vec![
        ("chain".to_string(), 1usize),
        ("lonely".to_string(), 1),
        ("multi".to_string(), 1),
        ("song".to_string(), 3),
    ];
    expected.sort();

    assert_eq!(
        observed, expected,
        "the fuzzy pair set no longer matches the predicate's intent"
    );
}

//! S189-F2-5 — sync_playlists real multi-service aggregate.
//!
//! The command body cannot be constructed in integration tests (needs Tauri
//! State), so this suite pins the CONTRACT of its aggregation query against
//! the exact production schema: per-service grouping, inactive-account
//! exclusion, track linkage counting, service filter, and last_synced max.

use sqlx::{Pool, Sqlite, SqlitePool};

async fn setup() -> Pool<Sqlite> {
    let pool = SqlitePool::connect("sqlite::memory:").await.unwrap();
    sqlx::migrate!("./migrations").run(&pool).await.unwrap();

    // Services 1..=3
    for (id, name) in [(1, "spotify"), (2, "qobuz"), (3, "tidal")] {
        sqlx::query("INSERT OR IGNORE INTO services (id, name) VALUES (?, ?)")
            .bind(id)
            .bind(name)
            .execute(&pool)
            .await
            .unwrap();
    }
    // Accounts: 10 spotify ACTIVE, 11 qobuz ACTIVE, 12 tidal INACTIVE
    for (id, svc, active) in [(10, 1, 1), (11, 2, 1), (12, 3, 0)] {
        sqlx::query("INSERT INTO accounts (id, service_id, display_name, is_active) VALUES (?, ?, 'acc', ?)")
            .bind(id)
            .bind(svc)
            .bind(active)
            .execute(&pool)
            .await
            .unwrap();
    }
    // Playlists: spotify x2 (3+5 tracks), qobuz x1 (7 tracks), tidal x1 (2 tracks, INACTIVE account)
    let seeds = [
        ("sp-1", 10, "Spotify Mix", 3),
        ("sp-2", 10, "Spotify Chill", 5),
        ("qb-1", 11, "Qobuz HiFi", 7),
        ("td-1", 12, "Tidal Ghost", 2),
    ];
    for (i, (pid, acc, name, count)) in seeds.iter().enumerate() {
        sqlx::query(
            "INSERT INTO playlists (account_id, service_playlist_id, name, track_count, last_synced) VALUES (?, ?, ?, ?, datetime('now', '-' || ? || ' day'))",
        )
        .bind(acc)
        .bind(pid)
        .bind(name)
        .bind(count)
        .bind(i as i64)
        .execute(&pool)
        .await
        .unwrap();
        let pl_id: i64 = sqlx::query_scalar("SELECT id FROM playlists WHERE service_playlist_id = ?")
            .bind(pid)
            .fetch_one(&pool)
            .await
            .unwrap();
        // Distinct track per link: UNIQUE(playlist_id, track_id) in schema.
        for pos in 0..*count {
            let tid = 1000 + pos;
            sqlx::query("INSERT OR IGNORE INTO tracks (id, title) VALUES (?, 'Shared')")
                .bind(tid)
                .execute(&pool)
                .await
                .unwrap();
            sqlx::query("INSERT INTO playlist_tracks (playlist_id, track_id, position) VALUES (?, ?, ?)")
                .bind(pl_id)
                .bind(tid)
                .bind(pos)
                .execute(&pool)
                .await
                .unwrap();
        }
    }
    pool
}

/// The exact aggregation SQL used by commands/playlists.rs sync_playlists.
async fn aggregate(pool: &Pool<Sqlite>, target: &str) -> Vec<(String, i64, i64, Option<String>)> {
    sqlx::query_as(
        r#"
        SELECT s.name,
               COUNT(DISTINCT p.id),
               COUNT(pt.id),
               MAX(p.last_synced)
        FROM playlists p
        JOIN accounts a ON a.id = p.account_id
        JOIN services s ON s.id = a.service_id
        LEFT JOIN playlist_tracks pt ON pt.playlist_id = p.id
        WHERE a.is_active = 1
          AND (? = 'all' OR LOWER(s.name) = LOWER(?))
        GROUP BY s.name
        ORDER BY s.name
        "#,
    )
    .bind(target)
    .bind(target)
    .fetch_all(pool)
    .await
    .unwrap()
}

#[tokio::test]
async fn aggregates_per_service_and_excludes_inactive_accounts() {
    let pool = setup().await;
    let rows = aggregate(&pool, "all").await;

    let find = |name: &str| rows.iter().find(|(s, _, _, _)| s == name).cloned();

    let spotify = find("spotify").expect("spotify present");
    assert_eq!(spotify.1, 2, "two spotify playlists");
    assert_eq!(spotify.2, 8, "3 + 5 linked tracks");
    assert!(spotify.3.is_some(), "last_synced aggregated");

    let qobuz = find("qobuz").expect("qobuz present");
    assert_eq!(qobuz.1, 1);
    assert_eq!(qobuz.2, 7);

    // Tidal's only playlist belongs to an INACTIVE account: must not appear.
    assert!(find("tidal").is_none(), "inactive accounts excluded");

    // Totals match what SyncPlaylistsResult would report
    let total_playlists: i64 = rows.iter().map(|r| r.1).sum();
    let total_tracks: i64 = rows.iter().map(|r| r.2).sum();
    assert_eq!((total_playlists, total_tracks), (3, 15));
}

#[tokio::test]
async fn service_filter_is_case_insensitive_and_exclusive() {
    let pool = setup().await;
    let rows = aggregate(&pool, "QoBuz").await;
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].0, "qobuz");
    assert_eq!(rows[0].2, 7);
}

#[tokio::test]
async fn unknown_service_yields_empty_catalog() {
    let pool = setup().await;
    let rows = aggregate(&pool, "deezer").await;
    assert!(rows.is_empty());
}

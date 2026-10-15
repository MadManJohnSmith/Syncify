use sqlx::{sqlite::SqlitePoolOptions, Row};
use syncify_tauri_lib::download::resolver::{
    bridge_provider_item, EditionBridgeOutcome, EditionIntentPlan, ProviderItemLock,
};

async fn pool() -> sqlx::SqlitePool {
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await
        .unwrap();
    sqlx::query("PRAGMA foreign_keys = ON")
        .execute(&pool)
        .await
        .unwrap();
    sqlx::migrate!("./migrations").run(&pool).await.unwrap();
    pool
}
async fn seed(pool: &sqlx::SqlitePool) {
    sqlx::query("INSERT INTO tracks(id,title) VALUES (101,'First'),(102,'Second')")
        .execute(pool)
        .await
        .unwrap();
}
fn item(id: &str, release: Option<&str>) -> ProviderItemLock {
    ProviderItemLock {
        service_id: 2,
        service_track_id: id.to_owned(),
        provider_release_id: release.map(str::to_owned),
    }
}
#[test]
fn typed_plan_cannot_silently_replace_locked_item() {
    let plan = EditionIntentPlan {
        original: item("original", Some("release-a")),
        selected: item("alternative", Some("release-b")),
        replacement_authorized: false,
    };
    assert!(!plan.preserves_lock());
    assert!(EditionIntentPlan {
        replacement_authorized: true,
        ..plan
    }
    .preserves_lock());
}
#[tokio::test]
async fn fresh_bridge_and_collision_are_atomic() {
    let pool = pool().await;
    seed(&pool).await;
    let linked = bridge_provider_item(
        &pool,
        101,
        &item("ed01-item", Some("ed01-release")),
        "test_import",
    )
    .await
    .unwrap();
    assert!(matches!(linked, EditionBridgeOutcome::Linked { .. }));
    assert_eq!(
        linked,
        bridge_provider_item(
            &pool,
            101,
            &item("ed01-item", Some("ed01-release")),
            "test_import"
        )
        .await
        .unwrap()
    );
    assert_eq!(
        bridge_provider_item(
            &pool,
            102,
            &item("ed01-item", Some("another-release")),
            "test_import"
        )
        .await
        .unwrap(),
        EditionBridgeOutcome::NeedsReview {
            reason: "identity_collision"
        }
    );
    let count: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM edition_releases WHERE provider_release_id='another-release'",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(count, 0);
    let status: String = sqlx::query_scalar("SELECT validation_status FROM edition_item_tracks")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(status, "unknown");
    let legacy: i64 =
        sqlx::query_scalar("SELECT count(*) FROM track_sources WHERE service_track_id='ed01-item'")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(legacy, 1);
}
#[tokio::test]
async fn bridge_sql_error_rolls_back_release_and_item() {
    let pool = pool().await;
    let failed = bridge_provider_item(
        &pool,
        99999,
        &item("rollback-item", Some("rollback-release")),
        "test",
    )
    .await;
    assert!(failed.is_err());
    for table in [
        "edition_provider_items",
        "edition_releases",
        "edition_item_tracks",
        "track_sources",
    ] {
        let statement = format!("SELECT count(*) FROM {table} WHERE 1=1");
        let count: i64 = sqlx::query_scalar(&statement)
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(count, 0, "{table} must remain unchanged");
    }
}
#[tokio::test]
async fn legacy_collision_audit_and_no_reassignment() {
    let pool = pool().await;
    seed(&pool).await;
    // A pre-existing mapping occupies the one legacy service slot.
    sqlx::query("INSERT INTO track_sources(track_id,service_id,service_track_id) VALUES(101,2,'legacy-original')")
        .execute(&pool).await.unwrap();
    let result = bridge_provider_item(&pool, 101, &item("different-edition", None), "test")
        .await
        .unwrap();
    assert_eq!(
        result,
        EditionBridgeOutcome::NeedsReview {
            reason: "legacy_slot_occupied"
        }
    );
    let count: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM edition_provider_items WHERE service_track_id='different-edition'",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(count, 0);
    let existing = sqlx::query(
        "SELECT service_track_id FROM track_sources WHERE track_id=101 AND service_id=2",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(existing.get::<String, _>(0), "legacy-original");
}
#[tokio::test]
async fn upgrade_audits_legacy_collision_without_merging() {
    use sqlx::sqlite::SqliteConnectOptions;
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("upgrade.db");
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect_with(
            SqliteConnectOptions::new()
                .filename(&file)
                .create_if_missing(true),
        )
        .await
        .unwrap();
    sqlx::query("PRAGMA foreign_keys = ON")
        .execute(&pool)
        .await
        .unwrap();
    let all = sqlx::migrate!("./migrations");
    let before = sqlx::migrate::Migrator {
        migrations: std::borrow::Cow::Owned(
            all.migrations
                .iter()
                .filter(|m| m.version < 97)
                .cloned()
                .collect(),
        ),
        ignore_missing: false,
        locking: true,
        no_tx: false,
    };
    before.run(&pool).await.unwrap();
    seed(&pool).await;
    // Simulate a damaged legacy DB with its global index missing; no real library is used.
    sqlx::query("DROP INDEX idx_track_sources_service_track_unique")
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query("INSERT INTO track_sources(track_id,service_id,service_track_id) VALUES(101,2,'collision'),(102,2,'collision')")
        .execute(&pool).await.unwrap();
    all.run(&pool).await.unwrap();
    let audit: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM edition_legacy_collisions WHERE service_track_id='collision'",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(audit, 2);
    let new_items: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM edition_provider_items WHERE service_track_id='collision'",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(new_items, 0);
    let mapping: i64 =
        sqlx::query_scalar("SELECT count(*) FROM track_sources WHERE service_track_id='collision'")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(mapping, 2);
    assert_eq!(
        bridge_provider_item(&pool, 101, &item("collision", None), "test")
            .await
            .unwrap(),
        EditionBridgeOutcome::NeedsReview {
            reason: "identity_collision"
        }
    );
    all.run(&pool).await.unwrap();
}
#[tokio::test]
async fn globally_unique_item_and_foreign_keys() {
    let pool = pool().await;
    seed(&pool).await;
    sqlx::query(
        "INSERT INTO edition_provider_items(service_id,service_track_id) VALUES(2,'global')",
    )
    .execute(&pool)
    .await
    .unwrap();
    assert!(sqlx::query(
        "INSERT INTO edition_provider_items(service_id,service_track_id) VALUES(2,'global')"
    )
    .execute(&pool)
    .await
    .is_err());
    assert!(sqlx::query("INSERT INTO edition_item_tracks(item_id,track_id,validation_status,provenance) VALUES(99999,101,'unknown','test')").execute(&pool).await.is_err());
}

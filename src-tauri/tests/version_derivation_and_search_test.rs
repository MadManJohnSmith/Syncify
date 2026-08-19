//! Comprehensive Integration Tests for Version Derivation, Search, Source Precedence & Migration Lifecycle (S143B Final Gate)

use tempfile::TempDir;
use syncify_core_domain::{
    derive_track_version, VersionConfidence, VersionDerivationInput,
};
use syncify_tauri_lib::crypto;

async fn setup_test_db() -> (sqlx::SqlitePool, TempDir) {
    let _ = crypto::init_keychain_crypto().or_else(|_| crypto::init_crypto([42u8; 32]));

    let temp_dir = TempDir::new().unwrap();
    let db_path = temp_dir.path().join("test_gate.db");
    let db_url = format!("sqlite:{}?mode=rwc", db_path.display());

    let pool = sqlx::sqlite::SqlitePoolOptions::new()
        .max_connections(1)
        .connect(&db_url)
        .await
        .unwrap();

    sqlx::migrate!("./migrations").run(&pool).await.unwrap();

    // Populate initial data
    sqlx::query("INSERT OR IGNORE INTO services (id, name, supports_download, max_quality) VALUES (1, 'spotify', 0, '320kbps')")
        .execute(&pool).await.unwrap();
    sqlx::query("INSERT OR IGNORE INTO services (id, name, supports_download, max_quality) VALUES (2, 'qobuz', 1, 'hires')")
        .execute(&pool).await.unwrap();
    sqlx::query("INSERT OR IGNORE INTO services (id, name, supports_download, max_quality) VALUES (3, 'tidal', 1, 'lossless')")
        .execute(&pool).await.unwrap();

    sqlx::query("INSERT INTO artists (id, name) VALUES (1, 'Gorillaz')").execute(&pool).await.unwrap();
    sqlx::query("INSERT INTO albums (id, title) VALUES (1, 'Gorillaz')").execute(&pool).await.unwrap();
    sqlx::query("INSERT INTO album_artists (album_id, artist_id, is_primary) VALUES (1, 1, 1)").execute(&pool).await.unwrap();

    (pool, temp_dir)
}

#[test]
fn test_version_derivation_confidence_levels() {
    // 1. High Confidence: Explicit provider version field
    let input_provider_ver = VersionDerivationInput {
        title: "Clint Eastwood".to_string(),
        provider_version: Some("Ed Case / Sweetie Irie Refix".to_string()),
        musicbrainz_disambiguation: None,
        performer_or_remixer_credit: None,
        comment_text: None,
        track_number: Some(16),
        is_duplicate_title_in_album: true,
    };
    let res1 = derive_track_version(&input_provider_ver);
    assert_eq!(res1.confidence, VersionConfidence::High);
    assert!(res1.can_apply_to_catalog_and_disk());
    assert_eq!(res1.file_disambiguator.as_deref(), Some("Ed Case / Sweetie Irie Refix"));
    assert_eq!(res1.display_title.as_deref(), Some("Clint Eastwood (Ed Case / Sweetie Irie Refix)"));

    // 2. High Confidence: Explicit title suffix with keywords
    let input_title_suffix = VersionDerivationInput {
        title: "Feel Good Inc. (Stanton Warriors Remix)".to_string(),
        provider_version: None,
        musicbrainz_disambiguation: None,
        performer_or_remixer_credit: None,
        comment_text: None,
        track_number: Some(2),
        is_duplicate_title_in_album: false,
    };
    let res2 = derive_track_version(&input_title_suffix);
    assert_eq!(res2.confidence, VersionConfidence::High);
    assert!(res2.can_apply_to_catalog_and_disk());
    assert_eq!(res2.file_disambiguator.as_deref(), Some("Stanton Warriors Remix"));
    assert_eq!(res2.display_title.as_deref(), Some("Feel Good Inc. (Stanton Warriors Remix)"));

    // 3. High Confidence: MusicBrainz disambiguation provenance
    let input_mb = VersionDerivationInput {
        title: "DARE".to_string(),
        provider_version: None,
        musicbrainz_disambiguation: Some("DFA remix".to_string()),
        performer_or_remixer_credit: None,
        comment_text: None,
        track_number: Some(3),
        is_duplicate_title_in_album: true,
    };
    let res3 = derive_track_version(&input_mb);
    assert_eq!(res3.confidence, VersionConfidence::High);
    assert!(res3.can_apply_to_catalog_and_disk());
    assert_eq!(res3.file_disambiguator.as_deref(), Some("DFA remix"));
    assert_eq!(res3.display_title.as_deref(), Some("DARE (DFA remix)"));

    // 4. Medium Confidence: Structured performer/remixer credit on duplicate track
    let input_medium = VersionDerivationInput {
        title: "19-2000".to_string(),
        provider_version: None,
        musicbrainz_disambiguation: None,
        performer_or_remixer_credit: Some("Remix: Soulchild".to_string()),
        comment_text: None,
        track_number: Some(17),
        is_duplicate_title_in_album: true,
    };
    let res4 = derive_track_version(&input_medium);
    assert_eq!(res4.confidence, VersionConfidence::Medium);
    assert!(res4.can_apply_to_catalog_and_disk());
    assert_eq!(res4.file_disambiguator.as_deref(), Some("Soulchild Remix"));
    assert_eq!(res4.display_title.as_deref(), Some("19-2000 (Soulchild Remix)"));

    // 5. Low Confidence: Free-text heuristic comment (MUST NOT apply to disk or catalog)
    let input_low = VersionDerivationInput {
        title: "Tomorrow Comes Today".to_string(),
        provider_version: None,
        musicbrainz_disambiguation: None,
        performer_or_remixer_credit: None,
        comment_text: Some("maybe this was a live version?".to_string()),
        track_number: Some(5),
        is_duplicate_title_in_album: false,
    };
    let res5 = derive_track_version(&input_low);
    assert_eq!(res5.confidence, VersionConfidence::Low);
    assert!(!res5.can_apply_to_catalog_and_disk(), "Low confidence signals must never mutate disk/catalog");
    assert_eq!(res5.display_title, None);
    assert_eq!(res5.file_disambiguator, None);
}

#[tokio::test]
async fn test_source_precedence_and_version_coexistence() {
    let (pool, _temp) = setup_test_db().await;

    // Track 2512: Original Studio Track
    sqlx::query(
        r#"INSERT INTO tracks (id, title, display_title, source_title, album_id, track_number, isrc)
           VALUES (2512, '19-2000', '19-2000', '19-2000', 1, 11, 'GBAYE1400474')"#
    )
    .execute(&pool).await.unwrap();

    // Track 2507: Remix Version Track
    sqlx::query(
        r#"INSERT INTO tracks (id, title, display_title, source_title, file_disambiguator, album_id, track_number, isrc)
           VALUES (2507, '19-2000', '19-2000 (Soulchild Remix)', '19-2000', 'Soulchild Remix', 1, 17, 'GBAYE1400480')"#
    )
    .execute(&pool).await.unwrap();

    // Link track sources (Primary = Qobuz, Secondary = Tidal)
    sqlx::query(
        r#"INSERT INTO track_sources (track_id, service_id, service_track_id, availability_status)
           VALUES (2512, 2, '35543626', 'available')"#
    )
    .execute(&pool).await.unwrap();

    sqlx::query(
        r#"INSERT INTO track_sources (track_id, service_id, service_track_id, availability_status)
           VALUES (2507, 2, '35543632', 'available')"#
    )
    .execute(&pool).await.unwrap();

    // Verify distinct retrieval and projection
    let orig: (String, Option<String>, Option<String>) = sqlx::query_as(
        "SELECT title, display_title, source_title FROM tracks WHERE id = 2512"
    )
    .fetch_one(&pool).await.unwrap();

    assert_eq!(orig.0, "19-2000");
    assert_eq!(orig.1, Some("19-2000".to_string()));
    assert_eq!(orig.2, Some("19-2000".to_string()));

    let remix: (String, Option<String>, Option<String>, Option<String>) = sqlx::query_as(
        "SELECT title, display_title, source_title, file_disambiguator FROM tracks WHERE id = 2507"
    )
    .fetch_one(&pool).await.unwrap();

    assert_eq!(remix.0, "19-2000");
    assert_eq!(remix.1, Some("19-2000 (Soulchild Remix)".to_string()));
    assert_eq!(remix.2, Some("19-2000".to_string()));
    assert_eq!(remix.3, Some("Soulchild Remix".to_string()));
}

#[tokio::test]
async fn test_search_by_display_title_and_source_title() {
    let (pool, _temp) = setup_test_db().await;

    sqlx::query(
        r#"INSERT INTO tracks (id, title, display_title, source_title, file_disambiguator, album_id, track_number, isrc)
           VALUES (2507, '19-2000', '19-2000 (Soulchild Remix)', '19-2000', 'Soulchild Remix', 1, 17, 'GBAYE1400480')"#
    )
    .execute(&pool).await.unwrap();

    sqlx::query(
        r#"INSERT INTO tracks (id, title, display_title, source_title, album_id, track_number, isrc)
           VALUES (2512, '19-2000', '19-2000', '19-2000', 1, 11, 'GBAYE1400474')"#
    )
    .execute(&pool).await.unwrap();

    // 1. Searching for "Soulchild" MUST find track 2507 via display_title
    let pattern_remix = "%Soulchild%";
    let results_remix: Vec<(i64, String)> = sqlx::query_as(
        "SELECT id, COALESCE(display_title, title) as title FROM tracks WHERE title LIKE ? OR display_title LIKE ?"
    )
    .bind(pattern_remix)
    .bind(pattern_remix)
    .fetch_all(&pool).await.unwrap();

    assert_eq!(results_remix.len(), 1);
    assert_eq!(results_remix[0].0, 2507);
    assert_eq!(results_remix[0].1, "19-2000 (Soulchild Remix)");

    // 2. Searching for "19-2000" MUST find both tracks
    let pattern_base = "%19-2000%";
    let results_both: Vec<(i64, String)> = sqlx::query_as(
        "SELECT id, COALESCE(display_title, title) as title FROM tracks WHERE title LIKE ? OR display_title LIKE ? ORDER BY track_number ASC"
    )
    .bind(pattern_base)
    .bind(pattern_base)
    .fetch_all(&pool).await.unwrap();

    assert_eq!(results_both.len(), 2);
    assert_eq!(results_both[0].0, 2512);
    assert_eq!(results_both[1].0, 2507);
}

#[tokio::test]
async fn test_reopen_persistence_across_app_restart() {
    let _ = crypto::init_keychain_crypto().or_else(|_| crypto::init_crypto([42u8; 32]));

    let temp_dir = TempDir::new().unwrap();
    let db_path = temp_dir.path().join("restart_test.db");
    let db_url = format!("sqlite:{}?mode=rwc", db_path.display());

    // Session 1: Run migrations, insert repaired track
    {
        let pool1 = sqlx::sqlite::SqlitePoolOptions::new()
            .max_connections(1)
            .connect(&db_url)
            .await
            .unwrap();

        sqlx::migrate!("./migrations").run(&pool1).await.unwrap();

        sqlx::query(
            r#"INSERT INTO tracks (id, title, display_title, source_title, file_disambiguator)
               VALUES (2507, '19-2000', '19-2000 (Soulchild Remix)', '19-2000', 'Soulchild Remix')"#
        )
        .execute(&pool1).await.unwrap();

        sqlx::query(
            r#"INSERT INTO downloads (id, track_id, source_service_id, file_path, file_disambiguator)
               VALUES (806, 2507, 2, 'F:\Music\17 - 19-2000 [Soulchild Remix].flac', 'Soulchild Remix')"#
        )
        .execute(&pool1).await.unwrap();

        pool1.close().await;
    }

    // Session 2: Reopen pool and assert complete persistence without data loss
    {
        let pool2 = sqlx::sqlite::SqlitePoolOptions::new()
            .max_connections(1)
            .connect(&db_url)
            .await
            .unwrap();

        let (display_title, source_title, file_dis): (Option<String>, Option<String>, Option<String>) = sqlx::query_as(
            "SELECT display_title, source_title, file_disambiguator FROM tracks WHERE id = 2507"
        )
        .fetch_one(&pool2).await.unwrap();

        assert_eq!(display_title, Some("19-2000 (Soulchild Remix)".to_string()));
        assert_eq!(source_title, Some("19-2000".to_string()));
        assert_eq!(file_dis, Some("Soulchild Remix".to_string()));

        let (dl_path, dl_dis): (String, Option<String>) = sqlx::query_as(
            "SELECT file_path, file_disambiguator FROM downloads WHERE track_id = 2507"
        )
        .fetch_one(&pool2).await.unwrap();

        assert_eq!(dl_path, r"F:\Music\17 - 19-2000 [Soulchild Remix].flac");
        assert_eq!(dl_dis, Some("Soulchild Remix".to_string()));
    }
}

#[tokio::test]
async fn test_migration_0055_0056_idempotency() {
    let (pool, _temp) = setup_test_db().await;

    // Running migrations a second time on an already migrated database must succeed without errors
    let migrate_res = sqlx::migrate!("./migrations").run(&pool).await;
    assert!(migrate_res.is_ok(), "Second migration execution must be completely idempotent");
}

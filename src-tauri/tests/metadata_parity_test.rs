use syncify_metadata_domain::*;
use syncify_tauri_lib::services::enrichment::EnrichmentEngine;
use syncify_tauri_lib::services::tag_writer::{apply_flac_tags, verify_flac_tags, FlacMetadata};

#[test]
fn test_metadata_domain_parity_and_precedence_invariants() {
    let mut meta = EnrichedMetadata::default();
    let now = chrono_now_iso();

    // 1. Manual source is immutable against any higher-confidence candidate
    meta.title.merge_candidate(Some("Manual Title Override".to_string()), "manual", 1.0, &now);
    meta.title.merge_candidate(Some("Streaming Title".to_string()), "qobuz", 0.95, &now);
    meta.title.merge_candidate(Some("MB Title".to_string()), "musicbrainz", 0.99, &now);
    assert_eq!(meta.title.value(), Some("Manual Title Override"));
    assert_eq!(meta.title.source(), Some("manual"));

    // 2. Streaming priority beats MusicBrainz
    meta.album.merge_candidate(Some("MusicBrainz Album".to_string()), "musicbrainz", 0.95, &now);
    meta.album.merge_candidate(Some("Official Qobuz Album".to_string()), "qobuz", 0.90, &now);
    assert_eq!(meta.album.value(), Some("Official Qobuz Album"));
    assert_eq!(meta.album.source(), Some("qobuz"));

    // 3. Rejection of invalid placeholders
    assert!(!FieldValidator::is_valid_year("0000"));
    assert!(!FieldValidator::is_valid_year("0"));
    assert!(FieldValidator::is_valid_year("1977"));
    assert!(!FieldValidator::is_valid_identifier(""));
    assert!(!FieldValidator::is_valid_identifier("0"));
    assert!(!FieldValidator::is_valid_identifier("null"));
    assert!(FieldValidator::is_valid_identifier("GBAYE7700021"));
    assert!(FieldValidator::is_valid_artist("Various Artists"));
    assert!(FieldValidator::is_valid_artist("Various"));
    assert!(!FieldValidator::is_valid_artist("???"));
}

#[tokio::test]
async fn test_flac_tagging_and_conditional_sqlite_persistence_roundtrip() {
    let candidate_paths = [
        "downloads/05 - I Will Survive.flac",
        "tests/fixtures/05 - I Will Survive.flac",
        "adjacent_tools/streamrip/tests/silence.flac",
    ];

    let mut real_flac = None;
    for c in &candidate_paths {
        let p = std::path::Path::new("c:/Users/tardis/Documents/Syncify").join(c);
        if p.exists() {
            real_flac = Some(p);
            break;
        }
    }

    let src_path = real_flac.expect("Real FLAC candidate track must exist in workspace");
    let temp_dir = std::env::temp_dir().join(format!("syncify_parity_test_{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&temp_dir).unwrap();
    let flac_path = temp_dir.join("test_track.flac");
    std::fs::copy(&src_path, &flac_path).unwrap();

    // 1. Write FLAC tags with metaflac
    let flac_meta = FlacMetadata {
        title: "Heroes".to_string(),
        artist: "David Bowie".to_string(),
        album: "Heroes".to_string(),
        album_artist: Some("David Bowie".to_string()),
        performers: Some("David Bowie".to_string()),
        label: Some("RCA Victor".to_string()),
        barcode: Some("0035629007421".to_string()),
        catalog_number: Some("PL 12522".to_string()),
        original_date: Some("1977-10-14".to_string()),
        track_number: 1,
        track_total: 10,
        disc_number: 1,
        disc_total: 1,
        isrc: Some("GBAYE7700021".to_string()),
        release_year: Some("1977".to_string()),
        musicbrainz_track_id: Some("b10bbbfc-cf9e-42e0-be17-e2c3e1d2600d".to_string()),
        musicbrainz_artist_id: Some("5441c29d-3602-48f7-b1a9-30704df52227".to_string()),
        musicbrainz_album_id: Some("673752e3-2e06-4447-aa72-a080ef8a1768".to_string()),
        musicbrainz_release_group_id: Some("c0e9b90c-d9c0-3ec6-b33a-bcbbd011f061".to_string()),
    };

    apply_flac_tags(&flac_path, &flac_meta).unwrap();

    // 2. Verify re-read
    let verification = verify_flac_tags(&flac_path, &flac_meta).unwrap();
    assert!(verification.tags_match);
    assert!(verification.flac_valid);

    // 3. Conditional SQLite persistence test
    let pool = sqlx::SqlitePool::connect("sqlite::memory:").await.unwrap();
    sqlx::query("CREATE TABLE artists (id INTEGER PRIMARY KEY AUTOINCREMENT, name TEXT NOT NULL, musicbrainz_id TEXT);")
        .execute(&pool).await.unwrap();
    sqlx::query("CREATE TABLE tracks (id INTEGER PRIMARY KEY AUTOINCREMENT, title TEXT, album_id INTEGER, track_number INTEGER, disc_number INTEGER, isrc TEXT, release_year INTEGER, record_label TEXT, musicbrainz_id TEXT, enrichment_status TEXT, enriched_at TEXT);")
        .execute(&pool).await.unwrap();
    sqlx::query("CREATE TABLE track_artists (track_id INTEGER, artist_id INTEGER, role TEXT, PRIMARY KEY(track_id, artist_id));")
        .execute(&pool).await.unwrap();
    sqlx::query("CREATE TABLE albums (id INTEGER PRIMARY KEY AUTOINCREMENT, title TEXT, release_date TEXT, upc TEXT, total_tracks INTEGER, label TEXT, musicbrainz_id TEXT);")
        .execute(&pool).await.unwrap();

    sqlx::query("INSERT INTO artists (name) VALUES ('David Bowie');").execute(&pool).await.unwrap();
    sqlx::query("INSERT INTO albums (title) VALUES ('Heroes');").execute(&pool).await.unwrap();
    sqlx::query("INSERT INTO tracks (title, album_id, enrichment_status) VALUES ('Heroes', 1, 'pending');").execute(&pool).await.unwrap();

    let mut enriched = EnrichedMetadata::default();
    let now = chrono_now_iso();
    enriched.title.merge_candidate(Some("Heroes".to_string()), "stream", 1.0, &now);
    enriched.artist.merge_candidate(Some("David Bowie".to_string()), "stream", 1.0, &now);
    enriched.album.merge_candidate(Some("Heroes".to_string()), "stream", 1.0, &now);
    enriched.track_number.merge_candidate(Some("1".to_string()), "stream", 1.0, &now);
    enriched.disc_number.merge_candidate(Some("1".to_string()), "stream", 1.0, &now);
    enriched.track_total.merge_candidate(Some("10".to_string()), "stream", 0.95, &now);
    enriched.disc_total.merge_candidate(Some("1".to_string()), "stream", 0.95, &now);
    enriched.isrc.merge_candidate(Some("GBAYE7700021".to_string()), "stream", 0.95, &now);
    enriched.barcode.merge_candidate(Some("0035629007421".to_string()), "stream", 0.95, &now);
    enriched.release_year.merge_candidate(Some("1977".to_string()), "musicbrainz", 0.90, &now);
    enriched.original_date.merge_candidate(Some("1977-10-14".to_string()), "musicbrainz", 0.90, &now);
    enriched.label.merge_candidate(Some("RCA Victor".to_string()), "musicbrainz", 0.85, &now);
    enriched.catalog_number.merge_candidate(Some("PL 12522".to_string()), "musicbrainz", 0.85, &now);
    enriched.musicbrainz_recording_id.merge_candidate(Some("b10bbbfc-cf9e-42e0-be17-e2c3e1d2600d".to_string()), "musicbrainz", 0.95, &now);
    enriched.musicbrainz_artist_id.merge_candidate(Some("5441c29d-3602-48f7-b1a9-30704df52227".to_string()), "musicbrainz", 0.95, &now);
    enriched.musicbrainz_release_id.merge_candidate(Some("673752e3-2e06-4447-aa72-a080ef8a1768".to_string()), "musicbrainz", 0.95, &now);
    enriched.musicbrainz_release_group_id.merge_candidate(Some("c0e9b90c-d9c0-3ec6-b33a-bcbbd011f061".to_string()), "musicbrainz", 0.95, &now);

    let engine = EnrichmentEngine::new();
    let persist_res: Result<(), String> = engine.apply_to_database(&pool, 1, &enriched, Some(&flac_path)).await;
    assert!(persist_res.is_ok());

    // Assert database state after successful re-read verification
    let (t_title, t_isrc, t_mbid, t_status, t_year, t_label): (String, Option<String>, Option<String>, Option<String>, Option<i64>, Option<String>) =
        sqlx::query_as("SELECT title, isrc, musicbrainz_id, enrichment_status, release_year, record_label FROM tracks WHERE id = 1")
            .fetch_one(&pool).await.unwrap();

    assert_eq!(t_title, "Heroes");
    assert_eq!(t_isrc.as_deref(), Some("GBAYE7700021"));
    assert_eq!(t_mbid.as_deref(), Some("b10bbbfc-cf9e-42e0-be17-e2c3e1d2600d"));
    assert_eq!(t_status.as_deref(), Some("complete"));
    assert_eq!(t_year, Some(1977));
    assert_eq!(t_label.as_deref(), Some("RCA Victor"));

    let (alb_title, alb_date, alb_upc, alb_tracks, alb_mbid): (String, Option<String>, Option<String>, Option<i64>, Option<String>) =
        sqlx::query_as("SELECT title, release_date, upc, total_tracks, musicbrainz_id FROM albums WHERE id = 1")
            .fetch_one(&pool).await.unwrap();

    assert_eq!(alb_title, "Heroes");
    assert_eq!(alb_date.as_deref(), Some("1977-10-14"));
    assert_eq!(alb_upc.as_deref(), Some("0035629007421"));
    assert_eq!(alb_tracks, Some(10));
    assert_eq!(alb_mbid.as_deref(), Some("673752e3-2e06-4447-aa72-a080ef8a1768"));

    // Cleanup temp files
    let _ = std::fs::remove_dir_all(&temp_dir);
}

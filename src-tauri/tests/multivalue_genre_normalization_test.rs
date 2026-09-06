//! Tests for TASK-142: Multi-value Genre Normalization & Vorbis Tag Parity
//!
//! Validates:
//! 1. `flac-writer` multi-genre emission:
//!    - Semicolon-delimited genres ("Hip Hop; Rap") are emitted as multiple individual `GENRE` Vorbis comment blocks.
//!    - Slash-delimited genres ("Pop / Rock / Alternative") are emitted as multiple individual `GENRE` Vorbis comment blocks.
//!    - Compound mixed genres ("Soul; Funk / R&B") are emitted as multiple individual `GENRE` Vorbis comment blocks.
//!    - Roundtrip tag application and verification passes.
//! 2. SQLite Migration 0074 execution:
//!    - Pre-existing records with composite genres (';', '/') are cleaned to their primary genre.
//!    - Count of `tracks.genre` containing ';' or '/' is exactly 0 after migration.
//! 3. Durable Recurrence-Prevention Triggers:
//!    - Future `INSERT` with composite genre automatically normalizes to the clean primary genre.
//!    - Future `UPDATE` with composite genre automatically normalizes to the clean primary genre.
//! 4. `enrichment::clean_primary_genre` utility properly isolates the primary genre token.

use std::path::PathBuf;
use sqlx::sqlite::SqlitePoolOptions;
use sqlx::Row;
use syncify_flac_writer::{apply_and_verify_flac_tags, FlacMetadata};
use syncify_tauri_lib::services::enrichment::clean_primary_genre;
use tempfile::tempdir;

fn generate_synthetic_pcm() -> Vec<f32> {
    let sample_rate = 44100;
    let duration_sec = 0.2;
    let total_samples = (sample_rate as f64 * duration_sec) as usize;
    let mut samples = vec![0.0f32; total_samples];
    for i in 0..total_samples {
        let t = i as f32 / sample_rate as f32;
        samples[i] = (2.0 * std::f32::consts::PI * 440.0 * t).sin() * 0.5;
    }
    samples
}

fn create_synthetic_flac(path: &PathBuf) {
    let samples = generate_synthetic_pcm();
    let temp_wav = path.with_extension("wav");

    let mut wav_bytes = Vec::new();
    let num_samples = samples.len() as u32;
    let sample_rate = 44100u32;
    let byte_rate = sample_rate * 2;
    let block_align = 2u16;

    wav_bytes.extend_from_slice(b"RIFF");
    wav_bytes.extend_from_slice(&(36 + num_samples * 2).to_le_bytes());
    wav_bytes.extend_from_slice(b"WAVEfmt ");
    wav_bytes.extend_from_slice(&16u32.to_le_bytes());
    wav_bytes.extend_from_slice(&1u16.to_le_bytes());
    wav_bytes.extend_from_slice(&1u16.to_le_bytes());
    wav_bytes.extend_from_slice(&sample_rate.to_le_bytes());
    wav_bytes.extend_from_slice(&byte_rate.to_le_bytes());
    wav_bytes.extend_from_slice(&block_align.to_le_bytes());
    wav_bytes.extend_from_slice(&16u16.to_le_bytes());
    wav_bytes.extend_from_slice(b"data");
    wav_bytes.extend_from_slice(&(num_samples * 2).to_le_bytes());

    for &s in &samples {
        let i16_sample = (s.clamp(-1.0, 1.0) * 32767.0) as i16;
        wav_bytes.extend_from_slice(&i16_sample.to_le_bytes());
    }

    std::fs::write(&temp_wav, &wav_bytes).expect("Write temp WAV");

    let status = std::process::Command::new("ffmpeg")
        .args([
            "-y",
            "-i", temp_wav.to_str().unwrap(),
            "-c:a", "flac",
            path.to_str().unwrap(),
        ])
        .output()
        .expect("ffmpeg must execute");

    assert!(status.status.success(), "ffmpeg FLAC encoding must succeed");
    let _ = std::fs::remove_file(&temp_wav);
}

#[test]
fn test_flac_semicolon_genre_multi_comment_emission() {
    let dir = tempdir().expect("tempdir");
    let file_path = dir.path().join("semicolon_genre_test.flac");
    create_synthetic_flac(&file_path);

    let meta = FlacMetadata {
        title: "Track 1".to_string(),
        artist: "Artist 1".to_string(),
        album: "Album 1".to_string(),
        genre: Some("Hip Hop; Rap".to_string()),
        track_number: 1,
        track_total: 1,
        disc_number: 1,
        disc_total: 1,
        release_date: Some("2024-01-01".to_string()),
        ..Default::default()
    };

    apply_and_verify_flac_tags(&file_path, &meta).expect("FLAC write & verify");

    let read_tag = metaflac::Tag::read_from_path(&file_path).expect("Read FLAC");
    let vorbis = read_tag.vorbis_comments().expect("Vorbis comments");
    let genre_entries = vorbis.get("GENRE").expect("GENRE tags present");

    assert_eq!(genre_entries.len(), 2, "Must emit 2 distinct GENRE comment entries");
    assert_eq!(genre_entries[0], "Hip Hop");
    assert_eq!(genre_entries[1], "Rap");
}

#[test]
fn test_flac_slash_genre_multi_comment_emission() {
    let dir = tempdir().expect("tempdir");
    let file_path = dir.path().join("slash_genre_test.flac");
    create_synthetic_flac(&file_path);

    let meta = FlacMetadata {
        title: "Track 2".to_string(),
        artist: "Artist 2".to_string(),
        album: "Album 2".to_string(),
        genre: Some("Pop / Rock / Alternative".to_string()),
        track_number: 1,
        track_total: 1,
        disc_number: 1,
        disc_total: 1,
        release_date: Some("2024-01-01".to_string()),
        ..Default::default()
    };

    apply_and_verify_flac_tags(&file_path, &meta).expect("FLAC write & verify");

    let read_tag = metaflac::Tag::read_from_path(&file_path).expect("Read FLAC");
    let vorbis = read_tag.vorbis_comments().expect("Vorbis comments");
    let genre_entries = vorbis.get("GENRE").expect("GENRE tags present");

    assert_eq!(genre_entries.len(), 3, "Must emit 3 distinct GENRE comment entries");
    assert_eq!(genre_entries[0], "Pop");
    assert_eq!(genre_entries[1], "Rock");
    assert_eq!(genre_entries[2], "Alternative");
}

#[test]
fn test_flac_compound_mixed_genre_multi_comment_emission() {
    let dir = tempdir().expect("tempdir");
    let file_path = dir.path().join("mixed_genre_test.flac");
    create_synthetic_flac(&file_path);

    let meta = FlacMetadata {
        title: "Track 3".to_string(),
        artist: "Artist 3".to_string(),
        album: "Album 3".to_string(),
        genre: Some("Soul; Funk / R&B".to_string()),
        track_number: 1,
        track_total: 1,
        disc_number: 1,
        disc_total: 1,
        release_date: Some("2024-01-01".to_string()),
        ..Default::default()
    };

    apply_and_verify_flac_tags(&file_path, &meta).expect("FLAC write & verify");

    let read_tag = metaflac::Tag::read_from_path(&file_path).expect("Read FLAC");
    let vorbis = read_tag.vorbis_comments().expect("Vorbis comments");
    let genre_entries = vorbis.get("GENRE").expect("GENRE tags present");

    assert_eq!(genre_entries.len(), 3, "Must emit 3 distinct GENRE comment entries");
    assert_eq!(genre_entries[0], "Soul");
    assert_eq!(genre_entries[1], "Funk");
    assert_eq!(genre_entries[2], "R&B");
}

#[test]
fn test_clean_primary_genre_function() {
    assert_eq!(clean_primary_genre("Hip Hop; Rap"), Some("Hip Hop".to_string()));
    assert_eq!(clean_primary_genre("Pop / Rock"), Some("Pop".to_string()));
    assert_eq!(clean_primary_genre("Soul; Funk; R&B"), Some("Soul".to_string()));
    assert_eq!(clean_primary_genre("Electronic / Dance; House"), Some("Electronic".to_string()));
    assert_eq!(clean_primary_genre("Classical"), Some("Classical".to_string()));
    assert_eq!(clean_primary_genre("  Jazz  ; Blues "), Some("Jazz".to_string()));
    assert_eq!(clean_primary_genre("   "), None);
    assert_eq!(clean_primary_genre(";"), None);
    assert_eq!(clean_primary_genre("/"), None);
}

#[tokio::test]
async fn test_migration_0074_batch_normalization_and_triggers() {
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await
        .expect("Failed to connect to in-memory SQLite database");

    sqlx::query("PRAGMA foreign_keys = ON")
        .execute(&pool)
        .await
        .expect("Enable foreign keys");

    // 1. Apply migrations 0001 through 0073
    let migrator = sqlx::migrate!("./migrations");
    let migrations: Vec<_> = migrator.iter().collect();

    let partial_migrator = sqlx::migrate::Migrator {
        migrations: std::borrow::Cow::Owned(
            migrations
                .iter()
                .filter(|m| m.version <= 73)
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
        .expect("Run migrations through 0073");

    // 2. Seed tracks with multi-value genre strings with ';' and '/'
    sqlx::query(
        r#"
        INSERT INTO tracks (id, title, genre)
        VALUES
            (1, 'Track Multi 1', 'Hip Hop; Rap'),
            (2, 'Track Multi 2', 'Pop; Rock'),
            (3, 'Track Multi 3', 'Soul; Funk; R&B'),
            (4, 'Track Multi 4', 'Electronic / Dance; House'),
            (5, 'Track Multi 5', 'R&B / Soul'),
            (6, 'Track Clean 1', 'Indie Rock'),
            (7, 'Track No Genre', NULL);
        "#
    )
    .execute(&pool)
    .await
    .expect("Seed tracks with multi-value genres");

    // Verify raw multi-value strings exist before migration 0074
    let pre_semicolon_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM tracks WHERE genre LIKE '%;%'")
        .fetch_one(&pool)
        .await
        .expect("Count pre-migration semicolons");
    let pre_slash_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM tracks WHERE genre LIKE '%/%'")
        .fetch_one(&pool)
        .await
        .expect("Count pre-migration slashes");

    assert!(pre_semicolon_count > 0, "Must have tracks with semicolon before 0074");
    assert!(pre_slash_count > 0, "Must have tracks with slash before 0074");

    // 3. Apply full migrations including 0074
    let full_migrator = sqlx::migrate!("./migrations");
    full_migrator
        .run(&pool)
        .await
        .expect("Run all migrations through 0074");

    // 4. Verify post-migration state: exactly 0 tracks with ';' or '/'
    let post_semicolon_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM tracks WHERE genre LIKE '%;%'")
        .fetch_one(&pool)
        .await
        .expect("Count post-migration semicolons");
    let post_slash_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM tracks WHERE genre LIKE '%/%'")
        .fetch_one(&pool)
        .await
        .expect("Count post-migration slashes");

    assert_eq!(post_semicolon_count, 0, "Must have exactly 0 tracks with ';' after migration 0074");
    assert_eq!(post_slash_count, 0, "Must have exactly 0 tracks with '/' after migration 0074");

    // Verify expected primary genre values
    let rows = sqlx::query("SELECT id, genre FROM tracks ORDER BY id ASC")
        .fetch_all(&pool)
        .await
        .expect("Fetch normalized tracks");

    let get_genre = |id: i64| -> Option<String> {
        rows.iter()
            .find(|r| r.get::<i64, _>("id") == id)
            .and_then(|r| r.get::<Option<String>, _>("genre"))
    };

    assert_eq!(get_genre(1), Some("Hip Hop".to_string()));
    assert_eq!(get_genre(2), Some("Pop".to_string()));
    assert_eq!(get_genre(3), Some("Soul".to_string()));
    assert_eq!(get_genre(4), Some("Electronic".to_string()));
    assert_eq!(get_genre(5), Some("R&B".to_string()));
    assert_eq!(get_genre(6), Some("Indie Rock".to_string()));
    assert_eq!(get_genre(7), None);

    // 5. Verify Durable Recurrence-Prevention Trigger on INSERT
    sqlx::query("INSERT INTO tracks (id, title, genre) VALUES (10, 'New Track Semicolon', 'Hard Rock; Heavy Metal')")
        .execute(&pool)
        .await
        .expect("Insert with semicolon genre");

    let genre_10: Option<String> = sqlx::query_scalar("SELECT genre FROM tracks WHERE id = 10")
        .fetch_one(&pool)
        .await
        .expect("Query track 10");
    assert_eq!(genre_10, Some("Hard Rock".to_string()), "Trigger must normalize semicolon genre on INSERT");

    sqlx::query("INSERT INTO tracks (id, title, genre) VALUES (11, 'New Track Slash', 'Synthpop / New Wave')")
        .execute(&pool)
        .await
        .expect("Insert with slash genre");

    let genre_11: Option<String> = sqlx::query_scalar("SELECT genre FROM tracks WHERE id = 11")
        .fetch_one(&pool)
        .await
        .expect("Query track 11");
    assert_eq!(genre_11, Some("Synthpop".to_string()), "Trigger must normalize slash genre on INSERT");

    sqlx::query("INSERT INTO tracks (id, title, genre) VALUES (12, 'New Clean Track', 'Jazz')")
        .execute(&pool)
        .await
        .expect("Insert clean genre");

    let genre_12: Option<String> = sqlx::query_scalar("SELECT genre FROM tracks WHERE id = 12")
        .fetch_one(&pool)
        .await
        .expect("Query track 12");
    assert_eq!(genre_12, Some("Jazz".to_string()), "Clean genre must remain intact on INSERT");

    // 6. Verify Durable Recurrence-Prevention Trigger on UPDATE
    sqlx::query("UPDATE tracks SET genre = 'Thrash Metal; Speed Metal' WHERE id = 12")
        .execute(&pool)
        .await
        .expect("Update with semicolon genre");

    let genre_12_upd: Option<String> = sqlx::query_scalar("SELECT genre FROM tracks WHERE id = 12")
        .fetch_one(&pool)
        .await
        .expect("Query updated track 12");
    assert_eq!(genre_12_upd, Some("Thrash Metal".to_string()), "Trigger must normalize semicolon genre on UPDATE");

    sqlx::query("UPDATE tracks SET genre = 'Post-Punk / Gothic Rock' WHERE id = 12")
        .execute(&pool)
        .await
        .expect("Update with slash genre");

    let genre_12_upd2: Option<String> = sqlx::query_scalar("SELECT genre FROM tracks WHERE id = 12")
        .fetch_one(&pool)
        .await
        .expect("Query updated track 12 second time");
    assert_eq!(genre_12_upd2, Some("Post-Punk".to_string()), "Trigger must normalize slash genre on UPDATE");

    // 7. Verify Database Integrity
    let integrity: String = sqlx::query_scalar("PRAGMA integrity_check")
        .fetch_one(&pool)
        .await
        .expect("integrity_check");
    assert_eq!(integrity, "ok", "Database integrity must be ok");

    let fk_violations = sqlx::query("PRAGMA foreign_key_check")
        .fetch_all(&pool)
        .await
        .expect("foreign_key_check");
    assert!(fk_violations.is_empty(), "Database must have 0 foreign key violations");
}

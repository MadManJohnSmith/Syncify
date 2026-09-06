//! featuring_and_appearances_test.rs
//!
//! Regression test suite for [TASK-106]:
//! "Parseo de Featurings y Atribución de Appearances (304 Títulos '(feat.)' + 21 Físicos, 1.316 Desajustes Artista-Álbum)"
//!
//! Validates:
//! 1. Extraction of featured guest collaborators and clean track titles in `syncify-core-domain`.
//! 2. Multi-block `ARTIST` Vorbis comments in `syncify-flac-writer` preserving discrete artist indexing.
//! 3. Database association of guest artists into `track_artists` with `role = 'featured'` during enrichment.
//! 4. Querying of artist appearances (`fetch_artist_appearances` and `fetch_artist`).

use std::fs;
use std::path::PathBuf;
use std::process::Command;
use sqlx::SqlitePool;
use syncify_core_domain::metadata::clean_title_and_extract_featured;
use syncify_flac_writer::{apply_and_verify_flac_tags, FlacMetadata};
use syncify_tauri_lib::commands::library::{fetch_artist, fetch_artist_appearances};
use syncify_tauri_lib::services::enrichment::{EnrichmentEngine, OriginTrackMetadata, SyncTrackInput};
use tempfile::tempdir;

fn generate_synthetic_pcm() -> Vec<f32> {
    let sample_rate = 44100;
    let duration_sec = 0.1;
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
        let sample_i16 = (s * 32767.0).clamp(-32768.0, 32767.0) as i16;
        wav_bytes.extend_from_slice(&sample_i16.to_le_bytes());
    }

    fs::write(&temp_wav, &wav_bytes).expect("Write synthetic wav");

    let status = Command::new("flac")
        .args([
            "-f",
            "--silent",
            "-o",
            path.to_str().unwrap(),
            temp_wav.to_str().unwrap(),
        ])
        .status()
        .expect("Run flac encoder");

    let _ = fs::remove_file(&temp_wav);
    assert!(status.success(), "flac encode must succeed");
}

async fn setup_test_db() -> SqlitePool {
    let pool = SqlitePool::connect(":memory:").await.unwrap();
    if let Err(e) = sqlx::migrate!("./migrations").run(&pool).await {
        panic!("Migration failed in test: {}", e);
    }
    pool
}

#[test]
fn test_clean_title_and_extract_featured_edge_cases() {
    // 1. Parenthesized single guest
    let (t1, a1) = clean_title_and_extract_featured("23 (feat. Sasha Dobson)");
    assert_eq!(t1, "23");
    assert_eq!(a1, vec!["Sasha Dobson"]);

    // 2. Parenthesized multiple guests with &
    let (t2, a2) = clean_title_and_extract_featured("4 Minutes (feat. Justin Timberlake & Timbaland)");
    assert_eq!(t2, "4 Minutes");
    assert_eq!(a2, vec!["Justin Timberlake", "Timbaland"]);

    // 3. Parenthesized multiple guests with 'and' and Oxford comma
    let (t3, a3) = clean_title_and_extract_featured("Audio (feat. Sia, Diplo, and Labrinth)");
    assert_eq!(t3, "Audio");
    assert_eq!(a3, vec!["Sia", "Diplo", "Labrinth"]);

    // 4. Preserves internal commas in artist name (Tyler, The Creator)
    let (t4, a4) = clean_title_and_extract_featured("After The Storm (Ft. Tyler, The Creator)");
    assert_eq!(t4, "After The Storm");
    assert_eq!(a4, vec!["Tyler, The Creator"]);

    // 5. Square brackets variant
    let (t5, a5) = clean_title_and_extract_featured("Ain't No Love [feat. Melanie Williams]");
    assert_eq!(t5, "Ain't No Love");
    assert_eq!(a5, vec!["Melanie Williams"]);

    // 6. Bare feat at end or before dash
    let (t6, a6) = clean_title_and_extract_featured("202 feat. 泉まくら - New Mix");
    assert_eq!(t6, "202 - New Mix");
    assert_eq!(a6, vec!["泉まくら"]);

    // 7. False positive: legitimate song title "BIRDS OF A FEATHER"
    let (t7, a7) = clean_title_and_extract_featured("BIRDS OF A FEATHER");
    assert_eq!(t7, "BIRDS OF A FEATHER");
    assert!(a7.is_empty());

    // 8. False positive: soundtrack reference "as featured in ..."
    let (t8, a8) = clean_title_and_extract_featured(
        "Sexy Rouge (as featured in \"Sky Rojo\") (Remix) (Original TV Series Soundtrack)",
    );
    assert_eq!(
        t8,
        "Sexy Rouge (as featured in \"Sky Rojo\") (Remix) (Original TV Series Soundtrack)"
    );
    assert!(a8.is_empty());
}

#[test]
fn test_flac_metadata_multi_artist_vorbis_comments() {
    let dir = tempdir().expect("tempdir");
    let file_path = dir.path().join("featuring_track.flac");
    create_synthetic_flac(&file_path);

    let meta = FlacMetadata {
        title: "Paper Trails (feat. Nicolas Jaar & Dave Harrington)".to_string(),
        artist: "Darkside".to_string(),
        artists: Some(vec![
            "Darkside".to_string(),
            "Nicolas Jaar".to_string(),
            "Dave Harrington".to_string(),
        ]),
        album: "Psychic".to_string(),
        track_number: 3,
        track_total: 8,
        ..Default::default()
    };

    let result = apply_and_verify_flac_tags(&file_path, &meta);
    assert!(result.is_ok(), "Tag writing and verification must succeed: {:?}", result.err());

    // Inspect physical VorbisComment blocks via metaflac
    let tag = metaflac::Tag::read_from_path(&file_path).expect("Read tagged flac");
    let comments = tag.vorbis_comments().expect("Vorbis comments present");

    let title_comments = comments.get("TITLE").expect("TITLE comment present");
    assert_eq!(title_comments, &vec!["Paper Trails".to_string()]);

    let artist_comments = comments.get("ARTIST").expect("ARTIST comments present");
    assert_eq!(
        artist_comments,
        &vec![
            "Darkside".to_string(),
            "Nicolas Jaar".to_string(),
            "Dave Harrington".to_string(),
        ]
    );
}

#[tokio::test]
async fn test_enrichment_associates_featured_in_track_artists() {
    let pool = setup_test_db().await;
    let engine = EnrichmentEngine::new();

    let input = SyncTrackInput {
        service_name: "qobuz".to_string(),
        service_id: 1,
        account_id: 1,
        service_track_id: "q_feat_101".to_string(),
        duration_ms: Some(240000),
        format: Some("FLAC".to_string()),
        bit_depth: Some(24),
        sample_rate: Some(96000),
        quality_score: Some(95),
        audio_quality: Some("hires".to_string()),
        is_favorite: false,
        is_purchased: false,
        cover_art_url: None,
        album_is_favorite: false,
        album_provider_track_id: None,
        query_musicbrainz: false,
        origin_meta: OriginTrackMetadata {
            title: Some("4 Minutes (feat. Justin Timberlake & Timbaland)".to_string()),
            artist: Some("Madonna".to_string()),
            album: Some("Hard Candy".to_string()),
            track_number: Some(2),
            disc_number: Some(1),
            release_year: Some("2008".to_string()),
            isrc: Some("USWB10800889".to_string()),
            audio_source: Some("Qobuz".to_string()),
            ..Default::default()
        },
    };

    let result = engine.enrich_and_persist_sync_track(&pool, input).await;
    assert!(result.is_ok(), "Track persistence must succeed: {:?}", result.err());
    let res = result.unwrap();

    // 1. Verify tracks.title was cleaned
    let track_title: String = sqlx::query_scalar("SELECT title FROM tracks WHERE id = ?")
        .bind(res.track_id)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(track_title, "4 Minutes");

    // 2. Verify track_artists has Madonna as 'primary'
    let primary_artist: (String, String) = sqlx::query_as(
        r#"
        SELECT a.name, ta.role
        FROM track_artists ta
        JOIN artists a ON a.id = ta.artist_id
        WHERE ta.track_id = ? AND ta.role = 'primary'
        "#,
    )
    .bind(res.track_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(primary_artist.0, "Madonna");
    assert_eq!(primary_artist.1, "primary");

    // 3. Verify track_artists has Justin Timberlake and Timbaland as 'featured'
    let featured_artists: Vec<(String, String)> = sqlx::query_as(
        r#"
        SELECT a.name, ta.role
        FROM track_artists ta
        JOIN artists a ON a.id = ta.artist_id
        WHERE ta.track_id = ? AND ta.role = 'featured'
        ORDER BY a.name ASC
        "#,
    )
    .bind(res.track_id)
    .fetch_all(&pool)
    .await
    .unwrap();

    assert_eq!(featured_artists.len(), 2);
    assert_eq!(featured_artists[0].0, "Justin Timberlake");
    assert_eq!(featured_artists[0].1, "featured");
    assert_eq!(featured_artists[1].0, "Timbaland");
    assert_eq!(featured_artists[1].1, "featured");
}

#[tokio::test]
async fn test_appearances_query_and_attribution() {
    let pool = setup_test_db().await;

    // 1. Create Artists: Artist A ("Gorillaz") and Artist B ("Shaun Ryder")
    let gorillaz_id: i64 = sqlx::query_scalar("INSERT INTO artists (name) VALUES ('Gorillaz') RETURNING id")
        .fetch_one(&pool)
        .await
        .unwrap();

    let shaun_id: i64 = sqlx::query_scalar("INSERT INTO artists (name) VALUES ('Shaun Ryder') RETURNING id")
        .fetch_one(&pool)
        .await
        .unwrap();

    // 2. Create Album by Gorillaz ("Demon Days")
    let album_id: i64 = sqlx::query_scalar("INSERT INTO albums (title, release_date) VALUES ('Demon Days', '2005-05-23') RETURNING id")
        .fetch_one(&pool)
        .await
        .unwrap();

    sqlx::query("INSERT INTO album_artists (album_id, artist_id, is_primary) VALUES (?, ?, 1)")
        .bind(album_id)
        .bind(gorillaz_id)
        .execute(&pool)
        .await
        .unwrap();

    // 3. Track 1: "Feel Good Inc." (Gorillaz only)
    let t1_id: i64 = sqlx::query_scalar("INSERT INTO tracks (title, album_id, duration_ms) VALUES ('Feel Good Inc.', ?, 223000) RETURNING id")
        .bind(album_id)
        .fetch_one(&pool)
        .await
        .unwrap();

    sqlx::query("INSERT INTO track_artists (track_id, artist_id, role) VALUES (?, ?, 'primary')")
        .bind(t1_id)
        .bind(gorillaz_id)
        .execute(&pool)
        .await
        .unwrap();

    // 4. Track 2: "DARE" (Gorillaz primary, Shaun Ryder featured)
    let t2_id: i64 = sqlx::query_scalar("INSERT INTO tracks (title, album_id, duration_ms) VALUES ('DARE', ?, 244000) RETURNING id")
        .bind(album_id)
        .fetch_one(&pool)
        .await
        .unwrap();

    sqlx::query("INSERT INTO track_artists (track_id, artist_id, role) VALUES (?, ?, 'primary')")
        .bind(t2_id)
        .bind(gorillaz_id)
        .execute(&pool)
        .await
        .unwrap();

    sqlx::query("INSERT INTO track_artists (track_id, artist_id, role) VALUES (?, ?, 'featured')")
        .bind(t2_id)
        .bind(shaun_id)
        .execute(&pool)
        .await
        .unwrap();

    // 5. Track 3: Compilation guest track where Shaun Ryder is primary on a Various Artists album
    let comp_album_id: i64 = sqlx::query_scalar("INSERT INTO albums (title, release_date) VALUES ('Soundtrack 90s', '1998-01-01') RETURNING id")
        .fetch_one(&pool)
        .await
        .unwrap();

    let va_id: i64 = sqlx::query_scalar("INSERT INTO artists (name) VALUES ('Various Artists') RETURNING id")
        .fetch_one(&pool)
        .await
        .unwrap();

    sqlx::query("INSERT INTO album_artists (album_id, artist_id, is_primary) VALUES (?, ?, 1)")
        .bind(comp_album_id)
        .bind(va_id)
        .execute(&pool)
        .await
        .unwrap();

    let t3_id: i64 = sqlx::query_scalar("INSERT INTO tracks (title, album_id, duration_ms) VALUES ('Madchester Song', ?, 210000) RETURNING id")
        .bind(comp_album_id)
        .fetch_one(&pool)
        .await
        .unwrap();

    sqlx::query("INSERT INTO track_artists (track_id, artist_id, role) VALUES (?, ?, 'primary')")
        .bind(t3_id)
        .bind(shaun_id)
        .execute(&pool)
        .await
        .unwrap();

    // 6. Test fetch_artist_appearances for Shaun Ryder
    let appearances = fetch_artist_appearances(&pool, shaun_id).await.unwrap();
    assert_eq!(appearances.len(), 2, "Shaun Ryder should have exactly 2 appearances");

    let t2_appearance = appearances.iter().find(|a| a.id == t2_id).expect("DARE must be an appearance");
    assert_eq!(t2_appearance.title, "DARE");
    assert_eq!(t2_appearance.album.as_deref(), Some("Demon Days"));
    assert_eq!(t2_appearance.role.as_deref(), Some("featured"));

    let t3_appearance = appearances.iter().find(|a| a.id == t3_id).expect("Madchester Song must be an appearance");
    assert_eq!(t3_appearance.title, "Madchester Song");
    assert_eq!(t3_appearance.album.as_deref(), Some("Soundtrack 90s"));
    assert_eq!(t3_appearance.role.as_deref(), Some("primary"));

    // 7. Test fetch_artist includes appearances
    let artist_detail = fetch_artist(&pool, shaun_id).await.unwrap();
    assert_eq!(artist_detail.name, "Shaun Ryder");
    assert_eq!(artist_detail.appearances.len(), 2);
}

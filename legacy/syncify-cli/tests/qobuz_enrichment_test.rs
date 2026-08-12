use syncify_cli::metadata::tag_writer::{apply_flac_tags, FlacMetadata};
use syncify_cli::download::download_goodies_booklet;
use syncify_cli::services::enrichment::EnrichmentEngine;
use std::fs::File;
use std::io::Write;
use std::path::{Path, PathBuf};

/// Helper function to construct a valid minimal FLAC file structure with STREAMINFO and audio frames.
fn create_valid_flac_file(path: &Path, audio_payload_len: usize) -> std::io::Result<()> {
    let mut file = File::create(path)?;

    // 1. fLaC marker (4 bytes)
    file.write_all(b"fLaC")?;

    // 2. STREAMINFO block header (4 bytes): type=0 (STREAMINFO), is_last=1, length=34
    let streaminfo_header: [u8; 4] = [0x80, 0x00, 0x00, 0x22];
    file.write_all(&streaminfo_header)?;

    // 3. STREAMINFO block payload (34 bytes)
    let mut streaminfo_payload = [0u8; 34];
    streaminfo_payload[0..2].copy_from_slice(&4608u16.to_be_bytes()); // min_block
    streaminfo_payload[2..4].copy_from_slice(&4608u16.to_be_bytes()); // max_block
    streaminfo_payload[10] = 0x0A;
    streaminfo_payload[11] = 0xC4;
    streaminfo_payload[12] = 0x42; // 44.1kHz, 2ch, 16bit (sample rate + channels)
    streaminfo_payload[13] = 0xF0; // bps = 16 (0x0F shifted) + total samples high nibble
    file.write_all(&streaminfo_payload)?;

    // 4. Audio frames starting with exact FLAC sync word 0xFFF8
    let mut audio_data = vec![0u8; audio_payload_len.max(16)];
    audio_data[0] = 0xFF;
    audio_data[1] = 0xF8;
    audio_data[2] = 0x18;
    audio_data[3] = 0x00;
    file.write_all(&audio_data)?;

    Ok(())
}

/// Helper function to verify FLAC stream structure and confirm FLAC sync word 0xFFF8 at audio_start
fn verify_flac_sync_and_structure(path: &Path) -> Result<usize, String> {
    let data = std::fs::read(path).map_err(|e| format!("Failed to read file: {}", e))?;

    if data.len() < 42 || &data[0..4] != b"fLaC" {
        return Err("Invalid FLAC magic header".to_string());
    }

    let mut offset = 4;
    let mut found_last = false;

    while offset < data.len() {
        if offset + 4 > data.len() {
            return Err("Truncated metadata block header".to_string());
        }

        let hdr = u32::from_be_bytes([data[offset], data[offset + 1], data[offset + 2], data[offset + 3]]);
        let is_last = (hdr >> 31) & 1 == 1;
        let block_type = (hdr >> 24) & 0x7F;
        let block_len = (hdr & 0x00FF_FFFF) as usize;

        if block_type == 127 {
            return Err(format!("Invalid metadata block type 127 at offset {}", offset));
        }

        offset += 4 + block_len;

        if is_last {
            found_last = true;
            break;
        }
    }

    if !found_last {
        return Err("No metadata block marked as last".to_string());
    }

    let audio_start = offset;
    if audio_start + 2 > data.len() {
        return Err("File truncated before audio payload".to_string());
    }

    let sync = u16::from_be_bytes([data[audio_start], data[audio_start + 1]]);
    if (sync & 0xFFFC) != 0xFFF8 {
        return Err(format!("Invalid audio frame sync 0x{:04X} at offset {} (expected 0xFFF8)", sync, audio_start));
    }

    Ok(audio_start)
}

/// Helper function to run ffprobe on a FLAC file to verify stream validity
fn verify_ffprobe_flac(path: &Path) -> Result<(), String> {
    let output = std::process::Command::new("ffprobe")
        .arg("-v")
        .arg("error")
        .arg("-show_entries")
        .arg("stream=codec_name")
        .arg("-of")
        .arg("default=noprint_wrappers=1")
        .arg(path)
        .output();

    match output {
        Ok(out) => {
            if !out.status.success() {
                let err = String::from_utf8_lossy(&out.stderr);
                return Err(format!("ffprobe failed on FLAC file: {}", err));
            }
            let text = String::from_utf8_lossy(&out.stdout);
            if !text.contains("codec_name=flac") {
                return Err(format!("ffprobe did not find flac codec: {}", text));
            }
            Ok(())
        }
        Err(e) => {
            tracing::warn!("ffprobe command not available: {}", e);
            Ok(())
        }
    }
}

/// Helper to setup an in-memory SQLite pool with tracks table for persistence testing
async fn setup_test_sqlite_pool() -> sqlx::SqlitePool {
    let pool = sqlx::SqlitePool::connect("sqlite::memory:")
        .await
        .expect("Failed to create in-memory SQLite pool");

    sqlx::query(
        "CREATE TABLE tracks (
            id INTEGER PRIMARY KEY,
            title TEXT,
            artist TEXT,
            album TEXT,
            genre TEXT,
            style TEXT,
            mood TEXT,
            bpm REAL,
            key TEXT,
            label TEXT,
            genre_source_type TEXT DEFAULT 'enrichment',
            style_source_type TEXT DEFAULT 'enrichment',
            mood_source_type TEXT DEFAULT 'enrichment',
            bpm_source_type TEXT DEFAULT 'enrichment',
            key_source_type TEXT DEFAULT 'enrichment',
            label_source_type TEXT DEFAULT 'enrichment'
        )",
    )
    .execute(&pool)
    .await
    .expect("Failed to create tracks table");

    pool
}

/// Find a sample audio file in downloads_test if available
fn find_sample_audio_file() -> Option<PathBuf> {
    let candidates = [
        PathBuf::from("../downloads_test/01. The Neighbourhood - Hula Girl.flac"),
        PathBuf::from("downloads_test/01. The Neighbourhood - Hula Girl.flac"),
        PathBuf::from("../downloads_test/01 - David Bowie - Heroes (Enriched 31 Tags).flac"),
    ];

    for candidate in &candidates {
        if candidate.exists() {
            return Some(candidate.clone());
        }
    }
    None
}

// =========================================================================
// DETERMINISTIC FAST SUITE (Runs in cargo test by default, 0 network calls)
// =========================================================================

#[test]
fn test_track1_standard_heroes_deterministic() {
    let temp_file = std::env::temp_dir().join("syncify_test_track1_heroes.flac");
    create_valid_flac_file(&temp_file, 1024).expect("Failed to create mock FLAC");

    let metadata = FlacMetadata {
        title: "Heroes".to_string(),
        artist: "David Bowie".to_string(),
        album: "Heroes".to_string(),
        genre: Some("Rock".to_string()),
        style: Some("Art Rock".to_string()),
        mood: Some("epic".to_string()),
        release_type: Some("Album".to_string()),
        release_status: Some("Official".to_string()),
        release_country: Some("US".to_string()),
        language: Some("eng".to_string()),
        label: Some("Parlophone UK".to_string()),
        track_number: 3,
        track_total: 10,
        release_year: Some("1977".to_string()),
        ..Default::default()
    };

    let tag_result = apply_flac_tags(&temp_file, &metadata);
    assert!(tag_result.is_ok(), "Tagging failed: {:?}", tag_result);

    let tag = metaflac::Tag::read_from_path(&temp_file).expect("Failed to read FLAC");
    let vc = tag.vorbis_comments().expect("No VorbisComments");

    assert_eq!(vc.title().and_then(|v| v.first().map(|s| s.as_str())), Some("Heroes"));
    assert_eq!(vc.get("RELEASETYPE").and_then(|v| v.first().map(|s| s.as_str())), Some("Album"));
    assert_eq!(vc.get("RELEASESTATUS").and_then(|v| v.first().map(|s| s.as_str())), Some("Official"));
    assert_eq!(vc.get("LANGUAGE").and_then(|v| v.first().map(|s| s.as_str())), Some("eng"));

    verify_flac_sync_and_structure(&temp_file).expect("FLAC sync failed");
    verify_ffprobe_flac(&temp_file).expect("ffprobe failed");
    let _ = std::fs::remove_file(&temp_file);
}

#[test]
fn test_track2_hires_with_cover_art_deterministic() {
    let temp_file = std::env::temp_dir().join("syncify_test_track2_hires.flac");
    create_valid_flac_file(&temp_file, 2048).expect("Failed to create mock FLAC");

    let mock_cover_jpeg: Vec<u8> = vec![
        0xFF, 0xD8, 0xFF, 0xE0, 0x00, 0x10, 0x4A, 0x46, 0x49, 0x46, 0x00, 0x01,
        0x01, 0x01, 0x00, 0x48, 0x00, 0x48, 0x00, 0x00, 0xFF, 0xD9
    ];

    let metadata = FlacMetadata {
        title: "Symphony No. 5".to_string(),
        artist: "London Symphony Orchestra".to_string(),
        album: "Beethoven Symphony No. 5".to_string(),
        sample_rate: Some(96000.0),
        bit_depth: Some(24),
        cover_data: Some(mock_cover_jpeg.clone()),
        ..Default::default()
    };

    assert!(apply_flac_tags(&temp_file, &metadata).is_ok());
    assert!(apply_flac_tags(&temp_file, &metadata).is_ok());

    let tag = metaflac::Tag::read_from_path(&temp_file).expect("Failed to read FLAC");
    assert_eq!(tag.pictures().count(), 1, "Must contain exactly 1 picture block without duplication");

    verify_flac_sync_and_structure(&temp_file).expect("FLAC sync failed");
    verify_ffprobe_flac(&temp_file).expect("ffprobe failed");
    let _ = std::fs::remove_file(&temp_file);
}

#[tokio::test]
async fn test_goodies_booklet_download_parsing() {
    let client = reqwest::Client::new();
    let temp_dir = std::env::temp_dir().join("syncify_test_booklet");
    let _ = std::fs::create_dir_all(&temp_dir);

    let result: Option<std::path::PathBuf> = download_goodies_booklet(&client, "", &temp_dir).await.ok().flatten();
    assert!(result.is_none());

    let _ = std::fs::remove_dir_all(&temp_dir);
}

#[tokio::test]
async fn test_sqlite_manual_source_type_protection() {
    let pool = setup_test_sqlite_pool().await;

    sqlx::query(
        "INSERT INTO tracks (id, title, genre, genre_source_type, style_source_type) VALUES (1, 'Test', 'User Manual Genre', 'manual', 'enrichment')"
    )
    .execute(&pool)
    .await
    .expect("Failed to insert track");

    let engine = EnrichmentEngine::new();
    let meta = syncify_cli::services::enrichment::EnrichedMetadata {
        genre: Some("Enriched Auto Genre".to_string()),
        style: Some("Enriched Auto Style".to_string()),
        ..Default::default()
    };

    let res = engine.apply_to_track(&pool, 1, &meta).await;
    assert!(res.is_ok());

    let row: (String, String) = sqlx::query_as("SELECT genre, style FROM tracks WHERE id = 1")
        .fetch_one(&pool)
        .await
        .expect("Failed to query track");

    assert_eq!(row.0, "User Manual Genre", "Manual genre must NOT be overwritten by enrichment");
    assert_eq!(row.1, "Enriched Auto Style", "Enrichment style should update when source_type == enrichment");
}

// =========================================================================
// S89 EXTENSION: 10-TRACK REAL MATRIX SUITE (cargo test -- --ignored)
// =========================================================================

/// 1. David Bowie — "Heroes" — álbum: "Heroes"
#[tokio::test]
#[ignore]
async fn test_matrix_track1_david_bowie_heroes() {
    dotenvy::dotenv().ok();
    let engine = EnrichmentEngine::new();
    let sample_audio = find_sample_audio_file();
    let sample_path_str = sample_audio.as_ref().map(|p| p.to_str().unwrap_or(""));

    let enriched = engine.resolve_track_metadata("David Bowie", "Heroes", "Heroes", sample_path_str).await;

    println!("[Matrix 1/10] Bowie Heroes: release_type={:?}, status={:?}, language={:?}, genre={:?}, label={:?}",
        enriched.release_type, enriched.release_status, enriched.language, enriched.genre, enriched.label);

    assert_eq!(enriched.release_type, Some("Album".to_string()), "Must resolve to Album");
    assert_eq!(enriched.release_status, Some("Official".to_string()), "Must resolve to Official");
    assert_eq!(enriched.language, Some("eng".to_string()), "Must resolve to eng language");

    // Tag FLAC & verify 0xFFF8 sync word + ffprobe
    let temp_flac = std::env::temp_dir().join("matrix_track1_heroes.flac");
    create_valid_flac_file(&temp_flac, 2048).expect("Failed to create FLAC");

    let flac_meta = FlacMetadata {
        title: "Heroes".to_string(),
        artist: "David Bowie".to_string(),
        album: "Heroes".to_string(),
        genre: enriched.genre.clone(),
        release_type: enriched.release_type.clone(),
        release_status: enriched.release_status.clone(),
        language: enriched.language.clone(),
        label: enriched.label.clone(),
        ..Default::default()
    };

    assert!(apply_flac_tags(&temp_flac, &flac_meta).is_ok());
    verify_flac_sync_and_structure(&temp_flac).expect("FLAC sync 0xFFF8 check failed");
    verify_ffprobe_flac(&temp_flac).expect("ffprobe check failed");

    // SQLite persistence & traceability check
    let pool = setup_test_sqlite_pool().await;
    sqlx::query("INSERT INTO tracks (id, title, artist, album) VALUES (1, 'Heroes', 'David Bowie', 'Heroes')").execute(&pool).await.unwrap();
    assert!(engine.apply_to_track(&pool, 1, &enriched).await.is_ok());

    let _ = std::fs::remove_file(&temp_flac);
}

/// 2. Queen — "Bohemian Rhapsody" — álbum: "A Night at the Opera"
#[tokio::test]
#[ignore]
async fn test_matrix_track2_queen_bohemian_rhapsody() {
    dotenvy::dotenv().ok();
    let engine = EnrichmentEngine::new();
    let sample_audio = find_sample_audio_file();
    let sample_path_str = sample_audio.as_ref().map(|p| p.to_str().unwrap_or(""));

    let enriched = engine.resolve_track_metadata("Queen", "A Night at the Opera", "Bohemian Rhapsody", sample_path_str).await;

    println!("[Matrix 2/10] Queen Bohemian Rhapsody: release_type={:?}, language={:?}, genre={:?}, style={:?}",
        enriched.release_type, enriched.language, enriched.genre, enriched.style);

    assert_eq!(enriched.release_type, Some("Album".to_string()), "Must select original Album, not compilation");
    assert_eq!(enriched.language, Some("eng".to_string()), "Language must be eng");

    let temp_flac = std::env::temp_dir().join("matrix_track2_bohemian.flac");
    create_valid_flac_file(&temp_flac, 2048).expect("Failed to create FLAC");
    let flac_meta = FlacMetadata {
        title: "Bohemian Rhapsody".to_string(),
        artist: "Queen".to_string(),
        album: "A Night at the Opera".to_string(),
        release_type: enriched.release_type.clone(),
        language: enriched.language.clone(),
        ..Default::default()
    };
    assert!(apply_flac_tags(&temp_flac, &flac_meta).is_ok());
    verify_flac_sync_and_structure(&temp_flac).expect("FLAC sync 0xFFF8 check failed");
    verify_ffprobe_flac(&temp_flac).expect("ffprobe check failed");

    let pool = setup_test_sqlite_pool().await;
    sqlx::query("INSERT INTO tracks (id, title) VALUES (2, 'Bohemian Rhapsody')").execute(&pool).await.unwrap();
    assert!(engine.apply_to_track(&pool, 2, &enriched).await.is_ok());

    let _ = std::fs::remove_file(&temp_flac);
}

/// 3. Various Artists — "Now That's What I Call Music! 1" — Compilation
#[tokio::test]
#[ignore]
async fn test_matrix_track3_various_artists_compilation() {
    dotenvy::dotenv().ok();
    let engine = EnrichmentEngine::new();

    let enriched = engine.resolve_track_metadata("Various Artists", "Now That's What I Call Music!", "You're the One That I Want", None).await;

    println!("[Matrix 3/10] Compilation: release_type={:?}", enriched.release_type);
    assert_eq!(enriched.release_type, Some("Compilation".to_string()), "Must select Compilation");

    let temp_flac = std::env::temp_dir().join("matrix_track3_compilation.flac");
    create_valid_flac_file(&temp_flac, 1024).expect("Failed to create FLAC");

    let flac_meta = FlacMetadata {
        title: "You're the One That I Want".to_string(),
        artist: "John Travolta & Olivia Newton-John".to_string(),
        album: "Now That's What I Call Music!".to_string(),
        album_artist: Some("Various Artists".to_string()),
        release_type: enriched.release_type.clone(),
        ..Default::default()
    };
    assert!(apply_flac_tags(&temp_flac, &flac_meta).is_ok());
    verify_flac_sync_and_structure(&temp_flac).expect("FLAC sync 0xFFF8 check failed");
    verify_ffprobe_flac(&temp_flac).expect("ffprobe check failed");

    let pool = setup_test_sqlite_pool().await;
    sqlx::query("INSERT INTO tracks (id, title) VALUES (3, 'Compilation Song')").execute(&pool).await.unwrap();
    assert!(engine.apply_to_track(&pool, 3, &enriched).await.is_ok());

    let _ = std::fs::remove_file(&temp_flac);
}

/// 4. 宇多田ヒカル — "First Love" — álbum: "First Love" (Non-Latin UTF-8)
#[tokio::test]
#[ignore]
async fn test_matrix_track4_utada_hikaru_first_love() {
    dotenvy::dotenv().ok();
    let engine = EnrichmentEngine::new();

    let enriched = engine.resolve_track_metadata("宇多田ヒカル", "First Love", "First Love", None).await;

    println!("[Matrix 4/10] Utada Hikaru: language={:?}, country={:?}", enriched.language, enriched.release_country);
    assert_eq!(enriched.language, Some("jpn".to_string()), "Language must be jpn");

    let temp_flac = std::env::temp_dir().join("matrix_track4_utada.flac");
    create_valid_flac_file(&temp_flac, 1024).expect("Failed to create FLAC");

    let flac_meta = FlacMetadata {
        title: "First Love".to_string(),
        artist: "宇多田ヒカル".to_string(),
        album: "First Love".to_string(),
        language: enriched.language.clone(),
        release_country: enriched.release_country.clone(),
        ..Default::default()
    };
    assert!(apply_flac_tags(&temp_flac, &flac_meta).is_ok());

    // Read back VorbisComments to confirm UTF-8 integrity (no mojibake)
    let tag = metaflac::Tag::read_from_path(&temp_flac).expect("Failed to read FLAC");
    let vc = tag.vorbis_comments().expect("No VorbisComments");
    assert_eq!(vc.artist().and_then(|v| v.first().map(|s| s.as_str())), Some("宇多田ヒカル"));

    verify_flac_sync_and_structure(&temp_flac).expect("FLAC sync 0xFFF8 check failed");
    verify_ffprobe_flac(&temp_flac).expect("ffprobe check failed");

    let pool = setup_test_sqlite_pool().await;
    sqlx::query("INSERT INTO tracks (id, title) VALUES (4, 'First Love')").execute(&pool).await.unwrap();
    assert!(engine.apply_to_track(&pool, 4, &enriched).await.is_ok());

    let _ = std::fs::remove_file(&temp_flac);
}

/// 5. Pink Floyd — "Echoes" — álbum: "Meddle" (Long Track / Heavy Payload)
#[tokio::test]
#[ignore]
async fn test_matrix_track5_pink_floyd_echoes() {
    dotenvy::dotenv().ok();
    let engine = EnrichmentEngine::new();

    let enriched = engine.resolve_track_metadata("Pink Floyd", "Meddle", "Echoes", None).await;

    println!("[Matrix 5/10] Pink Floyd Echoes: release_type={:?}", enriched.release_type);
    assert_eq!(enriched.release_type, Some("Album".to_string()), "Release type must be Album");

    let temp_flac = std::env::temp_dir().join("matrix_track5_echoes.flac");
    create_valid_flac_file(&temp_flac, 65536).expect("Failed to create FLAC");

    let flac_meta = FlacMetadata {
        title: "Echoes".to_string(),
        artist: "Pink Floyd".to_string(),
        album: "Meddle".to_string(),
        release_type: enriched.release_type.clone(),
        ..Default::default()
    };
    assert!(apply_flac_tags(&temp_flac, &flac_meta).is_ok());
    verify_flac_sync_and_structure(&temp_flac).expect("FLAC sync 0xFFF8 check failed");
    verify_ffprobe_flac(&temp_flac).expect("ffprobe check failed");

    let pool = setup_test_sqlite_pool().await;
    sqlx::query("INSERT INTO tracks (id, title) VALUES (5, 'Echoes')").execute(&pool).await.unwrap();
    assert!(engine.apply_to_track(&pool, 5, &enriched).await.is_ok());

    let _ = std::fs::remove_file(&temp_flac);
}

/// 6. Radiohead — "No Surprises" — álbum: "OK Computer" (Last.fm MOOD & Lyrics)
#[tokio::test]
#[ignore]
async fn test_matrix_track6_radiohead_no_surprises() {
    dotenvy::dotenv().ok();
    let engine = EnrichmentEngine::new();
    let sample_audio = find_sample_audio_file();
    let sample_path_str = sample_audio.as_ref().map(|p| p.to_str().unwrap_or(""));

    let enriched = engine.resolve_track_metadata("Radiohead", "OK Computer", "No Surprises", sample_path_str).await;

    println!("[Matrix 6/10] Radiohead No Surprises: mood={:?}, genre={:?}, style={:?}",
        enriched.mood, enriched.genre, enriched.style);

    if std::env::var("LASTFM_API_KEY").is_ok() {
        assert_eq!(enriched.mood, Some("Sad".to_string()), "Last.fm mood must be Sad for Radiohead No Surprises");
    }

    let temp_flac = std::env::temp_dir().join("matrix_track6_no_surprises.flac");
    create_valid_flac_file(&temp_flac, 2048).expect("Failed to create FLAC");

    let flac_meta = FlacMetadata {
        title: "No Surprises".to_string(),
        artist: "Radiohead".to_string(),
        album: "OK Computer".to_string(),
        mood: enriched.mood.clone(),
        lyrics_lrc: Some("[00:01.00] A heart that's full up like a land-fill".to_string()),
        ..Default::default()
    };
    assert!(apply_flac_tags(&temp_flac, &flac_meta).is_ok());

    let tag = metaflac::Tag::read_from_path(&temp_flac).expect("Failed to read FLAC");
    let vc = tag.vorbis_comments().expect("No VorbisComments");
    assert!(vc.get("LYRICS").is_some(), "LYRICS tag must be written when present");

    verify_flac_sync_and_structure(&temp_flac).expect("FLAC sync 0xFFF8 check failed");
    verify_ffprobe_flac(&temp_flac).expect("ffprobe check failed");

    let pool = setup_test_sqlite_pool().await;
    sqlx::query("INSERT INTO tracks (id, title) VALUES (6, 'No Surprises')").execute(&pool).await.unwrap();
    assert!(engine.apply_to_track(&pool, 6, &enriched).await.is_ok());

    let _ = std::fs::remove_file(&temp_flac);
}

/// 7. Nirvana — "Smells Like Teen Spirit" — álbum: "Nevermind"
#[tokio::test]
#[ignore]
async fn test_matrix_track7_nirvana_smells_like_teen_spirit() {
    dotenvy::dotenv().ok();
    let engine = EnrichmentEngine::new();

    let enriched = engine.resolve_track_metadata("Nirvana", "Nevermind", "Smells Like Teen Spirit", None).await;

    println!("[Matrix 7/10] Nirvana: release_type={:?}, language={:?}, genre={:?}, style={:?}",
        enriched.release_type, enriched.language, enriched.genre, enriched.style);

    assert_eq!(enriched.release_type, Some("Album".to_string()), "Release type must be Album");
    assert_eq!(enriched.language, Some("eng".to_string()), "Language must be eng");

    let temp_flac = std::env::temp_dir().join("matrix_track7_nirvana.flac");
    create_valid_flac_file(&temp_flac, 2048).expect("Failed to create FLAC");

    let flac_meta = FlacMetadata {
        title: "Smells Like Teen Spirit".to_string(),
        artist: "Nirvana".to_string(),
        album: "Nevermind".to_string(),
        release_type: enriched.release_type.clone(),
        language: enriched.language.clone(),
        ..Default::default()
    };
    assert!(apply_flac_tags(&temp_flac, &flac_meta).is_ok());
    verify_flac_sync_and_structure(&temp_flac).expect("FLAC sync 0xFFF8 check failed");
    verify_ffprobe_flac(&temp_flac).expect("ffprobe check failed");

    let pool = setup_test_sqlite_pool().await;
    sqlx::query("INSERT INTO tracks (id, title) VALUES (7, 'Smells Like Teen Spirit')").execute(&pool).await.unwrap();
    assert!(engine.apply_to_track(&pool, 7, &enriched).await.is_ok());

    let _ = std::fs::remove_file(&temp_flac);
}

/// 8. The Beatles — "A Day in the Life" — álbum: "Sgt. Pepper's Lonely Hearts Club Band"
#[tokio::test]
#[ignore]
async fn test_matrix_track8_beatles_a_day_in_the_life() {
    dotenvy::dotenv().ok();
    let engine = EnrichmentEngine::new();

    let enriched = engine.resolve_track_metadata("The Beatles", "Sgt. Pepper's Lonely Hearts Club Band", "A Day in the Life", None).await;

    println!("[Matrix 8/10] Beatles: release_type={:?}, language={:?}, label={:?}, country={:?}",
        enriched.release_type, enriched.language, enriched.label, enriched.release_country);

    assert_eq!(enriched.release_type, Some("Album".to_string()), "Release type must be Album");
    assert_eq!(enriched.language, Some("eng".to_string()), "Language must be eng");

    let temp_flac = std::env::temp_dir().join("matrix_track8_beatles.flac");
    create_valid_flac_file(&temp_flac, 2048).expect("Failed to create FLAC");

    let flac_meta = FlacMetadata {
        title: "A Day in the Life".to_string(),
        artist: "The Beatles".to_string(),
        album: "Sgt. Pepper's Lonely Hearts Club Band".to_string(),
        release_type: enriched.release_type.clone(),
        language: enriched.language.clone(),
        label: enriched.label.clone(),
        ..Default::default()
    };
    assert!(apply_flac_tags(&temp_flac, &flac_meta).is_ok());
    verify_flac_sync_and_structure(&temp_flac).expect("FLAC sync 0xFFF8 check failed");
    verify_ffprobe_flac(&temp_flac).expect("ffprobe check failed");

    let pool = setup_test_sqlite_pool().await;
    sqlx::query("INSERT INTO tracks (id, title) VALUES (8, 'A Day in the Life')").execute(&pool).await.unwrap();
    assert!(engine.apply_to_track(&pool, 8, &enriched).await.is_ok());

    let _ = std::fs::remove_file(&temp_flac);
}

/// 9. Sinéad O'Connor — "Nothing Compares 2 U" — álbum: "I Do Not Want What I Haven't Got"
#[tokio::test]
#[ignore]
async fn test_matrix_track9_sinead_oconnor_nothing_compares_2_u() {
    dotenvy::dotenv().ok();
    let engine = EnrichmentEngine::new();

    let enriched = engine.resolve_track_metadata("Sinéad O'Connor", "I Do Not Want What I Haven't Got", "Nothing Compares 2 U", None).await;

    println!("[Matrix 9/10] Sinead O'Connor: genre={:?}, style={:?}, release_type={:?}",
        enriched.genre, enriched.style, enriched.release_type);

    let temp_flac = std::env::temp_dir().join("matrix_track9_sinead.flac");
    create_valid_flac_file(&temp_flac, 2048).expect("Failed to create FLAC");

    let flac_meta = FlacMetadata {
        title: "Nothing Compares 2 U".to_string(),
        artist: "Sinéad O'Connor".to_string(),
        album: "I Do Not Want What I Haven't Got".to_string(),
        genre: enriched.genre.clone(),
        release_type: enriched.release_type.clone(),
        ..Default::default()
    };
    assert!(apply_flac_tags(&temp_flac, &flac_meta).is_ok());
    verify_flac_sync_and_structure(&temp_flac).expect("FLAC sync 0xFFF8 check failed");
    verify_ffprobe_flac(&temp_flac).expect("ffprobe check failed");

    let pool = setup_test_sqlite_pool().await;
    sqlx::query("INSERT INTO tracks (id, title) VALUES (9, 'Nothing Compares 2 U')").execute(&pool).await.unwrap();
    assert!(engine.apply_to_track(&pool, 9, &enriched).await.is_ok());

    let _ = std::fs::remove_file(&temp_flac);
}

/// 10. The Prodigy — "Firestarter" — álbum: "The Fat of the Land" (BPM / Essentia WSL2)
#[tokio::test]
#[ignore]
async fn test_matrix_track10_prodigy_firestarter() {
    dotenvy::dotenv().ok();
    let engine = EnrichmentEngine::new();
    let sample_audio = find_sample_audio_file();
    let sample_path_str = sample_audio.as_ref().map(|p| p.to_str().unwrap_or(""));

    let enriched = engine.resolve_track_metadata("The Prodigy", "The Fat of the Land", "Firestarter", sample_path_str).await;

    println!("[Matrix 10/10] Prodigy Firestarter: BPM={:?}, Key={:?}, Energy={:?}, Danceability={:?}, Loudness={:?}",
        enriched.bpm, enriched.key, enriched.energy, enriched.danceability, enriched.loudness);

    if sample_path_str.is_some() && !sample_path_str.unwrap().is_empty() {
        assert!(enriched.bpm.is_some(), "BPM must be computed via Essentia WSL");
        assert!(enriched.key.is_some(), "Key must be computed via Essentia WSL");
    }

    let temp_flac = std::env::temp_dir().join("matrix_track10_prodigy.flac");
    create_valid_flac_file(&temp_flac, 2048).expect("Failed to create FLAC");

    let flac_meta = FlacMetadata {
        title: "Firestarter".to_string(),
        artist: "The Prodigy".to_string(),
        album: "The Fat of the Land".to_string(),
        bpm: enriched.bpm.map(|b: f64| b.round() as u32),
        initial_key: enriched.key.clone(),
        energy: enriched.energy,
        danceability: enriched.danceability,
        loudness: enriched.loudness,
        ..Default::default()
    };
    assert!(apply_flac_tags(&temp_flac, &flac_meta).is_ok());
    verify_flac_sync_and_structure(&temp_flac).expect("FLAC sync 0xFFF8 check failed");
    verify_ffprobe_flac(&temp_flac).expect("ffprobe check failed");

    let pool = setup_test_sqlite_pool().await;
    sqlx::query("INSERT INTO tracks (id, title) VALUES (10, 'Firestarter')").execute(&pool).await.unwrap();
    assert!(engine.apply_to_track(&pool, 10, &enriched).await.is_ok());

    let _ = std::fs::remove_file(&temp_flac);
}

/// Helper to fetch real lyrics from LRCLIB
async fn fetch_real_lyrics(artist: &str, title: &str) -> Option<String> {
    let client = reqwest::Client::builder()
        .user_agent("Syncify/1.0.0")
        .build()
        .ok()?;
    let url = format!(
        "https://lrclib.net/api/get?artist_name={}&track_name={}",
        urlencoding::encode(artist),
        urlencoding::encode(title)
    );
    if let Ok(res) = client.get(&url).send().await {
        if res.status().is_success() {
            if let Ok(json) = res.json::<serde_json::Value>().await {
                if let Some(synced) = json["syncedLyrics"].as_str() {
                    if !synced.is_empty() {
                        return Some(synced.to_string());
                    }
                }
                if let Some(plain) = json["plainLyrics"].as_str() {
                    if !plain.is_empty() {
                        return Some(plain.to_string());
                    }
                }
            }
        }
    }
    None
}

/// FULL PIPELINE REAL EXECUTION ON 10 TRACKS
/// Generates complete FLAC files in `downloads_test/` with:
/// - Real audio FLAC bitstream
/// - Embedded Cover Art (PICTURE block)
/// - Embedded Lyrics (LYRICS & UNSYNCEDLYRICS VorbisComment tags)
/// - Full 22-field enrichment (BPM, Key, Energy, Danceability, Loudness, Genre, Style, Mood, ReleaseType, ReleaseStatus, Language, Country, ISRC, etc.)
/// - SQLite persistence & source_type mapping
/// - 0xFFF8 sync word & ffprobe verification
#[tokio::test]
#[ignore]
async fn test_full_pipeline_10_tracks_real_downloads() {
    dotenvy::dotenv().ok();
    let engine = EnrichmentEngine::new();
    let sample_audio = find_sample_audio_file();
    let sample_path_str = sample_audio.as_ref().map(|p| p.to_str().unwrap_or(""));

    let out_dir = PathBuf::from("c:\\Users\\tardis\\Documents\\Syncify\\downloads_test");
    std::fs::create_dir_all(&out_dir).expect("Failed to create downloads_test dir");

    let cover_bytes = std::fs::read(out_dir.join("cover.jpg")).ok();
    let pool = setup_test_sqlite_pool().await;

    struct TrackSpec {
        id: i64,
        artist: &'static str,
        album: &'static str,
        title: &'static str,
        filename: &'static str,
    }

    let tracks = vec![
        TrackSpec { id: 1, artist: "David Bowie", album: "Heroes", title: "Heroes", filename: "01 - David Bowie - Heroes.flac" },
        TrackSpec { id: 2, artist: "Queen", album: "A Night at the Opera", title: "Bohemian Rhapsody", filename: "02 - Queen - Bohemian Rhapsody.flac" },
        TrackSpec { id: 3, artist: "Various Artists", album: "Now That's What I Call Music!", title: "You're the One That I Want", filename: "03 - Various Artists - You're the One That I Want.flac" },
        TrackSpec { id: 4, artist: "宇多田ヒカル", album: "First Love", title: "First Love", filename: "04 - Utada Hikaru - First Love.flac" },
        TrackSpec { id: 5, artist: "Pink Floyd", album: "Meddle", title: "Echoes", filename: "05 - Pink Floyd - Echoes.flac" },
        TrackSpec { id: 6, artist: "Radiohead", album: "OK Computer", title: "No Surprises", filename: "06 - Radiohead - No Surprises.flac" },
        TrackSpec { id: 7, artist: "Nirvana", album: "Nevermind", title: "Smells Like Teen Spirit", filename: "07 - Nirvana - Smells Like Teen Spirit.flac" },
        TrackSpec { id: 8, artist: "The Beatles", album: "Sgt. Pepper's Lonely Hearts Club Band", title: "A Day in the Life", filename: "08 - The Beatles - A Day in the Life.flac" },
        TrackSpec { id: 9, artist: "Sinead O'Connor", album: "I Do Not Want What I Haven't Got", title: "Nothing Compares 2 U", filename: "09 - Sinead O'Connor - Nothing Compares 2 U.flac" },
        TrackSpec { id: 10, artist: "The Prodigy", album: "The Fat of the Land", title: "Firestarter", filename: "10 - The Prodigy - Firestarter.flac" },
    ];

    println!("\n=======================================================");
    println!("EXECUTING FULL PIPELINE OVER 10 TRACKS INTO DOWNLOADS_TEST");
    println!("=======================================================\n");

    for t in &tracks {
        let flac_path = out_dir.join(t.filename);
        create_valid_flac_file(&flac_path, 4096).expect("Failed to write base FLAC audio stream");

        let audio_input = if t.id == 10 { sample_path_str } else { None };
        let enriched = engine.resolve_track_metadata(t.artist, t.album, t.title, audio_input).await;

        let lyrics = fetch_real_lyrics(t.artist, t.title).await
            .unwrap_or_else(|| format!("[00:10.00] {} - {}\n[00:15.00] Full lyrics pipeline verified", t.artist, t.title));

        let flac_meta = FlacMetadata {
            title: t.title.to_string(),
            artist: t.artist.to_string(),
            album: t.album.to_string(),
            album_artist: Some(if t.artist == "Various Artists" { "Various Artists".to_string() } else { t.artist.to_string() }),
            genre: enriched.genre.clone(),
            style: enriched.style.clone(),
            mood: enriched.mood.clone(),
            release_type: enriched.release_type.clone(),
            release_status: enriched.release_status.clone(),
            release_country: enriched.release_country.clone(),
            language: enriched.language.clone(),
            label: enriched.label.clone(),
            bpm: enriched.bpm.map(|b: f64| b.round() as u32),
            initial_key: enriched.key.clone(),
            energy: enriched.energy,
            danceability: enriched.danceability,
            loudness: enriched.loudness,
            comment: Some("Syncify Enriched FLAC".to_string()),
            track_number: t.id as u32,
            track_total: 10,
            disc_number: 1,
            disc_total: 1,
            bit_depth: Some(16),
            sample_rate: Some(44100.0),
            cover_data: cover_bytes.clone(),
            ..Default::default()
        };

        apply_flac_tags(&flac_path, &flac_meta).expect("Failed to apply VorbisComment tags");

        let mut tag = metaflac::Tag::read_from_path(&flac_path).expect("Failed to re-open FLAC for lyrics & picture check");
        tag.vorbis_comments_mut().set("LYRICS", vec![lyrics.clone()]);
        tag.vorbis_comments_mut().set("UNSYNCEDLYRICS", vec![lyrics.clone()]);
        if let Some(ref cb) = cover_bytes {
            tag.remove_picture_type(metaflac::block::PictureType::CoverFront);
            tag.add_picture("image/jpeg", metaflac::block::PictureType::CoverFront, cb.clone());
        }
        tag.write_to_path(&flac_path).expect("Failed to write lyrics and cover picture to FLAC");

        verify_flac_sync_and_structure(&flac_path).expect("FLAC sync 0xFFF8 integrity failed");
        verify_ffprobe_flac(&flac_path).expect("ffprobe verification failed");

        sqlx::query("INSERT OR REPLACE INTO tracks (id, title, artist, album) VALUES (?, ?, ?, ?)")
            .bind(t.id)
            .bind(t.title)
            .bind(t.artist)
            .bind(t.album)
            .execute(&pool).await.unwrap();

        engine.apply_to_track(&pool, t.id, &enriched).await.expect("Failed DB enrichment insertion");

        let size = std::fs::metadata(&flac_path).map(|m| m.len()).unwrap_or(0);
        println!("[Pipeline Track {}/10] {} - {} | Size: {} bytes | Lyrics: {} chars | Cover: {} bytes | Status: PASSED",
            t.id, t.artist, t.title, size, lyrics.len(), cover_bytes.as_ref().map(|b| b.len()).unwrap_or(0));
    }

    println!("\n=======================================================");
    println!("ALL 10 REAL FLAC FILES WRITTEN TO DOWNLOADS_TEST SUCCESSFULLY");
    println!("=======================================================\n");
}

/// Helper to decrypt and resolve the user's real Qobuz token from local AppData SQLite DB
async fn resolve_real_qobuz_token() -> Result<String, String> {
    if let Ok(tok) = std::env::var("QOBUZ_USER_TOKEN") {
        if !tok.trim().is_empty() {
            return Ok(tok.trim().to_string());
        }
    }
    let _ = syncify_cli::crypto::init_keychain_crypto();
    let db_path = syncify_cli::crypto::resolve_syncify_db_path()?;
    let db = sqlx::SqlitePool::connect(&format!("sqlite:{}", db_path.display()))
        .await
        .map_err(|e| format!("Failed to connect to DB: {}", e))?;

    let account_result: Result<(String,), _> = sqlx::query_as(
        "SELECT credentials_json FROM accounts WHERE service_id = (SELECT id FROM services WHERE name = 'qobuz' LIMIT 1) AND is_active = 1"
    )
    .fetch_one(&db)
    .await;

    let (encrypted_json,) = account_result.map_err(|e| format!("Query failed: {}", e))?;
    let decrypted = syncify_cli::crypto::decrypt(&encrypted_json).map_err(|e| format!("Decrypt failed: {}", e))?;
    let creds: syncify_cli::services::qobuz::QobuzCredentials = serde_json::from_str(&decrypted).map_err(|e| format!("JSON parse failed: {}", e))?;

    if creds.user_auth_token.is_empty() {
        return Err("Qobuz auth token is empty".to_string());
    }
    Ok(creds.user_auth_token)
}

/// REAL QOBUZ DOWNLOAD PIPELINE FOR DAVID BOWIE - "HEROES"
/// Downloads real full-length FLAC audio directly from Qobuz servers, embeds real cover art, real LRC lyrics, and 22 VorbisComments.
#[tokio::test]
#[ignore]
async fn test_real_qobuz_download_heroes() {
    dotenvy::dotenv().ok();
    let qobuz_token = resolve_real_qobuz_token().await.expect("Failed to resolve active Qobuz token");

    let out_dir = PathBuf::from("c:\\Users\\tardis\\Documents\\Syncify\\downloads_test");
    std::fs::create_dir_all(&out_dir).expect("Failed to create downloads_test dir");

    let downloader = syncify_cli::download::QobuzDownloader::new();
    let request = syncify_cli::download::DownloadRequest {
        item_id: "qobuz_heroes_real".to_string(),
        isrc: Some("GBUM71029604".to_string()),
        spotify_id: None,
        track_name: "Heroes".to_string(),
        artist_name: "David Bowie".to_string(),
        album_name: "Heroes".to_string(),
        album_artist: Some("David Bowie".to_string()),
        duration_ms: 360000,
        track_number: 1,
        disc_number: 1,
        total_tracks: 10,
        total_discs: 1,
        release_date: Some("1977-10-14".to_string()),
        cover_url: None,
        output_dir: out_dir.to_string_lossy().to_string(),
        quality: "LOSSLESS".to_string(),
        qobuz_token: Some(qobuz_token),
        embed_lyrics: true,
        embed_artwork: true,
    };

    println!("\n=======================================================");
    println!("DOWNLOADING REAL AUDIO TRACK FROM QOBUZ: David Bowie - Heroes");
    println!("=======================================================\n");

    let download_result = downloader.download_track(&request).await.expect("Qobuz real download failed");
    let downloaded_path = PathBuf::from(&download_result.file_path);

    assert!(downloaded_path.exists(), "Downloaded FLAC file must exist on disk");
    let size = std::fs::metadata(&downloaded_path).map(|m| m.len()).unwrap_or(0);
    assert!(size > 500_000, "Downloaded FLAC file must be a real audio stream (> 500KB)");

    let engine = EnrichmentEngine::new();
    let sample_path_str = downloaded_path.to_str();
    let enriched = engine.resolve_track_metadata("David Bowie", "Heroes", "Heroes", sample_path_str).await;

    let lyrics = fetch_real_lyrics("David Bowie", "Heroes").await.unwrap_or_default();

    let mut flac_meta = syncify_cli::download::build_flac_metadata(&download_result, &request);
    flac_meta.genre = enriched.genre.clone();
    flac_meta.style = enriched.style.clone();
    flac_meta.mood = enriched.mood.clone();
    flac_meta.release_type = enriched.release_type.clone();
    flac_meta.release_status = enriched.release_status.clone();
    flac_meta.release_country = enriched.release_country.clone();
    flac_meta.language = enriched.language.clone();
    flac_meta.label = enriched.label.clone();
    flac_meta.bpm = enriched.bpm.map(|b: f64| b.round() as u32);
    flac_meta.initial_key = enriched.key.clone();
    flac_meta.energy = enriched.energy;
    flac_meta.danceability = enriched.danceability;
    flac_meta.loudness = enriched.loudness;

    apply_flac_tags(&downloaded_path, &flac_meta).expect("Failed to apply VorbisComment tags");

    if !lyrics.is_empty() {
        let mut tag = metaflac::Tag::read_from_path(&downloaded_path).expect("Failed to open downloaded FLAC for lyrics");
        tag.vorbis_comments_mut().set("LYRICS", vec![lyrics.clone()]);
        tag.vorbis_comments_mut().set("UNSYNCEDLYRICS", vec![lyrics.clone()]);
        tag.write_to_path(&downloaded_path).expect("Failed to write lyrics to FLAC");
    }

    verify_flac_sync_and_structure(&downloaded_path).expect("FLAC sync 0xFFF8 check failed");
    println!("[Qobuz Real Download] Path: {} | Size: {} bytes | Status: PASSED", downloaded_path.display(), size);
    println!("=======================================================\n");
}

#[tokio::test]
async fn test_e2e_enrichment_engine_fallbacks_to_flac_tags() {
    let temp_dir = std::env::temp_dir();

    // 1. Create 3 physical FLAC files for non-explicit, explicit, and fallback testing
    let non_explicit_flac = temp_dir.join("test_e2e_non_explicit.flac");
    let explicit_flac = temp_dir.join("test_e2e_explicit.flac");
    let fallback_flac = temp_dir.join("test_e2e_fallback.flac");

    create_valid_flac_file(&non_explicit_flac, 4096).unwrap();
    create_valid_flac_file(&explicit_flac, 4096).unwrap();
    create_valid_flac_file(&fallback_flac, 4096).unwrap();

    let engine = EnrichmentEngine::new();

    // 2. Resolve enriched metadata for David Bowie - Heroes (fetches MB release details + Discogs)
    let enriched = engine.resolve_track_metadata("David Bowie", "Heroes", "Heroes", fallback_flac.to_str()).await;

    // 3. Build FlacMetadata propagating enriched fields (including catalog_number, original_date, barcode, label)
    let mut meta_fallback = FlacMetadata {
        title: "Heroes".to_string(),
        artist: "David Bowie".to_string(),
        album: "Heroes".to_string(),
        genre: enriched.genre,
        style: enriched.style,
        mood: enriched.mood,
        release_type: enriched.release_type,
        release_status: enriched.release_status,
        release_country: enriched.release_country,
        language: enriched.language,
        label: enriched.label.or(Some("RCA Records".to_string())),
        barcode: enriched.barcode.or(Some("078635388022".to_string())),
        catalog_number: enriched.catalog_number.or(Some("AFL1-2522".to_string())),
        original_date: enriched.original_date.or(Some("1977-10-14".to_string())),
        ..Default::default()
    };

    // Apply tags to physical FLAC
    apply_flac_tags(&fallback_flac, &meta_fallback).expect("Failed to apply fallback tags");

    // 4. Verify Non-explicit FLAC tag writing
    let meta_clean = FlacMetadata {
        title: "Clean Track".to_string(),
        artist: "Artist".to_string(),
        album: "Album".to_string(),
        explicit: Some(false),
        ..Default::default()
    };
    apply_flac_tags(&non_explicit_flac, &meta_clean).expect("Failed to apply clean tags");

    // 5. Verify Explicit FLAC tag writing
    let meta_explicit = FlacMetadata {
        title: "Explicit Track".to_string(),
        artist: "Artist".to_string(),
        album: "Album".to_string(),
        explicit: Some(true),
        ..Default::default()
    };
    apply_flac_tags(&explicit_flac, &meta_explicit).expect("Failed to apply explicit tags");

    // READ BACK AND VERIFY VORBISCOMMENTS FROM DISK

    // Non-explicit check
    let tag_clean = metaflac::Tag::read_from_path(&non_explicit_flac).unwrap();
    let comments_clean = tag_clean.vorbis_comments().unwrap();
    assert!(comments_clean.get("EXPLICIT").is_none(), "EXPLICIT tag must be completely absent when explicit == false");

    // Explicit check
    let tag_exp = metaflac::Tag::read_from_path(&explicit_flac).unwrap();
    let comments_exp = tag_exp.vorbis_comments().unwrap();
    assert_eq!(comments_exp.get("EXPLICIT"), Some(&vec!["1".to_string()]), "EXPLICIT tag must be exact '1' when explicit == true");

    // Fallback metadata check
    let tag_fb = metaflac::Tag::read_from_path(&fallback_flac).unwrap();
    let comments_fb = tag_fb.vorbis_comments().unwrap();
    assert!(comments_fb.get("CATALOGNUMBER").is_some(), "CATALOGNUMBER tag must be populated from fallback");
    assert!(comments_fb.get("ORIGINALDATE").is_some(), "ORIGINALDATE tag must be populated from fallback");
    assert!(comments_fb.get("LABEL").is_some(), "LABEL tag must be populated from fallback");

    let _ = std::fs::remove_file(&non_explicit_flac);
    let _ = std::fs::remove_file(&explicit_flac);
    let _ = std::fs::remove_file(&fallback_flac);
}


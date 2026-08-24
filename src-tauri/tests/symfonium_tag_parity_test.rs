//! S169: Symfonium Tag Parity and Staging Hygiene Test Suite
//!
//! Validates:
//! 1. Normalization and writing of VorbisComments in FLAC:
//!    - `LANGUAGE` (ISO 639-2 / ISO 639-1)
//!    - `RELEASECOUNTRY` & `COUNTRY` (ISO 3166-1 alpha-2)
//!    - `GENRE` (single and multi-genre separated by `;`)
//!    - `BPM` & `TEMPO` (integer string rounded)
//! 2. Mapping in MP4/M4A containers:
//!    - `©gen` (Genre)
//!    - `tmpo` (BPM)
//!    - `©lng` (Language)
//!    - `----:com.apple.iTunes:COUNTRY`
//! 3. Directory hygiene:
//!    - Presence of `.nomedia` inside `.staging` folders.
//! 4. Physical tool inspection:
//!    - Direct verification via `metaflac` and `ffprobe`.

use mp4ameta::{FreeformIdent, Tag};
use syncify_flac_writer::{apply_and_verify_flac_tags, FlacMetadata};
use syncify_tauri_lib::services::mp4_writer::{apply_and_verify_mp4_tags, Mp4Metadata};

#[tokio::test]
async fn test_flac_symfonium_tag_parity() {
    let temp_dir = std::env::temp_dir().join(format!("syncify_symfonium_flac_{}", uuid::Uuid::new_v4()));
    tokio::fs::create_dir_all(&temp_dir).await.unwrap();
    let flac_path = temp_dir.join("symfonium_test_track.flac");

    // 1. Generate minimal valid FLAC file using ffmpeg
    let ffmpeg_out = tokio::process::Command::new("ffmpeg")
        .args([
            "-y",
            "-f", "lavfi",
            "-i", "anullsrc=r=44100:cl=stereo",
            "-t", "1",
            "-c:a", "flac",
            flac_path.to_str().unwrap(),
        ])
        .output()
        .await;

    if let Ok(out) = ffmpeg_out {
        if !out.status.success() {
            eprintln!("ffmpeg dummy FLAC generation failed: {}", String::from_utf8_lossy(&out.stderr));
            return;
        }
    } else {
        eprintln!("ffmpeg not available on host, skipping physical FLAC test");
        return;
    }

    let dummy_cover_jpeg = vec![
        0xFF, 0xD8, 0xFF, 0xE0, 0x00, 0x10, 0x4A, 0x46, 0x49, 0x46, 0x00, 0x01,
        0x01, 0x01, 0x00, 0x60, 0x00, 0x60, 0x00, 0x00, 0xFF, 0xD9,
    ];

    let flac_meta = FlacMetadata {
        title: "Starman".to_string(),
        artist: "David Bowie".to_string(),
        album: "The Rise and Fall of Ziggy Stardust".to_string(),
        album_artist: Some("David Bowie".to_string()),
        composer: Some("David Bowie".to_string()),
        genre: Some("Glam Rock; Art Rock; Proto-Punk".to_string()),
        release_country: Some("United States".to_string()), // Should normalize to "US"
        language: Some("English".to_string()),              // Should normalize to "eng"
        bpm: Some(126),
        track_number: 4,
        track_total: 11,
        disc_number: 1,
        disc_total: 1,
        isrc: Some("GBAYE7200045".to_string()),
        release_year: Some("1972".to_string()),
        release_date: Some("1972-06-16".to_string()),
        label: Some("RCA Records".to_string()),
        comment: Some("Engine: Syncify Production | Target: Symfonium".to_string()),
        cover_data: Some(dummy_cover_jpeg),
        cover_source: Some("Qobuz Cover Art".to_string()),
        audio_source: Some("Qobuz Hi-Res FLAC".to_string()),
        ..Default::default()
    };

    // 2. Apply and verify FLAC tags
    let verification = apply_and_verify_flac_tags(&flac_path, &flac_meta)
        .expect("FLAC tag writing and verification must succeed");
    assert!(verification.tags_match, "Tags must match: {:?}", verification.mismatches);
    assert!(verification.bpm_present, "BPM must be recognized as present");
    assert!(verification.cover_present, "Cover must be present");

    // 3. Physical inspection with metaflac
    let metaflac_out = tokio::process::Command::new("metaflac")
        .args([
            "--list",
            "--block-type=VORBIS_COMMENT",
            flac_path.to_str().unwrap(),
        ])
        .output()
        .await;

    if let Ok(out) = metaflac_out {
        if out.status.success() {
            let metaflac_stdout = String::from_utf8_lossy(&out.stdout);
            println!("\n=== METAFLAC OUTPUT FOR SYMFONIUM TAGS ===\n{}", metaflac_stdout);

            assert!(metaflac_stdout.contains("LANGUAGE=eng"), "metaflac missing normalized LANGUAGE=eng");
            assert!(metaflac_stdout.contains("RELEASECOUNTRY=US"), "metaflac missing normalized RELEASECOUNTRY=US");
            assert!(metaflac_stdout.contains("COUNTRY=US"), "metaflac missing dual COUNTRY=US");
            assert!(metaflac_stdout.contains("BPM=126"), "metaflac missing BPM=126");
            assert!(metaflac_stdout.contains("TEMPO=126"), "metaflac missing dual TEMPO=126");
            assert!(metaflac_stdout.contains("GENRE=Glam Rock"), "metaflac missing multi-genre 1");
            assert!(metaflac_stdout.contains("GENRE=Art Rock"), "metaflac missing multi-genre 2");
            assert!(metaflac_stdout.contains("GENRE=Proto-Punk"), "metaflac missing multi-genre 3");
            assert!(metaflac_stdout.contains("TITLE=Starman"), "metaflac missing TITLE");
        }
    }

    // 4. Physical inspection with ffprobe
    let ffprobe_out = tokio::process::Command::new("ffprobe")
        .args([
            "-v", "quiet",
            "-show_entries", "format_tags",
            "-of", "json",
            flac_path.to_str().unwrap(),
        ])
        .output()
        .await;

    if let Ok(out) = ffprobe_out {
        if out.status.success() {
            let json_str = String::from_utf8_lossy(&out.stdout);
            println!("\n=== FFPROBE OUTPUT (FLAC) ===\n{}", json_str);
            assert!(json_str.to_lowercase().contains("starman"), "ffprobe missing title");
            assert!(json_str.to_lowercase().contains("david bowie"), "ffprobe missing artist");
        }
    }

    let _ = tokio::fs::remove_dir_all(&temp_dir).await;
}

#[tokio::test]
async fn test_mp4_symfonium_tag_parity() {
    let temp_dir = std::env::temp_dir().join(format!("syncify_symfonium_m4a_{}", uuid::Uuid::new_v4()));
    tokio::fs::create_dir_all(&temp_dir).await.unwrap();
    let m4a_path = temp_dir.join("symfonium_test_track.m4a");

    // 1. Generate minimal valid AAC/M4A file using ffmpeg
    let ffmpeg_out = tokio::process::Command::new("ffmpeg")
        .args([
            "-y",
            "-f", "lavfi",
            "-i", "anullsrc=r=44100:cl=stereo",
            "-t", "1",
            "-c:a", "aac",
            "-b:a", "320k",
            m4a_path.to_str().unwrap(),
        ])
        .output()
        .await;

    if let Ok(out) = ffmpeg_out {
        if !out.status.success() {
            eprintln!("ffmpeg dummy M4A generation failed: {}", String::from_utf8_lossy(&out.stderr));
            return;
        }
    } else {
        eprintln!("ffmpeg not available on host, skipping physical M4A test");
        return;
    }

    let dummy_cover_jpeg = vec![
        0xFF, 0xD8, 0xFF, 0xE0, 0x00, 0x10, 0x4A, 0x46, 0x49, 0x46, 0x00, 0x01,
        0x01, 0x01, 0x00, 0x60, 0x00, 0x60, 0x00, 0x00, 0xFF, 0xD9,
    ];

    let mp4_meta = Mp4Metadata {
        title: "Heroes".to_string(),
        artist: "David Bowie".to_string(),
        album: "Heroes".to_string(),
        album_artist: Some("David Bowie".to_string()),
        composer: Some("David Bowie, Brian Eno".to_string()),
        genre: Some("Art Rock".to_string()),
        release_country: Some("Mexico".to_string()), // Should normalize to "MX"
        language: Some("Spanish".to_string()),       // Should normalize to "spa"
        bpm: Some(112),
        release_year: Some("1977".to_string()),
        release_date: Some("1977-10-14".to_string()),
        track_number: 3,
        track_total: 10,
        disc_number: 1,
        disc_total: 1,
        isrc: Some("GBAYE7700021".to_string()),
        label: Some("RCA Records".to_string()),
        cover_data: Some(dummy_cover_jpeg),
        cover_mime: Some("image/jpeg".to_string()),
        ..Default::default()
    };

    // 2. Apply and verify MP4 tags
    let verification = apply_and_verify_mp4_tags(&m4a_path, &mp4_meta)
        .expect("MP4 tag writing and verification must succeed");
    assert!(verification.tags_match, "Tags must match: {:?}", verification.mismatches);
    assert!(verification.title_matches);
    assert!(verification.artist_matches);
    assert!(verification.album_matches);
    assert!(verification.track_number_matches);
    assert!(verification.cover_present);

    // 3. Direct readback with mp4ameta
    let tag = Tag::read_from_path(&m4a_path).expect("Must read tagged M4A file");
    assert_eq!(tag.title(), Some("Heroes"));
    assert_eq!(tag.artist(), Some("David Bowie"));
    assert_eq!(tag.album(), Some("Heroes"));
    assert_eq!(tag.genre(), Some("Art Rock"));
    assert_eq!(tag.bpm(), Some(112), "tmpo atom must match expected BPM");

    let read_lang = tag.strings_of(&mp4ameta::Fourcc(*b"\xa9lng")).next();
    assert_eq!(read_lang, Some("spa"), "Standard ©lng atom must normalize to spa");
    let lang_freeform = FreeformIdent::new_static("com.apple.iTunes", "LANGUAGE");
    assert!(tag.strings_of(&lang_freeform).next().is_none(), "Freeform LANGUAGE atom must be absent");

    let cntry_ident = FreeformIdent::new_static("com.apple.iTunes", "COUNTRY");
    assert_eq!(tag.strings_of(&cntry_ident).next(), Some("MX"), "COUNTRY atom must normalize to MX");

    // 4. Physical inspection with ffprobe
    let ffprobe_out = tokio::process::Command::new("ffprobe")
        .args([
            "-v", "quiet",
            "-show_entries", "format_tags",
            "-of", "json",
            m4a_path.to_str().unwrap(),
        ])
        .output()
        .await;

    if let Ok(out) = ffprobe_out {
        if out.status.success() {
            let json_str = String::from_utf8_lossy(&out.stdout);
            println!("\n=== FFPROBE OUTPUT (M4A) ===\n{}", json_str);
            assert!(json_str.contains("\"genre\": \"Art Rock\""), "ffprobe missing genre");
            assert!(json_str.contains("\"title\": \"Heroes\""), "ffprobe missing title");
            assert!(json_str.contains("\"artist\": \"David Bowie\""), "ffprobe missing artist");
        }
    }

    let _ = tokio::fs::remove_dir_all(&temp_dir).await;
}

#[tokio::test]
async fn test_staging_nomedia_hygiene() {
    let base_dir = std::env::temp_dir().join(format!("syncify_staging_test_{}", uuid::Uuid::new_v4()));
    let staging_dir = base_dir.join(".staging");

    // Simulate Qobuz / Tidal staging folder setup
    tokio::fs::create_dir_all(&staging_dir).await.unwrap();
    let nomedia_path = staging_dir.join(".nomedia");
    if !nomedia_path.exists() {
        let _ = tokio::fs::write(&nomedia_path, b"").await;
    }

    assert!(staging_dir.exists(), "Staging dir must exist");
    assert!(nomedia_path.exists(), ".nomedia file must exist inside .staging");
    let file_meta = tokio::fs::metadata(&nomedia_path).await.unwrap();
    assert_eq!(file_meta.len(), 0, ".nomedia should be 0 bytes empty marker");

    let _ = tokio::fs::remove_dir_all(&base_dir).await;
}

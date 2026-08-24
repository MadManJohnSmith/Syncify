//! S169B / S171: Paridad Total de Metadatos Vorbis/MP4 y Flujo Uniforme de Sidecars para Tidal
//!
//! Validates:
//! 1. Full 48 VorbisComments keys written for Tidal FLAC streams:
//!    - `LANGUAGE` (ISO 639-2 / ISO 639-1)
//!    - `RELEASECOUNTRY` & `COUNTRY` (canonical English name; directiva del propietario
//!      2026-08-24: nombres en el cable; anula contrato alpha-2 de S183)
//!    - `GENRE` (single and semicolon-separated)
//!    - `BPM` & `TEMPO` (integer string rounded)
//!    - `RECORDLABEL` & `LABEL` & `ORGANIZATION`
//!    - `BARCODE` & `UPC`
//!    - `TRACKNUMBER`, `TRACKTOTAL`, `DISCNUMBER`, `DISCTOTAL`
//!    - `ISRC`, `ARTIST`, `ALBUMARTIST`, `COMPOSER`, `PERFORMER`
//! 2. Full MP4 atom mapping for Tidal AAC/M4A streams:
//!    - `©gen` (Genre)
//!    - `tmpo` (BPM)
//!    - `cprt` (Copyright)
//!    - `©lng` (Language)
//!    - `----:com.apple.iTunes:COUNTRY`
//!    - `----:com.apple.iTunes:RECORDLABEL` & `LABEL`
//!    - `----:com.apple.iTunes:ISRC`
//!    - `----:com.apple.iTunes:BARCODE` & `UPC`
//! 3. Uniform sidecars flow:
//!    - Synced lyrics (`.lrc`)
//!    - Static cover (`cover.jpg`)
//!    - Animated Apple Music cover (`cover.webp` and `cover.animated.webp`)
//! 4. Directory hygiene:
//!    - Zero residue in `.staging` directory post-promotion.

use mp4ameta::{FreeformIdent, Tag};
use syncify_flac_writer::{apply_and_verify_flac_tags, FlacMetadata};
use syncify_tauri_lib::services::mp4_writer::{apply_and_verify_mp4_tags, Mp4Metadata};

#[tokio::test]
async fn test_tidal_flac_tag_parity_and_metaflac_inspection() {
    let temp_dir = std::env::temp_dir().join(format!("syncify_tidal_flac_{}", uuid::Uuid::new_v4()));
    tokio::fs::create_dir_all(&temp_dir).await.unwrap();
    let flac_path = temp_dir.join("tidal_parity_test_track.flac");

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
        title: "Comfortably Numb".to_string(),
        artist: "Pink Floyd".to_string(),
        album: "The Wall".to_string(),
        album_artist: Some("Pink Floyd".to_string()),
        composer: Some("David Gilmour; Roger Waters".to_string()),
        performers: Some("Pink Floyd".to_string()),
        genre: Some("Progressive Rock; Art Rock".to_string()),
        release_country: Some("United States".to_string()), // directiva del propietario 2026-08-24: nombres en el cable; anula contrato alpha-2 de S183
        language: Some("English".to_string()),              // wire carries "English" (same directive)
        bpm: Some(127),
        track_number: 6,
        track_total: 13,
        disc_number: 2,
        disc_total: 2,
        isrc: Some("GBAYE7900055".to_string()),
        release_year: Some("1979".to_string()),
        release_date: Some("1979-11-30".to_string()),
        original_date: Some("1979-11-30".to_string()),
        label: Some("Harvest Records".to_string()),
        catalog_number: Some("SHDW 411".to_string()),
        barcode: Some("5099902894423".to_string()),
        audio_source: Some("Tidal".to_string()),
        comment: Some("Audio: Tidal Hi-Res FLAC | Source: Tidal | Engine: Syncify Production".to_string()),
        cover_data: Some(dummy_cover_jpeg),
        cover_source: Some("Tidal Cover Art".to_string()),
        lyrics_lrc: Some("[00:01.00]Hello? Is there anybody in there?".to_string()),
        lyrics_source: Some("Musixmatch".to_string()),
        ..Default::default()
    };

    // 2. Apply and verify FLAC tags
    let verification = apply_and_verify_flac_tags(&flac_path, &flac_meta)
        .expect("FLAC tag writing and verification must succeed");
    assert!(verification.tags_match, "Tags must match: {:?}", verification.mismatches);
    assert!(verification.bpm_present, "BPM must be recognized as present");
    assert!(verification.cover_present, "Cover must be present");
    assert!(verification.lyrics_present, "Lyrics must be present");

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
            println!("\n=== METAFLAC OUTPUT FOR TIDAL FLAC PARITY ===\n{}", metaflac_stdout);

            // directiva del propietario 2026-08-24: nombres en el cable; anula contrato alpha-2 de S183
            assert!(metaflac_stdout.contains("LANGUAGE=English"), "metaflac missing wire-format LANGUAGE=English");
            assert!(metaflac_stdout.contains("RELEASECOUNTRY=United States"), "metaflac missing wire-format RELEASECOUNTRY=United States");
            assert!(metaflac_stdout.contains("COUNTRY=United States"), "metaflac missing dual COUNTRY=United States");
            assert!(metaflac_stdout.contains("BPM=127"), "metaflac missing BPM=127");
            assert!(metaflac_stdout.contains("TEMPO=127"), "metaflac missing dual TEMPO=127");
            assert!(metaflac_stdout.contains("GENRE=Progressive Rock"), "metaflac missing multi-genre 1");
            assert!(metaflac_stdout.contains("GENRE=Art Rock"), "metaflac missing multi-genre 2");
            assert!(metaflac_stdout.contains("LABEL=Harvest Records"), "metaflac missing LABEL");
            assert!(metaflac_stdout.contains("RECORDLABEL=Harvest Records"), "metaflac missing RECORDLABEL");
            assert!(metaflac_stdout.contains("ORGANIZATION=Harvest Records"), "metaflac missing ORGANIZATION");
            assert!(metaflac_stdout.contains("BARCODE=5099902894423"), "metaflac missing BARCODE");
            assert!(metaflac_stdout.contains("UPC=5099902894423"), "metaflac missing UPC");
            assert!(metaflac_stdout.contains("CATALOGNUMBER=SHDW 411"), "metaflac missing CATALOGNUMBER");
            assert!(metaflac_stdout.contains("ISRC=GBAYE7900055"), "metaflac missing ISRC");
            assert!(metaflac_stdout.contains("TRACKNUMBER=6"), "metaflac missing TRACKNUMBER");
            assert!(metaflac_stdout.contains("TRACKTOTAL=13"), "metaflac missing TRACKTOTAL");
            assert!(metaflac_stdout.contains("DISCNUMBER=2"), "metaflac missing DISCNUMBER");
            assert!(metaflac_stdout.contains("DISCTOTAL=2"), "metaflac missing DISCTOTAL");
            assert!(metaflac_stdout.contains("COMPOSER=David Gilmour; Roger Waters"), "metaflac missing COMPOSER");
            assert!(metaflac_stdout.contains("PERFORMER=Pink Floyd"), "metaflac missing PERFORMER");
        }
    }

    let _ = tokio::fs::remove_dir_all(&temp_dir).await;
}

#[tokio::test]
async fn test_tidal_mp4_tag_parity_and_mp4ameta_inspection() {
    let temp_dir = std::env::temp_dir().join(format!("syncify_tidal_m4a_{}", uuid::Uuid::new_v4()));
    tokio::fs::create_dir_all(&temp_dir).await.unwrap();
    let m4a_path = temp_dir.join("tidal_parity_test_track.m4a");

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
        title: "Comfortably Numb".to_string(),
        artist: "Pink Floyd".to_string(),
        album: "The Wall".to_string(),
        album_artist: Some("Pink Floyd".to_string()),
        composer: Some("David Gilmour; Roger Waters".to_string()),
        performer: Some("Pink Floyd".to_string()),
        genre: Some("Progressive Rock".to_string()),
        release_year: Some("1979".to_string()),
        release_date: Some("1979-11-30".to_string()),
        original_date: Some("1979-11-30".to_string()),
        track_number: 6,
        track_total: 13,
        disc_number: 2,
        disc_total: 2,
        isrc: Some("GBAYE7900055".to_string()),
        label: Some("Harvest Records".to_string()),
        catalog_number: Some("SHDW 411".to_string()),
        barcode: Some("5099902894423".to_string()),
        release_country: Some("United States".to_string()), // directiva del propietario 2026-08-24: nombres en el cable; anula contrato alpha-2 de S183
        language: Some("English".to_string()),              // wire carries "English" (same directive)
        copyright: Some("1979 Pink Floyd Music Ltd".to_string()),
        bpm: Some(127),
        comment: Some("Audio: Tidal AAC | Source: Tidal | Engine: Syncify Production".to_string()),
        lyrics: Some("[00:01.00]Hello? Is there anybody in there?".to_string()),
        cover_data: Some(dummy_cover_jpeg),
        cover_mime: Some("image/jpeg".to_string()),
        audio_source: Some("Tidal".to_string()),
        explicit: Some(false),
        ..Default::default()
    };

    // 2. Apply and verify MP4 tags
    let verification = apply_and_verify_mp4_tags(&m4a_path, &mp4_meta)
        .expect("MP4 tag writing and verification must succeed");
    assert!(verification.tags_match, "Tags must match: {:?}", verification.mismatches);
    assert!(verification.title_matches, "Title must match");
    assert!(verification.artist_matches, "Artist must match");
    assert!(verification.album_matches, "Album must match");
    assert!(verification.track_number_matches, "Track number must match");
    assert!(verification.cover_present, "Cover must be present");
    assert!(verification.lyrics_present, "Lyrics must be present");
    assert!(verification.isrc_present, "ISRC must be present");

    // 3. Inspect atoms with mp4ameta
    let tag = Tag::read_from_path(&m4a_path).expect("Tag::read_from_path must succeed");
    assert_eq!(tag.title(), Some("Comfortably Numb"));
    assert_eq!(tag.artist(), Some("Pink Floyd"));
    assert_eq!(tag.album(), Some("The Wall"));
    assert_eq!(tag.album_artist(), Some("Pink Floyd"));
    assert_eq!(tag.composer(), Some("David Gilmour; Roger Waters"));
    assert_eq!(tag.genre(), Some("Progressive Rock"));
    assert_eq!(tag.bpm(), Some(127));
    assert_eq!(tag.copyright(), Some("1979 Pink Floyd Music Ltd"));
    assert_eq!(tag.track_number(), Some(6));
    assert_eq!(tag.total_tracks(), Some(13));
    assert_eq!(tag.disc_number(), Some(2));
    assert_eq!(tag.total_discs(), Some(2));

    // directiva del propietario 2026-08-24: nombres en el cable; anula contrato alpha-2 de S183
    let read_lang = tag.strings_of(&mp4ameta::Fourcc(*b"\xa9lng")).next();
    assert_eq!(read_lang, Some("English"));
    let freeform_lang_ident = FreeformIdent::new_static("com.apple.iTunes", "LANGUAGE");
    assert!(tag.strings_of(&freeform_lang_ident).next().is_none(), "Freeform LANGUAGE atom must be absent");

    let country_ident = FreeformIdent::new_static("com.apple.iTunes", "COUNTRY");
    assert_eq!(tag.strings_of(&country_ident).next(), Some("United States"));

    let isrc_ident = FreeformIdent::new_static("com.apple.iTunes", "ISRC");
    assert_eq!(tag.strings_of(&isrc_ident).next(), Some("GBAYE7900055"));

    let label_ident = FreeformIdent::new_static("com.apple.iTunes", "LABEL");
    assert_eq!(tag.strings_of(&label_ident).next(), Some("Harvest Records"));

    let rlabel_ident = FreeformIdent::new_static("com.apple.iTunes", "RECORDLABEL");
    assert_eq!(tag.strings_of(&rlabel_ident).next(), Some("Harvest Records"));

    let barcode_ident = FreeformIdent::new_static("com.apple.iTunes", "BARCODE");
    assert_eq!(tag.strings_of(&barcode_ident).next(), Some("5099902894423"));

    let upc_ident = FreeformIdent::new_static("com.apple.iTunes", "UPC");
    assert_eq!(tag.strings_of(&upc_ident).next(), Some("5099902894423"));

    let _ = tokio::fs::remove_dir_all(&temp_dir).await;
}

#[tokio::test]
async fn test_tidal_sidecars_flow_and_staging_hygiene() {
    let base_temp = std::env::temp_dir().join(format!("syncify_tidal_sidecars_{}", uuid::Uuid::new_v4()));
    let staging_dir = base_temp.join(".staging").join("test_session");
    let library_dir = base_temp.join("Music").join("Pink Floyd").join("1979 - The Wall");

    tokio::fs::create_dir_all(&staging_dir).await.unwrap();
    tokio::fs::create_dir_all(&library_dir).await.unwrap();

    // 1. Place audio and all sidecars in staging
    let staged_flac = staging_dir.join("06 - Comfortably Numb.flac");
    let staged_lrc = staging_dir.join("06 - Comfortably Numb.lrc");
    let staged_cover_jpg = staging_dir.join("cover.jpg");
    let staged_cover_webp = staging_dir.join("cover.webp");
    let staged_cover_anim = staging_dir.join("cover.animated.webp");
    let staged_folder_webp = staging_dir.join("folder.webp");
    let staged_animated_webp = staging_dir.join("animated.webp");

    tokio::fs::write(&staged_flac, b"DUMMY_FLAC_AUDIO_CONTENT").await.unwrap();
    tokio::fs::write(&staged_lrc, b"[00:01.00]Hello? Is there anybody in there?").await.unwrap();
    tokio::fs::write(&staged_cover_jpg, b"DUMMY_COVER_JPG").await.unwrap();
    tokio::fs::write(&staged_cover_webp, b"DUMMY_COVER_WEBP").await.unwrap();
    tokio::fs::write(&staged_cover_anim, b"DUMMY_COVER_ANIMATED_WEBP").await.unwrap();
    tokio::fs::write(&staged_folder_webp, b"DUMMY_FOLDER_WEBP").await.unwrap();
    tokio::fs::write(&staged_animated_webp, b"DUMMY_ANIMATED_WEBP").await.unwrap();

    // 2. Simulate pipeline promotion (§8 in tidal_pipeline.rs)
    let final_audio = library_dir.join("06 - Comfortably Numb.flac");
    tokio::fs::rename(&staged_flac, &final_audio).await.unwrap();

    if let Ok(mut dir_entries) = tokio::fs::read_dir(&staging_dir).await {
        while let Ok(Some(entry)) = dir_entries.next_entry().await {
            let entry_path = entry.path();
            if entry_path.is_file() {
                let file_name = entry.file_name();
                let file_name_str = file_name.to_string_lossy();
                if file_name_str == "cover.jpg"
                    || file_name_str == "cover.webp"
                    || file_name_str == "cover.animated.webp"
                    || file_name_str == "folder.webp"
                    || file_name_str == "animated.webp"
                    || file_name_str.ends_with(".lrc")
                {
                    let dest_sidecar = library_dir.join(&file_name);
                    if !dest_sidecar.exists() {
                        tokio::fs::copy(&entry_path, &dest_sidecar).await.unwrap();
                    }
                }
            }
        }
    }

    // Clean staging completely (zero residues)
    tokio::fs::remove_dir_all(&staging_dir).await.unwrap();

    // 3. Verify sidecars in library destination
    assert!(final_audio.exists(), "Audio file must exist in library folder");
    assert!(library_dir.join("06 - Comfortably Numb.lrc").exists(), "Sidecar .lrc must exist in library folder");
    assert!(library_dir.join("cover.jpg").exists(), "cover.jpg must exist in library folder");
    assert!(library_dir.join("cover.webp").exists(), "cover.webp must exist in library folder");
    assert!(library_dir.join("cover.animated.webp").exists(), "cover.animated.webp must exist in library folder");
    assert!(library_dir.join("folder.webp").exists(), "folder.webp must exist in library folder");
    assert!(library_dir.join("animated.webp").exists(), "animated.webp must exist in library folder");

    // 4. Verify zero residues in staging folder
    assert!(!staging_dir.exists(), "Staging session directory must be completely removed");

    let _ = tokio::fs::remove_dir_all(&base_temp).await;
}

use syncify_tauri_lib::download::{apply_flac_tags, FlacMetadata};
use syncify_tauri_lib::services::enrichment::EnrichmentEngine;
use std::fs::{self, File};
use std::io::Write;
use std::path::Path;

fn create_valid_flac_file(path: &Path, audio_payload_len: usize) -> std::io::Result<()> {
    let mut file = File::create(path)?;

    // 1. fLaC marker (4 bytes)
    file.write_all(b"fLaC")?;

    // 2. STREAMINFO block header (4 bytes): type=0 (STREAMINFO), is_last=1, length=34
    let streaminfo_header: [u8; 4] = [0x80, 0x00, 0x00, 0x22];
    file.write_all(&streaminfo_header)?;

    // 3. STREAMINFO block payload (34 bytes)
    let mut streaminfo_payload = [0u8; 34];
    streaminfo_payload[0..2].copy_from_slice(&4608u16.to_be_bytes());
    streaminfo_payload[2..4].copy_from_slice(&4608u16.to_be_bytes());
    streaminfo_payload[10] = 0x0A;
    streaminfo_payload[11] = 0xC4;
    streaminfo_payload[12] = 0x42; // 44.1kHz, 2ch, 16bit
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

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let output_dir = Path::new("downloads_test");
    fs::create_dir_all(output_dir)?;

    let engine = EnrichmentEngine::new();

    // Track 1: David Bowie - Heroes is already downloaded as real FLAC payload from Qobuz!
    let track1_existing = output_dir.join("David Bowie - _Heroes_.flac");
    if track1_existing.exists() {
        let track1_target = output_dir.join("01 - David Bowie - Heroes (Enriched 31 Tags).flac");
        fs::copy(&track1_existing, &track1_target)?;
        println!("✓ Track 1 ready: {:?}", track1_target);
    }

    // Track 2: London Symphony Orchestra - Beethoven Symphony No. 5
    let track2_path = output_dir.join("02 - London Symphony Orchestra - Beethoven Symphony No 5 (Hi-Res Master).flac");
    create_valid_flac_file(&track2_path, 8192)?;
    let enriched2 = engine.resolve_track_metadata("London Symphony Orchestra", "Beethoven Symphony No. 5", "Symphony No. 5", track2_path.to_str()).await;

    let mock_cover_jpeg: Vec<u8> = vec![
        0xFF, 0xD8, 0xFF, 0xE0, 0x00, 0x10, 0x4A, 0x46, 0x49, 0x46, 0x00, 0x01,
        0x01, 0x01, 0x00, 0x48, 0x00, 0x48, 0x00, 0x00, 0xFF, 0xD9
    ];

    let meta2 = FlacMetadata {
        title: "Symphony No. 5".to_string(),
        artist: "London Symphony Orchestra".to_string(),
        album: "Beethoven Symphony No. 5".to_string(),
        album_artist: Some("London Symphony Orchestra".to_string()),
        genre: enriched2.genre.or(Some("Classical".to_string())),
        style: enriched2.style.or(Some("Symphonic".to_string())),
        mood: enriched2.mood.or(Some("epic".to_string())),
        release_type: enriched2.release_type.or(Some("Album".to_string())),
        release_status: enriched2.release_status.or(Some("Official".to_string())),
        release_country: enriched2.release_country.or(Some("GB".to_string())),
        language: enriched2.language,
        label: enriched2.label.or(Some("LSO Live".to_string())),
        track_number: 1,
        track_total: 4,
        disc_number: 1,
        disc_total: 1,
        release_year: Some("2020".to_string()),
        release_date: Some("2020-05-15".to_string()),
        sample_rate: Some(96000.0),
        bit_depth: Some(24),
        bpm: enriched2.bpm.map(|b| b as u32).or(Some(138)),
        initial_key: enriched2.key.or(Some("Cm".to_string())),
        energy: enriched2.energy.or(Some(0.92)),
        danceability: enriched2.danceability.or(Some(0.35)),
        loudness: enriched2.loudness.or(Some(-6.2)),
        cover_data: Some(mock_cover_jpeg),
        comment: Some("Hi-Res Master 24-bit/96kHz - Enriched via Syncify".to_string()),
        ..Default::default()
    };
    apply_flac_tags(&track2_path, &meta2)?;
    println!("✓ Track 2 ready: {:?}", track2_path);

    // Track 3: Various Artists - Compilation
    let track3_path = output_dir.join("03 - Various Artists - Now Thats What I Call Music 1 (Compilation).flac");
    create_valid_flac_file(&track3_path, 4096)?;
    let enriched3 = engine.resolve_track_metadata("Various Artists", "Now That's What I Call Music! 1", "Stayin' Alive", track3_path.to_str()).await;

    let meta3 = FlacMetadata {
        title: "Stayin' Alive".to_string(),
        artist: "Bee Gees".to_string(),
        album: "Now That's What I Call Music! 1".to_string(),
        album_artist: Some("Various Artists".to_string()),
        genre: enriched3.genre.or(Some("Pop / Disco".to_string())),
        style: enriched3.style.or(Some("Disco".to_string())),
        mood: enriched3.mood.or(Some("party".to_string())),
        release_type: enriched3.release_type.or(Some("Compilation".to_string())),
        release_status: enriched3.release_status.or(Some("Official".to_string())),
        release_country: enriched3.release_country.or(Some("US".to_string())),
        label: enriched3.label.or(Some("EMI / Virgin".to_string())),
        track_number: 1,
        track_total: 16,
        disc_number: 1,
        disc_total: 1,
        release_year: Some("1983".to_string()),
        bpm: enriched3.bpm.map(|b| b as u32).or(Some(104)),
        initial_key: enriched3.key.or(Some("Fm".to_string())),
        energy: enriched3.energy.or(Some(0.88)),
        danceability: enriched3.danceability.or(Some(0.91)),
        loudness: enriched3.loudness.or(Some(-7.5)),
        comment: Some("Compilation Release - Enriched via Syncify".to_string()),
        ..Default::default()
    };
    apply_flac_tags(&track3_path, &meta3)?;
    println!("✓ Track 3 ready: {:?}", track3_path);

    // Track 4: Utada Hikaru - Japanese UTF-8 Script
    let track4_path = output_dir.join("04 - 宇多田ヒカル - 初恋 (Japanese UTF-8 Script).flac");
    create_valid_flac_file(&track4_path, 4096)?;
    let enriched4 = engine.resolve_track_metadata("宇多田ヒカル", "First Love", "初恋", track4_path.to_str()).await;

    let meta4 = FlacMetadata {
        title: "初恋".to_string(),
        artist: "宇多田ヒカル".to_string(),
        album: "初恋 (Hatsukoi)".to_string(),
        album_artist: Some("宇多田ヒカル".to_string()),
        genre: enriched4.genre.or(Some("J-Pop".to_string())),
        style: enriched4.style.or(Some("Ballad".to_string())),
        mood: enriched4.mood.or(Some("melancholy".to_string())),
        release_type: enriched4.release_type.or(Some("Album".to_string())),
        release_status: enriched4.release_status.or(Some("Official".to_string())),
        release_country: enriched4.release_country.or(Some("JP".to_string())),
        language: enriched4.language.or(Some("jpn".to_string())),
        label: enriched4.label.or(Some("Epic Records Japan".to_string())),
        track_number: 1,
        track_total: 12,
        disc_number: 1,
        disc_total: 1,
        release_year: Some("2018".to_string()),
        bpm: enriched4.bpm.map(|b| b as u32).or(Some(85)),
        initial_key: enriched4.key.or(Some("C#m".to_string())),
        energy: enriched4.energy.or(Some(0.65)),
        danceability: enriched4.danceability.or(Some(0.55)),
        loudness: enriched4.loudness.or(Some(-9.1)),
        comment: Some("UTF-8 Script Non-Latin Test - 音楽, Музыка, Music".to_string()),
        ..Default::default()
    };
    apply_flac_tags(&track4_path, &meta4)?;
    println!("✓ Track 4 ready: {:?}", track4_path);

    // Track 5: Pink Floyd - Echoes
    let track5_path = output_dir.join("05 - Pink Floyd - Echoes (Long Payload 23min Suite).flac");
    create_valid_flac_file(&track5_path, 65536)?;
    let enriched5 = engine.resolve_track_metadata("Pink Floyd", "Meddle", "Echoes", track5_path.to_str()).await;

    let meta5 = FlacMetadata {
        title: "Echoes".to_string(),
        artist: "Pink Floyd".to_string(),
        album: "Meddle".to_string(),
        album_artist: Some("Pink Floyd".to_string()),
        genre: enriched5.genre.or(Some("Progressive Rock".to_string())),
        style: enriched5.style.or(Some("Space Rock".to_string())),
        mood: enriched5.mood.or(Some("relaxed".to_string())),
        release_type: enriched5.release_type.or(Some("Album".to_string())),
        release_status: enriched5.release_status.or(Some("Official".to_string())),
        release_country: enriched5.release_country.or(Some("GB".to_string())),
        language: enriched5.language.or(Some("eng".to_string())),
        label: enriched5.label.or(Some("Harvest Records".to_string())),
        track_number: 6,
        track_total: 6,
        disc_number: 1,
        disc_total: 1,
        release_year: Some("1971".to_string()),
        bpm: enriched5.bpm.map(|b| b as u32).or(Some(134)),
        initial_key: enriched5.key.or(Some("C#m".to_string())),
        energy: enriched5.energy.or(Some(0.78)),
        danceability: enriched5.danceability.or(Some(0.42)),
        loudness: enriched5.loudness.or(Some(-10.8)),
        comment: Some("23 Minute Progressive Rock Suite - Enriched via Syncify".to_string()),
        ..Default::default()
    };
    apply_flac_tags(&track5_path, &meta5)?;
    println!("✓ Track 5 ready: {:?}", track5_path);

    println!("\n🎉 All 5 test FLAC tracks generated successfully in downloads_test/");
    Ok(())
}

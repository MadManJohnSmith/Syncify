use std::path::Path;
use syncify_core_domain::{
    FolderFileTemplateConfig, LibraryLayout, TrackLayoutContext,
    BatchDownloadManifest,
};
use syncify_tauri_lib::services::ManifestWriter;
use sqlx::sqlite::SqlitePoolOptions;
use tempfile::TempDir;

#[test]
fn test_default_layout_cli_parity() {
    let base_dir = Path::new("C:/Music");
    let layout = LibraryLayout::new(base_dir);

    let ctx = TrackLayoutContext {
        artist: "David Bowie",
        album_artist: Some("David Bowie"),
        album: "\"Heroes\"",
        title: "\"Heroes\" (2017 Remaster)",
        year: Some(1977),
        original_date: Some("1977-10-14"),
        track_number: 3,
        track_total: Some(10),
        disc_number: 1,
        total_discs: 1,
        format: "flac",
        bit_depth: Some(24),
        sample_rate: Some(96000.0),
    };

    let path = layout.resolve_track_path(&ctx);
    let path_str = path.to_string_lossy().replace('\\', "/");
    assert_eq!(path_str, "C:/Music/David Bowie/[1977] _Heroes_/03 - _Heroes_ (2017 Remaster).flac");
}

#[test]
fn test_windows_forbidden_chars_and_reserved_names() {
    let base_dir = Path::new("C:/Music");
    let layout = LibraryLayout::new(base_dir);

    let ctx = TrackLayoutContext {
        artist: "AC/DC: Band <Rock>*",
        album_artist: Some("AC/DC: Band <Rock>*"),
        album: "AUX.",
        title: "CON|NUL? Track 1.",
        year: Some(1980),
        original_date: Some("1980-01-01"),
        track_number: 1,
        track_total: Some(8),
        disc_number: 1,
        total_discs: 1,
        format: "flac",
        bit_depth: Some(16),
        sample_rate: Some(44100.0),
    };

    let path = layout.resolve_track_path(&ctx);
    let path_str = path.to_string_lossy().replace('\\', "/");
    assert!(path_str.contains("AC_DC_ Band _Rock__"));
    assert!(path_str.contains("[1980] AUX_"));
    assert!(path_str.contains("01 - CON_NUL_ Track 1.flac"));
}

#[test]
fn test_custom_templates_and_space_replacement() {
    let base_dir = Path::new("C:/Music");
    let config = FolderFileTemplateConfig {
        folder_template: "{Artist}/{Year} - {Album}".to_string(),
        file_template: "{TrackNumber} - {Title}".to_string(),
        artist_separator: ", ".to_string(),
        replace_spaces_with: Some("_".to_string()),
        max_path_length: 260,
    };
    let layout = LibraryLayout::with_config(base_dir, config);

    let ctx = TrackLayoutContext {
        artist: "Pink Floyd",
        album_artist: None,
        album: "The Wall",
        title: "Comfortably Numb",
        year: Some(1979),
        original_date: Some("1979-11-30"),
        track_number: 6,
        track_total: Some(13),
        disc_number: 2,
        total_discs: 2,
        format: "flac",
        bit_depth: Some(16),
        sample_rate: Some(44100.0),
    };

    let path = layout.resolve_track_path(&ctx);
    let path_str = path.to_string_lossy().replace('\\', "/");
    assert_eq!(path_str, "C:/Music/Pink_Floyd/1979_-_The_Wall/Disc 2/6_-_Comfortably_Numb.flac");
}

#[test]
fn test_various_artists_layout() {
    let base_dir = Path::new("C:/Music");
    let layout = LibraryLayout::new(base_dir);

    let ctx = TrackLayoutContext {
        artist: "Queen",
        album_artist: Some("Various Artists"),
        album: "Top 80s Hits",
        title: "Radio Ga Ga",
        year: Some(1984),
        original_date: Some("1984-01-01"),
        track_number: 5,
        track_total: Some(20),
        disc_number: 1,
        total_discs: 1,
        format: "flac",
        bit_depth: Some(16),
        sample_rate: Some(44100.0),
    };

    let path = layout.resolve_track_path(&ctx);
    let path_str = path.to_string_lossy().replace('\\', "/");
    assert_eq!(path_str, "C:/Music/Various Artists/[1984] Top 80s Hits/05 - Queen - Radio Ga Ga.flac");
}

#[test]
fn test_max_path_length_truncation() {
    let base_dir = Path::new("C:/Music");
    let config = FolderFileTemplateConfig {
        folder_template: "{Artist}/{Album}".to_string(),
        file_template: "{TrackNumber:pad2} - {Title}".to_string(),
        artist_separator: ", ".to_string(),
        replace_spaces_with: None,
        max_path_length: 50,
    };
    let layout = LibraryLayout::with_config(base_dir, config);

    let ctx = TrackLayoutContext {
        artist: "An Extremely Long Artist Name For Testing Truncation In Systems",
        album_artist: None,
        album: "An Extremely Long Album Title That Exceeds Normal Lengths",
        title: "A Very Long Track Title That Would Exceed Max Windows Path Length",
        year: None,
        original_date: None,
        track_number: 1,
        track_total: None,
        disc_number: 1,
        total_discs: 1,
        format: "flac",
        bit_depth: None,
        sample_rate: None,
    };

    let path = layout.resolve_track_path(&ctx);
    let path_str = path.to_string_lossy();
    assert!(path_str.len() <= 50);
    assert!(path_str.ends_with(".flac"));
}

#[test]
fn test_sidecar_paths_derivation() {
    let base_dir = Path::new("C:/Music");
    let layout = LibraryLayout::new(base_dir);
    let track_path = Path::new("C:/Music/David Bowie/[1977] Heroes/03 - Heroes.flac");

    let lrc = layout.lyrics_path_for_track(track_path);
    assert_eq!(lrc.to_string_lossy().replace('\\', "/"), "C:/Music/David Bowie/[1977] Heroes/03 - Heroes.lrc");

    let cover_jpg = layout.cover_image_path("David Bowie", "Heroes", Some(1977));
    assert_eq!(cover_jpg.to_string_lossy().replace('\\', "/"), "C:/Music/David Bowie/[1977] Heroes/cover.jpg");

    let cover_webp = layout.cover_webp_path("David Bowie", "Heroes", Some(1977));
    assert_eq!(cover_webp.to_string_lossy().replace('\\', "/"), "C:/Music/David Bowie/[1977] Heroes/cover.webp");

    let folder_webp = layout.folder_webp_path("David Bowie", "Heroes", Some(1977));
    assert_eq!(folder_webp.to_string_lossy().replace('\\', "/"), "C:/Music/David Bowie/[1977] Heroes/folder.webp");

    let anim_webp = layout.animated_webp_path("David Bowie", "Heroes", Some(1977));
    assert_eq!(anim_webp.to_string_lossy().replace('\\', "/"), "C:/Music/David Bowie/[1977] Heroes/animated.webp");

    let booklet = layout.booklet_path("David Bowie", "Heroes", Some(1977));
    assert_eq!(booklet.to_string_lossy().replace('\\', "/"), "C:/Music/David Bowie/[1977] Heroes/booklet.pdf");

    let art_jpg = layout.artist_image_path("David Bowie");
    assert_eq!(art_jpg.to_string_lossy().replace('\\', "/"), "C:/Music/David Bowie/artist.jpg");

    let fanart = layout.artist_fanart_path("David Bowie");
    assert_eq!(fanart.to_string_lossy().replace('\\', "/"), "C:/Music/David Bowie/fanart.jpg");

    let nfo = layout.artist_nfo_path("David Bowie");
    assert_eq!(nfo.to_string_lossy().replace('\\', "/"), "C:/Music/David Bowie/artist.nfo");

    let bio = layout.artist_biography_path("David Bowie");
    assert_eq!(bio.to_string_lossy().replace('\\', "/"), "C:/Music/David Bowie/biography.txt");
}

#[test]
fn test_collision_resolution() {
    let temp_dir = TempDir::new().unwrap();
    let base_dir = temp_dir.path();
    let layout = LibraryLayout::new(base_dir);

    let original = base_dir.join("track.flac");
    std::fs::write(&original, b"dummy audio").unwrap();

    let unique1 = layout.resolve_unique_path(&original);
    assert_eq!(unique1, base_dir.join("track (1).flac"));

    std::fs::write(&unique1, b"dummy audio 2").unwrap();
    let unique2 = layout.resolve_unique_path(&original);
    assert_eq!(unique2, base_dir.join("track (2).flac"));
}

#[tokio::test]
async fn test_manifest_writer_reconciliation() {
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await
        .unwrap();

    // Create minimal schema for download_queue, tracks, albums, artists, downloads
    sqlx::query(
        r#"
        CREATE TABLE artists (id INTEGER PRIMARY KEY, name TEXT);
        CREATE TABLE albums (id INTEGER PRIMARY KEY, title TEXT);
        CREATE TABLE tracks (id INTEGER PRIMARY KEY, title TEXT, isrc TEXT, album_id INTEGER, artist_id INTEGER);
        CREATE TABLE track_artists (track_id INTEGER, artist_id INTEGER);
        CREATE TABLE download_queue (
            id INTEGER PRIMARY KEY,
            track_id INTEGER,
            service_name TEXT,
            service_track_id TEXT,
            target_title TEXT,
            target_artist TEXT,
            target_album TEXT,
            target_isrc TEXT,
            status TEXT,
            error_message TEXT,
            quality_preference TEXT,
            created_at TEXT DEFAULT CURRENT_TIMESTAMP,
            completed_at TEXT
        );
        CREATE TABLE downloads (
            id INTEGER PRIMARY KEY,
            track_id INTEGER UNIQUE,
            file_path TEXT,
            file_format TEXT,
            bit_depth INTEGER,
            sample_rate INTEGER,
            file_size_bytes INTEGER,
            downloaded_at TEXT
        );
        "#
    )
    .execute(&pool)
    .await
    .unwrap();

    let temp_dir = TempDir::new().unwrap();
    let out_dir = temp_dir.path();
    let fake_audio = out_dir.join("01 - Test Song.flac");
    let fake_lrc = out_dir.join("01 - Test Song.lrc");
    let fake_cover = out_dir.join("cover.jpg");
    let _: std::io::Result<()> = tokio::fs::write(&fake_audio, b"FLAC DATA").await;
    let _: std::io::Result<()> = tokio::fs::write(&fake_lrc, b"[00:01.00] Test").await;
    let _: std::io::Result<()> = tokio::fs::write(&fake_cover, b"JPEG DATA").await;

    // Seed: 1 complete, 1 skipped, 1 stale source, 1 source identity missing, 1 failed
    sqlx::query(
        r#"
        INSERT INTO download_queue (id, track_id, service_name, service_track_id, target_title, target_artist, target_album, target_isrc, status, quality_preference)
        VALUES (1, 101, 'qobuz', '999111', 'Test Song', 'Test Artist', 'Test Album', 'USRC12345678', 'complete', '24-96');
        
        INSERT INTO downloads (track_id, file_path, file_format, bit_depth, sample_rate, file_size_bytes)
        VALUES (101, ?, 'FLAC', 24, 96000, 1024);

        INSERT INTO download_queue (id, track_id, service_name, service_track_id, target_title, target_artist, target_album, target_isrc, status, quality_preference)
        VALUES (2, 102, 'qobuz', '999222', 'Skipped Song', 'Skipped Artist', 'Skipped Album', 'USRC87654321', 'skipped', '16-44');

        INSERT INTO download_queue (id, track_id, service_name, service_track_id, target_title, target_artist, target_album, target_isrc, status, error_message, quality_preference)
        VALUES (3, 103, 'qobuz', '999333', 'Stale Song', 'Stale Artist', 'Stale Album', 'USRC11223344', 'failed', 'StaleSource: 404 track not found', '24-96');

        INSERT INTO download_queue (id, track_id, service_name, service_track_id, target_title, target_artist, target_album, target_isrc, status, error_message, quality_preference)
        VALUES (4, 104, 'qobuz', '', 'Missing ID Song', 'Missing Artist', 'Missing Album', 'USRC55667788', 'failed', 'SourceIdentityMissing: No locked service_track_id', '16-44');

        INSERT INTO download_queue (id, track_id, service_name, service_track_id, target_title, target_artist, target_album, target_isrc, status, error_message, quality_preference)
        VALUES (5, 105, 'qobuz', '999555', 'Auth Expired Song', 'Auth Artist', 'Auth Album', 'USRC99887766', 'failed', 'RequiresAuth: token expired 401', '16-44');
        "#
    )
    .bind(fake_audio.to_string_lossy().to_string())
    .execute(&pool)
    .await
    .unwrap();

    let manifest: BatchDownloadManifest = ManifestWriter::generate_and_save_manifest(&pool, out_dir).await.unwrap();

    assert_eq!(manifest.total_requested, 5);
    assert_eq!(manifest.total_succeeded, 1);
    assert_eq!(manifest.total_failed, 3);
    assert_eq!(manifest.total_skipped, 1);
    assert_eq!(manifest.entries.len(), 5);

    // 1. Success entry
    let success_entry = &manifest.entries[0];
    assert_eq!(success_entry.title, "Test Song");
    assert_eq!(success_entry.download_result, "Success");
    assert_eq!(success_entry.bit_depth, Some(24));
    assert_eq!(success_entry.sample_rate, Some(96000));
    assert!(success_entry.created_artifacts.iter().any(|a: &String| a.ends_with("01 - Test Song.flac")));
    assert!(success_entry.created_artifacts.iter().any(|a: &String| a.ends_with("01 - Test Song.lrc")));
    assert!(success_entry.created_artifacts.iter().any(|a: &String| a.ends_with("cover.jpg")));
    // Non-existent booklet.pdf must NOT be in created_artifacts
    assert!(!success_entry.created_artifacts.iter().any(|a: &String| a.ends_with("booklet.pdf")));

    // 2. Skipped entry
    let skipped_entry = &manifest.entries[1];
    assert_eq!(skipped_entry.title, "Skipped Song");
    assert_eq!(skipped_entry.download_result, "Skipped");
    assert!(skipped_entry.created_artifacts.is_empty());

    // 3. Stale source entry
    let stale_entry = &manifest.entries[2];
    assert_eq!(stale_entry.title, "Stale Song");
    assert_eq!(stale_entry.download_result, "StaleSource");

    // 4. Source identity missing entry
    let missing_id_entry = &manifest.entries[3];
    assert_eq!(missing_id_entry.title, "Missing ID Song");
    assert_eq!(missing_id_entry.download_result, "SourceIdentityMissing");

    // 5. Auth error entry
    let auth_entry = &manifest.entries[4];
    assert_eq!(auth_entry.title, "Auth Expired Song");
    assert_eq!(auth_entry.download_result, "RequiresAuth");

    // Verify manifest.json exists on disk and is readable
    let manifest_file = out_dir.join("manifest.json");
    assert!(manifest_file.exists());
    let manifest_content: String = tokio::fs::read_to_string(&manifest_file).await.unwrap();
    let parsed: BatchDownloadManifest = serde_json::from_str(&manifest_content).unwrap();
    assert_eq!(parsed.total_requested, 5);
}

#[test]
fn test_animated_webp_structure_validation() {
    use syncify_core_domain::byte_validators::{WebpByteValidator, WebpValidationError};

    // 1. Construct valid synthetic animated WebP (RIFF ... WEBP VP8X ANIM ANMF)
    let mut valid_webp = Vec::new();
    valid_webp.extend_from_slice(b"RIFF");
    valid_webp.extend_from_slice(&(60u32).to_le_bytes()); // placeholder size
    valid_webp.extend_from_slice(b"WEBP");

    // VP8X Chunk (size 10, flags 0x02 = animated, canvas 500x500 -> 0x01F3)
    valid_webp.extend_from_slice(b"VP8X");
    valid_webp.extend_from_slice(&(10u32).to_le_bytes());
    valid_webp.push(0x02); // animation flag set
    valid_webp.extend_from_slice(&[0x00, 0x00, 0x00]); // reserved
    valid_webp.extend_from_slice(&[0xF3, 0x01, 0x00]); // 500 px width (24-bit 1-based: 499 + 1 = 500)
    valid_webp.extend_from_slice(&[0xF3, 0x01, 0x00]); // 500 px height

    // ANIM Chunk (size 6)
    valid_webp.extend_from_slice(b"ANIM");
    valid_webp.extend_from_slice(&(6u32).to_le_bytes());
    valid_webp.extend_from_slice(&[0xFF, 0xFF, 0xFF, 0xFF, 0x00, 0x00]); // bg color + loop count

    // ANMF Frame 1 Chunk (size 16)
    valid_webp.extend_from_slice(b"ANMF");
    valid_webp.extend_from_slice(&(16u32).to_le_bytes());
    valid_webp.extend_from_slice(&[0x00; 16]); // frame payload

    // ANMF Frame 2 Chunk (size 16)
    valid_webp.extend_from_slice(b"ANMF");
    valid_webp.extend_from_slice(&(16u32).to_le_bytes());
    valid_webp.extend_from_slice(&[0x00; 16]); // frame payload

    let info = WebpByteValidator::validate_animated_webp(&valid_webp).expect("Valid animated WebP should succeed");
    assert!(info.is_animated);
    assert_eq!(info.canvas_width, 500);
    assert_eq!(info.canvas_height, 500);
    assert_eq!(info.anmf_frame_count, 2);

    // 2. Corrupt: animation bit cleared
    let mut non_anim_webp = valid_webp.clone();
    non_anim_webp[20] = 0x00; // clear animation flag
    let err = WebpByteValidator::validate_animated_webp(&non_anim_webp).unwrap_err();
    assert_eq!(err, WebpValidationError::AnimationBitNotSet);

    // 3. Corrupt: missing VP8X chunk
    let corrupt_header = b"RIFF\x20\x00\x00\x00WEBPVP8 \x0A\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00";
    let err2 = WebpByteValidator::validate_animated_webp(corrupt_header).unwrap_err();
    assert_eq!(err2, WebpValidationError::MissingVp8xChunk);
}

#[tokio::test]
async fn test_qobuz_goodies_and_extended_sidecars_e2e_staging_promotion() {
    let staging_temp = TempDir::new().unwrap();
    let staging_dir = staging_temp.path();

    let library_temp = TempDir::new().unwrap();
    let library_dir = library_temp.path();

    let layout = LibraryLayout::new(library_dir);
    let target_album_dir = layout.album_dir("Pink Floyd", "The Dark Side of the Moon", Some(1973));
    let target_artist_dir = layout.artist_dir("Pink Floyd");
    tokio::fs::create_dir_all(&target_album_dir).await.unwrap();
    tokio::fs::create_dir_all(&target_artist_dir).await.unwrap();

    // 1. Stage audio and sidecars in staging_dir
    let item_id = "track_101";
    let staged_flac = staging_dir.join(format!("{}.flac", item_id));
    let staged_lrc = staging_dir.join(format!("{}.lrc", item_id));
    let staged_cover_jpg = staging_dir.join(format!("{}.cover.jpg", item_id));
    let staged_cover_webp = staging_dir.join(format!("{}.cover.webp", item_id));
    let staged_booklet_pdf = staging_dir.join(format!("{}.booklet.pdf", item_id));

    tokio::fs::write(&staged_flac, b"fLaC FLAC PAYLOAD DATA").await.unwrap();
    tokio::fs::write(&staged_lrc, b"[00:01.00] Money, get away").await.unwrap();
    tokio::fs::write(&staged_cover_jpg, b"\xFF\xD8\xFF JPEG COVER").await.unwrap();
    tokio::fs::write(&staged_cover_webp, b"RIFF WEBP ANIMATED DATA").await.unwrap();
    tokio::fs::write(&staged_booklet_pdf, b"%PDF-1.4 DIGITAL BOOKLET GOODIES").await.unwrap();

    // 2. Perform atomic promotion mirroring Qobuz / Tidal pipeline Step 8 & 9
    let final_track_path = target_album_dir.join("06 - Money.flac");
    tokio::fs::rename(&staged_flac, &final_track_path).await.unwrap();

    // Promote .lrc
    let final_lrc = layout.lyrics_path_for_track(&final_track_path);
    tokio::fs::rename(&staged_lrc, &final_lrc).await.unwrap();

    // Promote cover.jpg
    let final_cover_jpg = target_album_dir.join("cover.jpg");
    tokio::fs::copy(&staged_cover_jpg, &final_cover_jpg).await.unwrap();
    tokio::fs::remove_file(&staged_cover_jpg).await.unwrap();

    // Promote cover.webp, folder.webp, animated.webp
    let final_cover_webp = target_album_dir.join("cover.webp");
    let final_folder_webp = target_album_dir.join("folder.webp");
    let final_anim_webp = target_album_dir.join("animated.webp");
    tokio::fs::copy(&staged_cover_webp, &final_cover_webp).await.unwrap();
    tokio::fs::copy(&staged_cover_webp, &final_folder_webp).await.unwrap();
    tokio::fs::copy(&staged_cover_webp, &final_anim_webp).await.unwrap();
    tokio::fs::remove_file(&staged_cover_webp).await.unwrap();

    // Promote digital booklet.pdf (Qobuz goodies)
    let final_booklet = target_album_dir.join("booklet.pdf");
    tokio::fs::copy(&staged_booklet_pdf, &final_booklet).await.unwrap();
    tokio::fs::remove_file(&staged_booklet_pdf).await.unwrap();

    // Promote artist sidecars into artist directory
    let final_artist_nfo = target_artist_dir.join("artist.nfo");
    let final_artist_bio = target_artist_dir.join("biography.txt");
    let final_artist_fanart = target_artist_dir.join("fanart.jpg");
    tokio::fs::write(&final_artist_nfo, b"<artist><name>Pink Floyd</name></artist>").await.unwrap();
    tokio::fs::write(&final_artist_bio, b"English rock band formed in London in 1965.").await.unwrap();
    tokio::fs::write(&final_artist_fanart, b"\xFF\xD8\xFF FANART").await.unwrap();

    // 3. Verify destination artifacts exist with accurate contents
    assert!(final_track_path.exists());
    assert!(final_lrc.exists());
    assert!(final_cover_jpg.exists());
    assert!(final_cover_webp.exists());
    assert!(final_folder_webp.exists());
    assert!(final_anim_webp.exists());
    assert!(final_booklet.exists());
    assert!(final_artist_nfo.exists());
    assert!(final_artist_bio.exists());
    assert!(final_artist_fanart.exists());

    let booklet_bytes = tokio::fs::read(&final_booklet).await.unwrap();
    assert_eq!(booklet_bytes, b"%PDF-1.4 DIGITAL BOOKLET GOODIES");

    // 4. Invariant: Staging directory must be 100% clean (0 orphan files)
    let mut staging_entries = tokio::fs::read_dir(staging_dir).await.unwrap();
    let mut staged_count = 0;
    while let Ok(Some(_)) = staging_entries.next_entry().await {
        staged_count += 1;
    }
    assert_eq!(staged_count, 0, "Staging directory must have 0 orphan files post-promotion");
}

#[tokio::test]
async fn test_manifest_writer_registers_all_extended_sidecars_when_present() {
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await
        .unwrap();

    sqlx::query(
        r#"
        CREATE TABLE artists (id INTEGER PRIMARY KEY, name TEXT);
        CREATE TABLE albums (id INTEGER PRIMARY KEY, title TEXT);
        CREATE TABLE tracks (id INTEGER PRIMARY KEY, title TEXT, isrc TEXT, album_id INTEGER, artist_id INTEGER);
        CREATE TABLE track_artists (track_id INTEGER, artist_id INTEGER);
        CREATE TABLE download_queue (
            id INTEGER PRIMARY KEY,
            track_id INTEGER,
            service_name TEXT,
            service_track_id TEXT,
            target_title TEXT,
            target_artist TEXT,
            target_album TEXT,
            target_isrc TEXT,
            status TEXT,
            error_message TEXT,
            quality_preference TEXT,
            created_at TEXT DEFAULT CURRENT_TIMESTAMP,
            completed_at TEXT
        );
        CREATE TABLE downloads (
            id INTEGER PRIMARY KEY,
            track_id INTEGER UNIQUE,
            file_path TEXT,
            file_format TEXT,
            bit_depth INTEGER,
            sample_rate INTEGER,
            file_size_bytes INTEGER,
            downloaded_at TEXT
        );
        "#
    )
    .execute(&pool)
    .await
    .unwrap();

    let temp_dir = TempDir::new().unwrap();
    let base_dir = temp_dir.path();
    let artist_dir = base_dir.join("Pink Floyd");
    let album_dir = artist_dir.join("[1973] The Dark Side of the Moon");
    tokio::fs::create_dir_all(&album_dir).await.unwrap();

    let audio_file = album_dir.join("06 - Money.flac");
    let lrc_file = album_dir.join("06 - Money.lrc");
    let cover_jpg = album_dir.join("cover.jpg");
    let cover_webp = album_dir.join("cover.webp");
    let folder_webp = album_dir.join("folder.webp");
    let anim_webp = album_dir.join("animated.webp");
    let booklet_pdf = album_dir.join("booklet.pdf");

    let artist_nfo = artist_dir.join("artist.nfo");
    let artist_bio = artist_dir.join("biography.txt");
    let artist_fanart = artist_dir.join("fanart.jpg");

    tokio::fs::write(&audio_file, b"FLAC DATA").await.unwrap();
    tokio::fs::write(&lrc_file, b"[00:01.00] Lyrics").await.unwrap();
    tokio::fs::write(&cover_jpg, b"JPEG").await.unwrap();
    tokio::fs::write(&cover_webp, b"WEBP").await.unwrap();
    tokio::fs::write(&folder_webp, b"WEBP FOLDER").await.unwrap();
    tokio::fs::write(&anim_webp, b"WEBP ANIM").await.unwrap();
    tokio::fs::write(&booklet_pdf, b"%PDF GOODIES").await.unwrap();
    tokio::fs::write(&artist_nfo, b"<nfo/>").await.unwrap();
    tokio::fs::write(&artist_bio, b"Bio text").await.unwrap();
    tokio::fs::write(&artist_fanart, b"Fanart JPEG").await.unwrap();

    sqlx::query(
        r#"
        INSERT INTO download_queue (id, track_id, service_name, service_track_id, target_title, target_artist, target_album, target_isrc, status, quality_preference)
        VALUES (1, 201, 'qobuz', '999888', 'Money', 'Pink Floyd', 'The Dark Side of the Moon', 'GBAYE7300006', 'complete', '24-192');

        INSERT INTO downloads (track_id, file_path, file_format, bit_depth, sample_rate, file_size_bytes)
        VALUES (201, ?, 'FLAC', 24, 192000, 45000000);
        "#
    )
    .bind(audio_file.to_string_lossy().to_string())
    .execute(&pool)
    .await
    .unwrap();

    let manifest = ManifestWriter::generate_and_save_manifest(&pool, base_dir).await.unwrap();
    assert_eq!(manifest.entries.len(), 1);

    let entry = &manifest.entries[0];
    assert_eq!(entry.download_result, "Success");
    assert_eq!(entry.cover_result, "StaticAndAnimated");
    assert_eq!(entry.lyrics_result, "WordSynced");

    // Verify all physical sidecars are in created_artifacts
    let artifacts = &entry.created_artifacts;
    assert!(artifacts.iter().any(|a| a.ends_with("06 - Money.flac")));
    assert!(artifacts.iter().any(|a| a.ends_with("06 - Money.lrc")));
    assert!(artifacts.iter().any(|a| a.ends_with("cover.jpg")));
    assert!(artifacts.iter().any(|a| a.ends_with("cover.webp")));
    assert!(artifacts.iter().any(|a| a.ends_with("folder.webp")));
    assert!(artifacts.iter().any(|a| a.ends_with("animated.webp")));
    assert!(artifacts.iter().any(|a| a.ends_with("booklet.pdf")));
    assert!(artifacts.iter().any(|a| a.ends_with("artist.nfo")));
    assert!(artifacts.iter().any(|a| a.ends_with("biography.txt")));
    assert!(artifacts.iter().any(|a| a.ends_with("fanart.jpg")));

    // Verify non-existent sidecar is NOT in created_artifacts
    assert!(!artifacts.iter().any(|a| a.ends_with("artist.jpg")));
    assert!(!artifacts.iter().any(|a| a.ends_with("nonexistent.pdf")));
}

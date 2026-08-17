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

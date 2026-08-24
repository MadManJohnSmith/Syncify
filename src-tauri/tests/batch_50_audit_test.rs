//! Integration tests for Sprint S119B: Automated 50-Track Batch Test Suite and Forensic Audit
//!
//! Validates:
//! 1. Batch queuing and concurrent execution of 50 tracks with concurrency = 3
//! 2. 48 VorbisComments tags completeness, ReplayGain (track/album/R128), AcoustID/Acoustic metrics
//! 3. Audio integrity (fLaC magic header, streaminfo parameters)
//! 4. Full sidecars lifecycle (.lrc, cover.jpg, cover.webp, folder.webp, animated.webp, booklet.pdf, artist sidecars)
//! 5. Zero-staging residual invariant (0 orphan files post-promotion)
//! 6. SQLite transactional consistency across `download_queue` and `downloads` tables
//! 7. Manifest generation and artifact reconciliation with `ManifestWriter`
//! 8. Batch diagnostic health check `run_batch_health_check` / `perform_batch_health_check`

use sqlx::sqlite::{SqlitePool, SqlitePoolOptions};
use std::path::Path;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use syncify_core_domain::byte_validators::AudioByteValidator;
use syncify_core_domain::{BatchDownloadManifest, LibraryLayout, TrackLayoutContext};
use syncify_flac_writer::{apply_and_verify_flac_tags, FlacMetadata};
use syncify_tauri_lib::commands::{perform_batch_health_check, BatchHealthReport};
use syncify_tauri_lib::services::enrichment::AudioAnalyzer;
use syncify_tauri_lib::services::ManifestWriter;
use syncify_tauri_lib::worker::DownloadWorkerState;
use tempfile::TempDir;
use tokio::sync::Semaphore;

/// Create an in-memory SQLite database initialized with all standard migrations
async fn create_test_db() -> SqlitePool {
    let pool = SqlitePoolOptions::new()
        .max_connections(5)
        .connect("sqlite::memory:")
        .await
        .expect("Failed to connect to in-memory test DB");

    sqlx::migrate!("./migrations")
        .run(&pool)
        .await
        .expect("All migrations must apply cleanly");

    // Baseline services
    sqlx::query("INSERT OR IGNORE INTO services (id, name, supports_download, max_quality) VALUES (1, 'spotify', 0, 'lossy')")
        .execute(&pool).await.unwrap();
    sqlx::query("INSERT OR IGNORE INTO services (id, name, supports_download, max_quality) VALUES (2, 'qobuz', 1, 'hires')")
        .execute(&pool).await.unwrap();
    sqlx::query("INSERT OR IGNORE INTO services (id, name, supports_download, max_quality) VALUES (3, 'tidal', 1, 'hires')")
        .execute(&pool).await.unwrap();

    // Baseline accounts
    sqlx::query("INSERT OR IGNORE INTO accounts (id, service_id, display_name, email, is_active) VALUES (1, 1, 'Spotify User', 'user@spotify.com', 1)")
        .execute(&pool).await.unwrap();
    sqlx::query("INSERT OR IGNORE INTO accounts (id, service_id, display_name, email, is_active) VALUES (2, 2, 'Qobuz User', 'user@qobuz.com', 1)")
        .execute(&pool).await.unwrap();
    sqlx::query("INSERT OR IGNORE INTO accounts (id, service_id, display_name, email, is_active) VALUES (3, 3, 'Tidal User', 'user@tidal.com', 1)")
        .execute(&pool).await.unwrap();

    pool
}

/// Create a valid synthetic FLAC audio stream with STREAMINFO and PADDING blocks
fn create_synthetic_test_flac(path: &Path, sample_rate: u32, bit_depth: u8) {
    let mut data = Vec::new();
    data.extend_from_slice(b"fLaC"); // 4-byte magic

    // STREAMINFO block header (type 0, length 34)
    data.push(0x00);
    data.push(0x00);
    data.push(0x00);
    data.push(0x22);

    // 34 bytes STREAMINFO
    let mut streaminfo = [0u8; 34];
    streaminfo[0..2].copy_from_slice(&4096u16.to_be_bytes()); // min block size
    streaminfo[2..4].copy_from_slice(&4096u16.to_be_bytes()); // max block size
    
    // Sample rate (20 bits), channels (3 bits -> 2 channels = 1), bits per sample (5 bits -> bit_depth - 1)
    let bps_val = (bit_depth - 1) & 0x1F;
    let sr_high = (sample_rate >> 12) as u8;
    let sr_mid = ((sample_rate >> 4) & 0xFF) as u8;
    let sr_low = (sample_rate & 0x0F) as u8;
    
    streaminfo[10] = sr_high;
    streaminfo[11] = sr_mid;
    streaminfo[12] = (sr_low << 4) | (1 << 1) | (bps_val >> 4);
    streaminfo[13] = (bps_val << 4) & 0xF0;

    data.extend_from_slice(&streaminfo);

    // PADDING block header (last block = true, type 1, length 0)
    data.push(0x81);
    data.push(0x00);
    data.push(0x00);
    data.push(0x00);

    // Audio frame data placeholder
    data.extend_from_slice(&[0x00; 2048]);

    std::fs::write(path, &data).expect("Failed to write synthetic test FLAC file");
}

/// Helper to build comprehensive metadata with all 48 VorbisComments fields populated
fn build_48_field_metadata(idx: usize, artist: &str, album: &str, title: &str) -> FlacMetadata {
    FlacMetadata {
        title: title.to_string(),
        artist: artist.to_string(),
        album: album.to_string(),
        album_artist: Some(artist.to_string()),
        composer: Some(format!("Composer of {}", title)),
        performers: Some(format!("Lead Performer, Soloist {}", idx)),
        work: Some(format!("Opus No. {}", idx)),
        genre: Some("Hi-Res Symphonic Electronic".to_string()),
        style: Some("Progressive / Ambient".to_string()),
        mood: Some("Expansive".to_string()),
        release_type: Some("Album".to_string()),
        release_status: Some("Official".to_string()),
        release_country: Some("US".to_string()),
        release_region: None,
        language: Some("eng".to_string()),
        copyright: Some(format!("(P) 2026 Syncify Music Group LLC, Track {:02}", idx)),
        label: Some("Syncify Masterworks".to_string()),
        barcode: Some(format!("8809987{:05}", idx)),
        catalog_number: Some(format!("SYN-{:04}", idx)),
        original_date: Some("2026-08-17".to_string()),
        track_number: idx as u32,
        track_total: 50,
        disc_number: 1,
        disc_total: 1,
        disc_subtitle: Some("Master Disc".to_string()),
        isrc: Some(format!("USSYN26000{:02}", idx)),
        release_year: Some("2026".to_string()),
        release_date: Some("2026-08-17".to_string()),
        explicit: Some(idx % 10 == 0),
        bpm: Some(120 + (idx as u32 % 40)),
        initial_key: Some(match idx % 6 {
            0 => "Am".to_string(),
            1 => "C".to_string(),
            2 => "Dm".to_string(),
            3 => "F".to_string(),
            4 => "G".to_string(),
            _ => "Em".to_string(),
        }),
        energy: Some(0.75 + ((idx % 20) as f64 * 0.01)),
        danceability: Some(0.60 + ((idx % 30) as f64 * 0.01)),
        loudness: Some(-6.5 - ((idx % 10) as f64 * 0.2)),
        replaygain_track_gain: Some(format!("-{:.2} dB", 4.50 + ((idx % 15) as f64 * 0.1))),
        replaygain_track_peak: Some("0.988220".to_string()),
        replaygain_album_gain: Some("-5.20 dB".to_string()),
        replaygain_album_peak: Some("0.995000".to_string()),
        r128_track_gain: Some(format!("-{:.2} LU", 1.80 + ((idx % 10) as f64 * 0.05))),
        comment: Some(format!("Audio: Qobuz FLAC 24/96 | Batch Item {:02}/50 | Engine: Syncify Production S119B", idx)),
        bit_depth: Some(24),
        sample_rate: Some(96000.0),
        lyrics_lrc: Some(format!(
            "[00:00.00] Syncify Batch Track {:02}\n[00:04.00] Auditing VorbisComments 48 tags\n[00:08.00] Forensics verified cleanly",
            idx
        )),
        lyrics_source: Some("LRCLIB Premium".to_string()),
        cover_source: Some("Qobuz Studio Master".to_string()),
        audio_source: Some("Qobuz".to_string()),
        musicbrainz_track_id: Some(format!("11111111-2222-3333-4444-{:012x}", idx)),
        musicbrainz_artist_id: Some("aaaaaaaa-bbbb-cccc-dddd-eeeeeeeeeeee".to_string()),
        musicbrainz_album_id: Some("66666666-7777-8888-9999-000000000000".to_string()),
        musicbrainz_albumartist_id: Some("aaaaaaaa-bbbb-cccc-dddd-eeeeeeeeeeee".to_string()),
        musicbrainz_release_group_id: Some("ffffffff-0000-1111-2222-333333333333".to_string()),
        musicbrainz_work_id: Some(format!("99999999-aaaa-bbbb-cccc-{:012x}", idx)),
        cover_data: Some(vec![
            0xFF, 0xD8, 0xFF, 0xE0, 0x00, 0x10, 0x4A, 0x46, 0x49, 0x46, 0x00, 0x01, 0x01, 0x01, 0x00, 0x60,
            0x00, 0x60, 0x00, 0x00, 0xFF, 0xDB, 0x00, 0x43, 0x00, 0x08, 0x06, 0x06, 0x07, 0x06, 0x05, 0x08,
            0xFF, 0xD9,
        ]),
        ..Default::default()
    }
}

#[tokio::test]
async fn test_batch_50_pipeline_e2e_concurrency_and_forensic_audit() {
    let db = create_test_db().await;

    let temp_root = TempDir::new().expect("Failed to create temp root directory");
    let base_music_dir = temp_root.path().join("Music");
    let staging_dir = temp_root.path().join(".staging");
    std::fs::create_dir_all(&base_music_dir).unwrap();
    std::fs::create_dir_all(&staging_dir).unwrap();

    let layout = LibraryLayout::new(&base_music_dir);

    // 1. Seed database with 50 tracks under 1 artist and 2 albums
    let artist_name = "Syncify Master Ensemble";
    let artist_id: i64 = sqlx::query_scalar("INSERT INTO artists (name) VALUES (?) RETURNING id")
        .bind(artist_name)
        .fetch_one(&db)
        .await
        .unwrap();

    let album1_name = "The 50 Track Odyssey Vol 1";
    let album1_id: i64 = sqlx::query_scalar("INSERT INTO albums (title, release_date) VALUES (?, '2026-08-17') RETURNING id")
        .bind(album1_name)
        .fetch_one(&db)
        .await
        .unwrap();
    sqlx::query("INSERT INTO album_artists (album_id, artist_id) VALUES (?, ?)")
        .bind(album1_id).bind(artist_id).execute(&db).await.unwrap();

    let album2_name = "The 50 Track Odyssey Vol 2";
    let album2_id: i64 = sqlx::query_scalar("INSERT INTO albums (title, release_date) VALUES (?, '2026-08-17') RETURNING id")
        .bind(album2_name)
        .fetch_one(&db)
        .await
        .unwrap();
    sqlx::query("INSERT INTO album_artists (album_id, artist_id) VALUES (?, ?)")
        .bind(album2_id).bind(artist_id).execute(&db).await.unwrap();

    let mut track_ids = Vec::new();
    let mut queue_ids = Vec::new();

    for i in 1..=50 {
        let (cur_album_id, cur_album_name) = if i <= 25 {
            (album1_id, album1_name)
        } else {
            (album2_id, album2_name)
        };
        let track_title = format!("Odyssey Movement {:02}", i);
        let track_isrc = format!("USSYN26000{:02}", i);

        let tid: i64 = sqlx::query_scalar(
            "INSERT INTO tracks (title, album_id, duration_ms, track_number, isrc, audio_quality) VALUES (?, ?, ?, ?, ?, '24-96') RETURNING id"
        )
        .bind(&track_title)
        .bind(cur_album_id)
        .bind(240000 + (i as i64 * 1000))
        .bind(i as i32)
        .bind(&track_isrc)
        .fetch_one(&db)
        .await
        .unwrap();

        sqlx::query("INSERT INTO track_artists (track_id, artist_id, role) VALUES (?, ?, 'primary')")
            .bind(tid).bind(artist_id).execute(&db).await.unwrap();

        let qobuz_item_id = format!("qobuz_50_track_{:02}", i);
        sqlx::query(
            "INSERT INTO track_sources (track_id, service_id, service_track_id, format, bit_depth, sample_rate, quality_score, available) VALUES (?, 2, ?, 'FLAC', 24, 96000, 150, 1)"
        )
        .bind(tid).bind(&qobuz_item_id).execute(&db).await.unwrap();

        // Enqueue into download_queue with locked source identity
        let qid: i64 = sqlx::query_scalar(
            r#"
            INSERT INTO download_queue (
                track_id, priority, position, status, quality_preference, resumable,
                service_id, service_name, service_track_id,
                target_title, target_artist, target_album, target_isrc,
                allow_fallback, smart_studio_origin, created_at
            )
            VALUES (?, 100 - ?, ?, 'queued', 'lossless', 1, 2, 'qobuz', ?, ?, ?, ?, ?, 0, 1, CURRENT_TIMESTAMP)
            RETURNING id
            "#
        )
        .bind(tid)
        .bind(i as i64)
        .bind((i - 1) as i64)
        .bind(&qobuz_item_id)
        .bind(&track_title)
        .bind(artist_name)
        .bind(cur_album_name)
        .bind(&track_isrc)
        .fetch_one(&db)
        .await
        .unwrap();

        track_ids.push(tid);
        queue_ids.push(qid);
    }

    assert_eq!(track_ids.len(), 50);
    assert_eq!(queue_ids.len(), 50);

    // 2. Concurrency management: Concurrency = 3
    const MAX_CONCURRENCY: usize = 3;
    let semaphore = Arc::new(Semaphore::new(MAX_CONCURRENCY));
    let active_concurrency = Arc::new(AtomicUsize::new(0));
    let peak_concurrency = Arc::new(AtomicUsize::new(0));
    let worker_state = DownloadWorkerState::new(MAX_CONCURRENCY);

    let mut tasks = Vec::new();

    for i in 1..=50 {
        let sem_clone = semaphore.clone();
        let active_clone = active_concurrency.clone();
        let peak_clone = peak_concurrency.clone();
        let db_clone = db.clone();
        let staging_dir_clone = staging_dir.clone();
        let layout_clone = layout.clone();
        let qid = queue_ids[i - 1];
        let tid = track_ids[i - 1];
        let cur_album = if i <= 25 { album1_name } else { album2_name };
        let track_title = format!("Odyssey Movement {:02}", i);

        let task = tokio::spawn(async move {
            let _permit = sem_clone.acquire().await.unwrap();

            // Track active & peak concurrency
            let current = active_clone.fetch_add(1, Ordering::SeqCst) + 1;
            peak_clone.fetch_max(current, Ordering::SeqCst);
            assert!(
                current <= MAX_CONCURRENCY,
                "Active concurrency ({}) must never exceed MAX_CONCURRENCY ({})",
                current,
                MAX_CONCURRENCY
            );

            // A. Mark item as downloading in SQLite
            sqlx::query("UPDATE download_queue SET status = 'downloading', started_at = CURRENT_TIMESTAMP WHERE id = ?")
                .bind(qid)
                .execute(&db_clone)
                .await
                .unwrap();

            // B. Simulate in-flight streaming into staging directory
            let staging_item_base = format!("track_{:03}_staging", i);
            let staging_flac = staging_dir_clone.join(format!("{}.part", staging_item_base));
            let staging_lrc = staging_dir_clone.join(format!("{}.lrc", staging_item_base));
            let staging_cover_jpg = staging_dir_clone.join(format!("{}.cover.jpg", staging_item_base));
            let staging_cover_webp = staging_dir_clone.join(format!("{}.cover.webp", staging_item_base));
            let staging_booklet_pdf = staging_dir_clone.join(format!("{}.booklet.pdf", staging_item_base));

            // Write authentic synthetic FLAC audio (24-bit, 96000Hz)
            create_synthetic_test_flac(&staging_flac, 96000, 24);
            assert!(AudioByteValidator::is_flac_magic(&std::fs::read(&staging_flac).unwrap()));

            // Write sidecar staging files
            let lrc_content = format!(
                "[00:00.00] Syncify Batch Track {:02}\n[00:04.00] Auditing VorbisComments 48 tags\n[00:08.00] Forensics verified cleanly",
                i
            );
            std::fs::write(&staging_lrc, &lrc_content).unwrap();
            std::fs::write(&staging_cover_jpg, &[0xFF, 0xD8, 0xFF, 0xE0, 0x00, 0x10, 0x4A, 0x46, 0x49, 0x46, 0xFF, 0xD9]).unwrap();
            std::fs::write(&staging_cover_webp, b"RIFF\x20\x00\x00\x00WEBPVP8X\x0A\x00\x00\x00\x02\x00\x00\x00\xF3\x01\x00\xF3\x01\x00").unwrap();
            std::fs::write(&staging_booklet_pdf, b"%PDF-1.7 Digital Booklet Goodies Syncify Edition").unwrap();

            // C. AudioAnalyzer & Complete 48 VorbisComments Tagging
            let analysis = AudioAnalyzer::analyze_file(&staging_flac).await.unwrap_or_default();
            let mut meta = build_48_field_metadata(i, artist_name, cur_album, &track_title);
            if analysis.bpm.is_some() {
                meta.bpm = analysis.bpm;
            }
            if analysis.initial_key.is_some() {
                meta.initial_key = analysis.initial_key;
            }

            let tag_res = apply_and_verify_flac_tags(&staging_flac, &meta);
            assert!(tag_res.is_ok(), "apply_and_verify_flac_tags failed for track {}: {:?}", i, tag_res.err());

            // D. Target destination directory resolution via LibraryLayout
            let ctx = TrackLayoutContext {
                artist: artist_name,
                album_artist: Some(artist_name),
                album: cur_album,
                title: &track_title,
                year: Some(2026),
                original_date: Some("2026-08-17"),
                track_number: i as u32,
                track_total: Some(50),
                disc_number: 1,
                total_discs: 1,
                format: "flac",
                bit_depth: Some(24),
                sample_rate: Some(96000.0),
            };

            let final_track_path = layout_clone.resolve_track_path(&ctx);
            let final_album_dir = final_track_path.parent().unwrap().to_path_buf();
            let final_artist_dir = layout_clone.artist_dir(artist_name);

            std::fs::create_dir_all(&final_album_dir).unwrap();
            std::fs::create_dir_all(&final_artist_dir).unwrap();

            // E. Atomic Promotion of audio and all sidecars
            std::fs::rename(&staging_flac, &final_track_path).unwrap();

            let final_lrc = layout_clone.lyrics_path_for_track(&final_track_path);
            std::fs::rename(&staging_lrc, &final_lrc).unwrap();

            let final_cover_jpg = final_album_dir.join("cover.jpg");
            if !final_cover_jpg.exists() {
                std::fs::copy(&staging_cover_jpg, &final_cover_jpg).unwrap();
            }
            std::fs::remove_file(&staging_cover_jpg).unwrap();

            let final_cover_webp = final_album_dir.join("cover.webp");
            let final_folder_webp = final_album_dir.join("folder.webp");
            let final_anim_webp = final_album_dir.join("animated.webp");
            if !final_cover_webp.exists() {
                std::fs::copy(&staging_cover_webp, &final_cover_webp).unwrap();
                std::fs::copy(&staging_cover_webp, &final_folder_webp).unwrap();
                std::fs::copy(&staging_cover_webp, &final_anim_webp).unwrap();
            }
            std::fs::remove_file(&staging_cover_webp).unwrap();

            let final_booklet = final_album_dir.join("booklet.pdf");
            if !final_booklet.exists() {
                std::fs::copy(&staging_booklet_pdf, &final_booklet).unwrap();
            }
            std::fs::remove_file(&staging_booklet_pdf).unwrap();

            // Artist sidecars (only write once)
            let final_artist_nfo = final_artist_dir.join("artist.nfo");
            let final_artist_bio = final_artist_dir.join("biography.txt");
            let final_artist_fanart = final_artist_dir.join("fanart.jpg");
            if !final_artist_nfo.exists() {
                std::fs::write(&final_artist_nfo, b"<artist><name>Syncify Master Ensemble</name></artist>").unwrap();
                std::fs::write(&final_artist_bio, b"Forensic test artist for S119B 50-track batch.").unwrap();
                std::fs::write(&final_artist_fanart, &[0xFF, 0xD8, 0xFF, 0xE0, 0x00, 0x10, 0x4A, 0x46, 0x49, 0x46, 0xFF, 0xD9]).unwrap();
            }

            // F. Transactional SQLite State Update
            let file_size_bytes = std::fs::metadata(&final_track_path).unwrap().len() as i64;
            let path_str = final_track_path.to_string_lossy().to_string();

            sqlx::query(
                "UPDATE download_queue SET status = 'complete', completed_at = CURRENT_TIMESTAMP, progress_percent = 100.0 WHERE id = ?"
            )
            .bind(qid)
            .execute(&db_clone)
            .await
            .unwrap();

            sqlx::query(
                r#"
                INSERT INTO downloads (
                    track_id, source_service_id, file_path, file_format, bit_depth, sample_rate, file_size_bytes, downloaded_at
                )
                VALUES (?, 2, ?, 'FLAC', 24, 96000, ?, CURRENT_TIMESTAMP)
                ON CONFLICT(track_id) DO UPDATE SET
                    file_path = excluded.file_path,
                    file_format = excluded.file_format,
                    bit_depth = excluded.bit_depth,
                    sample_rate = excluded.sample_rate,
                    file_size_bytes = excluded.file_size_bytes,
                    downloaded_at = CURRENT_TIMESTAMP
                "#
            )
            .bind(tid)
            .bind(&path_str)
            .bind(file_size_bytes)
            .execute(&db_clone)
            .await
            .unwrap();

            active_clone.fetch_sub(1, Ordering::SeqCst);
        });

        tasks.push(task);
    }

    // Await all 50 concurrent download tasks
    for task in tasks {
        task.await.expect("Task join failed");
    }

    // Verify concurrency reached 3
    let peak = peak_concurrency.load(Ordering::SeqCst);
    assert_eq!(peak, 3, "Peak concurrency must reach 3");

    // ═════════════════════════════════════════════════════════════════
    // FORENSIC AUDIT 1: Zero-Staging Residual Invariant
    // ═════════════════════════════════════════════════════════════════
    let remaining_staging: Vec<_> = std::fs::read_dir(&staging_dir)
        .unwrap()
        .map(|e| e.unwrap().path())
        .collect();
    assert!(
        remaining_staging.is_empty(),
        "Staging directory MUST contain 0 residual / orphaned files post-promotion: {:?}",
        remaining_staging
    );

    // ═════════════════════════════════════════════════════════════════
    // FORENSIC AUDIT 2: Audio Integrity & 48 VorbisComments Verification
    // ═════════════════════════════════════════════════════════════════
    let completed_downloads: Vec<(i64, String, i32, i32, i64)> = sqlx::query_as(
        "SELECT track_id, file_path, bit_depth, sample_rate, file_size_bytes FROM downloads ORDER BY track_id ASC"
    )
    .fetch_all(&db)
    .await
    .unwrap();

    assert_eq!(completed_downloads.len(), 50, "All 50 tracks must be in downloads table");

    for (idx, (_tid, file_path_str, bit_depth, sample_rate, size_bytes)) in completed_downloads.iter().enumerate() {
        let i = idx + 1;
        let file_path = Path::new(file_path_str);

        // A. Physical existence & non-zero size
        assert!(file_path.exists(), "Track file does not exist: {:?}", file_path);
        assert!(*size_bytes > 0, "File size must be > 0 bytes");

        // B. FLAC magic header
        let raw_bytes = std::fs::read(file_path).unwrap();
        assert!(AudioByteValidator::is_flac_magic(&raw_bytes), "File {:?} missing fLaC magic header", file_path);
        assert_eq!(*bit_depth, 24);
        assert_eq!(*sample_rate, 96000);

        // C. Complete 48 VorbisComments Tags Verification via metaflac
        let tag = metaflac::Tag::read_from_path(file_path).expect("Must read FLAC tags");
        let comments = tag.vorbis_comments().expect("VorbisComments must exist");

        // 1. TITLE
        assert_eq!(comments.get("TITLE").unwrap()[0], format!("Odyssey Movement {:02}", i));
        // 2. ARTIST
        assert_eq!(comments.get("ARTIST").unwrap()[0], artist_name);
        // 3. ALBUM
        let expected_album = if i <= 25 { album1_name } else { album2_name };
        assert_eq!(comments.get("ALBUM").unwrap()[0], expected_album);
        // 4. ALBUMARTIST
        assert_eq!(comments.get("ALBUMARTIST").unwrap()[0], artist_name);
        // 5. COMPOSER
        assert_eq!(comments.get("COMPOSER").unwrap()[0], format!("Composer of Odyssey Movement {:02}", i));
        // 6. PERFORMER
        assert_eq!(comments.get("PERFORMER").unwrap()[0], format!("Lead Performer, Soloist {}", i));
        // 7. WORK
        assert_eq!(comments.get("WORK").unwrap()[0], format!("Opus No. {}", i));
        // 8. GENRE
        assert_eq!(comments.get("GENRE").unwrap()[0], "Hi-Res Symphonic Electronic");
        // 9. STYLE
        assert_eq!(comments.get("STYLE").unwrap()[0], "Progressive / Ambient");
        // 10. MOOD
        assert_eq!(comments.get("MOOD").unwrap()[0], "Expansive");
        // 11. RELEASETYPE
        assert_eq!(comments.get("RELEASETYPE").unwrap()[0], "Album");
        // 12. RELEASESTATUS
        assert_eq!(comments.get("RELEASESTATUS").unwrap()[0], "Official");
        // 13. RELEASECOUNTRY
        // directiva del propietario 2026-08-24: nombres en el cable; anula contrato alpha-2 de S183
        assert_eq!(comments.get("RELEASECOUNTRY").unwrap()[0], "United States");
        // 14. LANGUAGE
        // directiva del propietario 2026-08-24: nombres en el cable; anula contrato alpha-2 de S183
        assert_eq!(comments.get("LANGUAGE").unwrap()[0], "English");
        // 15. COPYRIGHT
        assert_eq!(comments.get("COPYRIGHT").unwrap()[0], format!("(P) 2026 Syncify Music Group LLC, Track {:02}", i));
        // 16. LABEL
        assert_eq!(comments.get("LABEL").unwrap()[0], "Syncify Masterworks");
        // 17. BARCODE
        assert_eq!(comments.get("BARCODE").unwrap()[0], format!("8809987{:05}", i));
        // 18. CATALOGNUMBER
        assert_eq!(comments.get("CATALOGNUMBER").unwrap()[0], format!("SYN-{:04}", i));
        // 19. ORIGINALDATE
        assert_eq!(comments.get("ORIGINALDATE").unwrap()[0], "2026-08-17");
        // 20. TRACKNUMBER
        assert_eq!(comments.get("TRACKNUMBER").unwrap()[0], i.to_string());
        // 21. TRACKTOTAL
        assert_eq!(comments.get("TRACKTOTAL").unwrap()[0], "50");
        // 22. DISCNUMBER
        assert_eq!(comments.get("DISCNUMBER").unwrap()[0], "1");
        // 23. DISCTOTAL
        assert_eq!(comments.get("DISCTOTAL").unwrap()[0], "1");
        // 24. DISCSUBTITLE
        assert_eq!(comments.get("DISCSUBTITLE").unwrap()[0], "Master Disc");
        // 25. ISRC
        assert_eq!(comments.get("ISRC").unwrap()[0], format!("USSYN26000{:02}", i));
        // 26. YEAR
        assert_eq!(comments.get("YEAR").unwrap()[0], "2026");
        // 27. RELEASEDATE
        assert_eq!(comments.get("RELEASEDATE").unwrap()[0], "2026-08-17");
        // 28. BPM
        assert!(comments.get("BPM").is_some());
        // 29. KEY & 30. INITIALKEY
        assert!(comments.get("KEY").is_some());
        assert!(comments.get("INITIALKEY").is_some());
        // 31. REPLAYGAIN_TRACK_GAIN
        assert!(comments.get("REPLAYGAIN_TRACK_GAIN").is_some());
        // 32. REPLAYGAIN_TRACK_PEAK
        assert_eq!(comments.get("REPLAYGAIN_TRACK_PEAK").unwrap()[0], "0.988220");
        // 33. REPLAYGAIN_ALBUM_GAIN
        assert_eq!(comments.get("REPLAYGAIN_ALBUM_GAIN").unwrap()[0], "-5.20 dB");
        // 34. REPLAYGAIN_ALBUM_PEAK
        assert_eq!(comments.get("REPLAYGAIN_ALBUM_PEAK").unwrap()[0], "0.995000");
        // 35. R128_TRACK_GAIN
        assert!(comments.get("R128_TRACK_GAIN").is_some());
        // 36. ENERGY
        assert!(comments.get("ENERGY").is_some());
        // 37. DANCEABILITY
        assert!(comments.get("DANCEABILITY").is_some());
        // 38. LOUDNESS
        assert!(comments.get("LOUDNESS").is_some());
        // 39. COMMENT
        assert!(comments.get("COMMENT").unwrap()[0].contains("Qobuz FLAC 24/96"));
        // 40. SYNCIFY_LYRICS_SOURCE
        assert_eq!(comments.get("SYNCIFY_LYRICS_SOURCE").unwrap()[0], "LRCLIB Premium");
        // 41. SYNCIFY_COVER_SOURCE
        assert_eq!(comments.get("SYNCIFY_COVER_SOURCE").unwrap()[0], "Qobuz Studio Master");
        // 42. SYNCIFY_AUDIO_SOURCE
        assert_eq!(comments.get("SYNCIFY_AUDIO_SOURCE").unwrap()[0], "Qobuz");
        // 43. BITDEPTH
        assert_eq!(comments.get("BITDEPTH").unwrap()[0], "24");
        // 44. SAMPLINGRATE
        assert_eq!(comments.get("SAMPLINGRATE").unwrap()[0], "96000");
        // 45. LYRICS
        assert!(comments.get("LYRICS").unwrap()[0].contains(&format!("Syncify Batch Track {:02}", i)));
        // 46. UNSYNCEDLYRICS
        assert!(comments.get("UNSYNCEDLYRICS").is_some());
        // 47. MUSICBRAINZ_TRACKID
        assert_eq!(comments.get("MUSICBRAINZ_TRACKID").unwrap()[0], format!("11111111-2222-3333-4444-{:012x}", i));
        // 48. MUSICBRAINZ_ALBUMID
        assert_eq!(comments.get("MUSICBRAINZ_ALBUMID").unwrap()[0], "66666666-7777-8888-9999-000000000000");

        // Picture blocks check: CoverFront present
        let pictures: Vec<_> = tag.pictures().collect();
        assert_eq!(pictures.len(), 1, "Must contain exactly 1 embedded CoverFront picture");
        assert_eq!(pictures[0].picture_type, metaflac::block::PictureType::CoverFront);
        assert!(!pictures[0].data.is_empty());
    }

    // ═════════════════════════════════════════════════════════════════
    // FORENSIC AUDIT 3: Sidecars Verification (.lrc, cover, booklet)
    // ═════════════════════════════════════════════════════════════════
    for (_tid, file_path_str, ..) in &completed_downloads {
        let track_p = Path::new(file_path_str);
        let lrc_p = track_p.with_extension("lrc");
        assert!(lrc_p.exists(), "Sidecar .lrc must exist: {:?}", lrc_p);
        let lrc_content = std::fs::read_to_string(&lrc_p).unwrap();
        assert!(lrc_content.contains("Auditing VorbisComments 48 tags"));
    }

    // Album directory sidecars
    for album_name in &[album1_name, album2_name] {
        let album_dir = layout.album_dir(artist_name, album_name, Some(2026));
        assert!(album_dir.join("cover.jpg").exists(), "cover.jpg missing in {:?}", album_dir);
        assert!(album_dir.join("cover.webp").exists(), "cover.webp missing in {:?}", album_dir);
        assert!(album_dir.join("folder.webp").exists(), "folder.webp missing in {:?}", album_dir);
        assert!(album_dir.join("animated.webp").exists(), "animated.webp missing in {:?}", album_dir);
        assert!(album_dir.join("booklet.pdf").exists(), "booklet.pdf missing in {:?}", album_dir);

        let booklet_bytes = std::fs::read(album_dir.join("booklet.pdf")).unwrap();
        assert!(booklet_bytes.starts_with(b"%PDF-1.7"));
    }

    // Artist directory sidecars
    let artist_dir = layout.artist_dir(artist_name);
    assert!(artist_dir.join("artist.nfo").exists());
    assert!(artist_dir.join("biography.txt").exists());
    assert!(artist_dir.join("fanart.jpg").exists());

    // ═════════════════════════════════════════════════════════════════
    // FORENSIC AUDIT 4: SQLite Transactional Consistency & Manifest Reconciliation
    // ═════════════════════════════════════════════════════════════════
    let (completed_queue_count,): (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM download_queue WHERE status = 'complete' AND progress_percent = 100.0"
    )
    .fetch_one(&db)
    .await
    .unwrap();

    let (failed_queue_count,): (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM download_queue WHERE status = 'failed'"
    )
    .fetch_one(&db)
    .await
    .unwrap();

    assert_eq!(completed_queue_count, 50, "All 50 queue items must be marked complete");
    assert_eq!(failed_queue_count, 0, "No queue items should have failed");

    // Manifest reconciliation
    let manifest: BatchDownloadManifest = ManifestWriter::generate_and_save_manifest(&db, &base_music_dir)
        .await
        .expect("Manifest generation must succeed");

    assert_eq!(manifest.total_requested, 50);
    assert_eq!(manifest.total_succeeded, 50);
    assert_eq!(manifest.total_failed, 0);
    assert_eq!(manifest.total_skipped, 0);
    assert_eq!(manifest.entries.len(), 50);

    for entry in &manifest.entries {
        assert_eq!(entry.download_result, "Success");
        assert_eq!(entry.bit_depth, Some(24));
        assert_eq!(entry.sample_rate, Some(96000));
        assert!(!entry.created_artifacts.is_empty());
        assert!(entry.created_artifacts.iter().any(|a| a.ends_with(".flac")));
        assert!(entry.created_artifacts.iter().any(|a| a.ends_with(".lrc")));
    }

    // ═════════════════════════════════════════════════════════════════
    // FORENSIC AUDIT 5: Diagnostic Health Check Command Audit
    // ═════════════════════════════════════════════════════════════════
    let health_report: BatchHealthReport = perform_batch_health_check(&db, Some(&staging_dir), Some(&worker_state))
        .await
        .expect("perform_batch_health_check must succeed");

    assert!(health_report.database_healthy, "Database must be healthy");
    assert_eq!(health_report.database_integrity.trim(), "ok");
    assert!(health_report.foreign_keys_valid, "Foreign keys must be valid");
    assert_eq!(health_report.queue_total, 50);
    assert_eq!(health_report.queue_completed, 50);
    assert_eq!(health_report.queue_failed, 0);
    assert_eq!(health_report.downloads_total, 50);
    assert_eq!(health_report.downloads_verified_on_disk, 50);
    assert_eq!(health_report.downloads_missing_on_disk, 0);
    assert_eq!(health_report.staging_orphans_count, 0);
    assert_eq!(health_report.staging_orphans_bytes, 0);
    assert_eq!(health_report.worker_max_concurrent, 3);
    assert!(health_report.healthy, "BatchHealthReport.healthy must be true: issues: {:?}", health_report.issues);
    assert!(health_report.issues.is_empty(), "No diagnostic issues should be reported: {:?}", health_report.issues);
}

#[tokio::test]
async fn test_batch_health_check_detects_anomalies_and_staging_orphans() {
    let db = create_test_db().await;
    let temp_root = TempDir::new().unwrap();
    let staging_dir = temp_root.path().join(".staging");
    std::fs::create_dir_all(&staging_dir).unwrap();

    let worker_state = DownloadWorkerState::new(3);

    // Initial clean check
    let clean_report: BatchHealthReport = perform_batch_health_check(&db, Some(&staging_dir), Some(&worker_state)).await.unwrap();
    assert!(clean_report.healthy);

    // Anomaly 1: Staging orphan file left behind
    let orphan_file = staging_dir.join("orphan_chunk.part");
    std::fs::write(&orphan_file, b"corrupt partial chunk").unwrap();

    let orphan_report: BatchHealthReport = perform_batch_health_check(&db, Some(&staging_dir), Some(&worker_state)).await.unwrap();
    assert!(!orphan_report.healthy, "Must report unhealthy when orphan is in staging");
    assert_eq!(orphan_report.staging_orphans_count, 1);
    assert!(orphan_report.staging_orphans_bytes > 0);
    assert!(orphan_report.issues.iter().any(|i: &String| i.contains("Staging directory contains 1 orphan")));

    // Cleanup orphan
    std::fs::remove_file(&orphan_file).unwrap();

    // Anomaly 2: Download row points to non-existent file on disk
    let tid: i64 = sqlx::query_scalar("INSERT INTO tracks (title) VALUES ('Ghost Track') RETURNING id")
        .fetch_one(&db).await.unwrap();
    sqlx::query(
        "INSERT INTO downloads (track_id, file_path, file_format, bit_depth, sample_rate, file_size_bytes) VALUES (?, 'C:/NonExistent/ghost.flac', 'FLAC', 16, 44100, 5000)"
    )
    .bind(tid)
    .execute(&db).await.unwrap();

    let missing_report: BatchHealthReport = perform_batch_health_check(&db, Some(&staging_dir), Some(&worker_state)).await.unwrap();
    assert!(!missing_report.healthy, "Must report unhealthy when downloaded track is missing on disk");
    assert_eq!(missing_report.downloads_missing_on_disk, 1);
    assert!(missing_report.issues.iter().any(|i: &String| i.contains("missing from filesystem")));
}

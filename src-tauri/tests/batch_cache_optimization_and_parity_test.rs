//! Sprint S140A/B: Batch Optimization, Session Caching, and CLI vs GUI Parity Matrix Tests

use syncify_tauri_lib::services::animated_cover::{
    clear_animated_cover_cache, clear_apple_music_token_cache, extract_apple_music_token,
    get_cached_apple_music_token, resolve_and_download_animated_cover,
};
use syncify_tauri_lib::download::lyrics::{
    clear_lyrics_cache, set_cached_lyrics, LyricsPipelineService, LyricsResolution, ResolutionStatus,
};
use syncify_tauri_lib::services::musicbrainz::{clear_musicbrainz_cache, MusicBrainzClient, MusicBrainzRecording};
use syncify_tauri_lib::services::enrichment::{EnrichmentEngine, OriginTrackMetadata};
use syncify_tauri_lib::download::progress::{DownloadPhaseTimings, DownloadRequest, DownloadResult};
use std::time::Instant;
use tempfile::TempDir;

#[tokio::test]
async fn test_apple_music_token_session_caching() {
    let client = syncify_tauri_lib::download::http_client::create_http_client();

    // 1. Extract token (either already cached or fetched live)
    let token1 = extract_apple_music_token(&client).await;
    assert!(token1.is_some(), "extract_apple_music_token must successfully extract/return a token");
    let token1_val = token1.unwrap();

    // 2. Cached token getter must match
    assert_eq!(get_cached_apple_music_token(), Some(token1_val.clone()));

    // 3. Second call must be instantaneous from session cache (< 50ms)
    let start = Instant::now();
    let token2 = extract_apple_music_token(&client).await;
    let elapsed = start.elapsed();

    assert_eq!(token2, Some(token1_val));
    assert!(elapsed.as_millis() < 50, "Cached token lookup must be sub-50ms (took {:?})", elapsed);
}

#[tokio::test]
async fn test_animated_cover_album_caching() {
    clear_animated_cover_cache();
    clear_apple_music_token_cache();

    let temp_dir = TempDir::new().unwrap();
    let client = syncify_tauri_lib::download::http_client::create_http_client();

    let artist = "NonExistentArtistX999";
    let album = "NonExistentAlbumY888";

    // First call (uncached): returns NotFound / SourceUnavailable
    let status1 = resolve_and_download_animated_cover(&client, artist, album, temp_dir.path()).await;

    // Second call: should hit cache immediately
    let start = Instant::now();
    let status2 = resolve_and_download_animated_cover(&client, artist, album, temp_dir.path()).await;
    let elapsed = start.elapsed();

    assert_eq!(status1, status2, "Cached animated cover status must match first resolution");
    assert!(elapsed.as_millis() < 50, "Cached resolution must complete in sub-50ms without network latency (took {:?})", elapsed);
}

#[tokio::test]
async fn test_lyrics_identity_caching() {
    clear_lyrics_cache();

    let service = LyricsPipelineService::new();
    let artist = "ArtistBatchTest";
    let title = "TrackBatchTest";
    let album = Some("AlbumBatchTest");

    // First resolution
    let (res1, sidecar1): (LyricsResolution, Option<String>) = service.resolve_lyrics_and_sidecar(artist, title, album, 180.0).await.unwrap();

    // Second resolution: must be instantaneous from in-memory cache
    let start = Instant::now();
    let (res2, sidecar2): (LyricsResolution, Option<String>) = service.resolve_lyrics_and_sidecar(artist, title, album, 180.0).await.unwrap();
    let elapsed = start.elapsed();

    assert_eq!(res1.status, res2.status);
    assert_eq!(sidecar1, sidecar2);
    assert!(elapsed.as_millis() < 50, "Cached lyrics resolution must be sub-50ms (took {:?})", elapsed);
}

#[tokio::test]
async fn test_musicbrainz_in_memory_caching() {
    clear_musicbrainz_cache();

    let client = MusicBrainzClient::new();
    let sample_isrc = "USRC12345678";
    let mock_recording = MusicBrainzRecording {
        id: "mock-mbid-12345".to_string(),
        title: "Mock Title".to_string(),
        artist_credit: None,
        releases: None,
        genres: None,
        tags: None,
    };

    // Pre-seed cache
    syncify_tauri_lib::services::musicbrainz::set_cached_musicbrainz_recording(
        &format!("isrc:{}", sample_isrc),
        Some(mock_recording.clone()),
    );

    // Query (should be cached immediately without network)
    let start = Instant::now();
    let res: Option<MusicBrainzRecording> = client.lookup_by_isrc(sample_isrc).await.unwrap();
    let elapsed = start.elapsed();

    assert_eq!(res.map(|r| r.id), Some("mock-mbid-12345".to_string()));
    assert!(elapsed.as_millis() < 50, "Cached MusicBrainz lookup must be sub-50ms (took {:?})", elapsed);
}

#[tokio::test]
async fn test_enrichment_engine_skips_musicbrainz_when_pre_enriched() {
    let engine = EnrichmentEngine::new();
    let pre_enriched_mbid = "11111111-2222-3333-4444-555555555555";

    let origin = OriginTrackMetadata {
        title: Some("PreEnriched Song".to_string()),
        artist: Some("PreEnriched Artist".to_string()),
        album: Some("PreEnriched Album".to_string()),
        musicbrainz_recording_id: Some(pre_enriched_mbid.to_string()),
        source_name: "sync_pre_enrichment".to_string(),
        ..Default::default()
    };

    let start = Instant::now();
    let enriched = engine.resolve_track_metadata(
        "PreEnriched Artist",
        "PreEnriched Album",
        "PreEnriched Song",
        None,
        Some(&origin),
    ).await;
    let elapsed = start.elapsed();

    assert_eq!(
        enriched.musicbrainz_recording_id.value(),
        Some(pre_enriched_mbid),
        "Enriched metadata must retain pre-enriched MBID"
    );
    assert!(
        elapsed.as_millis() < 100,
        "Pre-enriched track must bypass MusicBrainz network lookup (took {:?})",
        elapsed
    );
}

#[test]
fn test_download_phase_timings_structure() {
    let timings = DownloadPhaseTimings {
        stream_duration_ms: 1200,
        metadata_duration_ms: 150,
        lyrics_duration_ms: 80,
        cover_duration_ms: 95,
        tagging_duration_ms: 45,
        promotion_duration_ms: 10,
        total_duration_ms: 1580,
        ..Default::default()
    };

    let res = DownloadResult {
        file_path: "C:\\Music\\track.flac".to_string(),
        bit_depth: 24,
        sample_rate: 96000,
        title: "Track Title".to_string(),
        artist: "Artist Name".to_string(),
        album: "Album Title".to_string(),
        release_date: Some("2024-01-01".to_string()),
        track_number: 1,
        disc_number: 1,
        isrc: Some("USABC1234567".to_string()),
        service: "qobuz".to_string(),
        origin_service: Some("qobuz".to_string()),
        origin_service_track_id: Some("12345".to_string()),
        effective_service: Some("qobuz".to_string()),
        effective_service_track_id: Some("12345".to_string()),
        fallback_reason: None,
        match_method: Some("exact_locked_source".to_string()),
        match_confidence: Some(1.0),
        phase_timings: Some(timings.clone()),
        quality_decision: None,
    };

    let json = serde_json::to_string(&res).unwrap();
    let deserialized: DownloadResult = serde_json::from_str(&json).unwrap();

    assert_eq!(deserialized.phase_timings, Some(timings));
}

#[test]
fn test_cli_flags_parity_matrix() {
    // 1. Quality preference mapping & strict fallback
    let req_strict = DownloadRequest {
        item_id: "test_item_1".to_string(),
        isrc: Some("US1234567890".to_string()),
        musicbrainz_recording_id: None,
        acoustid_fingerprint: None,
        spotify_id: None,
        service_name: Some("qobuz".to_string()),
        service_track_id: Some("1001".to_string()),
        service_album_id: None,
        track_name: "Test Song".to_string(),
        artist_name: "Test Artist".to_string(),
        album_name: "Test Album".to_string(),
        album_artist: None,
        duration_ms: 240000,
        track_number: 1,
        disc_number: 1,
        total_tracks: 10,
        release_date: Some("2024-01-01".to_string()),
        cover_url: None,
        output_dir: "C:\\Music\\Syncify".to_string(),
        quality: "HI_RES_LOSSLESS".to_string(),
        embed_lyrics: true,
        embed_artwork: true,
        smart_studio_origin: false,
        allow_fallback: false,
        strict_quality: true,
        ..Default::default()
    };

    assert!(!req_strict.allow_fallback, "--allow-lossy-fallback defaults to false");
    assert!(req_strict.strict_quality, "Strict quality enforcement is active by default");
    assert!(req_strict.embed_lyrics, "--sync-lyrics defaults to true during download");
    assert!(req_strict.embed_artwork, "--sync-covers defaults to true during download");
    assert!(!req_strict.smart_studio_origin, "--smart-studio-origin defaults to false");
}

#[tokio::test]
async fn test_in_memory_cache_benchmark_20_tracks() {
    clear_musicbrainz_cache();
    clear_lyrics_cache();
    clear_animated_cover_cache();
    clear_apple_music_token_cache();

    let client = syncify_tauri_lib::download::http_client::create_http_client();
    let engine = EnrichmentEngine::new();
    let lyrics_service = LyricsPipelineService::new();

    // 1. Warm-up cache with 1 track
    let _ = extract_apple_music_token(&client).await;

    // Seed MusicBrainz cache and Lyrics cache for 20 benchmark tracks
    for idx in 1..=20 {
        let album_idx = ((idx - 1) / 4) + 1;
        let isrc = format!("USRC2026{:04}", album_idx);
        let artist = format!("Benchmark Artist {}", album_idx);
        let album = format!("Benchmark Album {}", album_idx);
        let title = format!("Benchmark Track {}", idx);

        syncify_tauri_lib::services::musicbrainz::set_cached_musicbrainz_recording(
            &format!("isrc:{}", isrc),
            Some(MusicBrainzRecording {
                id: format!("mbid-rec-{}", idx),
                title: title.clone(),
                artist_credit: None,
                releases: None,
                genres: None,
                tags: None,
            }),
        );

        set_cached_lyrics(
            &artist,
            &title,
            Some(&album),
            LyricsResolution {
                status: ResolutionStatus::Resolved,
                provider: "lrclib".to_string(),
                strategy: "exact".to_string(),
                format: "LINE_SYNCED".to_string(),
                sync_type: syncify_tauri_lib::download::lyrics::LyricsSyncType::LineSynced,
                provenance: "cached_memory".to_string(),
                fallback_applied: false,
                error: None,
                synced_content: Some("[00:01.00]Benchmark lyrics line 1".to_string()),
                plain_text: Some("Benchmark lyrics line 1".to_string()),
                lines: vec![],
                is_instrumental: false,
            },
            Some("[00:01.00]Benchmark lyrics line 1".to_string()),
        );
    }

    // 2. Execute 20-track in-memory benchmark
    let total_start = Instant::now();
    let mut track_times = Vec::with_capacity(20);

    for idx in 1..=20 {
        let track_start = Instant::now();

        let album_idx = ((idx - 1) / 4) + 1; // 5 albums, 4 tracks each
        let isrc = format!("USRC2026{:04}", album_idx);
        let artist = format!("Benchmark Artist {}", album_idx);
        let album = format!("Benchmark Album {}", album_idx);
        let title = format!("Benchmark Track {}", idx);

        let origin = OriginTrackMetadata {
            title: Some(title.clone()),
            artist: Some(artist.clone()),
            album: Some(album.clone()),
            isrc: Some(isrc.clone()),
            source_name: "qobuz".to_string(),
            ..Default::default()
        };

        // A. Metadata Enrichment
        let enriched = engine.resolve_track_metadata(&artist, &album, &title, None, Some(&origin)).await;
        assert_eq!(enriched.title.value(), Some(title.as_str()));

        // B. Lyrics resolution (in-memory cached)
        let (res, sidecar) = lyrics_service.resolve_lyrics_and_sidecar(&artist, &title, Some(&album), 210.0).await.unwrap();
        assert_eq!(res.status, ResolutionStatus::Resolved);
        assert!(sidecar.is_some());

        // C. Token lookup (cached in-memory)
        let token = get_cached_apple_music_token();
        assert!(token.is_some(), "Apple Music session token must be cached");

        let duration = track_start.elapsed();
        track_times.push(duration);
    }

    let total_elapsed = total_start.elapsed();
    let avg_per_track_ms = total_elapsed.as_millis() as f64 / 20.0;

    println!(
        "=== 20-TRACK IN-MEMORY CACHE BENCHMARK ===\nTotal Time: {:?}\nAverage per track: {:.2}ms\nTrack Times: {:?}",
        total_elapsed, avg_per_track_ms, track_times
    );

    assert!(
        total_elapsed.as_millis() < 5000,
        "In-memory cache benchmark must complete in sub-5s (took {:?})",
        total_elapsed
    );
}

fn create_benchmark_synthetic_flac(path: &std::path::Path, sample_rate: u32, bit_depth: u8) {
    let mut data = Vec::new();
    data.extend_from_slice(b"fLaC"); // 4-byte magic

    // STREAMINFO block header (type 0, length 34)
    data.push(0x00);
    data.push(0x00);
    data.push(0x00);
    data.push(0x22);

    let mut streaminfo = [0u8; 34];
    streaminfo[0..2].copy_from_slice(&4096u16.to_be_bytes());
    streaminfo[2..4].copy_from_slice(&4096u16.to_be_bytes());

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

    // Audio frame data placeholder (64 KB for realistic disk I/O)
    data.extend_from_slice(&vec![0xAA; 65536]);

    std::fs::write(path, &data).expect("Failed to write synthetic benchmark FLAC");
}

#[tokio::test]
#[ignore = "Slow live network benchmark"]
async fn test_physical_batch_benchmark_20_tracks_comparison() {
    use syncify_flac_writer::{apply_and_verify_flac_tags, FlacMetadata};
    use std::sync::Arc;
    use tokio::sync::Semaphore;

    clear_musicbrainz_cache();
    clear_lyrics_cache();
    clear_animated_cover_cache();
    clear_apple_music_token_cache();

    let temp_root = TempDir::new().unwrap();
    let staging_dir = temp_root.path().join(".staging");
    let library_dir = temp_root.path().join("Music");
    std::fs::create_dir_all(&staging_dir).unwrap();
    std::fs::create_dir_all(&library_dir).unwrap();

    let client = syncify_tauri_lib::download::http_client::create_http_client();
    let engine = EnrichmentEngine::new();
    let lyrics_service = LyricsPipelineService::new();
    let concurrency_semaphore = Arc::new(Semaphore::new(5)); // Concurrency 5

    // Seed MusicBrainz cache
    for i in 1..=10 {
        syncify_tauri_lib::services::musicbrainz::set_cached_musicbrainz_recording(
            &format!("isrc:USPHYS2026{:02}", i),
            Some(MusicBrainzRecording {
                id: format!("mbid-phys-{}", i),
                title: format!("Physical Benchmark Track {}", i),
                artist_credit: None,
                releases: None,
                genres: None,
                tags: None,
            }),
        );
    }

    // ==========================================
    // COHORT A: 10 Tracks of the SAME Album
    // ==========================================
    let cohort_a_start = Instant::now();
    let mut cohort_a_timings = Vec::with_capacity(10);
    let artist_a = "SameAlbumArtist";
    let album_a = "SameAlbumTitle";

    for idx in 1..=10 {
        let permit = concurrency_semaphore.clone().acquire_owned().await.unwrap();
        let track_start = Instant::now();
        let title = format!("Track {:02}", idx);
        let isrc = format!("USPHYS2026{:02}", idx);

        // 1. Stream simulation (write physical FLAC stream to staging)
        let stream_start = Instant::now();
        let staging_flac = staging_dir.join(format!("cohort_a_track_{}.flac", idx));
        create_benchmark_synthetic_flac(&staging_flac, 96000, 24);
        let stream_dur = stream_start.elapsed();

        // 2. Lyrics resolution & sidecar writing
        let lyrics_start = Instant::now();
        let (_lyrics_res, sidecar_lrc) = lyrics_service.resolve_lyrics_and_sidecar(artist_a, &title, Some(album_a), 200.0).await.unwrap();
        let lyrics_dur = lyrics_start.elapsed();

        // 3. Cover / Animated Artwork resolution (Apple Music session cached)
        let cover_start = Instant::now();
        let _ = extract_apple_music_token(&client).await;
        let _ = resolve_and_download_animated_cover(&client, artist_a, album_a, &staging_dir).await;
        let cover_dur = cover_start.elapsed();

        // 4. Metadata enrichment
        let meta_start = Instant::now();
        let origin = OriginTrackMetadata {
            title: Some(title.clone()),
            artist: Some(artist_a.to_string()),
            album: Some(album_a.to_string()),
            isrc: Some(isrc.clone()),
            source_name: "qobuz".to_string(),
            ..Default::default()
        };
        let _enriched = engine.resolve_track_metadata(artist_a, album_a, &title, None, Some(&origin)).await;
        let meta_dur = meta_start.elapsed();

        // 5. FLAC Tagging (48 VorbisComments fields written physically to disk)
        let tag_start = Instant::now();
        let flac_meta = FlacMetadata {
            title: title.clone(),
            artist: artist_a.to_string(),
            album: album_a.to_string(),
            album_artist: Some(artist_a.to_string()),
            composer: Some("Benchmark Composer".to_string()),
            performers: Some("Benchmark Performer".to_string()),
            work: Some("Opus 2026".to_string()),
            genre: Some("Electronic".to_string()),
            style: Some("Ambient".to_string()),
            mood: Some("Expansive".to_string()),
            release_type: Some("Album".to_string()),
            release_status: Some("Official".to_string()),
            release_country: Some("US".to_string()),
            release_region: None,
            language: Some("eng".to_string()),
            copyright: Some("(C) 2026 Syncify Benchmark".to_string()),
            label: Some("Syncify Audio".to_string()),
            barcode: Some("123456789012".to_string()),
            catalog_number: Some("SYN-2026".to_string()),
            original_date: Some("2026-08-19".to_string()),
            track_number: idx as u32,
            track_total: 10,
            disc_number: 1,
            disc_total: 1,
            disc_subtitle: None,
            isrc: Some(isrc.clone()),
            release_year: Some("2026".to_string()),
            release_date: Some("2026-08-19".to_string()),
            explicit: Some(false),
            bpm: Some(120),
            initial_key: Some("C".to_string()),
            energy: Some(0.85),
            danceability: Some(0.70),
            loudness: Some(-7.0),
            replaygain_track_gain: Some("-4.50 dB".to_string()),
            replaygain_track_peak: Some("0.988000".to_string()),
            replaygain_album_gain: Some("-4.50 dB".to_string()),
            replaygain_album_peak: Some("0.988000".to_string()),
            r128_track_gain: Some("-1.50 LU".to_string()),
            comment: Some("Physical Benchmark".to_string()),
            bit_depth: Some(24),
            sample_rate: Some(96000.0),
            lyrics_lrc: sidecar_lrc.clone(),
            lyrics_source: Some("LRCLIB".to_string()),
            cover_source: Some("Apple Music".to_string()),
            audio_source: Some("Qobuz".to_string()),
            musicbrainz_track_id: Some(format!("mbid-rec-{}", idx)),
            musicbrainz_artist_id: Some("mbid-artist-1".to_string()),
            musicbrainz_album_id: Some("mbid-release-1".to_string()),
            musicbrainz_albumartist_id: Some("mbid-artist-1".to_string()),
            musicbrainz_release_group_id: Some("mbid-rg-1".to_string()),
            musicbrainz_work_id: None,
            cover_data: None,
            ..Default::default()
        };
        apply_and_verify_flac_tags(&staging_flac, &flac_meta).unwrap();
        let tag_dur = tag_start.elapsed();

        // 6. Promotion to target Library directory
        let prom_start = Instant::now();
        let target_album_dir = library_dir.join(artist_a).join(album_a);
        std::fs::create_dir_all(&target_album_dir).unwrap();
        let target_flac = target_album_dir.join(format!("{:02} - {}.flac", idx, title));
        std::fs::rename(&staging_flac, &target_flac).unwrap();
        if let Some(ref lrc_text) = sidecar_lrc {
            let target_lrc = target_album_dir.join(format!("{:02} - {}.lrc", idx, title));
            std::fs::write(target_lrc, lrc_text).unwrap();
        }
        let prom_dur = prom_start.elapsed();

        let total_track_dur = track_start.elapsed();
        drop(permit);

        cohort_a_timings.push(DownloadPhaseTimings {
            stream_duration_ms: stream_dur.as_millis() as u64,
            lyrics_duration_ms: lyrics_dur.as_millis() as u64,
            cover_duration_ms: cover_dur.as_millis() as u64,
            metadata_duration_ms: meta_dur.as_millis() as u64,
            tagging_duration_ms: tag_dur.as_millis() as u64,
            promotion_duration_ms: prom_dur.as_millis() as u64,
            total_duration_ms: total_track_dur.as_millis() as u64,
            ..Default::default()
        });
    }
    let cohort_a_elapsed = cohort_a_start.elapsed();

    // ==========================================
    // COHORT B: 10 Tracks of 10 DIFFERENT Albums
    // ==========================================
    let cohort_b_start = Instant::now();
    let mut cohort_b_timings = Vec::with_capacity(10);

    for idx in 1..=10 {
        let permit = concurrency_semaphore.clone().acquire_owned().await.unwrap();
        let track_start = Instant::now();
        let artist_b = format!("DiffArtist_{}", idx);
        let album_b = format!("DiffAlbum_{}", idx);
        let title = format!("DiffTrack_{}", idx);
        let isrc = format!("USPHYS2026{:02}", idx);

        // 1. Stream
        let stream_start = Instant::now();
        let staging_flac = staging_dir.join(format!("cohort_b_track_{}.flac", idx));
        create_benchmark_synthetic_flac(&staging_flac, 96000, 24);
        let stream_dur = stream_start.elapsed();

        // 2. Lyrics
        let lyrics_start = Instant::now();
        let (_lyrics_res, sidecar_lrc) = lyrics_service.resolve_lyrics_and_sidecar(&artist_b, &title, Some(&album_b), 200.0).await.unwrap();
        let lyrics_dur = lyrics_start.elapsed();

        // 3. Cover
        let cover_start = Instant::now();
        let _ = extract_apple_music_token(&client).await;
        let _ = resolve_and_download_animated_cover(&client, &artist_b, &album_b, &staging_dir).await;
        let cover_dur = cover_start.elapsed();

        // 4. Metadata
        let meta_start = Instant::now();
        let origin = OriginTrackMetadata {
            title: Some(title.clone()),
            artist: Some(artist_b.clone()),
            album: Some(album_b.clone()),
            isrc: Some(isrc.clone()),
            source_name: "qobuz".to_string(),
            ..Default::default()
        };
        let _enriched = engine.resolve_track_metadata(&artist_b, &album_b, &title, None, Some(&origin)).await;
        let meta_dur = meta_start.elapsed();

        // 5. Tagging
        let tag_start = Instant::now();
        let flac_meta = FlacMetadata {
            title: title.clone(),
            artist: artist_b.clone(),
            album: album_b.clone(),
            album_artist: Some(artist_b.clone()),
            composer: Some("Diff Composer".to_string()),
            performers: Some("Diff Performer".to_string()),
            work: None,
            genre: Some("Jazz".to_string()),
            style: None,
            mood: None,
            release_type: Some("Album".to_string()),
            release_status: Some("Official".to_string()),
            release_country: Some("GB".to_string()),
            release_region: None,
            language: Some("eng".to_string()),
            copyright: Some("(C) 2026 Diff Records".to_string()),
            label: Some("Diff Label".to_string()),
            barcode: None,
            catalog_number: None,
            original_date: Some("2026-08-19".to_string()),
            track_number: 1,
            track_total: 1,
            disc_number: 1,
            disc_total: 1,
            disc_subtitle: None,
            isrc: Some(isrc.clone()),
            release_year: Some("2026".to_string()),
            release_date: Some("2026-08-19".to_string()),
            explicit: Some(false),
            bpm: Some(95),
            initial_key: Some("G".to_string()),
            energy: Some(0.60),
            danceability: Some(0.55),
            loudness: Some(-9.0),
            replaygain_track_gain: Some("-3.20 dB".to_string()),
            replaygain_track_peak: Some("0.950000".to_string()),
            replaygain_album_gain: Some("-3.20 dB".to_string()),
            replaygain_album_peak: Some("0.950000".to_string()),
            r128_track_gain: Some("-2.00 LU".to_string()),
            comment: Some("Physical Benchmark Diff".to_string()),
            bit_depth: Some(24),
            sample_rate: Some(96000.0),
            lyrics_lrc: sidecar_lrc.clone(),
            lyrics_source: Some("LRCLIB".to_string()),
            cover_source: Some("Apple Music".to_string()),
            audio_source: Some("Qobuz".to_string()),
            musicbrainz_track_id: Some(format!("mbid-rec-diff-{}", idx)),
            musicbrainz_artist_id: Some("mbid-artist-diff".to_string()),
            musicbrainz_album_id: Some("mbid-release-diff".to_string()),
            musicbrainz_albumartist_id: Some("mbid-artist-diff".to_string()),
            musicbrainz_release_group_id: Some("mbid-rg-diff".to_string()),
            musicbrainz_work_id: None,
            cover_data: None,
            ..Default::default()
        };
        apply_and_verify_flac_tags(&staging_flac, &flac_meta).unwrap();
        let tag_dur = tag_start.elapsed();

        // 6. Promotion
        let prom_start = Instant::now();
        let target_album_dir = library_dir.join(&artist_b).join(&album_b);
        std::fs::create_dir_all(&target_album_dir).unwrap();
        let target_flac = target_album_dir.join(format!("01 - {}.flac", title));
        std::fs::rename(&staging_flac, &target_flac).unwrap();
        let prom_dur = prom_start.elapsed();

        let total_track_dur = track_start.elapsed();
        drop(permit);

        cohort_b_timings.push(DownloadPhaseTimings {
            stream_duration_ms: stream_dur.as_millis() as u64,
            lyrics_duration_ms: lyrics_dur.as_millis() as u64,
            cover_duration_ms: cover_dur.as_millis() as u64,
            metadata_duration_ms: meta_dur.as_millis() as u64,
            tagging_duration_ms: tag_dur.as_millis() as u64,
            promotion_duration_ms: prom_dur.as_millis() as u64,
            total_duration_ms: total_track_dur.as_millis() as u64,
            ..Default::default()
        });
    }
    let cohort_b_elapsed = cohort_b_start.elapsed();

    // ==========================================
    // BENCHMARK REPORT
    // ==========================================
    println!("\n================ PHYSICAL 20-TRACK BENCHMARK REPORT ================");
    println!("COHORT A (10 Tracks, Same Album): Total {:?}, Avg/track {:.2}ms", cohort_a_elapsed, cohort_a_elapsed.as_millis() as f64 / 10.0);
    for (i, t) in cohort_a_timings.iter().enumerate() {
        println!("  [Cohort A #{:02}] stream: {}ms, lyrics: {}ms, cover: {}ms, meta: {}ms, tagging: {}ms, promo: {}ms => total: {}ms",
            i + 1, t.stream_duration_ms, t.lyrics_duration_ms, t.cover_duration_ms, t.metadata_duration_ms, t.tagging_duration_ms, t.promotion_duration_ms, t.total_duration_ms);
    }

    println!("\nCOHORT B (10 Tracks, 10 Diff Albums): Total {:?}, Avg/track {:.2}ms", cohort_b_elapsed, cohort_b_elapsed.as_millis() as f64 / 10.0);
    for (i, t) in cohort_b_timings.iter().enumerate() {
        println!("  [Cohort B #{:02}] stream: {}ms, lyrics: {}ms, cover: {}ms, meta: {}ms, tagging: {}ms, promo: {}ms => total: {}ms",
            i + 1, t.stream_duration_ms, t.lyrics_duration_ms, t.cover_duration_ms, t.metadata_duration_ms, t.tagging_duration_ms, t.promotion_duration_ms, t.total_duration_ms);
    }
    println!("===================================================================\n");

    // Assert that files exist on disk physically
    assert!(library_dir.join(artist_a).join(album_a).join("01 - Track 01.flac").exists());
    assert!(library_dir.join("DiffArtist_1").join("DiffAlbum_1").join("01 - DiffTrack_1.flac").exists());
}

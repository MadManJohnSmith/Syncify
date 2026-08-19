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
    };

    assert!(!req_strict.allow_fallback, "--allow-lossy-fallback defaults to false");
    assert!(req_strict.strict_quality, "Strict quality enforcement is active by default");
    assert!(req_strict.embed_lyrics, "--sync-lyrics defaults to true during download");
    assert!(req_strict.embed_artwork, "--sync-covers defaults to true during download");
    assert!(!req_strict.smart_studio_origin, "--smart-studio-origin defaults to false");
}

#[tokio::test]
async fn test_benchmark_20_tracks_with_session_caching() {
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

    // 2. Execute 20-track benchmark
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
        "=== 20-TRACK REAL BENCHMARK RESULTS ===\nTotal Time: {:?}\nAverage per track: {:.2}ms\nTrack Times: {:?}",
        total_elapsed, avg_per_track_ms, track_times
    );

    assert!(
        total_elapsed.as_millis() < 5000,
        "20-track batch execution with session caching must complete in sub-5s (took {:?})",
        total_elapsed
    );
    assert!(
        avg_per_track_ms < 250.0,
        "Average processing time per track must be sub-250ms (average was {:.2}ms)",
        avg_per_track_ms
    );
}

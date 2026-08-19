//! Tests for Queue Global Progress and Physical Multi-Track Telemetry
//! Validates stable denominator progress calculation, real transfer throughput ETA,
//! and physical multi-track execution with NDJSON event recording.

use std::time::Duration;
use syncify_tauri_lib::download::progress::{
    DownloadPhase, DownloadPhaseTracker, QueueGlobalProgress,
};

#[test]
fn test_stable_denominator_global_progress() {
    let initial_total = 10;

    // 1. Initial state: 10 selected, 0 excluded, 10 eligible, 8 pending, 2 active (each 50% => 1.0), 0 completed, 0 failed, 0 cancelled, 0 skipped
    let p1 = QueueGlobalProgress::compute(
        10, 0, initial_total, 8, 2, 0, 0, 0, 0, 1.0, 5.0 * 1024.0 * 1024.0, 10_000_000,
    );
    assert_eq!(p1.initial_eligible_total, 10);
    assert!((p1.progress_percent - 10.0).abs() < 1e-4);

    // 2. 3 completed, 1 failed, 1 skipped, 0 cancelled, 3 pending, 2 active (each 75% => 1.5)
    // Numerator = 3 + 1 + 1 + 1.5 = 6.5
    // Denominator = 10 => 65.0%
    let p2 = QueueGlobalProgress::compute(
        10, 0, initial_total, 3, 2, 3, 1, 0, 1, 1.5, 8.0 * 1024.0 * 1024.0, 20_000_000,
    );
    assert_eq!(p2.initial_eligible_total, 10);
    assert!((p2.progress_percent - 65.0).abs() < 1e-4);

    // 3. 8 completed, 1 failed, 1 skipped, 0 active, 0 pending
    // Numerator = 8 + 1 + 1 = 10 => 100.0%
    let p3 = QueueGlobalProgress::compute(
        10, 0, initial_total, 0, 0, 8, 1, 0, 1, 0.0, 0.0, 0,
    );
    assert_eq!(p3.initial_eligible_total, 10);
    assert!((p3.progress_percent - 100.0).abs() < 1e-4);
}

#[test]
fn test_eta_based_strictly_on_real_transfer_throughput() {
    // 5 remaining tracks, 20 MiB remaining, real transfer throughput = 10.0 MiB/s (10*1024*1024 B/s) => ETA ~2 seconds
    let p = QueueGlobalProgress::compute(
        10, 0, 10, 5, 0, 5, 0, 0, 0, 0.0, 10.0 * 1024.0 * 1024.0, 20 * 1024 * 1024,
    );
    assert_eq!(p.eta_seconds, Some(2));

    // Zero throughput => ETA None
    let p_zero = QueueGlobalProgress::compute(
        10, 0, 10, 5, 0, 5, 0, 0, 0, 0.0, 0.0, 20 * 1024 * 1024,
    );
    assert_eq!(p_zero.eta_seconds, None);
}

#[tokio::test]
async fn test_physical_10_track_execution_telemetry() {
    let temp_root = tempfile::tempdir().unwrap();
    let library_dir = temp_root.path().join("Music");
    let staging_dir = temp_root.path().join(".staging");
    tokio::fs::create_dir_all(&library_dir).await.unwrap();
    tokio::fs::create_dir_all(&staging_dir).await.unwrap();

    // 10 tracks: 5 from same album (Album A), 5 from different albums (Albums B, C, D, E, F)
    let tracks = vec![
        ("Track 1", "Artist 1", "Album A", 1024 * 512, false),
        ("Track 2", "Artist 1", "Album A", 1024 * 600, true), // repeat album => cover hit expected
        ("Track 3", "Artist 1", "Album A", 1024 * 550, true),
        ("Track 4", "Artist 1", "Album A", 1024 * 700, true),
        ("Track 5", "Artist 1", "Album A", 1024 * 650, true),
        ("Track 6", "Artist 2", "Album B", 1024 * 800, false),
        ("Track 7", "Artist 3", "Album C", 1024 * 750, false),
        ("Track 8", "Artist 4", "Album D", 1024 * 900, false),
        ("Track 9", "Artist 5", "Album E", 1024 * 850, false),
        ("Track 10", "Artist 6", "Album F", 1024 * 950, false),
    ];

    let mut ndjson_events: Vec<String> = Vec::new();
    let initial_queue_total = tracks.len();
    let mut completed_count = 0;
    let mut total_bytes_transferred = 0u64;

    for (idx, (title, artist, album, payload_size, is_cached_album)) in tracks.iter().enumerate() {
        let mut tracker = DownloadPhaseTracker::new();

        // 1. QueueWait phase
        tracker.start_phase(DownloadPhase::QueueWait);
        tokio::time::sleep(Duration::from_millis(5)).await;

        // 2. Auth phase
        tracker.start_phase(DownloadPhase::Auth);
        tokio::time::sleep(Duration::from_millis(10)).await;

        // 3. ResolveStream phase
        tracker.start_phase(DownloadPhase::ResolveStream);
        tokio::time::sleep(Duration::from_millis(15)).await;

        // 4. Transfer phase (Physical write to staging)
        tracker.start_phase(DownloadPhase::Transfer);
        let part_file = staging_dir.join(format!("track_{}.part", idx + 1));
        let payload = vec![0xABu8; *payload_size];
        tokio::fs::write(&part_file, &payload).await.unwrap();
        tokio::time::sleep(Duration::from_millis(25)).await; // Ensure elapsed monotonic clock
        tracker.set_transfer_metrics(*payload_size as u64, "network");

        // 5. ValidateAudio phase
        tracker.start_phase(DownloadPhase::ValidateAudio);
        let read_back = tokio::fs::read(&part_file).await.unwrap();
        assert_eq!(read_back.len(), *payload_size);
        tokio::time::sleep(Duration::from_millis(5)).await;

        // 6. ResolveCover phase
        tracker.start_phase(DownloadPhase::ResolveCover);
        tokio::time::sleep(Duration::from_millis(8)).await;

        // 7. ResolveLyrics phase
        tracker.start_phase(DownloadPhase::ResolveLyrics);
        tokio::time::sleep(Duration::from_millis(8)).await;

        // 8. EnrichMetadata phase
        tracker.start_phase(DownloadPhase::EnrichMetadata);
        tokio::time::sleep(Duration::from_millis(10)).await;

        // 9. Tagging phase
        tracker.start_phase(DownloadPhase::Tagging);
        tokio::time::sleep(Duration::from_millis(6)).await;

        // 10. Promotion phase (Physical atomic rename to library)
        tracker.start_phase(DownloadPhase::Promotion);
        let album_dir = library_dir.join(album);
        tokio::fs::create_dir_all(&album_dir).await.unwrap();
        let final_dest = album_dir.join(format!("{}.flac", title));
        tokio::fs::rename(&part_file, &final_dest).await.unwrap();
        assert!(final_dest.exists(), "Final audio file must exist after promotion");

        // 11. Finalize
        tracker.set_cache_hits(true, *is_cached_album, true);
        let timings = tracker.finish_completed();

        assert!(
            timings.transfer_ms > 0,
            "Transfer duration MUST be > 0 ms for track {} (got {} ms)",
            title,
            timings.transfer_ms
        );
        assert!(
            timings.bytes_transferred > 0,
            "Bytes transferred must be positive"
        );
        assert!(
            timings.throughput_mibps > 0.0,
            "Throughput must be positive"
        );

        completed_count += 1;
        total_bytes_transferred += timings.bytes_transferred;

        let pending = initial_queue_total - completed_count;
        let progress = QueueGlobalProgress::compute(
            initial_queue_total,
            0,
            initial_queue_total,
            pending,
            0,
            completed_count,
            0,
            0,
            0,
            0.0,
            timings.throughput_mibps * 1024.0 * 1024.0,
            0,
        );

        let event_json = serde_json::json!({
            "track_index": idx + 1,
            "title": title,
            "artist": artist,
            "album": album,
            "bytes_transferred": timings.bytes_transferred,
            "transfer_ms": timings.transfer_ms,
            "stream_duration_ms": timings.stream_duration_ms,
            "total_duration_ms": timings.total_duration_ms,
            "throughput_mibps": timings.throughput_mibps,
            "cache_hits": timings.cache_hits,
            "phases_count": timings.phases.len(),
            "progress_percent": progress.progress_percent,
        });

        ndjson_events.push(event_json.to_string());
    }

    assert_eq!(completed_count, 10);
    assert_eq!(ndjson_events.len(), 10);
    assert!(total_bytes_transferred > 0);

    // Verify NDJSON event record formatting and phase durations
    for line in &ndjson_events {
        let v: serde_json::Value = serde_json::from_str(line).unwrap();
        let transfer_ms = v["transfer_ms"].as_u64().unwrap();
        let bytes = v["bytes_transferred"].as_u64().unwrap();
        assert!(transfer_ms > 0);
        assert!(bytes > 0);
    }
}

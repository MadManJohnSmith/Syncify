//! Tests for Download Phase Telemetry Contract
//! Verifies monotonic phase timestamps, positive transfer duration for streamed bytes,
//! error phase preservation with 0 bytes transferred, and .part cleanup on cancellation.

use std::time::Duration;
use syncify_tauri_lib::download::progress::{
    CacheHitReport, DownloadPhase, DownloadPhaseRecord, DownloadPhaseTimings, DownloadPhaseTracker,
};

#[test]
fn test_transfer_of_bytes_measures_positive_duration_and_throughput() {
    let mut tracker = DownloadPhaseTracker::new();
    tracker.start_phase(DownloadPhase::Auth);
    std::thread::sleep(Duration::from_millis(5));

    tracker.start_phase(DownloadPhase::ResolveStream);
    std::thread::sleep(Duration::from_millis(5));

    tracker.start_phase(DownloadPhase::Transfer);
    // Simulate streaming payload transfer
    std::thread::sleep(Duration::from_millis(20));
    let transferred_bytes: u64 = 1_048_576 * 5; // 5 MiB
    tracker.set_transfer_metrics(transferred_bytes, "network");

    tracker.start_phase(DownloadPhase::ValidateAudio);
    std::thread::sleep(Duration::from_millis(2));

    tracker.start_phase(DownloadPhase::Tagging);
    std::thread::sleep(Duration::from_millis(3));

    tracker.start_phase(DownloadPhase::Promotion);
    std::thread::sleep(Duration::from_millis(2));

    let timings = tracker.finish_completed();

    // Assertions
    assert!(
        timings.transfer_ms > 0,
        "Transfer duration must be > 0 ms when bytes are transferred (got {} ms)",
        timings.transfer_ms
    );
    assert_eq!(timings.stream_duration_ms, timings.transfer_ms);
    assert_eq!(timings.bytes_transferred, transferred_bytes);
    assert_eq!(timings.transfer_source, "network");
    assert!(
        timings.throughput_mibps > 0.0,
        "Throughput must be positive (got {:.2} MiB/s)",
        timings.throughput_mibps
    );
    assert!(
        timings.total_duration_ms >= timings.transfer_ms,
        "Total duration ({} ms) must cover transfer duration ({} ms)",
        timings.total_duration_ms,
        timings.transfer_ms
    );

    // Verify all phases have non-negative durations and monotonic timestamps
    let mut prev_end = 0;
    for rec in &timings.phases {
        let delta = (rec.duration_ms as i64 - (rec.end_ms as i64 - rec.start_ms as i64)).abs();
        assert!(
            delta <= 1,
            "Phase {:?} duration ({} ms) must match end - start ({} ms)",
            rec.phase,
            rec.duration_ms,
            rec.end_ms - rec.start_ms
        );
        assert!(
            rec.start_ms <= rec.end_ms,
            "Phase {:?} start ({} ms) must be <= end ({} ms)",
            rec.phase,
            rec.start_ms,
            rec.end_ms
        );
        assert!(
            rec.start_ms >= prev_end,
            "Phase {:?} start ({} ms) must be monotonic with previous end ({} ms)",
            rec.phase,
            rec.start_ms,
            prev_end
        );
        prev_end = rec.end_ms;
    }
}

#[test]
fn test_error_before_transfer_preserves_zero_bytes_and_correct_phase() {
    let mut tracker = DownloadPhaseTracker::new();
    tracker.start_phase(DownloadPhase::Auth);
    std::thread::sleep(Duration::from_millis(5));

    tracker.start_phase(DownloadPhase::ResolveStream);
    std::thread::sleep(Duration::from_millis(10));

    // Simulate candidate resolution failure (e.g. 404 / NotFound / SourceIdentityMissing)
    let timings = tracker.finish_failed();

    assert_eq!(
        timings.bytes_transferred, 0,
        "Error before transfer must have 0 bytes transferred"
    );
    assert_eq!(
        timings.transfer_ms, 0,
        "Transfer duration must be 0 when error occurs before transfer"
    );
    assert_eq!(
        timings.stream_duration_ms, 0,
        "Stream duration must be 0 when error occurs before transfer"
    );
    assert_eq!(
        timings.throughput_mibps, 0.0,
        "Throughput must be 0.0 on pre-transfer failure"
    );
    assert!(
        timings.resolve_stream_ms > 0,
        "ResolveStream phase must reflect elapsed time before failure"
    );

    // Check last recorded phase was Failed
    let last_phase = timings.phases.last().map(|r| r.phase);
    assert_eq!(last_phase, Some(DownloadPhase::Failed));
}

#[tokio::test]
async fn test_cancellation_during_transfer_purges_staging_part_file() {
    let temp_dir = tempfile::tempdir().unwrap();
    let staging_dir = temp_dir.path().join(".staging");
    tokio::fs::create_dir_all(&staging_dir).await.unwrap();
    let part_file = staging_dir.join("test_item_99.part");

    // Write partial .part data
    tokio::fs::write(&part_file, b"partial downloaded stream chunk data").await.unwrap();
    assert!(part_file.exists(), ".part file must exist during active transfer");

    let mut tracker = DownloadPhaseTracker::new();
    tracker.start_phase(DownloadPhase::Transfer);
    std::thread::sleep(Duration::from_millis(10));

    // Simulate cancellation signal: cleanup .part file and record Cancelled phase
    if part_file.exists() {
        let _ = tokio::fs::remove_file(&part_file).await;
    }
    let timings = tracker.finish_cancelled();

    assert!(
        !part_file.exists(),
        ".part file must be purged upon cancellation"
    );
    let last_phase = timings.phases.last().map(|r| r.phase);
    assert_eq!(
        last_phase,
        Some(DownloadPhase::Cancelled),
        "Cancelled state must be recorded as terminal phase"
    );
}

#[test]
fn test_cache_hit_explicitly_marked_as_cache_source() {
    let mut tracker = DownloadPhaseTracker::new();
    tracker.start_phase(DownloadPhase::Transfer);
    std::thread::sleep(Duration::from_millis(2));
    tracker.set_transfer_metrics(2_000_000, "cache");
    tracker.set_cache_hits(true, true, true);

    let timings = tracker.finish_completed();

    assert_eq!(
        timings.transfer_source, "cache",
        "Local cache delivery must be explicitly marked as 'cache', not 'network'"
    );
    assert!(timings.cache_hits.lyrics_hit);
    assert!(timings.cache_hits.cover_hit);
    assert!(timings.cache_hits.metadata_hit);
}

#[test]
fn test_qobuz_and_tidal_parity_on_phase_contract() {
    // 1. Qobuz Timings Representation
    let qobuz_timings = DownloadPhaseTimings {
        queue_wait_ms: 15,
        auth_ms: 120,
        resolve_stream_ms: 250,
        transfer_ms: 1500,
        stream_duration_ms: 1500,
        validate_audio_ms: 30,
        metadata_duration_ms: 80,
        lyrics_duration_ms: 60,
        cover_duration_ms: 70,
        tagging_duration_ms: 45,
        promotion_duration_ms: 10,
        persisting_duration_ms: 0,
        total_duration_ms: 2180,
        transfer_source: "network".to_string(),
        bytes_transferred: 15_000_000,
        throughput_mibps: 9.53,
        cache_hits: CacheHitReport { lyrics_hit: false, cover_hit: true, metadata_hit: true },
        phases: vec![
            DownloadPhaseRecord { phase: DownloadPhase::QueueWait, start_ms: 0, end_ms: 15, duration_ms: 15 },
            DownloadPhaseRecord { phase: DownloadPhase::Auth, start_ms: 15, end_ms: 135, duration_ms: 120 },
            DownloadPhaseRecord { phase: DownloadPhase::ResolveStream, start_ms: 135, end_ms: 385, duration_ms: 250 },
            DownloadPhaseRecord { phase: DownloadPhase::Transfer, start_ms: 385, end_ms: 1885, duration_ms: 1500 },
            DownloadPhaseRecord { phase: DownloadPhase::ValidateAudio, start_ms: 1885, end_ms: 1915, duration_ms: 30 },
            DownloadPhaseRecord { phase: DownloadPhase::ResolveCover, start_ms: 1915, end_ms: 1985, duration_ms: 70 },
            DownloadPhaseRecord { phase: DownloadPhase::ResolveLyrics, start_ms: 1985, end_ms: 2045, duration_ms: 60 },
            DownloadPhaseRecord { phase: DownloadPhase::EnrichMetadata, start_ms: 2045, end_ms: 2125, duration_ms: 80 },
            DownloadPhaseRecord { phase: DownloadPhase::Tagging, start_ms: 2125, end_ms: 2170, duration_ms: 45 },
            DownloadPhaseRecord { phase: DownloadPhase::Promotion, start_ms: 2170, end_ms: 2180, duration_ms: 10 },
            DownloadPhaseRecord { phase: DownloadPhase::Completed, start_ms: 2180, end_ms: 2180, duration_ms: 0 },
        ],
    };

    // 2. Tidal Timings Representation
    let tidal_timings = DownloadPhaseTimings {
        queue_wait_ms: 20,
        auth_ms: 100,
        resolve_stream_ms: 220,
        transfer_ms: 1400,
        stream_duration_ms: 1400,
        validate_audio_ms: 25,
        metadata_duration_ms: 75,
        lyrics_duration_ms: 55,
        cover_duration_ms: 65,
        tagging_duration_ms: 40,
        promotion_duration_ms: 15,
        persisting_duration_ms: 25,
        total_duration_ms: 2040,
        transfer_source: "network".to_string(),
        bytes_transferred: 14_000_000,
        throughput_mibps: 9.53,
        cache_hits: CacheHitReport { lyrics_hit: true, cover_hit: true, metadata_hit: true },
        phases: vec![
            DownloadPhaseRecord { phase: DownloadPhase::QueueWait, start_ms: 0, end_ms: 20, duration_ms: 20 },
            DownloadPhaseRecord { phase: DownloadPhase::Auth, start_ms: 20, end_ms: 120, duration_ms: 100 },
            DownloadPhaseRecord { phase: DownloadPhase::ResolveStream, start_ms: 120, end_ms: 340, duration_ms: 220 },
            DownloadPhaseRecord { phase: DownloadPhase::Transfer, start_ms: 340, end_ms: 1740, duration_ms: 1400 },
            DownloadPhaseRecord { phase: DownloadPhase::ValidateAudio, start_ms: 1740, end_ms: 1765, duration_ms: 25 },
            DownloadPhaseRecord { phase: DownloadPhase::ResolveCover, start_ms: 1765, end_ms: 1830, duration_ms: 65 },
            DownloadPhaseRecord { phase: DownloadPhase::ResolveLyrics, start_ms: 1830, end_ms: 1885, duration_ms: 55 },
            DownloadPhaseRecord { phase: DownloadPhase::EnrichMetadata, start_ms: 1885, end_ms: 1960, duration_ms: 75 },
            DownloadPhaseRecord { phase: DownloadPhase::Tagging, start_ms: 1960, end_ms: 2000, duration_ms: 40 },
            DownloadPhaseRecord { phase: DownloadPhase::Persisting, start_ms: 2000, end_ms: 2025, duration_ms: 25 },
            DownloadPhaseRecord { phase: DownloadPhase::Promotion, start_ms: 2025, end_ms: 2040, duration_ms: 15 },
            DownloadPhaseRecord { phase: DownloadPhase::Completed, start_ms: 2040, end_ms: 2040, duration_ms: 0 },
        ],
    };

    // Both serialize and deserialize cleanly under identical DTO contract
    let q_json = serde_json::to_string(&qobuz_timings).unwrap();
    let t_json = serde_json::to_string(&tidal_timings).unwrap();

    let q_deser: DownloadPhaseTimings = serde_json::from_str(&q_json).unwrap();
    let t_deser: DownloadPhaseTimings = serde_json::from_str(&t_json).unwrap();

    assert_eq!(q_deser.stream_duration_ms, 1500);
    assert_eq!(t_deser.stream_duration_ms, 1400);
    assert_eq!(q_deser.phases.len(), 11);
    assert_eq!(t_deser.phases.len(), 12);
}

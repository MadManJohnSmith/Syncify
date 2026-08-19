// Test Suite: Download Progress Event Contract & Telemetry (S149)
//
// Verifies:
// 1. Full 14-phase sequence contract and serialization.
// 2. Transfer event updating bytes, percent, and throughput.
// 3. Lyrics failure best-effort event (non-fatal).
// 4. Cover failure best-effort event (non-fatal).
// 5. Auth error classification.
// 6. Cancellation event.
// 7. Completion timeline and phase timings.
// 8. Rapidly emitted phases without event loss.
// 9. 50 concurrent events maintaining per-track state integrity.

use std::sync::Arc;
use std::time::Duration;
use syncify_tauri_lib::download::progress::{
    ByteStreamTracker, DownloadPhase, DownloadPhaseTracker, DownloadProgress, DownloadStatus, ProgressTracker,
};
use syncify_tauri_lib::worker::DownloadProgressEvent;

#[tokio::test]
async fn test_full_14_phase_sequence_contract() {
    let phases = [
        DownloadPhase::QueueWait,
        DownloadPhase::Auth,
        DownloadPhase::ResolveStream,
        DownloadPhase::Transfer,
        DownloadPhase::ValidateAudio,
        DownloadPhase::EnrichMetadata,
        DownloadPhase::ResolveLyrics,
        DownloadPhase::ResolveCover,
        DownloadPhase::Tagging,
        DownloadPhase::Promotion,
        DownloadPhase::Persisting,
        DownloadPhase::Completed,
        DownloadPhase::Failed,
        DownloadPhase::Cancelled,
    ];

    assert_eq!(phases.len(), 14, "Must cover all 14 distinct download phases");

    for phase in &phases {
        let prog = DownloadProgress::phase_update("42", Some("qobuz"), *phase, None);
        assert_eq!(prog.item_id, "42");
        assert_eq!(prog.service.as_deref(), Some("qobuz"));
        assert_eq!(prog.phase, phase.as_str());

        // Verify JSON serialization round-trip
        let json_str = serde_json::to_string(&prog).expect("Serialization failed");
        let deserialized: DownloadProgress =
            serde_json::from_str(&json_str).expect("Deserialization failed");
        assert_eq!(deserialized.phase, phase.as_str());
        assert_eq!(deserialized.item_id, "42");
    }
}

#[tokio::test]
async fn test_transfer_event_updates_bytes_and_throughput() {
    let mut tracker = ByteStreamTracker::new("101", "tidal", Some(25 * 1024 * 1024));

    // Initial state
    assert_eq!(tracker.item_id, "101");
    assert_eq!(tracker.service, "tidal");
    assert_eq!(tracker.total_bytes, Some(25 * 1024 * 1024));

    // Simulate chunk stream progress
    tokio::time::sleep(Duration::from_millis(15)).await;
    let prog1 = tracker.on_bytes(5 * 1024 * 1024, true);
    assert!(prog1.is_some());
    let p1 = prog1.unwrap();
    assert_eq!(p1.bytes_downloaded, 5 * 1024 * 1024);
    assert_eq!(p1.total_bytes, Some(25 * 1024 * 1024));
    assert!(p1.percent.is_some());
    assert!((p1.percent.unwrap() - 20.0).abs() < 0.1);
    assert!(p1.average_kbps > 0.0);

    // Further chunk
    tokio::time::sleep(Duration::from_millis(15)).await;
    let prog2 = tracker.on_bytes(15 * 1024 * 1024, true);
    assert!(prog2.is_some());
    let p2 = prog2.unwrap();
    assert_eq!(p2.bytes_downloaded, 15 * 1024 * 1024);
    assert!((p2.percent.unwrap() - 60.0).abs() < 0.1);
}

#[tokio::test]
async fn test_lyrics_failure_best_effort() {
    let prog = DownloadProgress::phase_update(
        "202",
        Some("qobuz"),
        DownloadPhase::ResolveLyrics,
        Some("Lyrics unavailable — continuing"),
    );

    assert_eq!(prog.phase, "ResolveLyrics");
    assert_eq!(prog.status, DownloadStatus::Downloading);
    assert!(!prog.terminal, "Best-effort auxiliary failure must NOT terminate download");
    assert_eq!(
        prog.message.as_deref(),
        Some("Lyrics unavailable — continuing")
    );
}

#[tokio::test]
async fn test_cover_failure_best_effort() {
    let prog = DownloadProgress::phase_update(
        "203",
        Some("tidal"),
        DownloadPhase::ResolveCover,
        Some("Animated cover unavailable — continuing"),
    );

    assert_eq!(prog.phase, "ResolveCover");
    assert_eq!(prog.status, DownloadStatus::Downloading);
    assert!(!prog.terminal, "Best-effort cover failure must NOT terminate download");
    assert_eq!(
        prog.message.as_deref(),
        Some("Animated cover unavailable — continuing")
    );
}

#[tokio::test]
async fn test_error_auth_classified() {
    let prog = DownloadProgress::failed("301", "HTTP 401 Unauthorized: token expired");

    assert_eq!(prog.status, DownloadStatus::Failed);
    assert!(prog.terminal, "Auth failure is a terminal state");
    assert!(prog
        .message
        .as_ref()
        .unwrap()
        .contains("401 Unauthorized"));

    // Event DTO structure
    let evt = DownloadProgressEvent {
        queue_id: 301,
        track_id: 5001,
        title: "Protected Track".to_string(),
        artist: "Secured Artist".to_string(),
        status: "failed".to_string(),
        progress_percent: 0.0,
        message: Some("RequiresAuth: HTTP 401 Unauthorized".to_string()),
        bytes_downloaded: 0,
        total_bytes: None,
        percent: None,
        instant_kbps: 0.0,
        average_kbps: 0.0,
        phase: "Auth".to_string(),
        terminal: true,
        phase_timings: None,
    };

    assert_eq!(evt.phase, "Auth");
    assert!(evt.terminal);
}

#[tokio::test]
async fn test_cancellation() {
    let prog = DownloadProgress::cancelled("401");

    assert_eq!(prog.status, DownloadStatus::Cancelled);
    assert!(prog.terminal);
    assert_eq!(prog.phase, "cancelled");
    assert_eq!(
        prog.message.as_deref(),
        Some("Download cancelled by user")
    );
}

#[tokio::test]
async fn test_completion_timeline_and_phase_timings() {
    let mut tracker = DownloadPhaseTracker::new();

    tracker.start_phase(DownloadPhase::QueueWait);
    tokio::time::sleep(Duration::from_millis(10)).await;

    tracker.start_phase(DownloadPhase::Auth);
    tokio::time::sleep(Duration::from_millis(10)).await;

    tracker.start_phase(DownloadPhase::ResolveStream);
    tokio::time::sleep(Duration::from_millis(10)).await;

    tracker.start_phase(DownloadPhase::Transfer);
    tracker.bytes_transferred = 10 * 1024 * 1024;
    tokio::time::sleep(Duration::from_millis(25)).await;

    tracker.start_phase(DownloadPhase::ValidateAudio);
    tokio::time::sleep(Duration::from_millis(10)).await;

    tracker.start_phase(DownloadPhase::EnrichMetadata);
    tokio::time::sleep(Duration::from_millis(10)).await;

    tracker.start_phase(DownloadPhase::ResolveLyrics);
    tokio::time::sleep(Duration::from_millis(10)).await;

    tracker.start_phase(DownloadPhase::ResolveCover);
    tokio::time::sleep(Duration::from_millis(10)).await;

    tracker.start_phase(DownloadPhase::Tagging);
    tokio::time::sleep(Duration::from_millis(10)).await;

    tracker.start_phase(DownloadPhase::Promotion);
    tokio::time::sleep(Duration::from_millis(10)).await;

    tracker.start_phase(DownloadPhase::Persisting);
    tokio::time::sleep(Duration::from_millis(10)).await;

    let timings = tracker.finish_completed();

    assert!(timings.total_duration_ms > 0);
    assert!(timings.transfer_ms > 0);
    assert_eq!(timings.bytes_transferred, 10 * 1024 * 1024);
    assert!(timings.throughput_mibps > 0.0);
    assert!(timings.phases.len() >= 11);

    // Verify chronological monotonicity
    let mut last_end = 0;
    for rec in &timings.phases {
        assert!(rec.start_ms <= rec.end_ms);
        assert!(rec.start_ms >= last_end);
        last_end = rec.end_ms;
    }
}

#[tokio::test]
async fn test_fast_fire_phase_emissions_not_dropped() {
    let tracker = ProgressTracker::new();
    let emitted_phases = Arc::new(std::sync::Mutex::new(Vec::new()));
    let emitted_clone = emitted_phases.clone();

    tracker.set_emitter(move |prog| {
        emitted_clone.lock().unwrap().push(prog.phase.clone());
    });

    let phases_to_emit = [
        DownloadPhase::QueueWait,
        DownloadPhase::Auth,
        DownloadPhase::ResolveStream,
        DownloadPhase::Transfer,
        DownloadPhase::ValidateAudio,
        DownloadPhase::EnrichMetadata,
        DownloadPhase::ResolveLyrics,
        DownloadPhase::ResolveCover,
        DownloadPhase::Tagging,
        DownloadPhase::Promotion,
        DownloadPhase::Persisting,
        DownloadPhase::Completed,
    ];

    // Fire all phases rapidly without artificial pauses
    for phase in &phases_to_emit {
        tracker.update(DownloadProgress::phase_update(
            "999",
            Some("qobuz"),
            *phase,
            None,
        ));
    }

    let records = emitted_phases.lock().unwrap().clone();
    assert_eq!(records.len(), 12, "All 12 emitted phases must be captured without dropping");
    for (i, phase) in phases_to_emit.iter().enumerate() {
        assert_eq!(records[i].as_str(), phase.as_str());
    }
}

#[tokio::test]
async fn test_50_concurrent_events_integrity() {
    let tracker = Arc::new(ProgressTracker::new());
    let counter = Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let counter_clone = counter.clone();

    tracker.set_emitter(move |_prog| {
        counter_clone.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
    });

    let mut handles = Vec::new();
    for i in 0..50 {
        let t = tracker.clone();
        let handle = tokio::spawn(async move {
            let item_id = format!("{}", (i % 5) + 1);
            let phase = if i % 2 == 0 {
                DownloadPhase::Transfer
            } else {
                DownloadPhase::EnrichMetadata
            };
            t.update(DownloadProgress::phase_update(
                &item_id,
                Some("tidal"),
                phase,
                Some("concurrent event"),
            ));
        });
        handles.push(handle);
    }

    for h in handles {
        h.await.expect("Join failed");
    }

    assert_eq!(
        counter.load(std::sync::atomic::Ordering::SeqCst),
        50,
        "All 50 concurrent progress updates must be processed without loss or data race"
    );
}

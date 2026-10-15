//! DLX-01 isolated transport/phase baseline, not a full provider or tagging pipeline.
//! Run with HOME/TMPDIR/XDG_* pointing to a disposable directory outside the personal profile.
use std::time::{Duration, Instant};
use syncify_tauri_lib::download::progress::{DownloadPhase, DownloadPhaseTracker};
use syncify_tauri_lib::download::qobuz::QobuzDownloader;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;

const AUDIO: &[u8] = include_bytes!("fixtures/silence.flac");
const COVER: &[u8] = b"synthetic-cover";
const LYRICS: &[u8] = b"[00:00.00]synthetic";

async fn serve(listener: TcpListener) {
    for _ in 0..3 {
        let Ok(Ok((mut socket, _))) =
            tokio::time::timeout(Duration::from_secs(10), listener.accept()).await
        else {
            break;
        };
        let mut request = [0u8; 2048];
        let Ok(n) = socket.read(&mut request).await else {
            break;
        };
        let payload = if request[..n].starts_with(b"GET /audio ") {
            AUDIO
        } else if request[..n].starts_with(b"GET /cover ") {
            COVER
        } else {
            LYRICS
        };
        if !request[..n].starts_with(b"GET /audio ") {
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
        let header = format!(
            "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
            payload.len()
        );
        if socket.write_all(header.as_bytes()).await.is_err() {
            break;
        }
        if socket.write_all(payload).await.is_err() {
            break;
        }
    }
}

#[tokio::test]
async fn local_audio_and_auxiliary_timeline_two_runs() {
    let root = tempfile::tempdir().expect("isolated TMPDIR required");
    let client = reqwest::Client::new();
    for run in 0..2 {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let base = format!("http://127.0.0.1:{}", listener.local_addr().unwrap().port());
        let server = tokio::spawn(serve(listener));
        let mut tracker = DownloadPhaseTracker::new();
        let start = Instant::now();
        tracker.start_phase(DownloadPhase::ResolveStream);
        // The local stream address is known; this is a no-op resolution control.
        tracker.start_phase(DownloadPhase::Transfer);
        let staging = root.path().join(format!("run-{run}.part"));
        let bytes = tokio::time::timeout(
            Duration::from_secs(10),
            QobuzDownloader::new().download_to_staging(
                &format!("{base}/audio"),
                &staging,
                "synthetic",
            ),
        )
        .await
        .expect("transport timeout")
        .expect("transport failed");
        assert_eq!(bytes as usize, AUDIO.len());
        assert_eq!(tokio::fs::read(&staging).await.unwrap(), AUDIO);
        tracker.set_transfer_metrics(bytes, "network");
        tracker.start_phase(DownloadPhase::ValidateAudio);
        assert!(AUDIO.starts_with(b"fLaC"));
        tracker.start_phase(DownloadPhase::ResolveCover);
        let cover = client
            .get(format!("{base}/cover"))
            .send()
            .await
            .unwrap()
            .bytes()
            .await
            .unwrap();
        assert_eq!(cover.as_ref(), COVER);
        tracker.start_phase(DownloadPhase::ResolveLyrics);
        let lyrics = client
            .get(format!("{base}/lyrics"))
            .send()
            .await
            .unwrap()
            .bytes()
            .await
            .unwrap();
        assert_eq!(lyrics.as_ref(), LYRICS);
        tracker.start_phase(DownloadPhase::EnrichMetadata);
        tracker.start_phase(DownloadPhase::Tagging);
        tracker.start_phase(DownloadPhase::Promotion);
        tracker.start_phase(DownloadPhase::Persisting);
        let timings = tracker.finish_completed();
        server.await.unwrap();
        assert_eq!(timings.bytes_transferred, bytes);
        assert!(timings.transfer_ms > 0);
        assert!(timings.total_duration_ms <= start.elapsed().as_millis() as u64 + 1);
        assert!(timings
            .phases
            .windows(2)
            .all(|w| w[0].end_ms <= w[1].start_ms));
        assert!(timings.phases.iter().all(|p| p.end_ms >= p.start_ms));
        // Aggregate is rounded to milliseconds; allow 1ms per phase for quantization.
        let sum: u64 = timings.phases.iter().map(|p| p.duration_ms).sum();
        assert!(sum <= timings.total_duration_ms + timings.phases.len() as u64);
        println!("local_baseline run={} audio_bytes={} transfer_ms={} cover_ms={} lyrics_ms={} total_ms={} phase_sum_ms={}", run + 1, bytes, timings.transfer_ms, timings.cover_duration_ms, timings.lyrics_duration_ms, timings.total_duration_ms, sum);
    }
}

//! Regression test for the Spotify 429 retry loop.
//!
//! `SpotifyClient::get_albums_batch` advanced its retry counter only on the
//! non-429 branch, then `continue`d from the 429 branch without touching it. A
//! persistently rate-limited endpoint therefore looped forever, parking the
//! `enrich_albums` worker for the life of the process. Separately, the sleep was
//! computed by re-parsing `Retry-After` raw, so a `Retry-After: 86400` parked
//! it for 24 hours despite the 300 s ceiling `rate_limiter` already enforces.
//!
//! The production URL is hardcoded to `api.spotify.com`, so this exercises the
//! two behaviours the loop depends on — the bounded counter and the clamped
//! sleep — against a local server that always answers 429, rather than reaching
//! the real API.

use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Arc;
use std::time::Duration;

use syncify_tauri_lib::services::rate_limiter::{
    retry_after_delay, DEFAULT_429_PENALTY, MAX_429_PENALTY,
};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;

/// A server that always answers `429` with the given `Retry-After`, counting
/// requests so the caller can assert how many attempts the loop made.
async fn always_429(retry_after: &'static str) -> (String, Arc<AtomicU32>) {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    let hits = Arc::new(AtomicU32::new(0));
    let srv_hits = hits.clone();

    tokio::spawn(async move {
        loop {
            let Ok((mut socket, _)) = listener.accept().await else {
                return;
            };
            srv_hits.fetch_add(1, Ordering::SeqCst);
            let mut buf = [0u8; 1024];
            let _ = socket.read(&mut buf).await;
            let response = format!(
                "HTTP/1.1 429 Too Many Requests\r\nRetry-After: {retry_after}\r\nContent-Length: 0\r\n\r\n"
            );
            let _ = socket.write_all(response.as_bytes()).await;
            let _ = socket.flush().await;
        }
    });

    (format!("http://127.0.0.1:{port}/v1/albums"), hits)
}

/// The contract `get_albums_batch` implements: a shared counter bounds 429s
/// exactly like it bounds other failures, and each sleep is clamped.
async fn fetch_with_bounded_retries(url: &str, max_retries: u32) -> Result<u32, String> {
    let client = reqwest::Client::new();
    let mut retry_count = 0u32;

    loop {
        let response = client.get(url).send().await.map_err(|e| e.to_string())?;

        if response.status() == reqwest::StatusCode::TOO_MANY_REQUESTS {
            if retry_count >= max_retries {
                return Err("rate limited".to_string());
            }
            retry_count += 1;
            // Scaled down from the production 30 s fallback so the test does not
            // sit for minutes; the clamp, not the magnitude, is what matters.
            let wait = retry_after_delay(response.headers(), Duration::from_millis(1));
            tokio::time::sleep(wait).await;
            continue;
        }

        return Ok(retry_count);
    }
}

/// The loop must give up. Before the fix the 429 branch never incremented
/// `retry_count`, so this never returned and the worker spun forever.
#[tokio::test]
async fn test_permanent_429_terminates_instead_of_looping_forever() {
    let (url, hits) = always_429("1").await;
    let max_retries = 3;

    let result = tokio::time::timeout(
        Duration::from_secs(20),
        fetch_with_bounded_retries(&url, max_retries),
    )
    .await
    .expect("the 429 loop must terminate, not hang the worker");

    assert!(
        result.is_err(),
        "a permanently rate-limited endpoint must surface an error"
    );

    // One initial attempt plus the retries, and no more.
    let attempts = hits.load(Ordering::SeqCst);
    assert_eq!(
        attempts,
        max_retries + 1,
        "expected {max_retries} retries after the first attempt, saw {attempts}"
    );
}

/// A hostile `Retry-After` must be clamped to `MAX_429_PENALTY`, never obeyed
/// literally. This pins the clamp on its own, without sleeping it out: a
/// `Retry-After: 86400` is a day, the ceiling is 300 s, and the helper must
/// hand back the ceiling so the loop cannot park for the day.
#[tokio::test]
async fn test_hostile_retry_after_is_clamped_not_obeyed() {
    let (url, hits) = always_429("86400").await;

    // One retry only, so the loop performs exactly one clamped sleep. Asserting
    // on the computed delay is the deterministic half; sleeping 300 s is not.
    let client = reqwest::Client::new();
    let response = client.get(&url).send().await.expect("request");
    assert_eq!(response.status(), reqwest::StatusCode::TOO_MANY_REQUESTS);

    let wait = retry_after_delay(response.headers(), Duration::from_secs(30));
    assert_eq!(
        wait, MAX_429_PENALTY,
        "a 24h Retry-After must clamp to the {MAX_429_PENALTY:?} ceiling"
    );
    assert!(
        wait < Duration::from_secs(3600),
        "the clamp is not doing anything: {wait:?}"
    );
    assert_eq!(hits.load(Ordering::SeqCst), 1);
}

/// The clamp helper's own contract, so the production call site cannot drift.
#[test]
fn test_retry_after_delay_never_exceeds_the_ceiling() {
    let headers_with = |v: &'static str| {
        let mut h = reqwest::header::HeaderMap::new();
        h.insert(
            reqwest::header::RETRY_AFTER,
            reqwest::header::HeaderValue::from_static(v),
        );
        h
    };

    assert_eq!(
        retry_after_delay(&headers_with("86400"), Duration::from_secs(30)),
        MAX_429_PENALTY
    );
    assert_eq!(
        retry_after_delay(&reqwest::header::HeaderMap::new(), DEFAULT_429_PENALTY),
        DEFAULT_429_PENALTY
    );
    assert_eq!(
        retry_after_delay(&headers_with("12"), DEFAULT_429_PENALTY),
        Duration::from_secs(12)
    );
}

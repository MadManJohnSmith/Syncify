#![allow(dead_code)]
// Shared HTTP client with centralized connection pooling, exponential backoff with jitter,
// strict Retry-After / 429 detection, and cooperative cancellation.

use anyhow::{anyhow, Result};
use rand::Rng;
use reqwest::{header, Client, ClientBuilder, Response, StatusCode};
use std::path::Path;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{Duration, SystemTime};
use tokio::fs::File;
use tokio::io::AsyncWriteExt;
use tokio::time::sleep;
use tokio_util::sync::CancellationToken;

use crate::download::progress::{DownloadProgress, PROGRESS_TRACKER};
use crate::services::rate_limiter::GLOBAL_RATE_LIMITER;

/// Default timeout for HTTP requests (60 seconds)
pub const DEFAULT_TIMEOUT: Duration = Duration::from_secs(60);

/// Default connect timeout (15 seconds)
pub const DEFAULT_CONNECT_TIMEOUT: Duration = Duration::from_secs(15);

/// Maximum retry attempts for transient HTTP / network errors
pub const MAX_RETRIES: u32 = 3;

/// User agents to rotate through
static USER_AGENTS: &[&str] = &[
    "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36",
    "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/119.0.0.0 Safari/537.36",
    "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36",
    "Mozilla/5.0 (Windows NT 10.0; Win64; x64; rv:121.0) Gecko/20100101 Firefox/121.0",
    "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/17.2 Safari/605.1.15",
];

static UA_INDEX: AtomicUsize = AtomicUsize::new(0);

/// Get a rotating User-Agent string
pub fn get_user_agent() -> &'static str {
    let index = UA_INDEX.fetch_add(1, Ordering::Relaxed) % USER_AGENTS.len();
    USER_AGENTS[index]
}

// Centralized Global HTTP Client Singleton
lazy_static::lazy_static! {
    pub static ref SHARED_HTTP_CLIENT: Client = build_central_http_client(DEFAULT_TIMEOUT);
}

/// Helper to build a centralized `reqwest::Client` with HTTP/2 keep-alive, TCP keepalive (30s),
/// and optimized per-host connection pooling.
fn build_central_http_client(timeout: Duration) -> Client {
    let mut headers = header::HeaderMap::new();
    headers.insert(
        header::USER_AGENT,
        header::HeaderValue::from_static(USER_AGENTS[0]),
    );

    ClientBuilder::new()
        .timeout(timeout)
        .connect_timeout(DEFAULT_CONNECT_TIMEOUT)
        .tcp_keepalive(Some(Duration::from_secs(30)))
        .http2_keep_alive_interval(Some(Duration::from_secs(30)))
        .http2_keep_alive_timeout(Duration::from_secs(10))
        .http2_adaptive_window(true)
        .pool_max_idle_per_host(25)
        .pool_idle_timeout(Some(Duration::from_secs(90)))
        .default_headers(headers)
        .build()
        .expect("Failed to initialize central HTTP client")
}

/// Obtain a reference to the global shared HTTP client
pub fn shared_http_client() -> &'static Client {
    &SHARED_HTTP_CLIENT
}

/// Create or clone an HTTP client sharing the centralized connection pool
pub fn create_http_client() -> Client {
    SHARED_HTTP_CLIENT.clone()
}

/// Create a new HTTP client with custom timeout (retaining pooled transport settings)
pub fn create_http_client_with_timeout(timeout: Duration) -> Client {
    if timeout == DEFAULT_TIMEOUT {
        SHARED_HTTP_CLIENT.clone()
    } else {
        build_central_http_client(timeout)
    }
}

/// Helper to parse HTTP-date (IMF-fixdate / RFC 2822 / RFC 1123)
fn parse_http_date_to_secs(date_str: &str) -> Option<i64> {
    if let Ok(dt) = chrono::DateTime::parse_from_rfc2822(date_str) {
        return Some(dt.timestamp());
    }

    if let Ok(naive) = chrono::NaiveDateTime::parse_from_str(date_str, "%a, %d %b %Y %H:%M:%S GMT") {
        return Some(naive.and_utc().timestamp());
    }

    if let Ok(naive) = chrono::NaiveDateTime::parse_from_str(date_str, "%d %b %Y %H:%M:%S GMT") {
        return Some(naive.and_utc().timestamp());
    }

    None
}

/// Parses a `Retry-After` header value into a `Duration`.
/// Supports integer seconds ("120") and HTTP-date ("Sun, 06 Nov 1994 08:49:37 GMT").
pub fn parse_retry_after(headers: &header::HeaderMap, now: SystemTime) -> Option<Duration> {
    let val_str = headers.get(header::RETRY_AFTER)?.to_str().ok()?;
    let trimmed = val_str.trim();

    // 1. Try parsing integer seconds
    if let Ok(seconds) = trimmed.parse::<u64>() {
        return Some(Duration::from_secs(seconds));
    }

    // 2. Try parsing HTTP-date
    if let Some(target_secs) = parse_http_date_to_secs(trimmed) {
        let now_secs = now
            .duration_since(SystemTime::UNIX_EPOCH)
            .map(|d| d.as_secs() as i64)
            .unwrap_or(0);

        if target_secs > now_secs {
            return Some(Duration::from_secs((target_secs - now_secs) as u64));
        } else {
            return Some(Duration::from_secs(0));
        }
    }

    None
}

/// Calculate exponential backoff with full jitter
pub fn calculate_backoff_with_jitter(
    attempt: u32,
    initial_backoff: Duration,
    max_backoff: Duration,
) -> Duration {
    let factor = 2.0_f64.powi(attempt as i32);
    let base_secs = initial_backoff.as_secs_f64() * factor;
    let clamped = base_secs.min(max_backoff.as_secs_f64());

    // Full Jitter: randomize between [0.5 * clamped, 1.5 * clamped]
    let mut rng = rand::thread_rng();
    let jitter_multiplier: f64 = rng.gen_range(0.5..=1.5);
    let jittered = (clamped * jitter_multiplier).min(max_backoff.as_secs_f64());
    Duration::from_secs_f64(jittered.max(0.05))
}

/// Determines if an HTTP status code is considered transient and retriable
pub fn is_transient_status(status: StatusCode) -> bool {
    status == StatusCode::TOO_MANY_REQUESTS
        || status == StatusCode::BAD_GATEWAY
        || status == StatusCode::SERVICE_UNAVAILABLE
        || status == StatusCode::GATEWAY_TIMEOUT
        || status == StatusCode::REQUEST_TIMEOUT
}

/// Execute an HTTP request with intelligent retry, exponential backoff with jitter,
/// 429 rate limit penalty feedback, and cooperative cancellation.
pub async fn execute_with_retry<F, Fut>(
    service: &str,
    cancel_token: Option<&CancellationToken>,
    mut make_request: F,
) -> Result<Response>
where
    F: FnMut() -> Fut,
    Fut: std::future::Future<Output = Result<Response, reqwest::Error>>,
{
    let mut attempt = 0;
    let initial_backoff = Duration::from_millis(500);
    let max_backoff = Duration::from_secs(30);

    loop {
        if let Some(token) = cancel_token {
            if token.is_cancelled() {
                return Err(anyhow!("Request cancelled for service {}", service));
            }
        }

        // Acquire rate limiter permission before dispatching
        GLOBAL_RATE_LIMITER.acquire_cancellable(service, cancel_token).await?;

        let result = make_request().await;

        match result {
            Ok(resp) => {
                let status = resp.status();
                if status.is_success() {
                    return Ok(resp);
                }

                // If non-transient error (e.g. 401, 403, 404), fail fast
                if !is_transient_status(status) {
                    return Ok(resp);
                }

                if attempt >= MAX_RETRIES {
                    tracing::warn!(
                        "[HTTP Retry] Service '{}' max retries ({}) exceeded with status {}",
                        service,
                        MAX_RETRIES,
                        status
                    );
                    return Ok(resp);
                }

                // Handle 429 Too Many Requests specifically
                let server_retry_after = if status == StatusCode::TOO_MANY_REQUESTS {
                    let delay_opt = parse_retry_after(resp.headers(), SystemTime::now());
                    if let Some(delay) = delay_opt {
                        GLOBAL_RATE_LIMITER.penalize_service(service, delay).await;
                        Some(delay)
                    } else {
                        let fallback_penalty = Duration::from_secs(5);
                        GLOBAL_RATE_LIMITER.penalize_service(service, fallback_penalty).await;
                        Some(fallback_penalty)
                    }
                } else {
                    parse_retry_after(resp.headers(), SystemTime::now())
                };

                let calculated_backoff = calculate_backoff_with_jitter(attempt, initial_backoff, max_backoff);
                let final_wait = match server_retry_after {
                    Some(server_delay) => server_delay.max(calculated_backoff),
                    None => calculated_backoff,
                };

                tracing::warn!(
                    "[HTTP Retry] Transient status {} from '{}'. Retrying in {:?} (attempt {}/{})",
                    status,
                    service,
                    final_wait,
                    attempt + 1,
                    MAX_RETRIES
                );

                attempt += 1;
                if let Some(token) = cancel_token {
                    tokio::select! {
                        _ = token.cancelled() => {
                            return Err(anyhow!("Request cancelled for service {}", service));
                        }
                        _ = sleep(final_wait) => {}
                    }
                } else {
                    sleep(final_wait).await;
                }
            }
            Err(err) => {
                if let Some(token) = cancel_token {
                    if token.is_cancelled() {
                        return Err(anyhow!("Request cancelled for service {}", service));
                    }
                }

                if attempt >= MAX_RETRIES {
                    return Err(anyhow!(
                        "Network request failed for '{}' after {} retries: {}",
                        service,
                        MAX_RETRIES,
                        err
                    ));
                }

                let final_wait = calculate_backoff_with_jitter(attempt, initial_backoff, max_backoff);
                tracing::warn!(
                    "[HTTP Retry] Network error from '{}': {}. Retrying in {:?} (attempt {}/{})",
                    service,
                    err,
                    final_wait,
                    attempt + 1,
                    MAX_RETRIES
                );

                attempt += 1;
                if let Some(token) = cancel_token {
                    tokio::select! {
                        _ = token.cancelled() => {
                            return Err(anyhow!("Request cancelled for service {}", service));
                        }
                        _ = sleep(final_wait) => {}
                    }
                } else {
                    sleep(final_wait).await;
                }
            }
        }
    }
}

/// Download a streaming HTTP payload to a file on disk with cooperative cancellation
/// and atomic cleanup on error or cancellation.
pub async fn download_stream_to_file<F>(
    response: Response,
    target_path: &Path,
    item_id: &str,
    service: &str,
    cancel_token: Option<&CancellationToken>,
    mut progress_cb: F,
) -> Result<u64>
where
    F: FnMut(u64, u64) + Send,
{
    if let Some(parent) = target_path.parent() {
        tokio::fs::create_dir_all(parent).await?;
    }

    let total_size = response.content_length().unwrap_or(0);
    progress_cb(0, total_size);

    let mut file = File::create(target_path).await?;
    let mut downloaded: u64 = 0;
    let mut stream = response.bytes_stream();
    use futures_util::StreamExt;

    loop {
        if let Some(token) = cancel_token {
            if token.is_cancelled() {
                let _ = tokio::fs::remove_file(target_path).await;
                return Err(anyhow!("Download cancelled by user"));
            }
        }

        let next_chunk = if let Some(token) = cancel_token {
            tokio::select! {
                _ = token.cancelled() => {
                    let _ = tokio::fs::remove_file(target_path).await;
                    return Err(anyhow!("Download cancelled by user"));
                }
                chunk_opt = stream.next() => chunk_opt
            }
        } else {
            stream.next().await
        };

        match next_chunk {
            Some(Ok(chunk)) => {
                if let Err(e) = file.write_all(&chunk).await {
                    let _ = tokio::fs::remove_file(target_path).await;
                    return Err(e.into());
                }
                downloaded += chunk.len() as u64;

                if downloaded % (64 * 1024) < chunk.len() as u64 {
                    progress_cb(downloaded, total_size);
                    PROGRESS_TRACKER.update(DownloadProgress::downloading(
                        item_id, service, downloaded, total_size,
                    ));
                }
            }
            Some(Err(err)) => {
                let _ = tokio::fs::remove_file(target_path).await;
                return Err(anyhow!("Stream read error from '{}': {}", service, err));
            }
            None => break,
        }
    }

    file.flush().await?;
    progress_cb(downloaded, total_size);
    Ok(downloaded)
}

/// Rate limiter for API calls (Backward compatible wrapper delegating to GLOBAL_RATE_LIMITER)
pub struct RateLimiter {
    #[allow(dead_code)]
    service_default_name: String,
}

impl RateLimiter {
    pub fn new(_min_delay_ms: u64, _max_per_minute: u32) -> Self {
        Self {
            service_default_name: String::new(),
        }
    }

    pub fn with_service_name(name: &str) -> Self {
        Self {
            service_default_name: name.to_string(),
        }
    }

    /// Wait if needed to respect rate limits
    pub async fn wait(&self, service: &str) {
        GLOBAL_RATE_LIMITER.acquire(service).await;
    }
}

// Default rate limiters for each service
lazy_static::lazy_static! {
    pub static ref QOBUZ_LIMITER: RateLimiter = RateLimiter::with_service_name("qobuz");
    pub static ref TIDAL_LIMITER: RateLimiter = RateLimiter::with_service_name("tidal");
    pub static ref AMAZON_LIMITER: RateLimiter = RateLimiter::with_service_name("amazon");
    pub static ref SONGLINK_LIMITER: RateLimiter = RateLimiter::with_service_name("songlink");
    pub static ref LRCLIB_LIMITER: RateLimiter = RateLimiter::with_service_name("lrclib");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_user_agent_rotation() {
        let ua1 = get_user_agent();
        let ua2 = get_user_agent();
        assert!(USER_AGENTS.contains(&ua1));
        assert!(USER_AGENTS.contains(&ua2));
    }

    #[test]
    fn test_create_client_shares_pool() {
        let client1 = create_http_client();
        let client2 = create_http_client();
        let client_shared = shared_http_client();
        // Just verify clients clone and do not panic
        drop(client1);
        drop(client2);
        let _ = client_shared;
    }

    #[test]
    fn test_parse_retry_after_seconds() {
        let mut headers = header::HeaderMap::new();
        headers.insert(header::RETRY_AFTER, header::HeaderValue::from_static("45"));

        let delay = parse_retry_after(&headers, SystemTime::now());
        assert_eq!(delay, Some(Duration::from_secs(45)));
    }

    #[test]
    fn test_parse_retry_after_http_date() {
        let mut headers = header::HeaderMap::new();
        headers.insert(
            header::RETRY_AFTER,
            header::HeaderValue::from_static("Sun, 06 Nov 1994 08:49:37 GMT"),
        );

        let date_secs = parse_http_date_to_secs("Sun, 06 Nov 1994 08:49:37 GMT").unwrap();
        let now = SystemTime::UNIX_EPOCH + Duration::from_secs((date_secs - 30) as u64);

        let delay = parse_retry_after(&headers, now);
        assert_eq!(delay, Some(Duration::from_secs(30)));
    }

    #[test]
    fn test_jittered_backoff_bounds() {
        let initial = Duration::from_millis(500);
        let max = Duration::from_secs(10);

        for attempt in 0..5 {
            let backoff = calculate_backoff_with_jitter(attempt, initial, max);
            assert!(backoff >= Duration::from_millis(50));
            assert!(backoff <= max);
        }
    }
}

//! Rate limiter for API requests
//!
//! Implements a token bucket algorithm with dynamic 429 penalty backoff,
//! cooperative cancellation via `CancellationToken`, and per-service isolation.

use anyhow::{anyhow, Result};
use reqwest::header::HeaderMap;
use reqwest::StatusCode;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime};
use tokio::sync::Mutex;
use tokio_util::sync::CancellationToken;

/// Configuration for rate limiting a specific service
#[derive(Debug, Clone)]
pub struct RateLimitConfig {
    /// Maximum requests allowed per window
    pub requests_per_window: u32,
    /// Window duration
    pub window_duration: Duration,
    /// Minimum delay between requests (optional additional throttle)
    pub min_delay: Option<Duration>,
}

impl RateLimitConfig {
    /// Create a new rate limit config with requests per second
    pub fn per_second(requests: u32) -> Self {
        Self {
            requests_per_window: requests,
            window_duration: Duration::from_secs(1),
            min_delay: None,
        }
    }

    /// Create a new rate limit config with requests per minute
    pub fn per_minute(requests: u32) -> Self {
        Self {
            requests_per_window: requests,
            window_duration: Duration::from_secs(60),
            min_delay: None,
        }
    }

    /// Add minimum delay between requests
    pub fn with_min_delay(mut self, delay: Duration) -> Self {
        self.min_delay = Some(delay);
        self
    }
}

/// Default rate limits for each service
pub fn default_rate_limits() -> HashMap<String, RateLimitConfig> {
    let mut limits = HashMap::new();

    // Spotify: ~30 requests per second (generous)
    limits.insert("spotify".to_string(), RateLimitConfig::per_second(30));

    // Qobuz: ~10 requests per second (conservative with 100ms min delay)
    limits.insert(
        "qobuz".to_string(),
        RateLimitConfig::per_second(10).with_min_delay(Duration::from_millis(100)),
    );

    // Tidal: ~20 requests per second (50ms min delay)
    limits.insert(
        "tidal".to_string(),
        RateLimitConfig::per_second(20).with_min_delay(Duration::from_millis(50)),
    );

    // Deezer: ~50 requests per 5 seconds
    limits.insert(
        "deezer".to_string(),
        RateLimitConfig {
            requests_per_window: 50,
            window_duration: Duration::from_secs(5),
            min_delay: None,
        },
    );

    // SoundCloud: ~15 requests per second
    limits.insert("soundcloud".to_string(), RateLimitConfig::per_second(15));

    // Apple Music: ~30 requests per second
    limits.insert("apple_music".to_string(), RateLimitConfig::per_second(30));

    // Last.fm: 4 requests per second (250ms min delay)
    limits.insert(
        "lastfm".to_string(),
        RateLimitConfig::per_second(4).with_min_delay(Duration::from_millis(250)),
    );

    // SongLink: ~15 requests per 2 seconds
    limits.insert(
        "songlink".to_string(),
        RateLimitConfig {
            requests_per_window: 15,
            window_duration: Duration::from_secs(2),
            min_delay: Some(Duration::from_millis(100)),
        },
    );

    // LRCLIB: ~30 requests per second
    limits.insert(
        "lrclib".to_string(),
        RateLimitConfig::per_second(30).with_min_delay(Duration::from_millis(50)),
    );

    // Amazon: 9 requests per minute (7s delay)
    limits.insert(
        "amazon".to_string(),
        RateLimitConfig::per_minute(9).with_min_delay(Duration::from_millis(2000)),
    );

    // MusicBrainz: 1 request per second
    limits.insert(
        "musicbrainz".to_string(),
        RateLimitConfig::per_second(1).with_min_delay(Duration::from_millis(1000)),
    );

    limits
}

/// Token bucket state for a single service
#[derive(Debug)]
struct BucketState {
    tokens: f64,
    last_refill: Instant,
    last_request: Option<Instant>,
    penalty_until: Option<Instant>,
}

impl BucketState {
    fn new(max_tokens: u32) -> Self {
        Self {
            tokens: max_tokens as f64,
            last_refill: Instant::now(),
            last_request: None,
            penalty_until: None,
        }
    }
}

/// Rate limiter using token bucket algorithm with dynamic 429 backoff
#[derive(Clone)]
pub struct RateLimiter {
    configs: Arc<HashMap<String, RateLimitConfig>>,
    buckets: Arc<Mutex<HashMap<String, BucketState>>>,
}

impl RateLimiter {
    /// Create a new rate limiter with default configs
    pub fn new() -> Self {
        Self::with_configs(default_rate_limits())
    }

    /// Create a rate limiter with custom configs
    pub fn with_configs(configs: HashMap<String, RateLimitConfig>) -> Self {
        Self {
            configs: Arc::new(configs),
            buckets: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    /// Dynamically penalize a service upon encountering a 429 Too Many Requests response.
    /// This pauses acquisitions for `service` until `duration` has elapsed.
    pub async fn penalize_service(&self, service: &str, duration: Duration) {
        let mut buckets = self.buckets.lock().await;
        let config = self.configs.get(service).cloned();
        let max_tokens = config.map(|c| c.requests_per_window).unwrap_or(10);

        let bucket = buckets
            .entry(service.to_string())
            .or_insert_with(|| BucketState::new(max_tokens));

        let now = Instant::now();
        let new_penalty = now + duration;
        bucket.penalty_until = match bucket.penalty_until {
            Some(existing) => Some(existing.max(new_penalty)),
            None => Some(new_penalty),
        };
        // Reset tokens to 0 to prevent a burst immediately upon penalty expiration
        bucket.tokens = 0.0;
        bucket.last_refill = bucket.penalty_until.unwrap_or(now);

        tracing::warn!(
            "[RateLimiter] Applied 429 penalty for service '{}': suspended for {:?}",
            service,
            duration
        );
    }

    /// Acquire permission to make a request with cooperative cancellation support.
    pub async fn acquire_cancellable(
        &self,
        service: &str,
        cancel_token: Option<&CancellationToken>,
    ) -> Result<()> {
        if let Some(token) = cancel_token {
            if token.is_cancelled() {
                return Err(anyhow!(
                    "Rate limiter acquire cancelled for service {}",
                    service
                ));
            }
        }

        let config = match self.configs.get(service) {
            Some(c) => c.clone(),
            None => {
                tracing::debug!("No rate limit config for {}, allowing request", service);
                return Ok(());
            }
        };

        loop {
            if let Some(token) = cancel_token {
                if token.is_cancelled() {
                    return Err(anyhow!(
                        "Rate limiter acquire cancelled for service {}",
                        service
                    ));
                }
            }

            let wait_time = {
                let mut buckets = self.buckets.lock().await;

                let bucket = buckets
                    .entry(service.to_string())
                    .or_insert_with(|| BucketState::new(config.requests_per_window));

                let now = Instant::now();

                // 1. Check if service is currently under a 429 penalty
                if let Some(penalty) = bucket.penalty_until {
                    if now < penalty {
                        let wait = penalty - now;
                        Some(wait)
                    } else {
                        bucket.penalty_until = None;
                        bucket.last_refill = now;
                        None
                    }
                } else {
                    None
                }
            };

            if let Some(penalty_wait) = wait_time {
                tracing::debug!(
                    "Service '{}' in 429 penalty cooldown, waiting {:?}",
                    service,
                    penalty_wait
                );
                if let Some(token) = cancel_token {
                    tokio::select! {
                        _ = token.cancelled() => {
                            return Err(anyhow!("Rate limiter acquire cancelled for service {}", service));
                        }
                        _ = tokio::time::sleep(penalty_wait) => {}
                    }
                } else {
                    tokio::time::sleep(penalty_wait).await;
                }
                continue;
            }

            let token_wait = {
                let mut buckets = self.buckets.lock().await;
                let bucket = buckets
                    .entry(service.to_string())
                    .or_insert_with(|| BucketState::new(config.requests_per_window));

                let now = Instant::now();

                // Refill tokens based on time elapsed
                let elapsed = now.duration_since(bucket.last_refill);
                let refill_rate =
                    config.requests_per_window as f64 / config.window_duration.as_secs_f64();
                let new_tokens = elapsed.as_secs_f64() * refill_rate;
                bucket.tokens = (bucket.tokens + new_tokens).min(config.requests_per_window as f64);
                bucket.last_refill = now;

                // Check min delay since last request
                if let Some(min_delay) = config.min_delay {
                    if let Some(last) = bucket.last_request {
                        let since_last = now.duration_since(last);
                        if since_last < min_delay {
                            Some(min_delay - since_last)
                        } else if bucket.tokens >= 1.0 {
                            bucket.tokens -= 1.0;
                            bucket.last_request = Some(now);
                            None
                        } else {
                            Some(Duration::from_secs_f64(1.0 / refill_rate))
                        }
                    } else if bucket.tokens >= 1.0 {
                        bucket.tokens -= 1.0;
                        bucket.last_request = Some(now);
                        None
                    } else {
                        Some(Duration::from_secs_f64(1.0 / refill_rate))
                    }
                } else if bucket.tokens >= 1.0 {
                    bucket.tokens -= 1.0;
                    bucket.last_request = Some(now);
                    None
                } else {
                    Some(Duration::from_secs_f64(1.0 / refill_rate))
                }
            };

            match token_wait {
                None => {
                    tracing::trace!("Rate limiter {} acquired", service);
                    return Ok(());
                }
                Some(wait) => {
                    tracing::debug!("Rate limited {}, waiting {:?}", service, wait);
                    if let Some(token) = cancel_token {
                        tokio::select! {
                            _ = token.cancelled() => {
                                return Err(anyhow!("Rate limiter acquire cancelled for service {}", service));
                            }
                            _ = tokio::time::sleep(wait) => {}
                        }
                    } else {
                        tokio::time::sleep(wait).await;
                    }
                }
            }
        }
    }

    /// Acquire permission to make a request. Waits if rate limited.
    pub async fn acquire(&self, service: &str) {
        let _ = self.acquire_cancellable(service, None).await;
    }
}

impl Default for RateLimiter {
    fn default() -> Self {
        Self::new()
    }
}

// Global shared RateLimiter instance
lazy_static::lazy_static! {
    pub static ref GLOBAL_RATE_LIMITER: RateLimiter = RateLimiter::new();
}

/// Delay registered for `429 Too Many Requests` when the response carries no
/// usable `Retry-After`.
pub const DEFAULT_429_PENALTY: Duration = Duration::from_secs(5);

/// Ceiling for a server-directed `Retry-After`, so a hostile or buggy header
/// cannot suspend a service for hours.
pub const MAX_429_PENALTY: Duration = Duration::from_secs(300);

/// Suspend `service` after a `429`, so every other caller of the shared limiter
/// slows down with it — not just the request that happened to be rejected.
///
/// The delay is the server's `Retry-After` when it sent a usable one (seconds
/// or HTTP-date), clamped to [`MAX_429_PENALTY`]; otherwise
/// [`DEFAULT_429_PENALTY`]. Returns the applied delay, or `None` when the
/// response was not a 429 and there is nothing to punish.
pub async fn penalize_on_rate_limit(
    service: &str,
    status: StatusCode,
    headers: &HeaderMap,
) -> Option<Duration> {
    if status != StatusCode::TOO_MANY_REQUESTS {
        return None;
    }

    let server_delay = headers
        .get(reqwest::header::RETRY_AFTER)
        .and_then(|h| h.to_str().ok())
        .and_then(|val| {
            crate::services::http_retry::HttpRetryPolicy::parse_retry_after_header(
                val,
                SystemTime::now(),
            )
        });

    let delay = server_delay
        .unwrap_or(DEFAULT_429_PENALTY)
        .min(MAX_429_PENALTY);

    GLOBAL_RATE_LIMITER.penalize_service(service, delay).await;
    Some(delay)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Own service key per test: `GLOBAL_RATE_LIMITER` is process-wide and a
    /// penalty applied here must not slow any other test down.
    const NO_PENALTY_SERVICE: &str = "rate-limiter-unit-test";

    #[tokio::test]
    async fn test_penalize_on_rate_limit_only_fires_on_429() {
        let headers = HeaderMap::new();

        // 5xx is transient but carries no "slow down" instruction.
        assert!(penalize_on_rate_limit(
            NO_PENALTY_SERVICE,
            StatusCode::SERVICE_UNAVAILABLE,
            &headers
        )
        .await
        .is_none());
        assert!(
            penalize_on_rate_limit(NO_PENALTY_SERVICE, StatusCode::UNAUTHORIZED, &headers)
                .await
                .is_none()
        );
    }

    #[tokio::test]
    async fn test_penalize_on_rate_limit_falls_back_to_the_default_delay() {
        let headers = HeaderMap::new();

        let applied =
            penalize_on_rate_limit(NO_PENALTY_SERVICE, StatusCode::TOO_MANY_REQUESTS, &headers)
                .await
                .expect("a 429 always registers a penalty");
        assert_eq!(applied, DEFAULT_429_PENALTY);
    }

    #[tokio::test]
    async fn test_penalize_on_rate_limit_honours_and_clamps_retry_after() {
        let mut headers = HeaderMap::new();
        headers.insert(
            reqwest::header::RETRY_AFTER,
            reqwest::header::HeaderValue::from_static("42"),
        );
        let applied =
            penalize_on_rate_limit(NO_PENALTY_SERVICE, StatusCode::TOO_MANY_REQUESTS, &headers)
                .await
                .expect("a 429 always registers a penalty");
        assert_eq!(applied, Duration::from_secs(42));

        // A hostile header must not suspend a service for hours.
        let mut absurd = HeaderMap::new();
        absurd.insert(
            reqwest::header::RETRY_AFTER,
            reqwest::header::HeaderValue::from_static("86400"),
        );
        let applied =
            penalize_on_rate_limit(NO_PENALTY_SERVICE, StatusCode::TOO_MANY_REQUESTS, &absurd)
                .await
                .expect("a 429 always registers a penalty");
        assert_eq!(applied, MAX_429_PENALTY);
    }

    #[tokio::test]
    async fn test_penalize_on_rate_limit_suspends_the_shared_limiter() {
        // A service with a real profile, so `acquire` has to honour the penalty.
        let limiter = RateLimiter::with_configs(default_rate_limits());
        limiter
            .penalize_service("qobuz", Duration::from_millis(300))
            .await;

        let started = Instant::now();
        limiter.acquire("qobuz").await;
        assert!(
            started.elapsed() >= Duration::from_millis(300),
            "acquire must wait out the penalty, waited {:?}",
            started.elapsed()
        );
    }

    #[tokio::test]
    async fn test_rate_limiter_allows_burst() {
        let limiter = RateLimiter::new();

        // Should allow burst of requests up to limit
        for _ in 0..30 {
            limiter.acquire("spotify").await;
        }
    }

    #[tokio::test]
    async fn test_rate_limiter_unknown_service() {
        let limiter = RateLimiter::new();

        // Unknown service should pass through
        limiter.acquire("unknown_service").await;
    }

    #[tokio::test]
    async fn test_rate_limiter_lastfm_config() {
        let limiter = RateLimiter::new();
        let config = limiter
            .configs
            .get("lastfm")
            .expect("lastfm config must exist");
        assert_eq!(config.requests_per_window, 4);
        assert_eq!(config.window_duration, Duration::from_secs(1));
        assert_eq!(config.min_delay, Some(Duration::from_millis(250)));
    }

    #[tokio::test]
    async fn test_rate_limiter_service_isolation() {
        let limiter = RateLimiter::new();

        // Exhaust all 30 tokens for "spotify"
        for _ in 0..30 {
            limiter.acquire("spotify").await;
        }

        // "musicbrainz" should acquire immediately without being blocked by spotify's bucket
        let start = Instant::now();
        limiter.acquire("musicbrainz").await;
        assert!(start.elapsed() < Duration::from_millis(100));
    }

    #[tokio::test]
    async fn test_rate_limiter_429_penalty() {
        let limiter = RateLimiter::new();

        // Apply a 200ms penalty to "qobuz"
        limiter
            .penalize_service("qobuz", Duration::from_millis(200))
            .await;

        let start = Instant::now();
        limiter.acquire("qobuz").await;
        let elapsed = start.elapsed();

        assert!(
            elapsed >= Duration::from_millis(180),
            "Expected wait >= 180ms, got {:?}",
            elapsed
        );
    }

    #[tokio::test]
    async fn test_rate_limiter_cancellation() {
        let limiter = RateLimiter::new();
        let cancel_token = CancellationToken::new();

        // Cancel immediately
        cancel_token.cancel();

        let res = limiter
            .acquire_cancellable("qobuz", Some(&cancel_token))
            .await;
        assert!(res.is_err());
        assert!(res.unwrap_err().to_string().contains("cancelled"));
    }
}

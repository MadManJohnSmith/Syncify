//! Rate limiter for API requests
//!
//! Implements a token bucket algorithm to control request rates per service.
//! This module will be integrated into service clients when rate limiting is needed.

#![allow(dead_code)] // Public API for future service integration

use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::Mutex;

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

    // Qobuz: ~10 requests per second (more conservative)
    limits.insert(
        "qobuz".to_string(),
        RateLimitConfig::per_second(10).with_min_delay(Duration::from_millis(100)),
    );

    // Tidal: ~20 requests per second
    limits.insert("tidal".to_string(), RateLimitConfig::per_second(20));

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

    limits
}

/// Token bucket state for a single service
#[derive(Debug)]
struct BucketState {
    tokens: f64,
    last_refill: Instant,
    last_request: Option<Instant>,
}

impl BucketState {
    fn new(max_tokens: u32) -> Self {
        Self {
            tokens: max_tokens as f64,
            last_refill: Instant::now(),
            last_request: None,
        }
    }
}

/// Rate limiter using token bucket algorithm
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

    /// Acquire permission to make a request. Waits if rate limited.
    pub async fn acquire(&self, service: &str) {
        let config = match self.configs.get(service) {
            Some(c) => c.clone(),
            None => {
                tracing::debug!("No rate limit config for {}, allowing request", service);
                return;
            }
        };

        loop {
            let wait_time = {
                let mut buckets = self.buckets.lock().await;

                let bucket = buckets
                    .entry(service.to_string())
                    .or_insert_with(|| BucketState::new(config.requests_per_window));

                // Refill tokens based on time elapsed
                let now = Instant::now();
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
                            // Need to wait for token
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
                    // Wait for token refill
                    Some(Duration::from_secs_f64(1.0 / refill_rate))
                }
            };

            match wait_time {
                None => {
                    tracing::trace!("Rate limiter {} acquired", service);
                    return;
                }
                Some(wait) => {
                    tracing::debug!("Rate limited {}, waiting {:?}", service, wait);
                    tokio::time::sleep(wait).await;
                }
            }
        }
    }
}

impl Default for RateLimiter {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
        let config = limiter.configs.get("lastfm").expect("lastfm config must exist");
        assert_eq!(config.requests_per_window, 4);
        assert_eq!(config.window_duration, Duration::from_secs(1));
        assert_eq!(config.min_delay, Some(Duration::from_millis(250)));
    }
}

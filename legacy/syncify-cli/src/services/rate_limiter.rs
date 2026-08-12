//! Rate limiter for API requests (CLI Standalone)

#![allow(dead_code)]

use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::Mutex;

#[derive(Debug, Clone)]
pub struct RateLimitConfig {
    pub requests_per_window: u32,
    pub window_duration: Duration,
    pub min_delay: Option<Duration>,
}

impl RateLimitConfig {
    pub fn per_second(requests: u32) -> Self {
        Self {
            requests_per_window: requests,
            window_duration: Duration::from_secs(1),
            min_delay: None,
        }
    }

    pub fn per_minute(requests: u32) -> Self {
        Self {
            requests_per_window: requests,
            window_duration: Duration::from_secs(60),
            min_delay: None,
        }
    }

    pub fn with_min_delay(mut self, delay: Duration) -> Self {
        self.min_delay = Some(delay);
        self
    }
}

pub fn default_rate_limits() -> HashMap<String, RateLimitConfig> {
    let mut limits = HashMap::new();

    limits.insert("spotify".to_string(), RateLimitConfig::per_second(30));
    limits.insert(
        "qobuz".to_string(),
        RateLimitConfig::per_second(10).with_min_delay(Duration::from_millis(100)),
    );
    limits.insert("tidal".to_string(), RateLimitConfig::per_second(20));
    limits.insert(
        "deezer".to_string(),
        RateLimitConfig {
            requests_per_window: 50,
            window_duration: Duration::from_secs(5),
            min_delay: None,
        },
    );
    limits.insert("soundcloud".to_string(), RateLimitConfig::per_second(15));
    limits.insert("apple_music".to_string(), RateLimitConfig::per_second(30));
    limits.insert(
        "lastfm".to_string(),
        RateLimitConfig::per_second(4).with_min_delay(Duration::from_millis(250)),
    );

    limits
}

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

#[derive(Clone)]
pub struct RateLimiter {
    configs: Arc<HashMap<String, RateLimitConfig>>,
    buckets: Arc<Mutex<HashMap<String, BucketState>>>,
}

impl RateLimiter {
    pub fn new() -> Self {
        Self::with_configs(default_rate_limits())
    }

    pub fn with_configs(configs: HashMap<String, RateLimitConfig>) -> Self {
        Self {
            configs: Arc::new(configs),
            buckets: Arc::new(Mutex::new(HashMap::new())),
        }
    }

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

                let now = Instant::now();
                let elapsed = now.duration_since(bucket.last_refill);
                let refill_rate =
                    config.requests_per_window as f64 / config.window_duration.as_secs_f64();
                let new_tokens = elapsed.as_secs_f64() * refill_rate;
                bucket.tokens = (bucket.tokens + new_tokens).min(config.requests_per_window as f64);
                bucket.last_refill = now;

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

// Shared HTTP client with rate limiting and User-Agent rotation

use reqwest::{header, Client, ClientBuilder};
use std::collections::HashMap;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::RwLock;
use std::time::Duration;
use std::time::Instant;
use tokio::time::sleep;

/// Default timeout for HTTP requests (60 seconds)
pub const DEFAULT_TIMEOUT: Duration = Duration::from_secs(60);

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

/// Create a new HTTP client with default settings
pub fn create_http_client() -> Client {
    create_http_client_with_timeout(DEFAULT_TIMEOUT)
}

/// Create a new HTTP client with custom timeout
pub fn create_http_client_with_timeout(timeout: Duration) -> Client {
    let mut headers = header::HeaderMap::new();
    headers.insert(
        header::USER_AGENT,
        header::HeaderValue::from_static(USER_AGENTS[0]),
    );

    ClientBuilder::new()
        .timeout(timeout)
        .default_headers(headers)
        .pool_max_idle_per_host(10)
        .build()
        .expect("Failed to create HTTP client")
}

/// Rate limiter for API calls
pub struct RateLimiter {
    /// Minimum delay between requests in milliseconds
    min_delay_ms: u64,
    /// Maximum requests per minute
    max_per_minute: u32,
    /// Track last request time per service
    last_request: RwLock<HashMap<String, Instant>>,
    /// Track request count per minute per service
    request_counts: RwLock<HashMap<String, (u32, Instant)>>,
}

impl RateLimiter {
    pub fn new(min_delay_ms: u64, max_per_minute: u32) -> Self {
        Self {
            min_delay_ms,
            max_per_minute,
            last_request: RwLock::new(HashMap::new()),
            request_counts: RwLock::new(HashMap::new()),
        }
    }

    /// Wait if needed to respect rate limits
    pub async fn wait(&self, service: &str) {
        let now = Instant::now();

        // 1. Check minimum delay
        let delay_wait = {
            let last = match self.last_request.read() {
                Ok(guard) => guard,
                Err(poisoned) => {
                    tracing::warn!("RateLimiter last_request lock poisoned, recovering");
                    poisoned.into_inner()
                }
            };
            if let Some(last_time) = last.get(service) {
                let elapsed = now.duration_since(*last_time);
                let min_delay = Duration::from_millis(self.min_delay_ms);
                if elapsed < min_delay {
                    Some(min_delay - elapsed)
                } else {
                    None
                }
            } else {
                None
            }
        };

        if let Some(wait) = delay_wait {
            sleep(wait).await;
        }

        // 2. Check requests per minute
        let rate_wait = {
            let mut counts = match self.request_counts.write() {
                Ok(guard) => guard,
                Err(poisoned) => {
                    tracing::warn!("RateLimiter request_counts lock poisoned, recovering");
                    poisoned.into_inner()
                }
            };
            let entry = counts
                .entry(service.to_string())
                .or_insert((0, Instant::now()));

            // Reset counter if minute has passed
            if entry.1.elapsed() >= Duration::from_secs(60) {
                *entry = (0, Instant::now());
            }

            // Check limit
            if entry.0 >= self.max_per_minute {
                // Calculate wait time
                let elapsed = entry.1.elapsed();
                let window = Duration::from_secs(60);
                if elapsed < window {
                    Some(window - elapsed)
                } else {
                    Some(Duration::from_millis(100)) // Should not happen given reset logic, but safe fallback
                }
            } else {
                None
            }
        };

        if let Some(wait) = rate_wait {
            sleep(wait).await;

            // Reset after waiting
            let mut counts = match self.request_counts.write() {
                Ok(guard) => guard,
                Err(poisoned) => {
                    tracing::warn!("RateLimiter request_counts lock poisoned, recovering");
                    poisoned.into_inner()
                }
            };
            counts.insert(service.to_string(), (0, Instant::now()));
        }

        // 3. Record this request
        {
            let mut last = match self.last_request.write() {
                Ok(guard) => guard,
                Err(poisoned) => {
                    tracing::warn!("RateLimiter last_request lock poisoned, recovering");
                    poisoned.into_inner()
                }
            };
            last.insert(service.to_string(), Instant::now());
        }
        {
            let mut counts = match self.request_counts.write() {
                Ok(guard) => guard,
                Err(poisoned) => {
                    tracing::warn!("RateLimiter request_counts lock poisoned, recovering");
                    poisoned.into_inner()
                }
            };
            let entry = counts
                .entry(service.to_string())
                .or_insert((0, Instant::now()));

            // Double safety: Reset counter if minute has passed (handles cases where we slept or race condition)
            if entry.1.elapsed() >= Duration::from_secs(60) {
                *entry = (0, Instant::now());
            }

            entry.0 += 1;
        }
    }
}

// Default rate limiters for each service
lazy_static::lazy_static! {
    // Qobuz: 60 req/min, 1s delay
    pub static ref QOBUZ_LIMITER: RateLimiter = RateLimiter::new(1000, 60);

    // Tidal: 60 req/min, 1s delay
    pub static ref TIDAL_LIMITER: RateLimiter = RateLimiter::new(1000, 60);

    // Amazon (DoubleDouble): 9 req/min, 7s delay
    pub static ref AMAZON_LIMITER: RateLimiter = RateLimiter::new(7000, 9);

    // SongLink: 30 req/min, 2s delay
    pub static ref SONGLINK_LIMITER: RateLimiter = RateLimiter::new(2000, 30);

    // LRCLIB: 60 req/min, 500ms delay
    pub static ref LRCLIB_LIMITER: RateLimiter = RateLimiter::new(500, 60);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_user_agent_rotation() {
        let ua1 = get_user_agent();
        let ua2 = get_user_agent();
        // They should be from our list
        assert!(USER_AGENTS.contains(&ua1));
        assert!(USER_AGENTS.contains(&ua2));
    }

    #[test]
    fn test_create_client() {
        let client = create_http_client();
        // Just verify it doesn't panic
        drop(client);
    }
}

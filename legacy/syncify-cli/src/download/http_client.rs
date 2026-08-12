//! Shared HTTP client with rate limiting and User-Agent rotation (CLI Standalone)

use reqwest::{header, Client, ClientBuilder};
use std::collections::HashMap;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::RwLock;
use std::time::Duration;
use std::time::Instant;
use tokio::time::sleep;

pub const DEFAULT_TIMEOUT: Duration = Duration::from_secs(60);

static USER_AGENTS: &[&str] = &[
    "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36",
    "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/119.0.0.0 Safari/537.36",
    "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36",
    "Mozilla/5.0 (Windows NT 10.0; Win64; x64; rv:121.0) Gecko/20100101 Firefox/121.0",
    "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/17.2 Safari/605.1.15",
];

static UA_INDEX: AtomicUsize = AtomicUsize::new(0);

pub fn get_user_agent() -> &'static str {
    let index = UA_INDEX.fetch_add(1, Ordering::Relaxed) % USER_AGENTS.len();
    USER_AGENTS[index]
}

pub fn create_http_client() -> Client {
    create_http_client_with_timeout(DEFAULT_TIMEOUT)
}

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

pub struct RateLimiter {
    min_delay_ms: u64,
    max_per_minute: u32,
    last_request: RwLock<HashMap<String, Instant>>,
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

    pub async fn wait(&self, service: &str) {
        let now = Instant::now();

        let delay_wait = {
            let last = match self.last_request.read() {
                Ok(guard) => guard,
                Err(poisoned) => poisoned.into_inner(),
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

        let rate_wait = {
            let mut counts = match self.request_counts.write() {
                Ok(guard) => guard,
                Err(poisoned) => poisoned.into_inner(),
            };
            let entry = counts
                .entry(service.to_string())
                .or_insert((0, Instant::now()));

            if entry.1.elapsed() >= Duration::from_secs(60) {
                *entry = (0, Instant::now());
            }

            if entry.0 >= self.max_per_minute {
                let elapsed = entry.1.elapsed();
                let window = Duration::from_secs(60);
                if elapsed < window {
                    Some(window - elapsed)
                } else {
                    Some(Duration::from_millis(100))
                }
            } else {
                None
            }
        };

        if let Some(wait) = rate_wait {
            sleep(wait).await;

            let mut counts = match self.request_counts.write() {
                Ok(guard) => guard,
                Err(poisoned) => poisoned.into_inner(),
            };
            counts.insert(service.to_string(), (0, Instant::now()));
        }

        {
            let mut last = match self.last_request.write() {
                Ok(guard) => guard,
                Err(poisoned) => poisoned.into_inner(),
            };
            last.insert(service.to_string(), Instant::now());
        }
        {
            let mut counts = match self.request_counts.write() {
                Ok(guard) => guard,
                Err(poisoned) => poisoned.into_inner(),
            };
            let entry = counts
                .entry(service.to_string())
                .or_insert((0, Instant::now()));

            if entry.1.elapsed() >= Duration::from_secs(60) {
                *entry = (0, Instant::now());
            }

            entry.0 += 1;
        }
    }
}

lazy_static::lazy_static! {
    pub static ref QOBUZ_LIMITER: RateLimiter = RateLimiter::new(1000, 60);
    pub static ref TIDAL_LIMITER: RateLimiter = RateLimiter::new(1000, 60);
    pub static ref AMAZON_LIMITER: RateLimiter = RateLimiter::new(7000, 9);
    pub static ref SONGLINK_LIMITER: RateLimiter = RateLimiter::new(2000, 30);
    pub static ref LRCLIB_LIMITER: RateLimiter = RateLimiter::new(500, 60);
}

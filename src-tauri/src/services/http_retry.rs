//! HTTP Retry Policy and Resilience Module
//!
//! Provides deterministic evaluation of HTTP response retries,
//! server-directed `Retry-After` header parsing (seconds or HTTP-date),
//! exponential backoff with jitter, and strict idempotency scoping.

#![allow(dead_code)]

use reqwest::{header, Method, StatusCode};
use std::time::{Duration, SystemTime};

/// Configuration for HTTP retry policy
#[derive(Debug, Clone)]
pub struct RetryConfig {
    /// Maximum number of retry attempts
    pub max_retries: u32,
    /// Base backoff duration (default: 500ms)
    pub initial_backoff: Duration,
    /// Maximum backoff duration cap (default: 30s)
    pub max_backoff: Duration,
    /// Backoff multiplier (default: 2.0)
    pub backoff_factor: f64,
}

impl Default for RetryConfig {
    fn default() -> Self {
        Self {
            max_retries: 3,
            initial_backoff: Duration::from_millis(500),
            max_backoff: Duration::from_secs(30),
            backoff_factor: 2.0,
        }
    }
}

/// Decision returned after evaluating an HTTP response or failure
#[derive(Debug, PartialEq, Eq)]
pub enum RetryDecision {
    /// Request succeeded (2xx) or does not require retry
    Success,
    /// Do not retry (permanent client error like 401/403/404, or non-idempotent operation)
    DoNotRetry(String),
    /// Retry after the calculated duration
    RetryAfter(Duration),
    /// Max retries exceeded
    MaxRetriesExceeded,
}

/// Evaluates HTTP responses and determines retry suitability
pub struct HttpRetryPolicy {
    config: RetryConfig,
}

impl HttpRetryPolicy {
    /// Create a new policy with default config
    pub fn new() -> Self {
        Self {
            config: RetryConfig::default(),
        }
    }

    /// Create a policy with custom config
    pub fn with_config(config: RetryConfig) -> Self {
        Self { config }
    }

    /// Determines if an HTTP method is considered inherently idempotent (e.g., GET, HEAD, PUT)
    pub fn is_method_idempotent(method: &Method) -> bool {
        matches!(*method, Method::GET | Method::HEAD | Method::PUT | Method::OPTIONS)
    }

    /// Helper to parse HTTP-date (IMF-fixdate / RFC 2822 / RFC 1123) without external crates
    fn parse_http_date_to_secs(date_str: &str) -> Option<i64> {
        let parts: Vec<&str> = date_str.split_whitespace().collect();
        let (day_str, month_str, year_str, time_str) = match parts.len() {
            6 => (parts[1], parts[2], parts[3], parts[4]), // Sun, 06 Nov 1994 08:49:37 GMT
            5 => (parts[0], parts[1], parts[2], parts[3]), // 06 Nov 1994 08:49:37 GMT
            _ => return None,
        };

        let day: u32 = day_str.parse().ok()?;
        let year: i32 = year_str.parse().ok()?;
        let month = match month_str {
            "Jan" => 1, "Feb" => 2, "Mar" => 3, "Apr" => 4,
            "May" => 5, "Jun" => 6, "Jul" => 7, "Aug" => 8,
            "Sep" => 9, "Oct" => 10, "Nov" => 11, "Dec" => 12,
            _ => return None,
        };

        let time_parts: Vec<&str> = time_str.split(':').collect();
        if time_parts.len() != 3 {
            return None;
        }
        let hour: u32 = time_parts[0].parse().ok()?;
        let min: u32 = time_parts[1].parse().ok()?;
        let sec: u32 = time_parts[2].parse().ok()?;

        let mut days = 0i64;
        for y in 1970..year {
            let is_leap = (y % 4 == 0 && y % 100 != 0) || (y % 400 == 0);
            days += if is_leap { 366 } else { 365 };
        }
        let is_current_leap = (year % 4 == 0 && year % 100 != 0) || (year % 400 == 0);
        let days_in_months = if is_current_leap {
            [31, 29, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31]
        } else {
            [31, 28, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31]
        };
        for m in 1..(month as usize) {
            days += days_in_months[m - 1] as i64;
        }
        days += (day as i64) - 1;

        let total_secs = days * 86400 + (hour as i64) * 3600 + (min as i64) * 60 + (sec as i64);
        Some(total_secs)
    }

    /// Parses a `Retry-After` header value into a `Duration`.
    /// Supports both integer seconds ("120") and HTTP-date ("Sun, 06 Nov 1994 08:49:37 GMT").
    pub fn parse_retry_after_header(header_val: &str, now: SystemTime) -> Option<Duration> {
        let trimmed = header_val.trim();

        // 1. Try parsing integer seconds
        if let Ok(seconds) = trimmed.parse::<u64>() {
            return Some(Duration::from_secs(seconds));
        }

        // 2. Try parsing HTTP-date (RFC 2822 / IMF-fixdate)
        if let Some(target_secs) = Self::parse_http_date_to_secs(trimmed) {
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

    /// Computes bounded exponential backoff for a given attempt (0-indexed)
    pub fn calculate_backoff(&self, attempt: u32) -> Duration {
        let factor = self.config.backoff_factor.powi(attempt as i32);
        let backoff_secs = self.config.initial_backoff.as_secs_f64() * factor;
        let clamped = backoff_secs.min(self.config.max_backoff.as_secs_f64());
        Duration::from_secs_f64(clamped)
    }

    /// Evaluates a response and attempt index to determine if a retry should occur.
    /// `is_idempotent_override` allows non-GET requests to explicitly opt-in to retries if safe.
    pub fn evaluate_response(
        &self,
        method: &Method,
        status: StatusCode,
        headers: &header::HeaderMap,
        attempt: u32,
        is_idempotent_override: bool,
        now: SystemTime,
    ) -> RetryDecision {
        if status.is_success() {
            return RetryDecision::Success;
        }

        // Permanent client errors (401, 403, 404, 422) must never be retried
        if matches!(
            status,
            StatusCode::UNAUTHORIZED
                | StatusCode::FORBIDDEN
                | StatusCode::NOT_FOUND
                | StatusCode::UNPROCESSABLE_ENTITY
        ) {
            return RetryDecision::DoNotRetry(format!("Permanent status code {}", status));
        }

        // Idempotency Check: POST/DELETE/PATCH are not retried unless explicitly overridden
        let is_safe = Self::is_method_idempotent(method) || is_idempotent_override;
        if !is_safe {
            return RetryDecision::DoNotRetry(format!(
                "Method {} is non-idempotent and retry is not explicitly enabled",
                method
            ));
        }

        // Max retries check
        if attempt >= self.config.max_retries {
            return RetryDecision::MaxRetriesExceeded;
        }

        // Check for 429 Too Many Requests or 5xx Transient Server Errors
        let is_transient = status == StatusCode::TOO_MANY_REQUESTS || status.is_server_error();
        if !is_transient {
            return RetryDecision::DoNotRetry(format!("Non-transient status code {}", status));
        }

        // Calculate base backoff
        let calculated_backoff = self.calculate_backoff(attempt);

        // Check for Retry-After header
        let server_retry_after = headers
            .get(header::RETRY_AFTER)
            .and_then(|h| h.to_str().ok())
            .and_then(|val| Self::parse_retry_after_header(val, now));

        // Rule: delay = max(retry_after, calculated_backoff)
        let final_delay = match server_retry_after {
            Some(server_delay) => server_delay.max(calculated_backoff),
            None => calculated_backoff,
        };

        RetryDecision::RetryAfter(final_delay)
    }
}

impl Default for HttpRetryPolicy {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_retry_after_seconds() {
        let now = SystemTime::UNIX_EPOCH;
        let delay = HttpRetryPolicy::parse_retry_after_header("120", now);
        assert_eq!(delay, Some(Duration::from_secs(120)));
    }

    #[test]
    fn test_parse_retry_after_http_date() {
        // "Sun, 06 Nov 1994 08:49:37 GMT"
        let date_str = "Sun, 06 Nov 1994 08:49:37 GMT";
        let target_secs = HttpRetryPolicy::parse_http_date_to_secs(date_str).unwrap();

        let now_secs = target_secs - 30;
        let now = SystemTime::UNIX_EPOCH + Duration::from_secs(now_secs as u64);

        let delay = HttpRetryPolicy::parse_retry_after_header(date_str, now);
        assert_eq!(delay, Some(Duration::from_secs(30)));
    }

    #[test]
    fn test_permanent_4xx_not_retried() {
        let policy = HttpRetryPolicy::new();
        let headers = header::HeaderMap::new();
        let now = SystemTime::now();

        let decision = policy.evaluate_response(
            &Method::GET,
            StatusCode::NOT_FOUND,
            &headers,
            0,
            false,
            now,
        );

        assert!(matches!(decision, RetryDecision::DoNotRetry(_)));
    }

    #[test]
    fn test_non_idempotent_post_not_retried_by_default() {
        let policy = HttpRetryPolicy::new();
        let headers = header::HeaderMap::new();
        let now = SystemTime::now();

        let decision = policy.evaluate_response(
            &Method::POST,
            StatusCode::TOO_MANY_REQUESTS,
            &headers,
            0,
            false, // no opt-in
            now,
        );

        assert!(matches!(decision, RetryDecision::DoNotRetry(_)));
    }

    #[test]
    fn test_non_idempotent_post_retried_with_opt_in() {
        let policy = HttpRetryPolicy::new();
        let headers = header::HeaderMap::new();
        let now = SystemTime::now();

        let decision = policy.evaluate_response(
            &Method::POST,
            StatusCode::TOO_MANY_REQUESTS,
            &headers,
            0,
            true, // explicit opt-in
            now,
        );

        assert!(matches!(decision, RetryDecision::RetryAfter(_)));
    }

    #[test]
    fn test_retry_after_header_takes_priority_if_larger() {
        let policy = HttpRetryPolicy::new();
        let mut headers = header::HeaderMap::new();
        headers.insert(header::RETRY_AFTER, header::HeaderValue::from_static("60"));
        let now = SystemTime::now();

        let decision = policy.evaluate_response(
            &Method::GET,
            StatusCode::TOO_MANY_REQUESTS,
            &headers,
            0, // attempt 0 -> backoff 500ms
            false,
            now,
        );

        assert_eq!(decision, RetryDecision::RetryAfter(Duration::from_secs(60)));
    }

    #[test]
    fn test_max_retries_exceeded() {
        let policy = HttpRetryPolicy::new();
        let headers = header::HeaderMap::new();
        let now = SystemTime::now();

        let decision = policy.evaluate_response(
            &Method::GET,
            StatusCode::INTERNAL_SERVER_ERROR,
            &headers,
            3, // attempt 3 >= max_retries 3
            false,
            now,
        );

        assert_eq!(decision, RetryDecision::MaxRetriesExceeded);
    }
}

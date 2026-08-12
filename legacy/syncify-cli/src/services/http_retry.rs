//! HTTP Retry Policy and Resilience Module (CLI Standalone)

#![allow(dead_code)]

use reqwest::{header, Method, StatusCode};
use std::time::{Duration, SystemTime};

#[derive(Debug, Clone)]
pub struct RetryConfig {
    pub max_retries: u32,
    pub initial_backoff: Duration,
    pub max_backoff: Duration,
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

#[derive(Debug, PartialEq, Eq)]
pub enum RetryDecision {
    Success,
    DoNotRetry(String),
    RetryAfter(Duration),
    MaxRetriesExceeded,
}

pub struct HttpRetryPolicy {
    config: RetryConfig,
}

impl HttpRetryPolicy {
    pub fn new() -> Self {
        Self {
            config: RetryConfig::default(),
        }
    }

    pub fn with_config(config: RetryConfig) -> Self {
        Self { config }
    }

    pub fn is_method_idempotent(method: &Method) -> bool {
        matches!(*method, Method::GET | Method::HEAD | Method::PUT | Method::OPTIONS)
    }

    fn parse_http_date_to_secs(date_str: &str) -> Option<i64> {
        let parts: Vec<&str> = date_str.split_whitespace().collect();
        let (day_str, month_str, year_str, time_str) = match parts.len() {
            6 => (parts[1], parts[2], parts[3], parts[4]),
            5 => (parts[0], parts[1], parts[2], parts[3]),
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

    pub fn parse_retry_after_header(header_val: &str, now: SystemTime) -> Option<Duration> {
        let trimmed = header_val.trim();

        if let Ok(seconds) = trimmed.parse::<u64>() {
            return Some(Duration::from_secs(seconds));
        }

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

    pub fn calculate_backoff(&self, attempt: u32) -> Duration {
        let factor = self.config.backoff_factor.powi(attempt as i32);
        let backoff_secs = self.config.initial_backoff.as_secs_f64() * factor;
        let clamped = backoff_secs.min(self.config.max_backoff.as_secs_f64());
        Duration::from_secs_f64(clamped)
    }

    pub fn evaluate_network_error(
        &self,
        method: &Method,
        attempt: u32,
        is_idempotent_override: bool,
        is_cancelled: bool,
    ) -> RetryDecision {
        if is_cancelled {
            return RetryDecision::DoNotRetry("Request cancelled".into());
        }

        let is_safe = Self::is_method_idempotent(method) || is_idempotent_override;
        if !is_safe {
            return RetryDecision::DoNotRetry(format!(
                "Method {} is non-idempotent and retry on network error is not explicitly enabled",
                method
            ));
        }

        if attempt >= self.config.max_retries {
            return RetryDecision::MaxRetriesExceeded;
        }

        let backoff = self.calculate_backoff(attempt);
        RetryDecision::RetryAfter(backoff)
    }

    pub fn evaluate_response(
        &self,
        method: &Method,
        status: StatusCode,
        headers: &header::HeaderMap,
        attempt: u32,
        is_idempotent_override: bool,
        is_cancelled: bool,
        now: SystemTime,
    ) -> RetryDecision {
        if is_cancelled {
            return RetryDecision::DoNotRetry("Request cancelled".into());
        }

        if status.is_success() {
            return RetryDecision::Success;
        }

        if matches!(
            status,
            StatusCode::UNAUTHORIZED
                | StatusCode::FORBIDDEN
                | StatusCode::NOT_FOUND
                | StatusCode::UNPROCESSABLE_ENTITY
        ) {
            return RetryDecision::DoNotRetry(format!("Permanent status code {}", status));
        }

        let is_safe = Self::is_method_idempotent(method) || is_idempotent_override;
        if !is_safe {
            return RetryDecision::DoNotRetry(format!(
                "Method {} is non-idempotent and retry is not explicitly enabled",
                method
            ));
        }

        if attempt >= self.config.max_retries {
            return RetryDecision::MaxRetriesExceeded;
        }

        let is_transient = status == StatusCode::TOO_MANY_REQUESTS || status.is_server_error();
        if !is_transient {
            return RetryDecision::DoNotRetry(format!("Non-transient status code {}", status));
        }

        let calculated_backoff = self.calculate_backoff(attempt);

        let server_retry_after = headers
            .get(header::RETRY_AFTER)
            .and_then(|h| h.to_str().ok())
            .and_then(|val| Self::parse_retry_after_header(val, now));

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

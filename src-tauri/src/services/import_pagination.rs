//! S189-Fase-0 / S198 — Shared import pagination policy.
//!
//! Generalizes the S187 Tidal semantics to every importer:
//! - Advance by the REAL page length (a short page is not the end when a
//!   provider total is known; server-side filtering can shrink pages).
//! - Stop as soon as `offset + page_len >= total` when the provider declared
//!   a total, taking the MAX of declared and observed totals.
//! - Without any total, stop on the first short page (classic offset paging).
//! - A zero-length page always ends pagination (defensive: avoids infinite
//!   loops against misbehaving endpoints).
//!
//! Pure decision logic only: each caller keeps its own fetch loop (response
//! types differ per service) but termination/advancement decisions live here
//! so they are unit-testable and identical across the 6 services.

/// Decide whether pagination continues and at which offset.
///
/// * `current_offset` — offset used for the page just received.
/// * `page_len` — number of items actually received in that page.
/// * `requested_limit` — limit the caller asked for (needed to detect a short
///   page when the endpoint exposes no total).
/// * `provider_total` — best-known total: max(provider-declared, locally
///   declared fallback). `None` when the endpoint exposes no total at all.
///
/// Returns `Some(next_offset)` to continue or `None` when done.
pub fn next_offset(
    current_offset: i32,
    page_len: i32,
    requested_limit: i32,
    provider_total: Option<i64>,
) -> Option<i32> {
    if page_len <= 0 {
        return None;
    }
    if let Some(total) = provider_total {
        if total > 0 && current_offset as i64 + page_len as i64 >= total {
            return None;
        }
        return Some(current_offset + page_len);
    }
    // No total known: continue only while pages come back full; a short page
    // is the natural end (legacy semantics for endpoints without totals).
    if page_len < requested_limit {
        return None;
    }
    Some(current_offset + page_len)
}

/// True when a page shorter than the requested limit should be surfaced as a
/// coverage gap even though pagination continues (S187 warn-and-continue).
pub fn is_short_page(page_len: i32, requested_limit: i32, provider_total: Option<i64>) -> bool {
    provider_total.is_some() && page_len > 0 && page_len < requested_limit
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stops_when_total_reached_exactly() {
        // 3 full pages of 100 over a total of 300 → end after third.
        assert_eq!(next_offset(0, 100, 100, Some(300)), Some(100));
        assert_eq!(next_offset(100, 100, 100, Some(300)), Some(200));
        assert_eq!(next_offset(200, 100, 100, Some(300)), None);
    }

    #[test]
    fn short_page_with_known_total_continues() {
        // Server filtered 2 items out of a 100-page; total still far away.
        assert_eq!(next_offset(0, 98, 100, Some(500)), Some(98));
        // 400 + 30 = 430 < 500 → still one more page to request.
        assert_eq!(next_offset(400, 30, 100, Some(500)), Some(430));
        assert_eq!(next_offset(430, 70, 100, Some(500)), None);
    }

    #[test]
    fn observed_total_can_exceed_declared() {
        // Declared fallback said 250 but pages keep coming with total 400.
        assert_eq!(next_offset(0, 100, 100, Some(250)), Some(100));
        // Caller keeps max(): after seeing total=400 the effective total grows.
        assert_eq!(next_offset(300, 100, 100, Some(400)), None);
    }

    #[test]
    fn empty_page_always_ends() {
        assert_eq!(next_offset(0, 0, 100, Some(500)), None);
        assert_eq!(next_offset(100, 0, 100, None), None);
    }

    #[test]
    fn without_total_short_page_ends() {
        assert_eq!(next_offset(0, 50, 50, None), Some(50));
        assert_eq!(next_offset(50, 49, 50, None), None);
    }

    #[test]
    fn zero_or_negative_total_is_ignored() {
        // Provider returned total=0 (unknown) → behave as if absent.
        assert_eq!(next_offset(0, 100, 100, Some(0)), Some(100));
        assert_eq!(next_offset(0, 100, 100, Some(-5)), Some(100));
    }

    #[test]
    fn short_page_detection_matches_s187_gap_reporting() {
        assert!(is_short_page(98, 100, Some(500)));
        assert!(!is_short_page(100, 100, Some(500)));
        assert!(!is_short_page(98, 100, None), "no total → no gap verdict");
        assert!(!is_short_page(0, 100, Some(500)), "empty page ends, not a gap");
    }
}

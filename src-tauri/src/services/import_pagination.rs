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
///
/// If `current_offset + page_len >= provider_total`, the short page is simply
/// the natural end of the collection and NOT a gap.
pub fn is_short_page(
    current_offset: i32,
    page_len: i32,
    requested_limit: i32,
    provider_total: Option<i64>,
) -> bool {
    if page_len <= 0 || page_len >= requested_limit {
        return false;
    }
    match provider_total {
        Some(total) if total > 0 => (current_offset as i64 + page_len as i64) < total,
        _ => false,
    }
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
        // Gap mid-stream: offset 0 + 98 items < total 500
        assert!(is_short_page(0, 98, 100, Some(500)));
        // Full page: no gap
        assert!(!is_short_page(0, 100, 100, Some(500)));
        // Natural end of collection: offset 100 + 50 items == total 150 (NOT a gap)
        assert!(!is_short_page(100, 50, 100, Some(150)));
        // Overshoot/exact end: offset 100 + 50 items >= total 140 (NOT a gap)
        assert!(!is_short_page(100, 50, 100, Some(140)));
        // Premature short page before total: offset 100 + 30 items < total 150 (IS a gap)
        assert!(is_short_page(100, 30, 100, Some(150)));
        // No total: no gap verdict
        assert!(!is_short_page(0, 98, 100, None), "no total → no gap verdict");
        // Empty page: ends pagination, not a gap
        assert!(!is_short_page(0, 0, 100, Some(500)), "empty page ends, not a gap");
    }
}

/// S189-Fase-2: continuation decision for cursor-paginated endpoints
/// (Spotify `/me/following`). Mirrors the S187 semantics generalized in
/// [`next_offset`]: when the provider declares a total, trust it over page
/// length; otherwise a full page with a non-empty cursor continues and a
/// short page terminates.
pub fn next_cursor(
    after: Option<String>,
    page_len: usize,
    requested_limit: i32,
    declared_total: Option<i64>,
    imported_so_far: u64,
) -> Option<String> {
    let cursor = after?;
    if cursor.is_empty() {
        return None;
    }
    match declared_total {
        Some(total) if total > 0 => {
            if imported_so_far < total as u64 {
                Some(cursor)
            } else {
                None
            }
        }
        _ => {
            if page_len >= requested_limit.max(1) as usize {
                Some(cursor)
            } else {
                None
            }
        }
    }
}

#[cfg(test)]
mod cursor_tests {
    use super::*;

    #[test]
    fn test_next_cursor_full_page_with_cursor_continues() {
        assert_eq!(
            next_cursor(Some("abc".into()), 50, 50, None, 50),
            Some("abc".to_string())
        );
    }

    #[test]
    fn test_next_cursor_short_page_without_total_terminates() {
        assert_eq!(next_cursor(Some("abc".into()), 30, 50, None, 30), None);
    }

    #[test]
    fn test_next_cursor_declared_total_overrides_short_page() {
        // Total 120, imported 80 from two pages (50+30): keep going despite
        // the short page because the provider says more exist.
        assert_eq!(
            next_cursor(Some("xyz".into()), 30, 50, Some(120), 80),
            Some("xyz".to_string())
        );
    }

    #[test]
    fn test_next_cursor_stops_when_total_reached() {
        assert_eq!(next_cursor(Some("xyz".into()), 20, 50, Some(100), 100), None);
    }

    #[test]
    fn test_next_cursor_missing_or_empty_cursor_terminates() {
        assert_eq!(next_cursor(None, 50, 50, None, 50), None);
        assert_eq!(next_cursor(Some(String::new()), 50, 50, None, 50), None);
    }
}

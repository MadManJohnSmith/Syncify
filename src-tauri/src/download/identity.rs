//! Shared identity-agreement rules for provider tracks.
//!
//! Provider catalogues are not always trustworthy. Observed live: Qobuz
//! serves the *same* ISRC on two different recordings ("After Dark" by Mang
//! Dynasty and "Run River" by Mozez both carry `GB53R1200006`), and a locked
//! `service_track_id` can resolve to a live-but-wrong track. These rules are
//! the single source of truth shared by:
//!
//! * the S197 live ISRC resolution (`commands/queue.rs`), which must never
//!   lock a catalogue hit that the provider metadata itself contradicts, and
//! * the download-time verification of a locked source (`download/qobuz.rs`,
//!   `services/tidal_pipeline.rs`), which must never silently download a
//!   recording the request metadata contradicts.
//!
//! A contradiction requires evidence on BOTH axes (title AND duration). One
//! matching axis is enough to keep the historical tolerant behavior: editions
//! legitimately differ in title (remaster/feat./re-release suffixes) or in
//! catalog duration (padded compilations), never in both at once for the same
//! recording.

use syncify_core_domain::metadata::{artist_matches, title_matches};

/// Max |expected − candidate| duration delta (ms) that still counts as a
/// plausible same-recording duration. Mirrors the ±10 s rule already used by
/// every ISRC search in the codebase; keeps mastering/padding differences from
/// being mistaken for a different recording.
pub const IDENTITY_DURATION_TOLERANCE_MS: i64 = 10_000;

/// Lightweight provider-track metadata used to prove identity. Callers build it
/// from whatever the provider returned (search hit or `track/get`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IdentityCandidate {
    pub title: String,
    pub artist: String,
    pub duration_ms: i64,
}

impl IdentityCandidate {
    pub fn new(title: impl Into<String>, artist: impl Into<String>, duration_ms: i64) -> Self {
        Self {
            title: title.into(),
            artist: artist.into(),
            duration_ms,
        }
    }
}

/// True when the provider metadata *contradicts* the expected recording on
/// both title and duration. A contradiction is proof the `(service, track_id)`
/// does not belong to the requested recording; anything else (title matches,
/// duration matches, or not enough data to judge) is not a contradiction and
/// keeps the tolerant legacy behavior.
pub fn identity_conflicts(
    expected_title: &str,
    expected_duration_ms: i64,
    candidate: &IdentityCandidate,
) -> bool {
    let title_mismatch = !expected_title.trim().is_empty()
        && !candidate.title.trim().is_empty()
        && !title_matches(expected_title, &candidate.title);

    let durations_known = expected_duration_ms > 0 && candidate.duration_ms > 0;
    let duration_mismatch = durations_known
        && (candidate.duration_ms - expected_duration_ms).abs() > IDENTITY_DURATION_TOLERANCE_MS;

    title_mismatch && duration_mismatch
}

/// Evidence score of a candidate against the expected recording. Only used to
/// break ties between same-ISRC catalogue duplicates, never to invent identity:
/// `0` means "no positive proof", not "contradiction".
pub fn identity_score(
    expected_title: &str,
    expected_artist: &str,
    expected_duration_ms: i64,
    candidate: &IdentityCandidate,
) -> u32 {
    let mut score = 0u32;

    if !expected_title.trim().is_empty() && title_matches(expected_title, &candidate.title) {
        score += 3;
    }
    if !expected_artist.trim().is_empty() && artist_matches(expected_artist, &candidate.artist) {
        score += 1;
    }
    if expected_duration_ms > 0 && candidate.duration_ms > 0 {
        let diff = (candidate.duration_ms - expected_duration_ms).abs();
        if diff <= 3_000 {
            score += 2;
        } else if diff <= IDENTITY_DURATION_TOLERANCE_MS {
            score += 1;
        }
    }

    score
}

/// Pick the best same-ISRC catalogue hit for the expected recording.
///
/// * Candidates the provider metadata *contradicts* are never chosen.
/// * Among the rest, the highest evidence score wins; ties keep catalogue
///   order (deterministic).
/// * `None` means every candidate contradicts the expected recording — the
///   caller must not lock any of them (better no source than a wrong one).
pub fn pick_identity_candidate(
    expected_title: &str,
    expected_artist: &str,
    expected_duration_ms: i64,
    candidates: &[IdentityCandidate],
) -> Option<usize> {
    let mut best: Option<(usize, u32)> = None;
    for (idx, candidate) in candidates.iter().enumerate() {
        if identity_conflicts(expected_title, expected_duration_ms, candidate) {
            continue;
        }
        let score = identity_score(expected_title, expected_artist, expected_duration_ms, candidate);
        if best.map_or(true, |(_, best_score)| score > best_score) {
            best = Some((idx, score));
        }
    }
    best.map(|(idx, _)| idx)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Regression case #607: Qobuz labels the same ISRC on Mang Dynasty's
    /// "After Dark - Dub Mix" (367 s) and Mozez's "Run River" (459 s). The
    /// wrong one must be rejected and the right one picked.
    #[test]
    fn duplicate_isrc_picks_the_recording_that_agrees() {
        let run_river = IdentityCandidate::new("Run River", "Mozez", 459_023);
        let after_dark = IdentityCandidate::new("After Dark", "Mang Dynasty", 367_000);

        assert!(identity_conflicts(
            "After Dark - Dub Mix",
            367_827,
            &run_river
        ));
        assert!(!identity_conflicts(
            "After Dark - Dub Mix",
            367_827,
            &after_dark
        ));

        let picked = pick_identity_candidate(
            "After Dark - Dub Mix",
            "Mang Dynasty",
            367_827,
            &[run_river, after_dark],
        );
        assert_eq!(picked, Some(1), "the contradicted hit must be skipped");
    }

    /// Regression case #256: the same recording re-released under a different
    /// title ("41421421412" vs "FUNK BAD DO BAD"). Duration agrees, so it is
    /// NOT a conflict and must stay downloadable.
    #[test]
    fn retitled_rerelease_with_matching_duration_is_not_a_conflict() {
        let funk = IdentityCandidate::new("FUNK BAD DO BAD", "DJ VTTT77", 105_417);
        assert!(!identity_conflicts("41421421412", 105_460, &funk));
    }

    /// Regression case #114: a padded compilation stores 191 s while the real
    /// master is ~172 s. Title agrees, so it is NOT a conflict.
    #[test]
    fn padded_duration_with_matching_title_is_not_a_conflict() {
        let edition = IdentityCandidate::new(
            "(Your Love Keeps Lifting Me) Higher & Higher",
            "Jackie Wilson",
            172_100,
        );
        assert!(!identity_conflicts(
            "(Your Love Keeps Lifting Me) Higher And Higher",
            191_000,
            &edition
        ));
    }

    /// Without a known duration there is no second axis, so a title mismatch
    /// alone can never prove a conflict (back-compat with unknown durations).
    #[test]
    fn unknown_duration_never_conflicts_on_title_alone() {
        let candidate = IdentityCandidate::new("Some Other Title", "Other Artist", 0);
        assert!(!identity_conflicts("Expected Title", 0, &candidate));
        assert!(!identity_conflicts("Expected Title", 180_000, &candidate));
    }

    /// When every same-ISRC candidate contradicts the request, nothing may be
    /// picked (no source is preferable to a wrong one).
    #[test]
    fn all_contradicted_candidates_yield_none() {
        let wrong_a = IdentityCandidate::new("Run River", "Mozez", 459_023);
        let wrong_b = IdentityCandidate::new("Run River", "Mozez", 463_000);
        assert_eq!(
            pick_identity_candidate(
                "After Dark - Dub Mix",
                "Mang Dynasty",
                367_827,
                &[wrong_a, wrong_b]
            ),
            None
        );
    }

    /// Tie-break: with no contradicted candidates, the best evidence wins, and
    /// with no evidence at all the first catalogue hit is kept (legacy order).
    #[test]
    fn tie_break_prefers_evidence_then_catalogue_order() {
        let unknown_a = IdentityCandidate::new("Whatever A", "Someone", 0);
        let unknown_b = IdentityCandidate::new("Whatever B", "Someone", 0);
        assert_eq!(
            pick_identity_candidate("Target", "Artist", 0, &[unknown_a, unknown_b]),
            Some(0)
        );

        let weak = IdentityCandidate::new("Something Else", "Artist", 100_000);
        let strong = IdentityCandidate::new("Target", "Artist", 180_000);
        assert_eq!(
            pick_identity_candidate("Target", "Artist", 180_000, &[weak, strong]),
            Some(1)
        );
    }
}

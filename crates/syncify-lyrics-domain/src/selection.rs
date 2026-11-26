//! Deterministic sequential selection for lyrics resolution.
//!
//! The automatic cascade probes **Fuentes de letras** in user priority order.
//! It stops at the first candidate that reaches the target encoded by the
//! `min_sync_level` setting, keeping today's early exit. While the best result
//! is below that target, the cascade keeps probing so a line-synced result does
//! not hide a word-synced one further down the list; at exhaustion the best
//! candidate seen is returned.
//!
//! This module is pure: it never performs I/O and does not know about HTTP,
//! rate limits or caching. The caller turns each source probe into a
//! [`SourceOutcome`] and obeys [`SelectionOutcome`].

use crate::LyricsSyncType;

/// Coarse ordering of the sync levels a lyrics source can produce.
///
/// Word and syllable timings share the finest level: both are ELRC payloads and
/// selection does not distinguish them. `Instrumental` carries no timings and
/// ranks with plain text, as the previous policy did.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum SyncLevel {
    Plain,
    Line,
    Word,
}

impl SyncLevel {
    /// Level guaranteed by a resolved payload.
    pub fn from_sync_type(sync_type: &LyricsSyncType) -> Self {
        match sync_type {
            LyricsSyncType::KaraokeWordSynced => Self::Word,
            LyricsSyncType::LineSynced => Self::Line,
            LyricsSyncType::Plain | LyricsSyncType::Instrumental | LyricsSyncType::None => {
                Self::Plain
            }
        }
    }

    /// Target level encoded by the user's `min_sync_level` setting.
    ///
    /// Unknown values keep the historical behaviour: every value other than
    /// `word`/`syllable` and `line` targets plain lyrics.
    pub fn target_from_min_sync_level(min_sync_level: &str) -> Self {
        match min_sync_level {
            "syllable" | "word" => Self::Word,
            "line" => Self::Line,
            _ => Self::Plain,
        }
    }
}

/// Result of probing one source in the cascade.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SourceOutcome<T> {
    /// The source produced a usable candidate at `level`.
    Candidate { level: SyncLevel, value: T },
    /// The source produced nothing usable: not found, rejected by the language
    /// or validity policy, rate-limited, or failed. It never aborts the cascade.
    NoCandidate,
}

/// What the caller must do after offering a [`SourceOutcome`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SelectionOutcome {
    /// The target was reached (or the finest available level was): stop probing.
    Stop,
    /// A better candidate may still be below in priority order: probe the next
    /// source. When the list ends, [`SequentialSelection::finish`] returns the
    /// best candidate seen.
    Continue,
}

/// Whether a provider-native candidate (Tidal's own lyrics, LY14/#192) takes
/// the seat of the result the generic cascade accepted.
///
/// The native candidate is one more **Versión de letra**, never a blind
/// replacement: only a strictly finer [`SyncLevel`] wins. On a tie the generic
/// result keeps the seat, which is the source-priority tie-break the spec orders
/// after sync level and language. `None` means the generic cascade resolved
/// nothing, so any genuine native level improves on it.
pub fn native_candidate_wins(native: SyncLevel, generic: Option<SyncLevel>) -> bool {
    match generic {
        Some(level) => native > level,
        None => true,
    }
}

/// Stepwise selector for the automatic resolution cascade.
///
/// The selector owns only the ordering decision; the caller keeps fetching while
/// it answers [`SelectionOutcome::Continue`].
#[derive(Debug, Clone)]
pub struct SequentialSelection<T> {
    target: SyncLevel,
    best: Option<Selected<T>>,
}

#[derive(Debug, Clone)]
struct Selected<T> {
    level: SyncLevel,
    value: T,
}

impl<T> SequentialSelection<T> {
    pub fn new(min_sync_level: &str) -> Self {
        Self {
            target: SyncLevel::target_from_min_sync_level(min_sync_level),
            best: None,
        }
    }

    /// Offers the outcome of probing the next source in priority order.
    #[must_use = "ignoring the outcome would keep probing after reaching the target"]
    pub fn offer(&mut self, probe: SourceOutcome<T>) -> SelectionOutcome {
        let SourceOutcome::Candidate { level, value } = probe else {
            return SelectionOutcome::Continue;
        };

        if level >= self.target {
            self.best = Some(Selected { level, value });
            return SelectionOutcome::Stop;
        }

        if self.best.as_ref().is_none_or(|best| level > best.level) {
            self.best = Some(Selected { level, value });
        }
        SelectionOutcome::Continue
    }

    /// Highest level kept so far, if any.
    pub fn best_level(&self) -> Option<SyncLevel> {
        self.best.as_ref().map(|best| best.level)
    }

    /// Best candidate seen, whether or not it reached the target.
    pub fn finish(self) -> Option<T> {
        self.best.map(|best| best.value)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fixtures::{
        fixture_line_synced_resolution, fixture_plain_resolution, fixture_word_synced_resolution,
    };

    fn candidate(
        level: SyncLevel,
        value: crate::LyricsResolution,
    ) -> SourceOutcome<crate::LyricsResolution> {
        SourceOutcome::Candidate { level, value }
    }

    #[test]
    fn target_level_mapping_keeps_historical_policy() {
        assert_eq!(
            SyncLevel::target_from_min_sync_level("syllable"),
            SyncLevel::Word
        );
        assert_eq!(
            SyncLevel::target_from_min_sync_level("word"),
            SyncLevel::Word
        );
        assert_eq!(
            SyncLevel::target_from_min_sync_level("line"),
            SyncLevel::Line
        );
        assert_eq!(
            SyncLevel::target_from_min_sync_level("none"),
            SyncLevel::Plain
        );
        assert_eq!(SyncLevel::target_from_min_sync_level(""), SyncLevel::Plain);
        assert_eq!(
            SyncLevel::target_from_min_sync_level("unexpected"),
            SyncLevel::Plain
        );

        assert_eq!(
            SyncLevel::from_sync_type(&LyricsSyncType::KaraokeWordSynced),
            SyncLevel::Word
        );
        assert_eq!(
            SyncLevel::from_sync_type(&LyricsSyncType::LineSynced),
            SyncLevel::Line
        );
        assert_eq!(
            SyncLevel::from_sync_type(&LyricsSyncType::Plain),
            SyncLevel::Plain
        );
        assert_eq!(
            SyncLevel::from_sync_type(&LyricsSyncType::Instrumental),
            SyncLevel::Plain
        );
    }

    #[test]
    fn improvement_prefers_word_over_earlier_line() {
        let mut selection = SequentialSelection::new("word");

        assert_eq!(
            selection.offer(candidate(
                SyncLevel::Line,
                fixture_line_synced_resolution("LRCLIB")
            )),
            SelectionOutcome::Continue
        );
        assert_eq!(selection.best_level(), Some(SyncLevel::Line));

        assert_eq!(
            selection.offer(candidate(
                SyncLevel::Word,
                fixture_word_synced_resolution("Spotify")
            )),
            SelectionOutcome::Stop
        );

        let winner = selection.finish().expect("word result must be kept");
        assert_eq!(winner.provider, "Spotify");
        assert_eq!(winner.sync_type, LyricsSyncType::KaraokeWordSynced);
    }

    #[test]
    fn target_met_stops_without_probing_more_sources() {
        let probes = [
            candidate(
                SyncLevel::Word,
                fixture_word_synced_resolution("Apple Music"),
            ),
            candidate(SyncLevel::Line, fixture_line_synced_resolution("LRCLIB")),
        ];
        let mut selection = SequentialSelection::new("word");
        let mut probed = 0;

        for probe in probes {
            probed += 1;
            if selection.offer(probe) == SelectionOutcome::Stop {
                break;
            }
        }

        assert_eq!(probed, 1, "a target-reaching result must not probe further");
        let winner = selection.finish().expect("target-reaching result kept");
        assert_eq!(winner.provider, "Apple Music");
    }

    #[test]
    fn exhaustion_keeps_the_highest_level_seen() {
        let mut selection = SequentialSelection::new("word");

        assert_eq!(
            selection.offer(candidate(
                SyncLevel::Plain,
                fixture_plain_resolution("Tekstowo")
            )),
            SelectionOutcome::Continue
        );
        assert_eq!(
            selection.offer(candidate(
                SyncLevel::Line,
                fixture_line_synced_resolution("LRCLIB")
            )),
            SelectionOutcome::Continue
        );
        assert_eq!(
            selection.offer(candidate(
                SyncLevel::Plain,
                fixture_plain_resolution("Musixmatch")
            )),
            SelectionOutcome::Continue,
            "a lower result never replaces the best one"
        );

        assert_eq!(selection.best_level(), Some(SyncLevel::Line));
        let winner = selection.finish().expect("best below target is kept");
        assert_eq!(winner.provider, "LRCLIB");
        assert_eq!(winner.sync_type, LyricsSyncType::LineSynced);
    }

    #[test]
    fn failed_probes_do_not_abort_the_cascade() {
        let mut selection = SequentialSelection::new("word");

        for _ in 0..3 {
            assert_eq!(
                selection.offer(SourceOutcome::NoCandidate),
                SelectionOutcome::Continue
            );
        }

        assert_eq!(
            selection.offer(candidate(
                SyncLevel::Line,
                fixture_line_synced_resolution("LRCLIB")
            )),
            SelectionOutcome::Continue
        );
        assert_eq!(
            selection.offer(candidate(
                SyncLevel::Word,
                fixture_word_synced_resolution("Spotify")
            )),
            SelectionOutcome::Stop
        );

        assert_eq!(
            selection
                .finish()
                .expect("word after failures wins")
                .provider,
            "Spotify"
        );
    }

    #[test]
    fn all_probes_failing_yields_no_candidate() {
        let mut selection: SequentialSelection<crate::LyricsResolution> =
            SequentialSelection::new("word");
        for _ in 0..4 {
            assert_eq!(
                selection.offer(SourceOutcome::NoCandidate),
                SelectionOutcome::Continue
            );
        }
        assert_eq!(selection.best_level(), None);
        assert!(selection.finish().is_none());
    }

    #[test]
    fn native_candidate_wins_only_with_a_finer_level() {
        assert!(native_candidate_wins(
            SyncLevel::Word,
            Some(SyncLevel::Line)
        ));
        assert!(native_candidate_wins(
            SyncLevel::Line,
            Some(SyncLevel::Plain)
        ));
        assert!(
            native_candidate_wins(SyncLevel::Word, None),
            "a native payload improves on no generic result"
        );
        assert!(
            native_candidate_wins(SyncLevel::Plain, None),
            "any genuine native level beats an unresolved cascade"
        );

        assert!(
            !native_candidate_wins(SyncLevel::Line, Some(SyncLevel::Line)),
            "a tie keeps the generic source (source priority)"
        );
        assert!(
            !native_candidate_wins(SyncLevel::Plain, Some(SyncLevel::Line)),
            "a native payload never degrades the accepted level"
        );
    }
}

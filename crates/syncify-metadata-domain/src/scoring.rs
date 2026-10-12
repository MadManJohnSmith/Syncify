//! Metadata Quality Scoring Domain Module.
//!
//! Provides deterministic metadata quality score calculations (0-100)
//! based on user-configured weights for both streaming downloads and
//! local library tracks.

use serde::{Deserialize, Serialize};

/// Configurable quality scoring weights matching `metadata_preferences` schema.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ScoringWeights {
    pub weight_album: i64,
    pub weight_isrc: i64,
    pub weight_mb_id: i64,
    pub weight_cover: i64,
    pub weight_year: i64,
    pub weight_genre: i64,
}

impl Default for ScoringWeights {
    fn default() -> Self {
        Self {
            weight_album: 1,
            weight_isrc: 1,
            weight_mb_id: 1,
            weight_cover: 1,
            weight_year: 1,
            weight_genre: 1,
        }
    }
}

/// Input metadata payload for scoring calculation.
/// Supports both streaming downloads and local files.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct TrackMetadataScoreInput {
    pub title: Option<String>,
    pub artist: Option<String>,
    pub album: Option<String>,
    pub isrc: Option<String>,
    pub musicbrainz_id: Option<String>,
    pub cover_art_url: Option<String>,
    pub release_year: Option<i32>,
    pub genre: Option<String>,
    /// Optional physical file path for local library files (SC-03 parity).
    pub file_path: Option<String>,
    /// Explicit flag if track originates from local library scan.
    pub is_local: bool,
}

impl TrackMetadataScoreInput {
    /// Helper to construct a local track input.
    pub fn new_local(
        title: Option<String>,
        artist: Option<String>,
        album: Option<String>,
        file_path: Option<String>,
    ) -> Self {
        Self {
            title,
            artist,
            album,
            file_path,
            is_local: true,
            ..Default::default()
        }
    }

    /// Helper to construct a streaming track input.
    pub fn new_streaming(
        title: Option<String>,
        artist: Option<String>,
        album: Option<String>,
    ) -> Self {
        Self {
            title,
            artist,
            album,
            is_local: false,
            ..Default::default()
        }
    }
}

/// Calculate a deterministic 0-100 metadata quality score.
///
/// Guaranteed invariants:
/// 1. Base essential fields (Title and Artist) constitute 20% (10% each) of the score.
/// 2. Configurable fields (Album, ISRC, MusicBrainz ID, Cover Art, Release Year, Genre)
///    distribute the remaining 80% proportionally according to the provided `ScoringWeights`.
/// 3. If all weights are 0, presence of title and artist scales up to 100%.
/// 4. Local library tracks (`is_local` or with `file_path`) use the EXACT same scoring engine
///    and weights as downloaded streaming tracks, satisfying SC-03.
/// 5. Sentinel MusicBrainz IDs (`NOT_FOUND`, `MISMATCH`, UUID-NIL) never grant the
///    `musicbrainz_id` points, matching the SQL readers in `library.rs`.
pub fn calculate_metadata_score(weights: &ScoringWeights, track: &TrackMetadataScoreInput) -> u32 {
    let has_title = track
        .title
        .as_deref()
        .map(|s| !s.trim().is_empty())
        .unwrap_or(false);

    let has_artist = track
        .artist
        .as_deref()
        .map(|s| !s.trim().is_empty())
        .unwrap_or(false);

    let has_album = track
        .album
        .as_deref()
        .map(|s| !s.trim().is_empty())
        .unwrap_or(false);

    let has_isrc = track
        .isrc
        .as_deref()
        .map(|s| !s.trim().is_empty())
        .unwrap_or(false);

    let has_mb_id = track
        .musicbrainz_id
        .as_deref()
        .map(|s| {
            let t = s.trim();
            !t.is_empty()
                && t != "NOT_FOUND"
                && t != "MISMATCH"
                && t != "00000000-0000-0000-0000-000000000000"
        })
        .unwrap_or(false);

    let has_cover = track
        .cover_art_url
        .as_deref()
        .map(|s| !s.trim().is_empty())
        .unwrap_or(false);

    let has_year = track.release_year.map(|y| y > 0).unwrap_or(false);

    let has_genre = track
        .genre
        .as_deref()
        .map(|s| !s.trim().is_empty())
        .unwrap_or(false);

    let w_album = weights.weight_album.max(0);
    let w_isrc = weights.weight_isrc.max(0);
    let w_mb_id = weights.weight_mb_id.max(0);
    let w_cover = weights.weight_cover.max(0);
    let w_year = weights.weight_year.max(0);
    let w_genre = weights.weight_genre.max(0);

    let total_configurable_weight = w_album + w_isrc + w_mb_id + w_cover + w_year + w_genre;

    // Base score for title and artist (10 points each)
    let mut base_score: f64 = 0.0;
    if has_title {
        base_score += 10.0;
    }
    if has_artist {
        base_score += 10.0;
    }

    if total_configurable_weight == 0 {
        // If no configurable weights are specified, score is solely based on title + artist
        return if has_title && has_artist {
            100
        } else if has_title || has_artist {
            50
        } else {
            0
        };
    }

    let configurable_pool = 80.0;
    let mut weighted_earned = 0.0;

    if has_album {
        weighted_earned += w_album as f64;
    }
    if has_isrc {
        weighted_earned += w_isrc as f64;
    }
    if has_mb_id {
        weighted_earned += w_mb_id as f64;
    }
    if has_cover {
        weighted_earned += w_cover as f64;
    }
    if has_year {
        weighted_earned += w_year as f64;
    }
    if has_genre {
        weighted_earned += w_genre as f64;
    }

    let configurable_score =
        (weighted_earned / total_configurable_weight as f64) * configurable_pool;
    let final_score = (base_score + configurable_score).round();

    final_score.clamp(0.0, 100.0) as u32
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_perfect_metadata_scores_100() {
        let weights = ScoringWeights::default();
        let track = TrackMetadataScoreInput {
            title: Some("Song Title".to_string()),
            artist: Some("Great Artist".to_string()),
            album: Some("Great Album".to_string()),
            isrc: Some("USRC12345678".to_string()),
            musicbrainz_id: Some("12345678-1234-1234-1234-123456789abc".to_string()),
            cover_art_url: Some("https://example.com/cover.jpg".to_string()),
            release_year: Some(2024),
            genre: Some("Rock".to_string()),
            file_path: None,
            is_local: false,
        };

        assert_eq!(calculate_metadata_score(&weights, &track), 100);
    }

    #[test]
    fn test_empty_metadata_scores_0() {
        let weights = ScoringWeights::default();
        let track = TrackMetadataScoreInput::default();
        assert_eq!(calculate_metadata_score(&weights, &track), 0);
    }

    #[test]
    fn test_local_track_parity_with_downloaded_track_sc_03() {
        let weights = ScoringWeights::default();
        let downloaded = TrackMetadataScoreInput {
            title: Some("Track 1".to_string()),
            artist: Some("Artist 1".to_string()),
            album: Some("Album 1".to_string()),
            isrc: Some("USRC12345678".to_string()),
            musicbrainz_id: None,
            cover_art_url: Some("cover.jpg".to_string()),
            release_year: Some(2020),
            genre: None,
            file_path: None,
            is_local: false,
        };

        let local = TrackMetadataScoreInput {
            title: Some("Track 1".to_string()),
            artist: Some("Artist 1".to_string()),
            album: Some("Album 1".to_string()),
            isrc: Some("USRC12345678".to_string()),
            musicbrainz_id: None,
            cover_art_url: Some("cover.jpg".to_string()),
            release_year: Some(2020),
            genre: None,
            file_path: Some("/music/local/Track 1.flac".to_string()),
            is_local: true,
        };

        let score_downloaded = calculate_metadata_score(&weights, &downloaded);
        let score_local = calculate_metadata_score(&weights, &local);

        assert_eq!(score_downloaded, score_local);
        assert!(score_downloaded > 0 && score_downloaded < 100);
    }

    #[test]
    fn test_custom_weights_affect_score_sc_02() {
        let track = TrackMetadataScoreInput {
            title: Some("Song".to_string()),
            artist: Some("Artist".to_string()),
            album: Some("Album".to_string()),
            isrc: None,
            musicbrainz_id: None,
            cover_art_url: None,
            release_year: None,
            genre: None,
            file_path: None,
            is_local: false,
        };

        // If album weight is high relative to others
        let weights_album_heavy = ScoringWeights {
            weight_album: 10,
            weight_isrc: 1,
            weight_mb_id: 1,
            weight_cover: 1,
            weight_year: 1,
            weight_genre: 1,
        };

        // If album weight is low relative to others
        let weights_album_light = ScoringWeights {
            weight_album: 1,
            weight_isrc: 10,
            weight_mb_id: 10,
            weight_cover: 10,
            weight_year: 10,
            weight_genre: 10,
        };

        let score_heavy = calculate_metadata_score(&weights_album_heavy, &track);
        let score_light = calculate_metadata_score(&weights_album_light, &track);

        assert!(score_heavy > score_light);
    }

    #[test]
    fn test_not_found_musicbrainz_id_not_counted() {
        let weights = ScoringWeights::default();
        let track_not_found = TrackMetadataScoreInput {
            title: Some("Song".to_string()),
            artist: Some("Artist".to_string()),
            musicbrainz_id: Some("NOT_FOUND".to_string()),
            ..Default::default()
        };

        let track_no_mb = TrackMetadataScoreInput {
            title: Some("Song".to_string()),
            artist: Some("Artist".to_string()),
            musicbrainz_id: None,
            ..Default::default()
        };

        assert_eq!(
            calculate_metadata_score(&weights, &track_not_found),
            calculate_metadata_score(&weights, &track_no_mb)
        );
    }

    #[test]
    fn test_mismatch_musicbrainz_id_not_counted_with_nonuniform_weights() {
        let weights = ScoringWeights {
            weight_mb_id: 9,
            weight_album: 1,
            weight_isrc: 2,
            weight_cover: 3,
            weight_year: 4,
            weight_genre: 5,
        };
        let mut track = TrackMetadataScoreInput {
            title: Some("Song".to_string()),
            artist: Some("Artist".to_string()),
            album: Some("Album".to_string()),
            ..Default::default()
        };
        let without_mb = calculate_metadata_score(&weights, &track);
        assert_eq!(without_mb, 23); // 20 base + round(80 * 1/24)

        for sentinel in [
            "MISMATCH",
            " MISMATCH ",
            "NOT_FOUND",
            "00000000-0000-0000-0000-000000000000",
        ] {
            track.musicbrainz_id = Some(sentinel.to_string());
            assert_eq!(
                calculate_metadata_score(&weights, &track),
                without_mb,
                "sentinel {sentinel} must not earn MBID points"
            );
        }

        track.musicbrainz_id = Some("12345678-1234-1234-1234-123456789abc".to_string());
        assert_eq!(calculate_metadata_score(&weights, &track), 53); // 20 + round(80 * 10/24)
    }
}

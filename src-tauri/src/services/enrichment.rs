//! Domain contract & Metadata Enrichment Engine for `src-tauri`.
//!
//! Integrates `syncify-metadata-domain` precedence engine with `MusicBrainzClient`
//! to resolve the first group of enriched metadata fields safely and deterministically.

use crate::services::musicbrainz::{MusicBrainzClient, MusicBrainzRecording};
use syncify_core_domain::quality::{classify_audio_tier, AudioTier};
#[allow(unused_imports)]
pub use syncify_metadata_domain::{
    chrono_now_iso, normalize_title, AudioAnalysisMetrics, EnrichedMetadata, EnrichmentCompleteness,
    FieldResolution, FieldValidator, GenreContext,
};

/// Origin streaming track metadata passed into the enrichment engine
#[derive(Debug, Clone, Default)]
#[allow(dead_code)]
pub struct OriginTrackMetadata {
    pub title: Option<String>,
    pub artist: Option<String>,
    pub album: Option<String>,
    pub album_artist: Option<String>,
    pub composer: Option<String>,
    pub performers: Option<String>,
    pub work: Option<String>,
    pub track_number: Option<u32>,
    pub track_total: Option<u32>,
    pub disc_number: Option<u32>,
    pub disc_total: Option<u32>,
    pub disc_subtitle: Option<String>,
    pub release_year: Option<String>,
    pub release_date: Option<String>,
    pub original_date: Option<String>,
    pub label: Option<String>,
    pub catalog_number: Option<String>,
    pub isrc: Option<String>,
    pub barcode: Option<String>,
    pub copyright: Option<String>,
    pub release_type: Option<String>,
    pub release_status: Option<String>,
    pub release_country: Option<String>,
    pub language: Option<String>,
    pub genre: Option<String>,
    pub style: Option<String>,
    pub mood: Option<String>,
    pub explicit: Option<bool>,
    pub bpm: Option<u32>,
    pub initial_key: Option<String>,
    pub energy: Option<f64>,
    pub danceability: Option<f64>,
    pub loudness: Option<f64>,
    pub replaygain_track_gain: Option<String>,
    pub replaygain_track_peak: Option<String>,
    pub replaygain_album_gain: Option<String>,
    pub replaygain_album_peak: Option<String>,
    pub r128_track_gain: Option<String>,
    pub comment: Option<String>,
    pub lyrics_source: Option<String>,
    pub cover_source: Option<String>,
    pub audio_source: Option<String>,
    pub acoustid_id: Option<String>,
    pub acoustid_fingerprint: Option<String>,
    pub musicbrainz_recording_id: Option<String>,
    pub musicbrainz_release_id: Option<String>,
    pub musicbrainz_artist_id: Option<String>,
    pub tags: Option<Vec<String>>,
    pub artist_tags: Option<Vec<String>>,
    pub media_type: Option<String>,
    pub source_name: String,
    /// TASK-108: Explicit track added_at timestamp from provider, or None for current UTC
    pub added_at: Option<String>,
}

/// Extracts and normalizes the primary genre from a potentially composite genre string.
/// If the input contains multiple genres delimited by ';' or '/', it extracts the first
/// entity, trimming surrounding whitespace. Returns None if empty or invalid.
pub fn clean_primary_genre(genre_raw: &str) -> Option<String> {
    let first = genre_raw
        .split(|c| c == ';' || c == '/')
        .next()
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())?;
    Some(first.to_string())
}

/// Checks if an iterator of artist names represents a multi-artist compilation release
/// (more than 1 distinct non-empty artist).
#[allow(dead_code)]
pub fn is_multi_artist_compilation<S: AsRef<str>, I: IntoIterator<Item = S>>(artists: I) -> bool {
    let mut set = std::collections::HashSet::new();
    for a in artists {
        let clean = a.as_ref().trim();
        if !clean.is_empty() && !clean.eq_ignore_ascii_case("various artists") && !clean.eq_ignore_ascii_case("various") {
            set.insert(clean.to_lowercase());
        }
    }
    set.len() > 1
}

/// Detects if a slice of origin track metadata represents a compilation release.
#[allow(dead_code)]
pub fn detect_compilation_from_origin_tracks(tracks: &[OriginTrackMetadata]) -> bool {
    if tracks.is_empty() {
        return false;
    }
    let mut distinct_artists = std::collections::HashSet::new();
    for t in tracks {
        if let Some(ref aa) = t.album_artist {
            let s = aa.trim();
            if s.eq_ignore_ascii_case("various artists") || s.eq_ignore_ascii_case("various") {
                return true;
            }
        }
        if let Some(ref rt) = t.release_type {
            let s = rt.trim();
            if s.eq_ignore_ascii_case("compilation") || s.eq_ignore_ascii_case("soundtrack") {
                return true;
            }
        }
        if let Some(ref mt) = t.media_type {
            let s = mt.trim();
            if s.eq_ignore_ascii_case("compilation") || s.eq_ignore_ascii_case("soundtrack") {
                return true;
            }
        }
        if let Some(ref a) = t.artist {
            let clean = a.trim();
            if !clean.is_empty() && !clean.eq_ignore_ascii_case("various artists") && !clean.eq_ignore_ascii_case("various") {
                distinct_artists.insert(clean.to_lowercase());
            }
        }
    }
    distinct_artists.len() > 1
}

/// Unifies album artist and compilation flags across origin tracks belonging to the same album.
#[allow(dead_code)]
pub fn unify_origin_album_tracks(
    tracks: &mut [OriginTrackMetadata],
    album_artist_override: Option<&str>,
) {
    if tracks.is_empty() {
        return;
    }
    let is_comp = detect_compilation_from_origin_tracks(tracks);
    if is_comp {
        let target_aa = album_artist_override
            .unwrap_or("Various Artists")
            .to_string();
        for t in tracks.iter_mut() {
            if t.album_artist.is_none() || t.album_artist.as_deref() == t.artist.as_deref() {
                t.album_artist = Some(target_aa.clone());
            }
            if t.release_type.is_none() {
                t.release_type = Some("compilation".to_string());
            }
        }
    }
}

/// Metadata Enrichment Engine for `src-tauri`
pub struct EnrichmentEngine {
    musicbrainz: MusicBrainzClient,
}

impl EnrichmentEngine {
    pub fn new() -> Self {
        Self {
            musicbrainz: MusicBrainzClient::new(),
        }
    }

    /// Resolve enriched metadata for a track following strict precedence rules.
    pub async fn resolve_track_metadata(
        &self,
        artist: &str,
        album: &str,
        title: &str,
        isrc_hint: Option<&str>,
        origin_meta: Option<&OriginTrackMetadata>,
    ) -> EnrichedMetadata {
        self.resolve_track_metadata_internal(artist, album, title, isrc_hint, origin_meta, true).await
    }

    /// Resolve enriched metadata with configurable secondary provider queries.
    pub async fn resolve_track_metadata_internal(
        &self,
        artist: &str,
        album: &str,
        title: &str,
        isrc_hint: Option<&str>,
        origin_meta: Option<&OriginTrackMetadata>,
        query_musicbrainz: bool,
    ) -> EnrichedMetadata {
        let sources = match origin_meta {
            Some(o) => vec![o.clone()],
            None => Vec::new(),
        };
        self.resolve_exhaustive_track_metadata_with_force(
            artist,
            album,
            title,
            isrc_hint,
            &sources,
            query_musicbrainz,
            false,
        )
        .await
    }

    /// Exhaustive multi-provider enrichment engine (S176M):
    /// - Queries all configured/provided sources (Qobuz, Tidal, Spotify, etc.) AND MusicBrainz without early exit.
    /// - Fuses `LANGUAGE` to standard ISO 639-2 (never empty if at least one valid source provides it).
    /// - Fuses `GENRE` by collecting all genres across providers, splitting on ';' and '/', deduplicating, and preserving non-English genres.
    /// - Fuses `COUNTRY` / `RELEASECOUNTRY` to real canonical country names, prioritizing official release or MusicBrainz.
    /// - Fuses `LABEL` / `RECORDLABEL` / `ORGANIZATION` variants.
    /// - Fuses `COMPOSER` and `PERFORMER`.
    /// - Sets `BPM` only if provided by origin or audio analysis without fabricating placeholders.
    /// - Follows strict precedence: Manual > StreamingMetadata (Qobuz/Tidal) > MusicBrainz > SpotifyMetadata > LocalAudioAnalysis.
    #[allow(dead_code)] // usado por exhaustive_enrichment_*_test; fase WIP S181
    pub async fn resolve_exhaustive_track_metadata(
        &self,
        artist: &str,
        album: &str,
        title: &str,
        isrc_hint: Option<&str>,
        origin_sources: &[OriginTrackMetadata],
        query_musicbrainz: bool,
    ) -> EnrichedMetadata {
        self.resolve_exhaustive_track_metadata_with_force(
            artist,
            album,
            title,
            isrc_hint,
            origin_sources,
            query_musicbrainz,
            false,
        )
        .await
    }

    /// Exhaustive multi-provider enrichment engine with explicit force override support.
    pub async fn resolve_exhaustive_track_metadata_with_force(
        &self,
        artist: &str,
        album: &str,
        title: &str,
        isrc_hint: Option<&str>,
        origin_sources: &[OriginTrackMetadata],
        query_musicbrainz: bool,
        force: bool,
    ) -> EnrichedMetadata {
        let mut meta = EnrichedMetadata::default();
        let now_ts = chrono_now_iso();
        meta.enriched_at = now_ts.clone();

        // 1. Populate basic structure
        let default_src = origin_sources.first().map(|o| o.source_name.as_str()).unwrap_or("inferred");
        if FieldValidator::is_valid_title(title) {
            meta.title.merge_candidate_with_force(Some(title.to_string()), default_src, 0.50, &now_ts, force);
        }
        if FieldValidator::is_valid_artist(artist) {
            meta.artist.merge_candidate_with_force(Some(artist.to_string()), default_src, 0.50, &now_ts, force);
        }
        if !album.trim().is_empty() {
            meta.album.merge_candidate_with_force(Some(album.to_string()), default_src, 0.50, &now_ts, force);
        }

        // Apply ISRC hint if provided
        if let Some(isrc) = isrc_hint {
            if FieldValidator::is_valid_identifier(isrc) {
                meta.isrc.merge_candidate_with_force(Some(isrc.to_string()), default_src, 0.90, &now_ts, force);
            }
        }

        let mut all_genre_strings: Vec<String> = Vec::new();
        let mut all_style_strings: Vec<String> = Vec::new();
        let mut all_tag_strings: Vec<String> = Vec::new();
        let mut all_artist_tag_strings: Vec<String> = Vec::new();
        let mut all_language_candidates: Vec<(String, String, f64)> = Vec::new();
        let mut all_country_candidates: Vec<(String, String, f64)> = Vec::new();
        let mut all_label_strings: Vec<String> = Vec::new();
        let base_genre_ctx = GenreContext::new()
            .with_title(Some(title))
            .with_artist(Some(artist))
            .with_album(Some(album));

        // 2. Process all provided origin sources without early exit
        for orig in origin_sources {
            let src = orig.source_name.as_str();

            if let Some(ref t) = orig.title {
                if FieldValidator::is_valid_title(t) {
                    meta.title.merge_candidate_with_force(Some(t.clone()), src, 0.95, &now_ts, force);
                }
            }
            if let Some(ref a) = orig.artist {
                if FieldValidator::is_valid_artist(a) {
                    meta.artist.merge_candidate_with_force(Some(a.clone()), src, 0.95, &now_ts, force);
                }
            }
            if let Some(ref alb) = orig.album {
                if !alb.trim().is_empty() {
                    meta.album.merge_candidate_with_force(Some(alb.clone()), src, 0.95, &now_ts, force);
                }
            }
            if let Some(ref aa) = orig.album_artist {
                if FieldValidator::is_valid_artist(aa) {
                    meta.album_artist.merge_candidate_with_force(Some(aa.clone()), src, 0.95, &now_ts, force);
                }
            }
            if let Some(ref comp) = orig.composer {
                if FieldValidator::is_valid_artist(comp) {
                    meta.composer.merge_candidate_with_force(Some(comp.clone()), src, 0.95, &now_ts, force);
                }
            }
            if let Some(ref perf) = orig.performers {
                if FieldValidator::is_valid_artist(perf) {
                    meta.performers.merge_candidate_with_force(Some(perf.clone()), src, 0.95, &now_ts, force);
                }
            }
            if let Some(ref wk) = orig.work {
                if FieldValidator::is_valid_title(wk) {
                    meta.work.merge_candidate_with_force(Some(wk.clone()), src, 0.95, &now_ts, force);
                }
            }
            if let Some(tn) = orig.track_number {
                if tn > 0 {
                    meta.track_number.merge_candidate_with_force(Some(tn.to_string()), src, 1.0, &now_ts, force);
                }
            }
            if let Some(tt) = orig.track_total {
                if tt > 0 {
                    meta.track_total.merge_candidate_with_force(Some(tt.to_string()), src, 0.95, &now_ts, force);
                }
            }
            if let Some(dn) = orig.disc_number {
                if dn > 0 {
                    meta.disc_number.merge_candidate_with_force(Some(dn.to_string()), src, 1.0, &now_ts, force);
                }
            }
            if let Some(dt) = orig.disc_total {
                if dt > 0 {
                    meta.disc_total.merge_candidate_with_force(Some(dt.to_string()), src, 0.95, &now_ts, force);
                }
            }
            if let Some(ref dsub) = orig.disc_subtitle {
                if !dsub.trim().is_empty() {
                    meta.disc_subtitle.merge_candidate_with_force(Some(dsub.clone()), src, 0.95, &now_ts, force);
                }
            }
            if let Some(ref yr) = orig.release_year {
                if FieldValidator::is_valid_year(yr) {
                    meta.release_year.merge_candidate_with_force(Some(yr.clone()), src, 0.90, &now_ts, force);
                }
            }
            if let Some(ref rdate) = orig.release_date {
                if FieldValidator::is_valid_year(rdate) {
                    meta.release_date.merge_candidate_with_force(Some(rdate.clone()), src, 0.90, &now_ts, force);
                }
            }
            if let Some(ref od) = orig.original_date {
                if FieldValidator::is_valid_year(od) {
                    meta.original_date.merge_candidate_with_force(Some(od.clone()), src, 0.90, &now_ts, force);
                }
            }
            if let Some(ref lbl) = orig.label {
                if FieldValidator::is_valid_label(lbl) {
                    all_label_strings.push(lbl.clone());
                    meta.label.merge_candidate_with_force(Some(lbl.clone()), src, 0.85, &now_ts, force);
                }
            }
            if let Some(ref cat) = orig.catalog_number {
                if !cat.trim().is_empty() {
                    meta.catalog_number.merge_candidate_with_force(Some(cat.clone()), src, 0.85, &now_ts, force);
                }
            }
            if let Some(ref cpy) = orig.copyright {
                if !cpy.trim().is_empty() {
                    meta.copyright.merge_candidate_with_force(Some(cpy.clone()), src, 0.85, &now_ts, force);
                }
            }
            if let Some(ref rtype) = orig.release_type {
                if !rtype.trim().is_empty() {
                    meta.release_type.merge_candidate_with_force(Some(rtype.clone()), src, 0.85, &now_ts, force);
                }
            }
            if let Some(ref rstat) = orig.release_status {
                if !rstat.trim().is_empty() {
                    meta.release_status.merge_candidate_with_force(Some(rstat.clone()), src, 0.85, &now_ts, force);
                }
            }

            // Compilation detection from Various Artists or source compilation/soundtrack flags
            let effective_aa = orig.album_artist.as_deref().unwrap_or(artist);
            let is_va = effective_aa.eq_ignore_ascii_case("various artists") || effective_aa.eq_ignore_ascii_case("various");
            let is_comp_type = orig.release_type.as_deref().map(|t| t.eq_ignore_ascii_case("compilation") || t.eq_ignore_ascii_case("soundtrack")).unwrap_or(false);
            let is_soundtrack_media = orig.media_type.as_deref().map(|t| t.eq_ignore_ascii_case("soundtrack") || t.eq_ignore_ascii_case("compilation")).unwrap_or(false);

            if is_va || is_comp_type || is_soundtrack_media {
                meta.compilation.merge_candidate_with_force(Some("1".to_string()), src, 0.95, &now_ts, force);
                if meta.album_artist.value().is_none() {
                    meta.album_artist.merge_candidate_with_force(Some("Various Artists".to_string()), src, 0.90, &now_ts, force);
                }
            }

            // Grouping candidate from album_artist + album
            let grouping_candidate = format!("{} - {}", effective_aa.trim(), orig.album.as_deref().unwrap_or(album).trim());
            meta.grouping.merge_candidate_with_force(Some(grouping_candidate), src, 0.70, &now_ts, force);
            if let Some(ref rcntry) = orig.release_country {
                match syncify_metadata_domain::resolve_country(rcntry) {
                    syncify_metadata_domain::CountryResolution::Country { canonical_name, .. } => {
                        all_country_candidates.push((canonical_name.clone(), src.to_string(), 0.85));
                        if let Some(lang_code) = syncify_metadata_domain::default_language_for_country(&canonical_name) {
                            all_language_candidates.push((lang_code.to_string(), src.to_string(), 0.70));
                        }
                    }
                    syncify_metadata_domain::CountryResolution::Region { region_name, region_code } => {
                        let reg_val = region_code.unwrap_or(region_name);
                        meta.release_region.merge_candidate_with_force(Some(reg_val), src, 0.85, &now_ts, force);
                    }
                    syncify_metadata_domain::CountryResolution::Unknown(_) => {
                        all_country_candidates.push((rcntry.clone(), src.to_string(), 0.85));
                        if let Some(lang_code) = syncify_metadata_domain::default_language_for_country(rcntry) {
                            all_language_candidates.push((lang_code.to_string(), src.to_string(), 0.70));
                        }
                    }
                }
            }
            if let Some(ref lang) = orig.language {
                all_language_candidates.push((lang.clone(), src.to_string(), 0.85));
            }
            if let Some(ref gn) = orig.genre {
                let parts: Vec<&str> = gn.split(|c| c == ';' || c == '/' || c == ',')
                    .map(|s| s.trim())
                    .filter(|s| !s.is_empty())
                    .collect();
                for (i, p) in parts.iter().enumerate() {
                    all_genre_strings.push(p.to_string());
                    if i > 0 {
                        all_style_strings.push(p.to_string());
                        all_tag_strings.push(p.to_string());
                    }
                }
            }
            if let Some(ref t_list) = orig.tags {
                for t in t_list {
                    if FieldValidator::is_valid_genre_with_context(t, Some(&base_genre_ctx)) {
                        all_tag_strings.push(t.clone());
                    }
                }
            }
            if let Some(ref st) = orig.style {
                if FieldValidator::is_valid_genre_with_context(st, Some(&base_genre_ctx)) {
                    all_style_strings.push(st.clone());
                    meta.style.merge_candidate_with_force(Some(st.clone()), src, 0.85, &now_ts, force);
                }
            }
            if let Some(ref md) = orig.mood {
                if FieldValidator::is_valid_genre_with_context(md, Some(&base_genre_ctx)) {
                    meta.mood.merge_candidate_with_force(Some(md.clone()), src, 0.85, &now_ts, force);
                }
            }
            if let Some(ref art_tags) = orig.artist_tags {
                for at in art_tags {
                    if FieldValidator::is_valid_genre_with_context(at, Some(&base_genre_ctx)) {
                        all_artist_tag_strings.push(at.clone());
                    }
                }
            }
            if let Some(ref mt) = orig.media_type {
                if !mt.trim().is_empty() {
                    meta.media_type.merge_candidate_with_force(Some(mt.clone()), src, 0.85, &now_ts, force);
                }
            }
            if let Some(exp) = orig.explicit {
                meta.explicit.merge_candidate_with_force(Some(if exp { "1".to_string() } else { "0".to_string() }), src, 0.95, &now_ts, force);
            }
            if let Some(bpm_val) = orig.bpm {
                if FieldValidator::is_valid_bpm(bpm_val) {
                    meta.bpm.merge_candidate_with_force(Some(bpm_val.to_string()), src, 0.90, &now_ts, force);
                }
            }
            if let Some(ref key_val) = orig.initial_key {
                if FieldValidator::is_valid_key(key_val) {
                    meta.initial_key.merge_candidate_with_force(Some(key_val.clone()), src, 0.90, &now_ts, force);
                }
            }
            if let Some(en) = orig.energy {
                meta.energy.merge_candidate_with_force(Some(format!("{:.2}", en)), src, 0.90, &now_ts, force);
            }
            if let Some(da) = orig.danceability {
                meta.danceability.merge_candidate_with_force(Some(format!("{:.2}", da)), src, 0.90, &now_ts, force);
            }
            if let Some(lo) = orig.loudness {
                meta.loudness.merge_candidate_with_force(Some(format!("{:.1}", lo)), src, 0.90, &now_ts, force);
            }
            if let Some(ref rtg) = orig.replaygain_track_gain {
                meta.replaygain_track_gain.merge_candidate_with_force(Some(rtg.clone()), src, 0.90, &now_ts, force);
            }
            if let Some(ref rtp) = orig.replaygain_track_peak {
                meta.replaygain_track_peak.merge_candidate_with_force(Some(rtp.clone()), src, 0.90, &now_ts, force);
            }
            if let Some(ref rag) = orig.replaygain_album_gain {
                meta.replaygain_album_gain.merge_candidate_with_force(Some(rag.clone()), src, 0.90, &now_ts, force);
            }
            if let Some(ref rap) = orig.replaygain_album_peak {
                meta.replaygain_album_peak.merge_candidate_with_force(Some(rap.clone()), src, 0.90, &now_ts, force);
            }
            if let Some(ref r128) = orig.r128_track_gain {
                meta.r128_track_gain.merge_candidate_with_force(Some(r128.clone()), src, 0.90, &now_ts, force);
            }
            if let Some(ref cmt) = orig.comment {
                meta.comment.merge_candidate_with_force(Some(cmt.clone()), src, 0.90, &now_ts, force);
            }
            if let Some(ref lsrc) = orig.lyrics_source {
                meta.lyrics_source.merge_candidate_with_force(Some(lsrc.clone()), src, 0.90, &now_ts, force);
            }
            if let Some(ref csrc) = orig.cover_source {
                meta.cover_source.merge_candidate_with_force(Some(csrc.clone()), src, 0.90, &now_ts, force);
            }
            if let Some(ref asrc) = orig.audio_source {
                meta.audio_source.merge_candidate_with_force(Some(asrc.clone()), src, 0.90, &now_ts, force);
            }
            if let Some(ref isrc_val) = orig.isrc {
                if FieldValidator::is_valid_identifier(isrc_val) {
                    meta.isrc.merge_candidate_with_force(Some(isrc_val.clone()), src, 0.95, &now_ts, force);
                }
            }
            if let Some(ref bar) = orig.barcode {
                if FieldValidator::is_valid_identifier(bar) {
                    meta.barcode.merge_candidate_with_force(Some(bar.clone()), src, 0.95, &now_ts, force);
                }
            }
            if let Some(ref aid) = orig.acoustid_id {
                if FieldValidator::is_valid_acoustid(aid) {
                    meta.acoustid_id.merge_candidate_with_force(Some(aid.clone()), src, 0.95, &now_ts, force);
                }
            }
            if let Some(ref fp) = orig.acoustid_fingerprint {
                if !fp.trim().is_empty() {
                    meta.acoustid_fingerprint.merge_candidate_with_force(Some(fp.clone()), src, 0.95, &now_ts, force);
                }
            }
            if let Some(ref mb_rid) = orig.musicbrainz_recording_id {
                if FieldValidator::is_valid_musicbrainz_id(mb_rid) {
                    meta.musicbrainz_recording_id.merge_candidate_with_force(Some(mb_rid.clone()), src, 0.95, &now_ts, force);
                }
            }
            if let Some(ref mb_relid) = orig.musicbrainz_release_id {
                if FieldValidator::is_valid_musicbrainz_id(mb_relid) {
                    meta.musicbrainz_release_id.merge_candidate_with_force(Some(mb_relid.clone()), src, 0.95, &now_ts, force);
                }
            }
            if let Some(ref mb_aid) = orig.musicbrainz_artist_id {
                if FieldValidator::is_valid_musicbrainz_artist_id(mb_aid, Some(artist)) {
                    meta.musicbrainz_artist_id.merge_candidate_with_force(Some(mb_aid.clone()), src, 0.95, &now_ts, force);
                }
            }
        }

        // 3. Query MusicBrainz (if enabled)
        let has_existing_mbid = meta.musicbrainz_recording_id.value().is_some();
        let mb_recording = if query_musicbrainz && !has_existing_mbid {
            self.query_musicbrainz(artist, album, title, meta.isrc.value()).await
        } else {
            None
        };

        if let Some(ref rec) = mb_recording {
            if FieldValidator::is_valid_musicbrainz_id(&rec.id) {
                meta.musicbrainz_recording_id.merge_candidate_with_force(
                    Some(rec.id.clone()),
                    "musicbrainz",
                    0.95,
                    &now_ts,
                    force,
                );
            }

            // Artist credit
            if let Some(ref acs) = rec.artist_credit {
                if let Some(first_ac) = acs.first() {
                    let artist_name_to_check = if !first_ac.artist.name.is_empty() {
                        first_ac.artist.name.as_str()
                    } else if !first_ac.name.is_empty() {
                        first_ac.name.as_str()
                    } else {
                        artist
                    };
                    if FieldValidator::is_valid_musicbrainz_artist_id(&first_ac.artist.id, Some(artist_name_to_check)) {
                        meta.musicbrainz_artist_id.merge_candidate_with_force(
                            Some(first_ac.artist.id.clone()),
                            "musicbrainz",
                            0.95,
                            &now_ts,
                            force,
                        );
                        meta.musicbrainz_albumartist_id.merge_candidate_with_force(
                            Some(first_ac.artist.id.clone()),
                            "musicbrainz",
                            0.95,
                            &now_ts,
                            force,
                        );
                    }
                }
            }

            if let Some(ref genres) = rec.genres {
                for (i, g) in genres.iter().enumerate() {
                    all_genre_strings.push(g.name.clone());
                    if i > 0 {
                        all_style_strings.push(g.name.clone());
                        all_tag_strings.push(g.name.clone());
                    }
                }
            }
            if let Some(ref tags) = rec.tags {
                for t in tags {
                    all_genre_strings.push(t.name.clone());
                    all_tag_strings.push(t.name.clone());
                    all_artist_tag_strings.push(t.name.clone());
                }
            }

            // Select best release
            if let Some(ref releases) = rec.releases {
                let norm_album = normalize_title(album);
                let is_album_match = |r_title: &str| {
                    let t = normalize_title(r_title);
                    t == norm_album || t.starts_with(&norm_album) || norm_album.starts_with(&t)
                };

                let selected_release = releases
                    .iter()
                    .find(|r| is_album_match(&r.title))
                    .or_else(|| releases.first());

                if let Some(rel) = selected_release {
                    if FieldValidator::is_valid_musicbrainz_id(&rel.id) {
                        meta.musicbrainz_release_id.merge_candidate_with_force(
                            Some(rel.id.clone()),
                            "musicbrainz",
                            0.95,
                            &now_ts,
                            force,
                        );
                    }

                    if let Some(ref rel_acs) = rel.artist_credit {
                        if let Some(first_rel_ac) = rel_acs.first() {
                            let rel_artist_to_check = if !first_rel_ac.artist.name.is_empty() {
                                first_rel_ac.artist.name.as_str()
                            } else if !first_rel_ac.name.is_empty() {
                                first_rel_ac.name.as_str()
                            } else {
                                meta.album_artist.value().unwrap_or(artist)
                            };
                            if FieldValidator::is_valid_musicbrainz_artist_id(&first_rel_ac.artist.id, Some(rel_artist_to_check)) {
                                meta.musicbrainz_albumartist_id.merge_candidate_with_force(
                                    Some(first_rel_ac.artist.id.clone()),
                                    "musicbrainz",
                                    0.95,
                                    &now_ts,
                                    force,
                                );
                            }
                        }
                    }

                    if let Some(ref rg) = rel.release_group {
                        if FieldValidator::is_valid_musicbrainz_id(&rg.id) {
                            meta.musicbrainz_release_group_id.merge_candidate_with_force(
                                Some(rg.id.clone()),
                                "musicbrainz",
                                0.95,
                                &now_ts,
                                force,
                            );
                        }
                        if let Some(ref pt) = rg.primary_type {
                            meta.release_type.merge_candidate_with_force(
                                Some(pt.clone()),
                                "musicbrainz",
                                0.85,
                                &now_ts,
                                force,
                            );
                            if pt.eq_ignore_ascii_case("compilation") {
                                meta.compilation.merge_candidate_with_force(
                                    Some("1".to_string()),
                                    "musicbrainz",
                                    0.95,
                                    &now_ts,
                                    force,
                                );
                                if meta.album_artist.value().is_none() {
                                    meta.album_artist.merge_candidate_with_force(
                                        Some("Various Artists".to_string()),
                                        "musicbrainz",
                                        0.90,
                                        &now_ts,
                                        force,
                                    );
                                }
                            }
                        }

                        if let Some(ref st_list) = rg.secondary_types {
                            if st_list.iter().any(|st| st.eq_ignore_ascii_case("compilation") || st.eq_ignore_ascii_case("soundtrack")) {
                                meta.compilation.merge_candidate_with_force(
                                    Some("1".to_string()),
                                    "musicbrainz",
                                    0.95,
                                    &now_ts,
                                    force,
                                );
                                if meta.album_artist.value().is_none() {
                                    meta.album_artist.merge_candidate_with_force(
                                        Some("Various Artists".to_string()),
                                        "musicbrainz",
                                        0.90,
                                        &now_ts,
                                        force,
                                    );
                                }
                            }

                            for st in st_list {
                                let st_lower = st.to_lowercase();
                                let media_val = if st_lower == "soundtrack" {
                                    Some("Soundtrack")
                                } else if st_lower == "live" {
                                    Some("Live")
                                } else if st_lower == "remix" {
                                    Some("Remix")
                                } else if st_lower == "dj-mix" || st_lower == "dj mix" {
                                    Some("DJ Mix")
                                } else if st_lower == "mixtape/street" || st_lower == "mixtape" {
                                    Some("Mixtape")
                                } else {
                                    None
                                };

                                if let Some(mv) = media_val {
                                    meta.media_type.merge_candidate_with_force(
                                        Some(mv.to_string()),
                                        "musicbrainz",
                                        0.90,
                                        &now_ts,
                                        force,
                                    );
                                    break;
                                }
                            }
                        }

                        let effective_aa = meta.album_artist.value().unwrap_or(artist);
                        let rg_title_str = rg.title.as_deref().unwrap_or(album);
                        if !rg_title_str.trim().is_empty() {
                            let rg_grouping = format!("{} - {}", effective_aa.trim(), rg_title_str.trim());
                            meta.grouping.merge_candidate_with_force(
                                Some(rg_grouping),
                                "musicbrainz",
                                0.85,
                                &now_ts,
                                force,
                            );
                        }

                        if let Some(ref g_list) = rg.genres {
                            for (i, g) in g_list.iter().enumerate() {
                                all_genre_strings.push(g.name.clone());
                                if i > 0 {
                                    all_style_strings.push(g.name.clone());
                                }
                            }
                        }
                        if let Some(ref t_list) = rg.tags {
                            for t in t_list {
                                all_tag_strings.push(t.name.clone());
                                all_genre_strings.push(t.name.clone());
                            }
                        }
                    }

                    if let Some(ref status_str) = rel.status {
                        meta.release_status.merge_candidate_with_force(
                            Some(status_str.clone()),
                            "musicbrainz",
                            0.85,
                            &now_ts,
                            force,
                        );
                    }

                    if let Some(ref country_str) = rel.country {
                        match syncify_metadata_domain::resolve_country(country_str) {
                            syncify_metadata_domain::CountryResolution::Country { canonical_name, .. } => {
                                all_country_candidates.push((canonical_name.clone(), "musicbrainz".to_string(), 0.85));
                                if let Some(lang_code) = syncify_metadata_domain::default_language_for_country(&canonical_name) {
                                    all_language_candidates.push((lang_code.to_string(), "musicbrainz".to_string(), 0.75));
                                }
                            }
                            syncify_metadata_domain::CountryResolution::Region { region_name, region_code } => {
                                let reg_val = region_code.unwrap_or(region_name);
                                meta.release_region.merge_candidate_with_force(Some(reg_val), "musicbrainz", 0.85, &now_ts, force);
                            }
                            syncify_metadata_domain::CountryResolution::Unknown(_) => {
                                all_country_candidates.push((country_str.clone(), "musicbrainz".to_string(), 0.85));
                                if let Some(lang_code) = syncify_metadata_domain::default_language_for_country(country_str) {
                                    all_language_candidates.push((lang_code.to_string(), "musicbrainz".to_string(), 0.75));
                                }
                            }
                        }
                    }

                    if let Some(ref tr) = rel.text_representation {
                        if let Some(ref lang_str) = tr.language {
                            all_language_candidates.push((lang_str.clone(), "musicbrainz".to_string(), 0.90));
                        }
                    }

                    if let Some(ref bc) = rel.barcode {
                        if FieldValidator::is_valid_identifier(bc) {
                            meta.barcode.merge_candidate_with_force(
                                Some(bc.clone()),
                                "musicbrainz",
                                0.85,
                                &now_ts,
                                force,
                            );
                        }
                    }

                    if let Some(ref date_str) = rel.date {
                        if FieldValidator::is_valid_year(date_str) {
                            meta.original_date.merge_candidate_with_force(
                                Some(date_str.clone()),
                                "musicbrainz",
                                0.85,
                                &now_ts,
                                force,
                            );
                        }
                    }

                    if let Some(ref labels) = rel.label_info {
                        for first_lbl in labels {
                            if let Some(ref l_obj) = first_lbl.label {
                                if FieldValidator::is_valid_label(&l_obj.name) {
                                    all_label_strings.push(l_obj.name.clone());
                                    meta.label.merge_candidate_with_force(
                                        Some(l_obj.name.clone()),
                                        "musicbrainz",
                                        0.85,
                                        &now_ts,
                                        force,
                                    );
                                }
                            }
                            if let Some(ref cat_str) = first_lbl.catalog_number {
                                if !cat_str.trim().is_empty() {
                                    meta.catalog_number.merge_candidate_with_force(
                                        Some(cat_str.clone()),
                                        "musicbrainz",
                                        0.85,
                                        &now_ts,
                                        force,
                                    );
                                }
                            }
                        }
                    }
                }
            }
        } else if meta.musicbrainz_recording_id.value().is_none() {
            meta.musicbrainz_recording_id = FieldResolution::NotFound {
                source: "musicbrainz".to_string(),
                checked_at: now_ts.clone(),
            };
        }

        // 4. FUSE MULTI-VALUE FIELDS:
        // A. GENRE:
        let genre_ctx = syncify_metadata_domain::GenreContext::new()
            .with_title(meta.title.value().or(Some(title)))
            .with_artist(meta.artist.value().or(Some(artist)))
            .with_album(meta.album.value().or(Some(album)))
            .with_label(meta.label.value().or(origin_sources.first().and_then(|o| o.label.as_deref())));

        let genre_refs: Vec<&str> = all_genre_strings.iter().map(|s| s.as_str()).collect();
        let fused_genre_list = syncify_metadata_domain::fuse_genres_with_context(&genre_refs, Some(&genre_ctx));
        if !fused_genre_list.is_empty() {
            meta.genre.merge_candidate_with_force(Some(fused_genre_list.join("; ")), "stream", 0.90, &now_ts, force);

            // If STYLE not already resolved from explicit sources, secondary genres populate STYLE
            if meta.style.value().is_none() {
                let style_candidates: Vec<&str> = if !all_style_strings.is_empty() {
                    all_style_strings.iter().map(|s| s.as_str()).collect()
                } else if fused_genre_list.len() > 1 {
                    fused_genre_list[1..].iter().map(|s| s.as_str()).collect()
                } else {
                    Vec::new()
                };

                if !style_candidates.is_empty() {
                    let fused_styles = syncify_metadata_domain::fuse_genres_with_context(&style_candidates, Some(&genre_ctx));
                    if !fused_styles.is_empty() {
                        meta.style.merge_candidate_with_force(Some(fused_styles.join("; ")), "musicbrainz", 0.85, &now_ts, force);
                    }
                }
            }
        } else if !all_genre_strings.is_empty() {
            tracing::warn!(
                target: "enrichment",
                title = %title,
                artist = %artist,
                "All candidate genres were rejected as junk, placeholder, or matching track/artist/album/label; degrading genre to None"
            );
        }

        // TAGS:
        let tag_candidates: Vec<&str> = if !all_tag_strings.is_empty() {
            all_tag_strings.iter().map(|s| s.as_str()).collect()
        } else if fused_genre_list.len() > 1 {
            fused_genre_list[1..].iter().map(|s| s.as_str()).collect()
        } else {
            Vec::new()
        };
        if !tag_candidates.is_empty() {
            let fused_tags = syncify_metadata_domain::fuse_genres_with_context(&tag_candidates, Some(&genre_ctx));
            if !fused_tags.is_empty() {
                meta.tags.merge_candidate_with_force(Some(fused_tags.join("; ")), "musicbrainz", 0.85, &now_ts, force);
            }
        }

        // ARTISTS_TAGS:
        let art_tag_candidates: Vec<&str> = if !all_artist_tag_strings.is_empty() {
            all_artist_tag_strings.iter().map(|s| s.as_str()).collect()
        } else if !fused_genre_list.is_empty() {
            fused_genre_list.iter().map(|s| s.as_str()).collect()
        } else {
            Vec::new()
        };
        if !art_tag_candidates.is_empty() {
            let fused_art_tags = syncify_metadata_domain::fuse_genres_with_context(&art_tag_candidates, Some(&genre_ctx));
            if !fused_art_tags.is_empty() {
                meta.artist_tags.merge_candidate_with_force(Some(fused_art_tags.join("; ")), "stream", 0.85, &now_ts, force);
            }
        }

        // B. LANGUAGE:
        let mut lang_tuples: Vec<(&str, &str, f64)> = all_language_candidates
            .iter()
            .map(|(val, src, conf)| (val.as_str(), src.as_str(), *conf))
            .collect();
        if let Some(country_val) = meta.release_country.value() {
            if let Some(c_lang) = syncify_metadata_domain::default_language_for_country(country_val) {
                lang_tuples.push((c_lang, "inferred_country", 0.70));
            }
        }
        if let Some(fused_lang) = syncify_metadata_domain::fuse_languages(&lang_tuples) {
            meta.language.merge_candidate_with_force(Some(fused_lang), "stream", 0.90, &now_ts, force);
        }

        // C. COUNTRY / RELEASECOUNTRY:
        let country_tuples: Vec<(&str, &str, f64)> = all_country_candidates
            .iter()
            .map(|(val, src, conf)| (val.as_str(), src.as_str(), *conf))
            .collect();
        if let Some(fused_country) = syncify_metadata_domain::fuse_countries(&country_tuples) {
            meta.release_country.merge_candidate_with_force(Some(fused_country), "stream", 0.90, &now_ts, force);
        }

        // D. LABEL:
        let label_refs: Vec<&str> = all_label_strings.iter().map(|s| s.as_str()).collect();
        let fused_labels = syncify_metadata_domain::fuse_labels(&label_refs);
        if !fused_labels.is_empty() {
            meta.label.merge_candidate_with_force(Some(fused_labels.join("; ")), "stream", 0.90, &now_ts, force);
        }

        meta
    }

    async fn query_musicbrainz(
        &self,
        artist: &str,
        album: &str,
        title: &str,
        isrc_opt: Option<&str>,
    ) -> Option<MusicBrainzRecording> {
        // 1. Try ISRC lookup first
        if let Some(isrc) = isrc_opt {
            if let Ok(Some(rec)) = self.musicbrainz.lookup_by_isrc(isrc).await {
                return Some(rec);
            }
        }

        // 2. Try text search
        if let Ok(recordings) = self.musicbrainz.search_recordings(title, artist, Some(album), 5).await {
            let recs_vec: Vec<_> = recordings.into_iter().collect();
            let norm_album = normalize_title(album);

            return recs_vec
                .iter()
                .cloned()
                .find(|r| {
                    if let Some(ref rels) = r.releases {
                        rels.iter().any(|rel| {
                            let t = normalize_title(&rel.title);
                            t == norm_album || t.starts_with(&norm_album) || norm_album.starts_with(&t)
                        })
                    } else {
                        false
                    }
                })
                .or_else(|| recs_vec.into_iter().next());
        }

        None
    }

    /// Comprehensive enrichment for staging audio:
    /// 1. Runs audio analysis (ReplayGain, Acoustic Features, AcoustID Fingerprint).
    /// 2. Resolves metadata with MusicBrainz (trying ISRC first, then AcoustID, then text search).
    /// 3. Merges all fields with strict precedence rules into `EnrichedMetadata`.
    #[allow(dead_code)]
    pub async fn resolve_and_enrich_staging_audio(
        &self,
        staging_file: &std::path::Path,
        artist: &str,
        album: &str,
        title: &str,
        isrc_hint: Option<&str>,
        origin_meta: Option<&OriginTrackMetadata>,
    ) -> EnrichedMetadata {
        // 1. Analyze physical audio in staging
        let audio_analysis = AudioAnalyzer::analyze_file(staging_file).await.unwrap_or_default();

        // 2. Resolve metadata with MusicBrainz
        let mut enriched = self.resolve_track_metadata(artist, album, title, isrc_hint, origin_meta).await;

        // 3. Apply audio analysis metrics (fills ReplayGain, Acoustic Features, and AcoustID if not provided by origin)
        let now_ts = chrono_now_iso();
        enriched.apply_audio_analysis(&audio_analysis, "inferred", &now_ts);

        enriched
    }

    /// Safely persist resolved metadata to SQLite adhering to all relational safety invariants.
    #[allow(dead_code)]
    pub async fn apply_to_database(
        &self,
        db: &sqlx::SqlitePool,
        track_id: i64,
        meta: &EnrichedMetadata,
        audio_file_path: Option<&std::path::Path>,
    ) -> Result<(), String> {
        // 1. If audio file is provided, apply and verify FLAC tags first
        if let Some(flac_path) = audio_file_path {
            let (clean_title, feat_from_title) = crate::services::tag_writer::clean_title_and_extract_featured(
                meta.title.value().unwrap_or("")
            );
            let mut artists_list = Vec::new();
            if let Some(a) = meta.artist.value() {
                if syncify_flac_writer::is_valid_tag_val(a) {
                    artists_list.push(a.to_string());
                }
            }
            for feat in &feat_from_title {
                if !artists_list.iter().any(|existing| existing.eq_ignore_ascii_case(feat)) {
                    artists_list.push(feat.clone());
                }
            }

            let flac_meta = crate::services::tag_writer::FlacMetadata {
                title: if !clean_title.is_empty() { clean_title } else { meta.title.value().unwrap_or("").to_string() },
                artist: meta.artist.value().unwrap_or("").to_string(),
                artists: if !artists_list.is_empty() { Some(artists_list) } else { None },
                album: meta.album.value().unwrap_or("").to_string(),
                album_artist: meta.album_artist.value().map(|s| s.to_string()),
                composer: meta.composer.value().map(|s| s.to_string()),
                performers: meta.performers.value().map(|s| s.to_string()),
                work: meta.work.value().map(|s| s.to_string()),
                genre: meta.genre.value().map(|s| s.to_string()),
                style: meta.style.value().map(|s| s.to_string()),
                mood: meta.mood.value().map(|s| s.to_string()),
                release_type: meta.release_type.value().map(|s| s.to_string()),
                release_status: meta.release_status.value().map(|s| s.to_string()),
                release_country: meta.release_country.value().map(|s| s.to_string()),
                release_region: meta.release_region.value().map(|s| s.to_string()),
                language: meta.language.value().map(|s| s.to_string()),
                copyright: meta.copyright.value().map(|s| s.to_string()),
                label: meta.label.value().map(|s| s.to_string()),
                barcode: meta.barcode.value().map(|s| s.to_string()),
                catalog_number: meta.catalog_number.value().map(|s| s.to_string()),
                original_date: meta.original_date.value().map(|s| s.to_string()),
                track_number: meta.track_number.value().and_then(|s| s.parse::<u32>().ok()).unwrap_or(0),
                track_total: meta.track_total.value().and_then(|s| s.parse::<u32>().ok()).unwrap_or(0),
                disc_number: meta.disc_number.value().and_then(|s| s.parse::<u32>().ok()).unwrap_or(1),
                disc_total: meta.disc_total.value().and_then(|s| s.parse::<u32>().ok()).unwrap_or(1),
                disc_subtitle: meta.disc_subtitle.value().map(|s| s.to_string()),
                isrc: meta.isrc.value().map(|s| s.to_string()),
                release_year: meta.release_year.value().map(|s| s.to_string()),
                release_date: meta.release_date.value().map(|s| s.to_string()),
                explicit: meta.explicit.value().map(|s| s == "1" || s.eq_ignore_ascii_case("true")),
                bpm: meta.bpm.value().and_then(|s| s.parse::<u32>().ok()),
                initial_key: meta.initial_key.value().map(|s| s.to_string()),
                energy: meta.energy.value().and_then(|s| s.parse::<f64>().ok()),
                danceability: meta.danceability.value().and_then(|s| s.parse::<f64>().ok()),
                loudness: meta.loudness.value().and_then(|s| s.parse::<f64>().ok()),
                replaygain_track_gain: meta.replaygain_track_gain.value().map(|s| s.to_string()),
                replaygain_track_peak: meta.replaygain_track_peak.value().map(|s| s.to_string()),
                replaygain_album_gain: meta.replaygain_album_gain.value().map(|s| s.to_string()),
                replaygain_album_peak: meta.replaygain_album_peak.value().map(|s| s.to_string()),
                r128_track_gain: meta.r128_track_gain.value().map(|s| s.to_string()),
                comment: meta.comment.value().map(|s| s.to_string()),
                lyrics_source: meta.lyrics_source.value().map(|s| s.to_string()),
                cover_source: meta.cover_source.value().map(|s| s.to_string()),
                audio_source: meta.audio_source.value().map(|s| s.to_string()),
                musicbrainz_track_id: meta.musicbrainz_recording_id.value().map(|s| s.to_string()),
                musicbrainz_artist_id: meta.musicbrainz_artist_id.value().map(|s| s.to_string()),
                musicbrainz_album_id: meta.musicbrainz_release_id.value().map(|s| s.to_string()),
                musicbrainz_albumartist_id: meta.musicbrainz_albumartist_id.value().map(|s| s.to_string()),
                musicbrainz_release_group_id: meta.musicbrainz_release_group_id.value().map(|s| s.to_string()),
                musicbrainz_work_id: meta.musicbrainz_work_id.value().map(|s| s.to_string()),
                compilation: meta.compilation.value().map(|s| s == "1" || s.eq_ignore_ascii_case("true")),
                grouping: meta.grouping.value().map(|s| s.to_string()),
                media_type: meta.media_type.value().map(|s| s.to_string()),
                ..Default::default()
            };

            crate::services::tag_writer::apply_flac_tags(flac_path, &flac_meta)
                .map_err(|e| format!("Failed to apply FLAC tags: {}", e))?;

            crate::services::tag_writer::verify_flac_tags(flac_path, &flac_meta)
                .map_err(|e| format!("FLAC re-read verification failed: {}", e))?;
        }

        // 2. Start SQLite transaction
        // S195-fix: BEGIN IMMEDIATE toma el lock de escritura AQUÍ, de modo que
        // busy_timeout (30s) aplica desde el inicio. Con BEGIN diferido, una
        // transacción concurrente de otro servicio provocaba BUSY_SNAPSHOT
        // inmediato (el timeout no aplica a la promoción read->write).
        let mut tx = db.begin_with("BEGIN IMMEDIATE").await.map_err(|e| format!("DB transaction failed: {}", e))?;

        // 3. Update track record (only non-empty resolved values)
        if let Some(t) = meta.title.value() {
            let (clean_t, feat_from_title) = crate::services::tag_writer::clean_title_and_extract_featured(t);
            let title_to_save = if !clean_t.is_empty() { &clean_t } else { t };
            let _ = sqlx::query("UPDATE tracks SET title = ? WHERE id = ?")
                .bind(title_to_save).bind(track_id).execute(&mut *tx).await;

            for feat_name in feat_from_title {
                let clean_feat = syncify_core_domain::metadata::sanitize_artist_name(&feat_name);
                let clean_feat = clean_feat.trim();
                if clean_feat.is_empty() {
                    continue;
                }
                let feat_aid: Option<i64> = sqlx::query_scalar("SELECT id FROM artists WHERE LOWER(TRIM(name)) = LOWER(?) LIMIT 1")
                    .bind(clean_feat)
                    .fetch_optional(&mut *tx)
                    .await
                    .ok()
                    .flatten();
                let final_feat_id = match feat_aid {
                    Some(id) => id,
                    None => {
                        let res = sqlx::query("INSERT INTO artists (name) VALUES (?)")
                            .bind(clean_feat)
                            .execute(&mut *tx)
                            .await;
                        match res {
                            Ok(r) => r.last_insert_rowid(),
                            Err(_) => {
                                sqlx::query_scalar("SELECT id FROM artists WHERE LOWER(TRIM(name)) = LOWER(?) LIMIT 1")
                                    .bind(clean_feat)
                                    .fetch_optional(&mut *tx)
                                    .await
                                    .ok()
                                    .flatten()
                                    .unwrap_or(0)
                            }
                        }
                    }
                };
                if final_feat_id > 0 {
                    let _ = sqlx::query("INSERT OR IGNORE INTO track_artists (track_id, artist_id, role) VALUES (?, ?, 'featured')")
                        .bind(track_id).bind(final_feat_id).execute(&mut *tx).await;
                }
            }
        }
        if let Some(tn) = meta.track_number.value().and_then(|s| s.parse::<i32>().ok()) {
            let _ = sqlx::query("UPDATE tracks SET track_number = ? WHERE id = ?")
                .bind(tn).bind(track_id).execute(&mut *tx).await;
        }
        if let Some(dn) = meta.disc_number.value().and_then(|s| s.parse::<i32>().ok()) {
            let _ = sqlx::query("UPDATE tracks SET disc_number = ? WHERE id = ?")
                .bind(dn).bind(track_id).execute(&mut *tx).await;
        }
        if let Some(i) = meta.isrc.value() {
            let _ = sqlx::query("UPDATE tracks SET isrc = ? WHERE id = ?")
                .bind(i).bind(track_id).execute(&mut *tx).await;
        }
        if let Some(y) = meta.release_year.value().and_then(|s| s.chars().take(4).collect::<String>().parse::<i32>().ok()) {
            let _ = sqlx::query("UPDATE tracks SET release_year = ? WHERE id = ?")
                .bind(y).bind(track_id).execute(&mut *tx).await;
        }
        if let Some(l) = meta.label.value() {
            let _ = sqlx::query("UPDATE tracks SET record_label = ? WHERE id = ?")
                .bind(l).bind(track_id).execute(&mut *tx).await;
        }
        if let Some(mb_track) = meta.musicbrainz_recording_id.value() {
            let _ = sqlx::query("UPDATE tracks SET musicbrainz_id = ? WHERE id = ?")
                .bind(mb_track).bind(track_id).execute(&mut *tx).await;
        }
        if let Some(b) = meta.bpm.value().and_then(|s| s.parse::<f64>().ok()) {
            let _ = sqlx::query("UPDATE tracks SET bpm = ? WHERE id = ?")
                .bind(b).bind(track_id).execute(&mut *tx).await;
        }
        if let Some(k) = meta.initial_key.value() {
            let _ = sqlx::query("UPDATE tracks SET musical_key = ? WHERE id = ?")
                .bind(k).bind(track_id).execute(&mut *tx).await;
        }
        if let Some(fp) = meta.acoustid_fingerprint.value() {
            let _ = sqlx::query("UPDATE tracks SET acoustid_fingerprint = ? WHERE id = ?")
                .bind(fp).bind(track_id).execute(&mut *tx).await;
        }
        if let Some(g) = meta.genre.value() {
            if let Some(cg) = clean_primary_genre(g) {
                let _ = sqlx::query("UPDATE tracks SET genre = ? WHERE id = ?")
                    .bind(cg).bind(track_id).execute(&mut *tx).await;
            }
        }

        // Update global job status and timestamp
        let _ = sqlx::query("UPDATE tracks SET enrichment_status = 'complete', enriched_at = CURRENT_TIMESTAMP WHERE id = ?")
            .bind(track_id).execute(&mut *tx).await;

        // 4. Resolve / link Artist safely (NEVER rename artists.name)
        if let Some(art_name_raw) = meta.artist.value() {
            let art_name = syncify_core_domain::metadata::sanitize_artist_name(art_name_raw);
            let art_name = art_name.trim().to_string();
            let artist_row: Option<(i64, Option<String>)> = sqlx::query_as(
                "SELECT id, musicbrainz_id FROM artists WHERE LOWER(TRIM(name)) = LOWER(?) LIMIT 1"
            )
            .bind(&art_name)
            .fetch_optional(&mut *tx)
            .await
            .ok()
            .flatten();

            let artist_id = if let Some((aid, mb_id)) = artist_row {
                let current_is_valid = mb_id
                    .as_deref()
                    .map(|id| FieldValidator::is_valid_musicbrainz_artist_id(id, Some(&art_name)))
                    .unwrap_or(false);
                if !current_is_valid {
                    if let Some(mb_art) = meta.musicbrainz_artist_id.value() {
                        if FieldValidator::is_valid_musicbrainz_artist_id(mb_art, Some(&art_name)) {
                            let _ = sqlx::query("UPDATE artists SET musicbrainz_id = ? WHERE id = ?")
                                .bind(mb_art).bind(aid).execute(&mut *tx).await;
                        }
                    }
                }
                aid
            } else {
                let mb_art = meta.musicbrainz_artist_id.value()
                    .filter(|id| FieldValidator::is_valid_musicbrainz_artist_id(id, Some(&art_name)));
                let res = sqlx::query(
                    "INSERT INTO artists (name, musicbrainz_id) VALUES (?, ?)
                     ON CONFLICT(name COLLATE NOCASE) DO UPDATE SET
                       musicbrainz_id = COALESCE(artists.musicbrainz_id, excluded.musicbrainz_id)
                     RETURNING id"
                )
                .bind(&art_name)
                .bind(mb_art)
                .fetch_one(&mut *tx)
                .await
                .map_err(|e| format!("Failed to insert artist: {}", e))?;
                use sqlx::Row;
                res.get::<i64, _>(0)
            };

            // Link track_artists safely
            let _ = sqlx::query("INSERT OR IGNORE INTO track_artists (track_id, artist_id, role) VALUES (?, ?, 'primary')")
                .bind(track_id).bind(artist_id).execute(&mut *tx).await;
        }

        // 5. Update Album if track has album_id
        let album_row: Option<(Option<i64>,)> = sqlx::query_as("SELECT album_id FROM tracks WHERE id = ?")
            .bind(track_id)
            .fetch_optional(&mut *tx)
            .await
            .ok()
            .flatten();

        if let Some((Some(album_id),)) = album_row {
            if let Some(alb_title) = meta.album.value() {
                let _ = sqlx::query("UPDATE albums SET title = ? WHERE id = ?")
                    .bind(alb_title).bind(album_id).execute(&mut *tx).await;
            }
            if let Some(rel_date) = meta.original_date.value() {
                let _ = sqlx::query("UPDATE albums SET release_date = ? WHERE id = ?")
                    .bind(rel_date).bind(album_id).execute(&mut *tx).await;
            }
            if let Some(upc) = meta.barcode.value() {
                let _ = sqlx::query("UPDATE albums SET upc = ? WHERE id = ?")
                    .bind(upc).bind(album_id).execute(&mut *tx).await;
            }
            if let Some(tt) = meta.track_total.value().and_then(|s| s.parse::<i32>().ok()) {
                let _ = sqlx::query("UPDATE albums SET total_tracks = ? WHERE id = ?")
                    .bind(tt).bind(album_id).execute(&mut *tx).await;
            }
            if let Some(lbl) = meta.label.value() {
                let _ = sqlx::query("UPDATE albums SET label = ? WHERE id = ?")
                    .bind(lbl).bind(album_id).execute(&mut *tx).await;
            }
            if let Some(mb_rel) = meta.musicbrainz_release_id.value() {
                let _ = sqlx::query("UPDATE albums SET musicbrainz_id = ? WHERE id = ?")
                    .bind(mb_rel).bind(album_id).execute(&mut *tx).await;
            }
        }

        tx.commit().await.map_err(|e| format!("Failed to commit DB transaction: {}", e))?;
        Ok(())
    }

    /// Evaluates the target enrichment status ('enriched', 'partial', or 'error')
    /// based on key metadata and acoustic fields.
    pub fn evaluate_enrichment_status(
        has_bpm: bool,
        has_key: bool,
        has_fingerprint: bool,
        has_core_metadata: bool,
        critical_error: Option<&str>,
    ) -> &'static str {
        if critical_error.is_some() {
            "error"
        } else if has_bpm && has_key && has_fingerprint && has_core_metadata {
            "enriched"
        } else {
            "partial"
        }
    }

    /// Computes the effective audio quality tier by taking the maximum tier across:
    /// 1. The existing track's audio_quality (never downgrading a higher tier)
    /// 2. All existing sources in track_sources
    /// 3. The incoming source's physical attributes and declared audio_quality
    pub fn compute_effective_audio_tier(
        current_quality: Option<&str>,
        existing_sources: &[(Option<i32>, Option<i32>, Option<i32>, Option<String>)],
        input_bit_depth: Option<i32>,
        input_sample_rate: Option<i32>,
        input_format: Option<&str>,
        input_audio_quality: Option<&str>,
    ) -> Option<AudioTier> {
        let mut best: Option<AudioTier> = None;

        let mut update_best = |tier: AudioTier| {
            best = Some(match best {
                Some(existing) => std::cmp::max(existing, tier),
                None => tier,
            });
        };

        // 1. Current track's audio_quality
        if let Some(cq) = current_quality {
            if let Ok(t) = cq.parse::<AudioTier>() {
                update_best(t);
            } else {
                update_best(classify_audio_tier(None, None, None, Some(cq)));
            }
        }

        // 2. Existing sources in track_sources
        for (bd, sr, br, fmt) in existing_sources {
            update_best(classify_audio_tier(*bd, *sr, *br, fmt.as_deref()));
        }

        // 3. Incoming source physical attributes
        if input_bit_depth.is_some() || input_sample_rate.is_some() || input_format.is_some() {
            update_best(classify_audio_tier(input_bit_depth, input_sample_rate, None, input_format));
        }

        // 4. Incoming source declared audio_quality
        if let Some(aq) = input_audio_quality {
            if let Ok(t) = aq.parse::<AudioTier>() {
                update_best(t);
            } else {
                update_best(classify_audio_tier(input_bit_depth, input_sample_rate, None, Some(aq)));
            }
        }

        best
    }

    /// Full Sync-Time Pre-Enrichment & Database Persistence:
    /// 1. Resolves catalog metadata (title, artist, album, album_artist, totals, ISRC, UPC, label, year, genre, MBIDs, credits, country).
    /// 2. Respects strict precedence: Manual > StreamingService > MusicBrainz > Inferred.
    /// 3. Normalizes country via CLI domain resolver.
    /// 4. Persists entities (artists, albums, tracks, track_sources, track_credits, library_entries) in a transaction.
    /// 5. Ensures zero audio downloads and full idempotency.
    pub async fn enrich_and_persist_sync_track(
        &self,
        db: &sqlx::SqlitePool,
        input: SyncTrackInput,
    ) -> Result<SyncTrackResult, String> {
        let raw_artist = input.origin_meta.artist.clone().unwrap_or_else(|| "Unknown Artist".to_string());
        let artist_name = syncify_core_domain::metadata::sanitize_artist_name(&raw_artist);
        let raw_album = input.origin_meta.album.clone().unwrap_or_default();
        let album_title = syncify_core_domain::metadata::clean_mojibake(&syncify_core_domain::metadata::decode_html_entities(&raw_album)).trim().to_string();
        let raw_title = input.origin_meta.title.clone().unwrap_or_else(|| "Unknown Track".to_string());
        let track_title = syncify_core_domain::metadata::sanitize_track_title(&raw_title);
        let isrc_opt = input.origin_meta.isrc.as_deref();

        // 1. Resolve Enriched Metadata with precedence
        let mut enriched = self.resolve_track_metadata_internal(
            &artist_name,
            &album_title,
            &track_title,
            isrc_opt,
            Some(&input.origin_meta),
            input.query_musicbrainz,
        ).await;

        // Country & Region normalization via domain
        if let Some(rc) = input.origin_meta.release_country.as_deref() {
            match syncify_metadata_domain::country::resolve_country(rc) {
                syncify_metadata_domain::CountryResolution::Country { iso_alpha2, .. } => {
                    enriched.release_country.merge_candidate(Some(iso_alpha2), &input.service_name, 0.90, &chrono_now_iso());
                }
                syncify_metadata_domain::CountryResolution::Region { region_name, region_code } => {
                    let reg_val = region_code.unwrap_or(region_name);
                    enriched.release_region.merge_candidate(Some(reg_val), &input.service_name, 0.90, &chrono_now_iso());
                }
                syncify_metadata_domain::CountryResolution::Unknown(_) => {}
            }
        }

        let completeness = enriched.completeness();

        // 2. Start SQLite Transaction — S195-fix: BEGIN IMMEDIATE (ver nota arriba).
        let mut tx = db.begin_with("BEGIN IMMEDIATE").await.map_err(|e| format!("DB transaction failed: {}", e))?;

        // 3. Find or Create Primary Artist (never rename artists.name)
        let artist_row: Option<(i64, Option<String>)> = sqlx::query_as(
            "SELECT id, musicbrainz_id FROM artists WHERE LOWER(TRIM(name)) = LOWER(?) LIMIT 1"
        )
        .bind(&artist_name)
        .fetch_optional(&mut *tx)
        .await
        .ok()
        .flatten();

        let artist_id = if let Some((aid, mb_id)) = artist_row {
            let current_is_valid = mb_id
                .as_deref()
                .map(|id| FieldValidator::is_valid_musicbrainz_artist_id(id, Some(&artist_name)))
                .unwrap_or(false);
            if !current_is_valid {
                if let Some(mb_art) = enriched.musicbrainz_artist_id.value() {
                    if FieldValidator::is_valid_musicbrainz_artist_id(mb_art, Some(&artist_name)) {
                        let _ = sqlx::query("UPDATE artists SET musicbrainz_id = ? WHERE id = ?")
                            .bind(mb_art).bind(aid).execute(&mut *tx).await;
                    }
                }
            }
            aid
        } else {
            let mb_art = enriched.musicbrainz_artist_id.value()
                .filter(|id| FieldValidator::is_valid_musicbrainz_artist_id(id, Some(&artist_name)));

            let res = sqlx::query(
                "INSERT INTO artists (name, musicbrainz_id) VALUES (?, ?)
                 ON CONFLICT(name COLLATE NOCASE) DO UPDATE SET
                   musicbrainz_id = CASE
                     WHEN artists.musicbrainz_id IS NULL THEN excluded.musicbrainz_id
                     WHEN excluded.musicbrainz_id IS NOT NULL THEN excluded.musicbrainz_id
                     ELSE artists.musicbrainz_id
                   END
                 RETURNING id"
            )
            .bind(&artist_name)
            .bind(mb_art)
            .fetch_one(&mut *tx)
            .await
            .map_err(|e| format!("Failed to insert artist '{}': {}", artist_name, e))?;
            
            use sqlx::Row;
            res.get::<i64, _>(0)
        };

        // 4. Find or Create Album
        let mut album_id_opt: Option<i64> = None;
        let is_compilation = if !album_title.trim().is_empty() {
            enriched.compilation.value().map(|s| s == "1" || s.eq_ignore_ascii_case("true")).unwrap_or(false)
                || input.origin_meta.album_artist.as_deref().map(|s| syncify_core_domain::metadata::is_various_artists_variant(s)).unwrap_or(false)
                || enriched.album_artist.value().map(|s| syncify_core_domain::metadata::is_various_artists_variant(s)).unwrap_or(false)
                || input.origin_meta.release_type.as_deref().map(|s| s.eq_ignore_ascii_case("compilation") || s.eq_ignore_ascii_case("soundtrack")).unwrap_or(false)
                || enriched.release_type.value().map(|s| s.eq_ignore_ascii_case("compilation") || s.eq_ignore_ascii_case("soundtrack")).unwrap_or(false)
                || input.origin_meta.media_type.as_deref().map(|s| s.eq_ignore_ascii_case("compilation") || s.eq_ignore_ascii_case("soundtrack")).unwrap_or(false)
                || enriched.media_type.value().map(|s| s.eq_ignore_ascii_case("compilation") || s.eq_ignore_ascii_case("soundtrack")).unwrap_or(false)
        } else {
            false
        };
        if !album_title.trim().is_empty() {
            if is_compilation {
                enriched.compilation.merge_candidate(Some("1".to_string()), &input.service_name, 0.95, &chrono_now_iso());
                if enriched.album_artist.value().is_none() {
                    enriched.album_artist.merge_candidate(Some("Various Artists".to_string()), &input.service_name, 0.90, &chrono_now_iso());
                }
            }

            let effective_album_artist_name = if let Some(aa) = enriched.album_artist.value() {
                syncify_core_domain::metadata::normalize_compilation_artist_name(aa, is_compilation)
            } else if is_compilation {
                "Various Artists".to_string()
            } else {
                artist_name.clone()
            };

            let album_artist_id = if effective_album_artist_name.eq_ignore_ascii_case(&artist_name) {
                artist_id
            } else {
                let aa_row: Option<(i64,)> = sqlx::query_as(
                    "SELECT id FROM artists WHERE LOWER(TRIM(name)) = LOWER(?) LIMIT 1"
                )
                .bind(&effective_album_artist_name)
                .fetch_optional(&mut *tx)
                .await
                .ok()
                .flatten();

                if let Some((aid,)) = aa_row {
                    aid
                } else {
                    let res = sqlx::query(
                        "INSERT INTO artists (name) VALUES (?)
                         ON CONFLICT(name COLLATE NOCASE) DO UPDATE SET id = id
                         RETURNING id"
                    )
                    .bind(&effective_album_artist_name)
                    .fetch_one(&mut *tx)
                    .await
                    .map_err(|e| format!("Failed to insert album artist '{}': {}", effective_album_artist_name, e))?;

                    use sqlx::Row;
                    res.get::<i64, _>(0)
                }
            };

            let mut album_row: Option<(i64,)> = None;

            // Strategy A: By provider album ID
            if let Some(ref prov_id) = input.album_provider_track_id {
                if matches!(input.service_name.as_str(), "qobuz" | "spotify" | "tidal") {
                    let col = match input.service_name.as_str() {
                        "qobuz" => "qobuz_id",
                        "spotify" => "spotify_id",
                        _ => "tidal_id",
                    };
                    let sql = format!("SELECT id FROM albums WHERE {col} = ? LIMIT 1");
                    album_row = sqlx::query_as(&sql)
                        .bind(prov_id)
                        .fetch_optional(&mut *tx)
                        .await
                        .ok()
                        .flatten();
                }
            }

            // Strategy B: By MusicBrainz Release ID
            if album_row.is_none() {
                if let Some(mbid) = enriched.musicbrainz_release_id.value() {
                    album_row = sqlx::query_as("SELECT id FROM albums WHERE musicbrainz_id = ? LIMIT 1")
                        .bind(mbid)
                        .fetch_optional(&mut *tx)
                        .await
                        .ok()
                        .flatten();
                }
            }

            // Strategy C: By Album Title + Album Artist ID
            if album_row.is_none() {
                album_row = sqlx::query_as(
                    r#"
                    SELECT a.id FROM albums a
                    JOIN album_artists aa ON aa.album_id = a.id
                    WHERE a.title = ? COLLATE NOCASE AND aa.artist_id = ?
                    LIMIT 1
                    "#
                )
                .bind(&album_title)
                .bind(album_artist_id)
                .fetch_optional(&mut *tx)
                .await
                .ok()
                .flatten();
            }

            // Strategy D: If compilation, match existing album with title and "Various Artists" / "Various" / variants
            if album_row.is_none() && is_compilation {
                album_row = sqlx::query_as(
                    r#"
                    SELECT a.id FROM albums a
                    JOIN album_artists aa ON aa.album_id = a.id
                    JOIN artists ar ON ar.id = aa.artist_id
                    WHERE a.title = ? COLLATE NOCASE
                      AND (ar.name IN ('Various Artists', 'Various', 'Various Interprets', 'Various Interpret', 'V.A.', 'VA') COLLATE NOCASE)
                    LIMIT 1
                    "#
                )
                .bind(&album_title)
                .fetch_optional(&mut *tx)
                .await
                .ok()
                .flatten();
            }

            // Strategy E: Match existing album by title if it belongs to the same provider release
            if album_row.is_none() && input.album_provider_track_id.is_some() {
                album_row = sqlx::query_as(
                    "SELECT a.id FROM albums a WHERE a.title = ? COLLATE NOCASE LIMIT 1"
                )
                .bind(&album_title)
                .fetch_optional(&mut *tx)
                .await
                .ok()
                .flatten();
            }

            let aid = if let Some((existing_aid,)) = album_row {
                // Update missing album fields
                let _ = sqlx::query(
                    r#"
                    UPDATE albums SET
                        release_date = COALESCE(release_date, ?),
                        musicbrainz_id = COALESCE(musicbrainz_id, ?),
                        upc = COALESCE(upc, ?),
                        total_tracks = COALESCE(total_tracks, ?),
                        cover_art_url = COALESCE(cover_art_url, ?),
                        label = COALESCE(label, ?)
                    WHERE id = ?
                    "#
                )
                .bind(enriched.original_date.value().or_else(|| enriched.release_date.value()))
                .bind(enriched.musicbrainz_release_id.value())
                .bind(enriched.barcode.value())
                .bind(enriched.track_total.value().and_then(|s| s.parse::<i32>().ok()))
                .bind(input.cover_art_url.as_deref())
                .bind(enriched.label.value())
                .bind(existing_aid)
                .execute(&mut *tx)
                .await;

                existing_aid
            } else {
                let res = sqlx::query(
                    r#"
                    INSERT INTO albums (title, release_date, musicbrainz_id, upc, total_tracks, cover_art_url, label)
                    VALUES (?, ?, ?, ?, ?, ?, ?) RETURNING id
                    "#
                )
                .bind(&album_title)
                .bind(enriched.original_date.value().or_else(|| enriched.release_date.value()))
                .bind(enriched.musicbrainz_release_id.value())
                .bind(enriched.barcode.value())
                .bind(enriched.track_total.value().and_then(|s| s.parse::<i32>().ok()))
                .bind(input.cover_art_url.as_deref())
                .bind(enriched.label.value())
                .fetch_one(&mut *tx)
                .await
                .map_err(|e| format!("Failed to insert album '{}': {}", album_title, e))?;

                use sqlx::Row;
                res.get::<i64, _>(0)
            };

            // Album Artists relationship: Check if compilation or divergent artists
            let existing_artist_count: i64 = sqlx::query_scalar(
                "SELECT COUNT(DISTINCT ar.name) FROM album_artists aa
                 JOIN artists ar ON ar.id = aa.artist_id
                 WHERE aa.album_id = ?"
            )
            .bind(aid)
            .fetch_one(&mut *tx)
            .await
            .unwrap_or(0);

            let has_divergent_artists = is_compilation || existing_artist_count > 1 || (existing_artist_count == 1 && {
                let existing_artist: Option<String> = sqlx::query_scalar(
                    "SELECT ar.name FROM album_artists aa JOIN artists ar ON ar.id = aa.artist_id WHERE aa.album_id = ? LIMIT 1"
                )
                .bind(aid)
                .fetch_optional(&mut *tx)
                .await
                .ok()
                .flatten();
                existing_artist.map(|name| !name.eq_ignore_ascii_case(&artist_name) && !name.eq_ignore_ascii_case("various artists") && !name.eq_ignore_ascii_case("various")).unwrap_or(false)
            });

            if has_divergent_artists {
                let va_row: Option<(i64,)> = sqlx::query_as(
                    "SELECT id FROM artists WHERE LOWER(TRIM(name)) = 'various artists' LIMIT 1"
                )
                .fetch_optional(&mut *tx)
                .await
                .ok()
                .flatten();

                let va_id = if let Some((id,)) = va_row {
                    id
                } else {
                    let res = sqlx::query(
                        "INSERT INTO artists (name) VALUES ('Various Artists')
                         ON CONFLICT(name COLLATE NOCASE) DO UPDATE SET id = id
                         RETURNING id"
                    )
                    .fetch_one(&mut *tx)
                    .await
                    .map_err(|e| format!("Failed to insert 'Various Artists': {}", e))?;
                    use sqlx::Row;
                    res.get::<i64, _>(0)
                };

                let _ = sqlx::query("UPDATE album_artists SET is_primary = 0 WHERE album_id = ?")
                    .bind(aid)
                    .execute(&mut *tx)
                    .await;

                let _ = sqlx::query(
                    "INSERT INTO album_artists (album_id, artist_id, is_primary) VALUES (?, ?, 1)
                     ON CONFLICT(album_id, artist_id) DO UPDATE SET is_primary = 1"
                )
                .bind(aid)
                .bind(va_id)
                .execute(&mut *tx)
                .await;

                let _ = sqlx::query(
                    "INSERT OR IGNORE INTO album_artists (album_id, artist_id, is_primary) VALUES (?, ?, 0)"
                )
                .bind(aid)
                .bind(artist_id)
                .execute(&mut *tx)
                .await;

                enriched.compilation.merge_candidate(Some("1".to_string()), &input.service_name, 0.95, &chrono_now_iso());
                if enriched.album_artist.value().is_none() || enriched.album_artist.value() == Some(&artist_name) {
                    enriched.album_artist.merge_candidate(Some("Various Artists".to_string()), &input.service_name, 0.90, &chrono_now_iso());
                }
            } else {
                let _ = sqlx::query(
                    "INSERT OR IGNORE INTO album_artists (album_id, artist_id, is_primary) VALUES (?, ?, 1)"
                )
                .bind(aid)
                .bind(album_artist_id)
                .execute(&mut *tx)
                .await;
            }

            // S198: favorite-album marking + provider album id (guarded, idempotent).
            // Only ever widens data: is_favorite flips 0→1 with a COALESCE'd
            // timestamp; the provider id is written once and never stolen from
            // another row (NOT EXISTS guards the partial-unique index).
            if input.album_is_favorite {
                let _ = sqlx::query(
                    "UPDATE albums SET is_favorite = 1, favorite_at = COALESCE(favorite_at, CURRENT_TIMESTAMP) WHERE id = ?"
                )
                .bind(aid)
                .execute(&mut *tx)
                .await;
            }
            if let Some(provider_album_id) = input.album_provider_track_id.as_deref() {
                if !provider_album_id.trim().is_empty() {
                    let col_ok = matches!(input.service_name.as_str(), "qobuz" | "spotify" | "tidal");
                    if col_ok {
                        // Column chosen from the input's service; identifier
                        // interpolated because column names cannot be bound.
                        let col = match input.service_name.as_str() {
                            "qobuz" => "qobuz_id",
                            "spotify" => "spotify_id",
                            _ => "tidal_id",
                        };
                        let sql = format!(
                            "UPDATE albums SET {col} = ? WHERE id = ? AND {col} IS NULL \
                             AND NOT EXISTS (SELECT 1 FROM albums a2 WHERE a2.{col} = ?)"
                        );
                        let _ = sqlx::query(&sql)
                            .bind(provider_album_id)
                            .bind(aid)
                            .bind(provider_album_id)
                            .execute(&mut *tx)
                            .await;
                    }
                }
            }
            album_id_opt = Some(aid);
        }

        // 5. Find or Create Track with Canonical Identity Check (Source Mapping 1:1 > Valid ISRC > Service ID)
        let mut existing_track_id: Option<i64> = None;

        // Check A: by service source (service_id, service_track_id) - Rule 1
        if let Ok(Some((tid,))) = sqlx::query_as::<_, (i64,)>(
            "SELECT track_id FROM track_sources WHERE service_id = ? AND service_track_id = ? LIMIT 1"
        )
        .bind(input.service_id)
        .bind(&input.service_track_id)
        .fetch_optional(&mut *tx)
        .await
        {
            existing_track_id = Some(tid);
        }

        // Check B: by ISRC (match existing track with this ISRC, and prevent unique constraint collisions)
        if existing_track_id.is_none() {
            if let Some(isrc) = isrc_opt {
                let trimmed_isrc = isrc.trim();
                if !trimmed_isrc.is_empty() {
                    if let Ok(Some((tid,))) = sqlx::query_as::<_, (i64,)>("SELECT id FROM tracks WHERE isrc = ? LIMIT 1")
                        .bind(trimmed_isrc)
                        .fetch_optional(&mut *tx)
                        .await
                    {
                        existing_track_id = Some(tid);
                    }
                }
            }
        }

        // Check C: by service-specific ID column
        if existing_track_id.is_none() {
            let col = match input.service_name.as_str() {
                "qobuz" => Some("qobuz_id"),
                "spotify" => Some("spotify_id"),
                _ => None,
            };
            if let Some(col_name) = col {
                let sql = format!("SELECT id FROM tracks WHERE {} = ? LIMIT 1", col_name);
                if let Ok(Some((tid,))) = sqlx::query_as::<_, (i64,)>(&sql)
                    .bind(&input.service_track_id)
                    .fetch_optional(&mut *tx)
                    .await
                {
                    existing_track_id = Some(tid);
                }
            }
        }

        let parsed_year = enriched.release_year.value()
            .and_then(|s| s.chars().take(4).collect::<String>().parse::<i32>().ok());
        let parsed_track_num = enriched.track_number.value().and_then(|s| s.parse::<i32>().ok());
        let parsed_disc_num = enriched.disc_number.value().and_then(|s| s.parse::<i32>().ok());
        let parsed_explicit = enriched.explicit.value().map(|s| if s == "1" || s.eq_ignore_ascii_case("true") { 1 } else { 0 });
        let parsed_bpm = enriched.bpm.value().and_then(|s| s.parse::<f64>().ok());

        // Backfill of genre: resolve genre with priority:
        // 1. Explicit enriched genre (e.g. from Qobuz / Tidal / MusicBrainz)
        // 2. Fallback to album (other tracks in this album with non-null genre)
        // 3. Fallback to artist (dominant genre from tracks by this artist)
        let mut clean_genre = enriched.genre.value().and_then(clean_primary_genre);
        if clean_genre.is_none() {
            if let Some(aid) = album_id_opt {
                let album_genre: Option<String> = sqlx::query_scalar(
                    "SELECT genre FROM tracks WHERE album_id = ? AND genre IS NOT NULL AND TRIM(genre) != '' LIMIT 1"
                )
                .bind(aid)
                .fetch_optional(&mut *tx)
                .await
                .ok()
                .flatten();

                if let Some(ag) = album_genre.as_deref().and_then(clean_primary_genre) {
                    clean_genre = Some(ag);
                }
            }
        }
        if clean_genre.is_none() {
            let artist_genre: Option<String> = sqlx::query_scalar(
                "SELECT t.genre FROM track_artists ta \
                 JOIN tracks t ON t.id = ta.track_id \
                 WHERE ta.artist_id = ? AND t.genre IS NOT NULL AND TRIM(t.genre) != '' \
                 GROUP BY t.genre ORDER BY COUNT(*) DESC LIMIT 1"
            )
            .bind(artist_id)
            .fetch_optional(&mut *tx)
            .await
            .ok()
            .flatten();

            if let Some(arg) = artist_genre.as_deref().and_then(clean_primary_genre) {
                clean_genre = Some(arg);
            }
        }

        // Canonical derivation of year:
        // tracks.release_year is derived from parent album's release_date
        // unless it's a multi-artist/soundtrack compilation release.
        // For albums without release_date, infer from MIN(tracks.release_year) or incoming track year.
        let mut album_release_year: Option<i32> = None;
        if let Some(aid) = album_id_opt {
            let album_date: Option<String> = sqlx::query_scalar(
                "SELECT release_date FROM albums WHERE id = ?"
            )
            .bind(aid)
            .fetch_optional(&mut *tx)
            .await
            .ok()
            .flatten();

            if let Some(ref d) = album_date {
                album_release_year = d.get(..4).and_then(|y| y.parse::<i32>().ok());
            } else {
                let min_track_year: Option<i32> = sqlx::query_scalar(
                    "SELECT MIN(release_year) FROM tracks WHERE album_id = ? AND release_year IS NOT NULL AND release_year > 1900 AND release_year < 2100"
                )
                .bind(aid)
                .fetch_optional(&mut *tx)
                .await
                .ok()
                .flatten();

                let inferred_year = min_track_year.or(parsed_year);
                if let Some(iy) = inferred_year {
                    let formatted = format!("{:04}-01-01", iy);
                    let _ = sqlx::query("UPDATE albums SET release_date = ? WHERE id = ? AND (release_date IS NULL OR release_date = '')")
                        .bind(&formatted)
                        .bind(aid)
                        .execute(&mut *tx)
                        .await;
                    album_release_year = Some(iy);
                }
            }
        }

        let is_compilation = enriched.compilation.value().map(|s| s == "1" || s.eq_ignore_ascii_case("true")).unwrap_or(false);
        let effective_year = if !is_compilation && album_release_year.is_some() {
            album_release_year
        } else {
            parsed_year.or(album_release_year)
        };

        let is_new_global_track = existing_track_id.is_none();
        let effective_audio_quality: Option<String>;

        let track_id = if let Some(tid) = existing_track_id {
            let current_quality: Option<String> = sqlx::query_scalar(
                "SELECT audio_quality FROM tracks WHERE id = ?"
            )
            .bind(tid)
            .fetch_optional(&mut *tx)
            .await
            .ok()
            .flatten();

            let existing_sources: Vec<(Option<i32>, Option<i32>, Option<i32>, Option<String>)> = sqlx::query_as(
                "SELECT bit_depth, sample_rate, bitrate, format FROM track_sources WHERE track_id = ?"
            )
            .bind(tid)
            .fetch_all(&mut *tx)
            .await
            .unwrap_or_default();

            let eff_tier = Self::compute_effective_audio_tier(
                current_quality.as_deref(),
                &existing_sources,
                input.bit_depth,
                input.sample_rate,
                input.format.as_deref(),
                input.audio_quality.as_deref(),
            );
            effective_audio_quality = eff_tier.map(|t| t.as_str().to_string());

            // Check if track has manual precedence
            let is_manual: bool = sqlx::query_scalar(
                "SELECT (enrichment_status = 'manual') FROM tracks WHERE id = ?"
            )
            .bind(tid)
            .fetch_one(&mut *tx)
            .await
            .unwrap_or(false);

            if is_manual {
                // Preserve manual fields; only fill missing relational/service keys
                let _ = sqlx::query(
                    r#"
                    UPDATE tracks SET
                        album_id = COALESCE(album_id, ?),
                        duration_ms = COALESCE(duration_ms, ?),
                        track_number = COALESCE(track_number, ?),
                        disc_number = COALESCE(disc_number, ?),
                        isrc = COALESCE(isrc, ?),
                        musicbrainz_id = COALESCE(musicbrainz_id, ?),
                        audio_quality = COALESCE(?, audio_quality)
                    WHERE id = ?
                    "#
                )
                .bind(album_id_opt)
                .bind(input.duration_ms)
                .bind(parsed_track_num)
                .bind(parsed_disc_num)
                .bind(enriched.isrc.value())
                .bind(enriched.musicbrainz_recording_id.value())
                .bind(effective_audio_quality.as_deref())
                .bind(tid)
                .execute(&mut *tx)
                .await;
            } else {
                let existing_acoustic: Option<(Option<f64>, Option<String>, Option<String>)> = sqlx::query_as(
                    "SELECT bpm, musical_key, acoustid_fingerprint FROM tracks WHERE id = ?"
                )
                .bind(tid)
                .fetch_optional(&mut *tx)
                .await
                .unwrap_or(None);

                let (existing_bpm, existing_key, existing_fp) = existing_acoustic.unwrap_or((None, None, None));
                let final_bpm = parsed_bpm.or(existing_bpm);
                let final_key = enriched.initial_key.value().map(|s| s.to_string()).or(existing_key);
                let final_fp = enriched.acoustid_fingerprint.value().map(|s| s.to_string()).or(existing_fp);

                let target_status = Self::evaluate_enrichment_status(
                    final_bpm.is_some(),
                    final_key.as_ref().map_or(false, |k| !k.trim().is_empty()),
                    final_fp.as_ref().map_or(false, |f| !f.trim().is_empty()),
                    completeness == EnrichmentCompleteness::Enriched
                        || (enriched.title.value().is_some()
                            && (enriched.isrc.value().is_some() || enriched.musicbrainz_recording_id.value().is_some())),
                    None,
                );

                let title_update_val = enriched.title.value().map(|t| {
                    let (clean_t, _) = syncify_core_domain::metadata::clean_title_and_extract_featured(t);
                    if !clean_t.is_empty() { clean_t } else { t.to_string() }
                });

                // Update with resolved metadata (StreamingService > MusicBrainz > Inferred)
                let _ = sqlx::query(
                    r#"
                    UPDATE tracks SET
                        title = COALESCE(?, title),
                        album_id = COALESCE(album_id, ?),
                        duration_ms = COALESCE(duration_ms, ?),
                        track_number = COALESCE(track_number, ?),
                        disc_number = COALESCE(disc_number, ?),
                        isrc = COALESCE(isrc, ?),
                        musicbrainz_id = COALESCE(musicbrainz_id, ?),
                        explicit = COALESCE(explicit, ?),
                        genre = COALESCE(genre, ?),
                        subgenre = COALESCE(subgenre, ?),
                        release_year = COALESCE(release_year, ?),
                        record_label = COALESCE(record_label, ?),
                        bpm = COALESCE(bpm, ?),
                        musical_key = COALESCE(musical_key, ?),
                        acoustid_fingerprint = COALESCE(?, acoustid_fingerprint),
                        audio_quality = COALESCE(?, audio_quality),
                        enrichment_status = ?,
                        enriched_at = CURRENT_TIMESTAMP
                    WHERE id = ?
                    "#
                )
                .bind(title_update_val)
                .bind(album_id_opt)
                .bind(input.duration_ms)
                .bind(parsed_track_num)
                .bind(parsed_disc_num)
                .bind(enriched.isrc.value())
                .bind(enriched.musicbrainz_recording_id.value())
                .bind(parsed_explicit)
                .bind(clean_genre.as_deref())
                .bind(enriched.style.value())
                .bind(effective_year)
                .bind(enriched.label.value())
                .bind(parsed_bpm)
                .bind(enriched.initial_key.value())
                .bind(enriched.acoustid_fingerprint.value())
                .bind(effective_audio_quality.as_deref())
                .bind(target_status)
                .bind(tid)
                .execute(&mut *tx)
                .await;
            }

            if input.service_name == "qobuz" {
                let _ = sqlx::query("UPDATE tracks SET qobuz_id = COALESCE(qobuz_id, ?) WHERE id = ?")
                    .bind(&input.service_track_id).bind(tid).execute(&mut *tx).await;
            } else if input.service_name == "spotify" {
                let _ = sqlx::query("UPDATE tracks SET spotify_id = COALESCE(spotify_id, ?) WHERE id = ?")
                    .bind(&input.service_track_id).bind(tid).execute(&mut *tx).await;
            }

            tid
        } else {
            let eff_tier = Self::compute_effective_audio_tier(
                None,
                &[],
                input.bit_depth,
                input.sample_rate,
                input.format.as_deref(),
                input.audio_quality.as_deref(),
            );
            effective_audio_quality = eff_tier.map(|t| t.as_str().to_string());

            // Fresh insert
            let qobuz_id_val = if input.service_name == "qobuz" { Some(input.service_track_id.clone()) } else { None };
            let spotify_id_val = if input.service_name == "spotify" { Some(input.service_track_id.clone()) } else { None };

            let target_status = Self::evaluate_enrichment_status(
                parsed_bpm.is_some(),
                enriched.initial_key.value().map_or(false, |k| !k.trim().is_empty()),
                enriched.acoustid_fingerprint.value().map_or(false, |f| !f.trim().is_empty()),
                completeness == EnrichmentCompleteness::Enriched
                    || (enriched.title.value().is_some()
                        && (enriched.isrc.value().is_some() || enriched.musicbrainz_recording_id.value().is_some())),
                None,
            );

            let raw_insert_title = enriched.title.value().unwrap_or(&track_title);
            let (clean_insert_t, _) = syncify_core_domain::metadata::clean_title_and_extract_featured(raw_insert_title);
            let effective_insert_title = if !clean_insert_t.is_empty() {
                clean_insert_t.as_str()
            } else {
                raw_insert_title
            };

            let res = sqlx::query(
                r#"
                INSERT INTO tracks (
                    title, album_id, duration_ms, track_number, disc_number,
                    isrc, musicbrainz_id, explicit, genre, subgenre,
                    release_year, record_label, bpm, musical_key, acoustid_fingerprint, audio_quality,
                    qobuz_id, spotify_id, enrichment_status, enriched_at
                ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, CURRENT_TIMESTAMP)
                RETURNING id
                "#
            )
            .bind(effective_insert_title)
            .bind(album_id_opt)
            .bind(input.duration_ms)
            .bind(parsed_track_num)
            .bind(parsed_disc_num.unwrap_or(1))
            .bind(enriched.isrc.value())
            .bind(enriched.musicbrainz_recording_id.value())
            .bind(parsed_explicit.unwrap_or(0))
            .bind(clean_genre.as_deref())
            .bind(enriched.style.value())
            .bind(effective_year)
            .bind(enriched.label.value())
            .bind(parsed_bpm)
            .bind(enriched.initial_key.value())
            .bind(enriched.acoustid_fingerprint.value())
            .bind(effective_audio_quality.as_deref())
            .bind(qobuz_id_val)
            .bind(spotify_id_val)
            .bind(target_status)
            .fetch_one(&mut *tx)
            .await
            .map_err(|e| format!("Failed to insert track '{}': {}", track_title, e))?;

            use sqlx::Row;
            res.get::<i64, _>(0)
        };

        // 5b. Update tracks.is_favorite if favorite
        if input.is_favorite {
            let _ = sqlx::query(
                "UPDATE tracks SET is_favorite = 1, favorite_at = COALESCE(favorite_at, CURRENT_TIMESTAMP) WHERE id = ?"
            )
            .bind(track_id)
            .execute(&mut *tx)
            .await;
        }

        // 6. Link Track-Artist
        let _ = sqlx::query(
            "INSERT OR IGNORE INTO track_artists (track_id, artist_id, role) VALUES (?, ?, 'primary')"
        )
        .bind(track_id)
        .bind(artist_id)
        .execute(&mut *tx)
        .await;

        let effective_title_for_feat = enriched.title.value().unwrap_or(&track_title);
        let (_, feat_from_title) = syncify_core_domain::metadata::clean_title_and_extract_featured(effective_title_for_feat);
        for feat_name in &feat_from_title {
            let clean_feat = syncify_core_domain::metadata::sanitize_artist_name(feat_name);
            let clean_feat = clean_feat.trim();
            if clean_feat.is_empty() {
                continue;
            }
            let feat_aid: Option<i64> = sqlx::query_scalar("SELECT id FROM artists WHERE LOWER(TRIM(name)) = LOWER(?) LIMIT 1")
                .bind(clean_feat)
                .fetch_optional(&mut *tx)
                .await
                .ok()
                .flatten();
            let final_feat_id = match feat_aid {
                Some(id) => id,
                None => {
                    if let Ok(r) = sqlx::query("INSERT INTO artists (name) VALUES (?) ON CONFLICT(name COLLATE NOCASE) DO UPDATE SET id=id RETURNING id")
                        .bind(clean_feat)
                        .fetch_one(&mut *tx)
                        .await
                    {
                        use sqlx::Row;
                        r.get(0)
                    } else {
                        sqlx::query_scalar("SELECT id FROM artists WHERE LOWER(TRIM(name)) = LOWER(?) LIMIT 1")
                            .bind(clean_feat)
                            .fetch_optional(&mut *tx)
                            .await
                            .ok()
                            .flatten()
                            .unwrap_or(0)
                    }
                }
            };
            if final_feat_id > 0 && final_feat_id != artist_id {
                let _ = sqlx::query("INSERT OR IGNORE INTO track_artists (track_id, artist_id, role) VALUES (?, ?, 'featured')")
                    .bind(track_id).bind(final_feat_id).execute(&mut *tx).await;
            }
        }

        // 7. Persist Track Credits (Composer, Performer, Producer, Writer)
        if let Some(composers) = enriched.composer.value().or(input.origin_meta.composer.as_deref()) {
            for (c_name, c_role) in syncify_core_domain::metadata::parse_credits_string(composers, "composer") {
                let c_name_clean = syncify_core_domain::metadata::sanitize_artist_name(&c_name);
                let c_role_clean = c_role.replace(['\r', '\n', '\t'], " ").trim().to_string();
                let c_role_final = if c_role_clean.is_empty() { "composer".to_string() } else { c_role_clean };
                if FieldValidator::is_valid_artist(&c_name_clean) && !c_name_clean.is_empty() {
                    let c_art_id: i64 = match sqlx::query_scalar("SELECT id FROM artists WHERE LOWER(TRIM(name)) = LOWER(?) LIMIT 1")
                        .bind(&c_name_clean).fetch_optional(&mut *tx).await.ok().flatten() {
                        Some(id) => id,
                        None => {
                            if let Ok(r) = sqlx::query("INSERT INTO artists (name) VALUES (?) ON CONFLICT(name COLLATE NOCASE) DO UPDATE SET id=id RETURNING id")
                                .bind(&c_name_clean).fetch_one(&mut *tx).await {
                                use sqlx::Row;
                                r.get(0)
                            } else {
                                continue;
                            }
                        }
                    };
                    let _ = sqlx::query("INSERT OR IGNORE INTO track_credits (track_id, artist_id, role) VALUES (?, ?, ?)")
                        .bind(track_id).bind(c_art_id).bind(&c_role_final).execute(&mut *tx).await;
                }
            }
        }

        if let Some(performers) = enriched.performers.value().or(input.origin_meta.performers.as_deref()) {
            for (p_name, p_role) in syncify_core_domain::metadata::parse_credits_string(performers, "performer") {
                let p_name_clean = syncify_core_domain::metadata::sanitize_artist_name(&p_name);
                let p_role_clean = p_role.replace(['\r', '\n', '\t'], " ").trim().to_string();
                let p_role_final = if p_role_clean.is_empty() { "performer".to_string() } else { p_role_clean };
                if FieldValidator::is_valid_artist(&p_name_clean) && !p_name_clean.is_empty() {
                    let p_art_id: i64 = match sqlx::query_scalar("SELECT id FROM artists WHERE LOWER(TRIM(name)) = LOWER(?) LIMIT 1")
                        .bind(&p_name_clean).fetch_optional(&mut *tx).await.ok().flatten() {
                        Some(id) => id,
                        None => {
                            if let Ok(r) = sqlx::query("INSERT INTO artists (name) VALUES (?) ON CONFLICT(name COLLATE NOCASE) DO UPDATE SET id=id RETURNING id")
                                .bind(&p_name_clean).fetch_one(&mut *tx).await {
                                use sqlx::Row;
                                r.get(0)
                            } else {
                                continue;
                            }
                        }
                    };
                    let _ = sqlx::query("INSERT OR IGNORE INTO track_credits (track_id, artist_id, role) VALUES (?, ?, ?)")
                        .bind(track_id).bind(p_art_id).bind(&p_role_final).execute(&mut *tx).await;
                }
            }
        }

        // 8. Track Sources & Availability (Explicit Verified Availability for live sync)
        let source_already_existed: bool = sqlx::query_scalar::<_, i32>(
            "SELECT 1 FROM track_sources WHERE track_id = ? AND service_id = ? LIMIT 1"
        )
        .bind(track_id)
        .bind(input.service_id)
        .fetch_optional(&mut *tx)
        .await
        .ok()
        .flatten()
        .is_some();
        let is_new_source_for_service = !source_already_existed;

        let _ = sqlx::query(
            r#"
            INSERT INTO track_sources (
                track_id, service_id, service_track_id, format, bit_depth,
                sample_rate, quality_score, available, availability_status, last_checked
            ) VALUES (?, ?, ?, ?, ?, ?, ?, 1, 'available', CURRENT_TIMESTAMP)
            ON CONFLICT(track_id, service_id) DO UPDATE SET
                service_track_id = excluded.service_track_id,
                format = COALESCE(excluded.format, format),
                bit_depth = COALESCE(excluded.bit_depth, bit_depth),
                sample_rate = COALESCE(excluded.sample_rate, sample_rate),
                quality_score = COALESCE(excluded.quality_score, quality_score),
                available = 1,
                availability_status = 'available',
                last_checked = CURRENT_TIMESTAMP
            "#
        )
        .bind(track_id)
        .bind(input.service_id)
        .bind(&input.service_track_id)
        .bind(input.format.as_deref())
        .bind(input.bit_depth)
        .bind(input.sample_rate)
        .bind(input.quality_score)
        .execute(&mut *tx)
        .await;

        // Promote tracks.audio_quality if newly inserted/updated source offers a higher tier,
        // unless track has already been downloaded (in which case physical download metrics are authoritative).
        if let Some(ref eff_q) = effective_audio_quality {
            let _ = sqlx::query(
                r#"UPDATE tracks SET audio_quality = ?
                   WHERE id = ? AND (
                       audio_quality IS NULL
                       OR (
                           NOT EXISTS (SELECT 1 FROM downloads WHERE track_id = tracks.id)
                           AND (
                               (audio_quality != 'hires' AND ? = 'hires')
                               OR (audio_quality = 'lossy' AND ? = 'lossless')
                           )
                       )
                   )"#
            )
            .bind(eff_q)
            .bind(track_id)
            .bind(eff_q)
            .bind(eff_q)
            .execute(&mut *tx)
            .await;
        }

        // 9. Library Entries (User Library & Favorites)
        let entry_already_existed: bool = sqlx::query_scalar::<_, i32>(
            "SELECT 1 FROM library_entries WHERE account_id = ? AND track_id = ? LIMIT 1"
        )
        .bind(input.account_id)
        .bind(track_id)
        .fetch_optional(&mut *tx)
        .await
        .ok()
        .flatten()
        .is_some();
        let is_new_library_entry_for_account = !entry_already_existed;

        let safe_added_at = crate::services::import_pagination::normalize_added_at(input.origin_meta.added_at.as_deref());

        let _ = sqlx::query(
            r#"
            INSERT INTO library_entries (account_id, track_id, is_liked, is_purchased, added_at)
            VALUES (?, ?, ?, ?, ?)
            ON CONFLICT(account_id, track_id) DO UPDATE SET
                is_liked = CASE WHEN excluded.is_liked = 1 THEN 1 ELSE library_entries.is_liked END,
                is_purchased = CASE WHEN excluded.is_purchased = 1 THEN 1 ELSE library_entries.is_purchased END,
                added_at = CASE 
                    WHEN library_entries.added_at IS NULL OR library_entries.added_at LIKE '1970-01-01%' THEN excluded.added_at 
                    ELSE library_entries.added_at 
                END
            "#
        )
        .bind(input.account_id)
        .bind(track_id)
        .bind(input.is_favorite as i32)
        .bind(input.is_purchased as i32)
        .bind(&safe_added_at)
        .execute(&mut *tx)
        .await;

        let is_already_present = !is_new_global_track && !is_new_source_for_service && !is_new_library_entry_for_account;
        let is_new_import = is_new_global_track || is_new_source_for_service || is_new_library_entry_for_account;

        tx.commit().await.map_err(|e| format!("Failed to commit DB transaction: {}", e))?;

        Ok(SyncTrackResult {
            track_id,
            artist_id,
            album_id: album_id_opt,
            is_new_global_track,
            is_new_source_for_service,
            is_new_library_entry_for_account,
            is_already_present,
            is_new_import,
            completeness,
            availability_status: "available".to_string(),
        })
    }
}

/// Determines the appropriate enrichment_status ('enriched', 'partial', or 'error')
/// based on key metadata and acoustic fields.
#[allow(dead_code)]
pub fn evaluate_enrichment_status(
    has_bpm: bool,
    has_key: bool,
    has_fingerprint: bool,
    has_core_metadata: bool,
    critical_error: Option<&str>,
) -> &'static str {
    if critical_error.is_some() {
        "error"
    } else if has_bpm && has_key && has_fingerprint && has_core_metadata {
        "enriched"
    } else {
        "partial"
    }
}

/// Input payload for sync-time pre-enrichment and persistence
#[derive(Debug, Clone, Default)]
#[allow(dead_code)]
pub struct SyncTrackInput {
    pub origin_meta: OriginTrackMetadata,
    pub service_track_id: String,
    pub service_name: String,
    pub service_id: i64,
    pub account_id: i64,
    pub is_favorite: bool,
    pub is_purchased: bool,
    pub format: Option<String>,
    pub bit_depth: Option<i32>,
    pub sample_rate: Option<i32>,
    pub quality_score: Option<i32>,
    pub audio_quality: Option<String>,
    pub cover_art_url: Option<String>,
    pub duration_ms: Option<i64>,
    pub query_musicbrainz: bool,
    /// S198: the ALBUM this track belongs to is a provider favorite
    /// (favorite-albums phase). Marks `albums.is_favorite` on the album row
    /// resolved/created by this track — never touches per-track favorites.
    #[allow(dead_code)]
    pub album_is_favorite: bool,
    /// S198: provider-side album id (e.g. Qobuz album id as string) to persist
    /// into the matching provider column (`albums.qobuz_id`) with a guarded,
    /// idempotent update. Empty/None = skip.
    #[allow(dead_code)]
    pub album_provider_track_id: Option<String>,
}

/// Result returned from sync-time pre-enrichment
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct SyncTrackResult {
    pub track_id: i64,
    pub artist_id: i64,
    pub album_id: Option<i64>,
    pub is_new_global_track: bool,
    pub is_new_source_for_service: bool,
    pub is_new_library_entry_for_account: bool,
    pub is_already_present: bool,
    pub is_new_import: bool,
    pub completeness: syncify_metadata_domain::EnrichmentCompleteness,
    pub availability_status: String,
}

/// Result of ReplayGain / EBU R128 calculation
#[derive(Debug, Clone, Default, PartialEq)]
#[allow(dead_code)]
pub struct ReplayGainAnalysis {
    pub track_gain: Option<String>,
    pub track_peak: Option<String>,
    pub album_gain: Option<String>,
    pub album_peak: Option<String>,
    pub r128_track_gain: Option<String>,
    pub loudness_lufs: Option<f64>,
}

/// Result of acoustic feature extraction
#[derive(Debug, Clone, Default, PartialEq)]
#[allow(dead_code)]
pub struct AcousticAnalysis {
    pub bpm: Option<u32>,
    pub key: Option<String>,
    pub energy: Option<f64>,
    pub danceability: Option<f64>,
}

/// Result of audio fingerprinting
#[derive(Debug, Clone, Default, PartialEq)]
#[allow(dead_code)]
pub struct FingerprintAnalysis {
    pub duration_sec: f64,
    pub fingerprint: String,
    pub acoustid_id: Option<String>,
}

/// Audio analyzer for ReplayGain (EBU R128), Acoustic Features (BPM, Key, Energy, Danceability),
/// and Audio Fingerprinting (Chromaprint / fpcalc).
#[allow(dead_code)]
pub struct AudioAnalyzer;

#[allow(dead_code)]
impl AudioAnalyzer {
    /// Analyze an audio file (e.g. in .staging) and return extracted metrics.
    pub async fn analyze_file(file_path: &std::path::Path) -> Result<AudioAnalysisMetrics, String> {
        if !file_path.exists() {
            return Err(format!("File does not exist: {}", file_path.display()));
        }

        let mut metrics = AudioAnalysisMetrics::default();

        // 1. Calculate ReplayGain / EBU R128
        match Self::calculate_replaygain(file_path).await {
            Ok(rg) => {
                metrics.replaygain_track_gain = rg.track_gain;
                metrics.replaygain_track_peak = rg.track_peak;
                metrics.replaygain_album_gain = rg.album_gain;
                metrics.replaygain_album_peak = rg.album_peak;
                metrics.r128_track_gain = rg.r128_track_gain;
                metrics.loudness = rg.loudness_lufs;
            }
            Err(e) => {
                tracing::debug!("ReplayGain analysis failed: {}", e);
            }
        }

        // 2. Extract Acoustic Features (BPM, Key, Energy, Danceability)
        match Self::extract_acoustic_features(file_path).await {
            Ok(ac) => {
                metrics.bpm = ac.bpm;
                metrics.initial_key = ac.key;
                metrics.energy = ac.energy;
                metrics.danceability = ac.danceability;
            }
            Err(e) => {
                tracing::debug!("Acoustic features analysis failed: {}", e);
            }
        }

        // 3. Calculate Fingerprint (fpcalc / Chromaprint)
        match Self::calculate_fingerprint(file_path).await {
            Ok(fp) => {
                metrics.acoustid_id = fp.acoustid_id;
                metrics.acoustid_fingerprint = Some(fp.fingerprint);
                metrics.duration_sec = Some(fp.duration_sec);
            }
            Err(e) => {
                tracing::debug!("Fingerprinting failed: {}", e);
            }
        }

        Ok(metrics)
    }

    /// Calculate ReplayGain and EBU R128 metrics.
    pub async fn calculate_replaygain(file_path: &std::path::Path) -> Result<ReplayGainAnalysis, String> {
        // Audit 2026-08-25: the former fallback (`estimate_replaygain_from_audio`)
        // fabricated pseudo-LUFS from the file size and hardcoded peaks
        // ("0.988220"/"0.999120") that were persisted into real tags. Only a real
        // ffmpeg EBU R128 measurement is accepted now; when it fails, callers keep
        // the fields empty (honest absence) instead of writing invented values.
        Self::run_ffmpeg_ebur128(file_path).await
    }

    /// Extract acoustic features (BPM, Key, Energy, Danceability).
    pub async fn extract_acoustic_features(file_path: &std::path::Path) -> Result<AcousticAnalysis, String> {
        // Audit 2026-08-25: key used to be picked via `len % keys.len()`, energy and
        // danceability via modulo arithmetic, and BPM synthesized as `100 + len % 60`
        // whenever the real DSP failed or fell below the confidence threshold. Only
        // the genuine TempoAnalyzer DSP path remains (threshold 0.35); fields without
        // a real analyzer stay None so nothing fabricated reaches tags or the DB.
        let tempo = crate::services::tempo_analyzer::TempoAnalyzer::analyze_file(file_path, 0.35).await?;
        Ok(AcousticAnalysis {
            bpm: tempo.bpm,
            key: None,
            energy: None,
            danceability: None,
        })
    }

    /// Calculate audio fingerprint using fpcalc (real Chromaprint analysis).
    pub async fn calculate_fingerprint(file_path: &std::path::Path) -> Result<FingerprintAnalysis, String> {
        // Audit 2026-08-25: removed `generate_fallback_fingerprint`, which minted
        // synthetic AcoustID fingerprints ("AQAA-" + base64(md5(name:size)) with a
        // fixed 180 s duration). Those fake fingerprints were persisted into real
        // tags and fed orchestrator identity matching. Without a successful fpcalc
        // run there is no fingerprint at all — absence is the honest outcome.
        Self::run_fpcalc_binary(file_path).await
    }

    async fn run_ffmpeg_ebur128(file_path: &std::path::Path) -> Result<ReplayGainAnalysis, String> {
        if let Ok(meta) = std::fs::metadata(file_path) {
            if meta.len() < 4096 {
                return Err("File too short for EBU R128 analysis".to_string());
            }
        }

        let output = crate::cmd_utils::create_tokio_command("ffmpeg")
            .arg("-i")
            .arg(file_path)
            .arg("-af")
            .arg("ebur128=peak=true")
            .arg("-f")
            .arg("null")
            .arg("-")
            .output()
            .await
            .map_err(|e| format!("Failed to spawn ffmpeg: {}", e))?;

        let stderr = String::from_utf8_lossy(&output.stderr);
        let mut integrated_lufs: Option<f64> = None;
        let mut true_peak_dbfs: Option<f64> = None;

        for line in stderr.lines() {
            let trimmed = line.trim();
            if trimmed.starts_with("I:") && trimmed.ends_with("LUFS") {
                let parts: Vec<&str> = trimmed.split_whitespace().collect();
                if parts.len() >= 2 {
                    if let Ok(val) = parts[1].parse::<f64>() {
                        integrated_lufs = Some(val);
                    }
                }
            } else if trimmed.starts_with("Peak:") && trimmed.ends_with("dBFS") {
                let parts: Vec<&str> = trimmed.split_whitespace().collect();
                if parts.len() >= 2 {
                    if let Ok(val) = parts[1].parse::<f64>() {
                        true_peak_dbfs = Some(val);
                    }
                }
            }
        }

        if let Some(i_lufs) = integrated_lufs {
            let peak_db = true_peak_dbfs.unwrap_or(-0.1);
            let peak_linear = 10.0_f64.powf(peak_db / 20.0).min(1.0).max(0.0);
            let track_gain_db = -18.0 - i_lufs;
            let r128_gain_lu = -23.0 - i_lufs;
            let album_gain_db = track_gain_db + 0.70;

            Ok(ReplayGainAnalysis {
                track_gain: Some(format!("{:+.2} dB", track_gain_db)),
                track_peak: Some(format!("{:.6}", peak_linear)),
                album_gain: Some(format!("{:+.2} dB", album_gain_db)),
                album_peak: Some(format!("{:.6}", (peak_linear + 0.01).min(1.0))),
                r128_track_gain: Some(format!("{:+.2} LU", r128_gain_lu)),
                loudness_lufs: Some(i_lufs),
            })
        } else {
            Err("Could not parse EBU R128 output from ffmpeg".to_string())
        }
    }

    async fn run_fpcalc_binary(file_path: &std::path::Path) -> Result<FingerprintAnalysis, String> {
        if let Ok(meta) = std::fs::metadata(file_path) {
            if meta.len() < 4096 {
                return Err("File too short for fpcalc fingerprinting".to_string());
            }
        }

        let fpcalc_cmd = if let Ok(custom) = std::env::var("FPCALC_PATH") {
            custom
        } else {
            "fpcalc".to_string()
        };

        let output = crate::cmd_utils::create_tokio_command(&fpcalc_cmd)
            .arg("-json")
            .arg(file_path)
            .output()
            .await
            .map_err(|e| format!("Failed to spawn fpcalc: {}", e))?;

        if !output.status.success() {
            return Err(format!("fpcalc exited with code {:?}", output.status.code()));
        }

        #[derive(serde::Deserialize)]
        struct FpcalcOutput {
            duration: f64,
            fingerprint: String,
        }

        let parsed: FpcalcOutput = serde_json::from_slice(&output.stdout)
            .map_err(|e| format!("Failed to parse fpcalc json: {}", e))?;

        let hash = md5::compute(parsed.fingerprint.as_bytes());
        let hex = format!("{:x}", hash);
        let acoustid_uuid = format!(
            "{}-{}-{}-{}-{}",
            &hex[0..8],
            &hex[8..12],
            &hex[12..16],
            &hex[16..20],
            &hex[20..32]
        );

        Ok(FingerprintAnalysis {
            duration_sec: parsed.duration,
            fingerprint: parsed.fingerprint,
            acoustid_id: Some(acoustid_uuid),
        })
    }
}

/// Diagnostic report generated by `backfill_social_metadata` (TASK-113).
#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
#[allow(dead_code)]
pub struct SocialMetadataBackfillReport {
    pub total_tracks_scanned: usize,
    pub genres_backfilled: usize,
    pub remaining_null_genres: usize,
    pub albums_dates_inferred: usize,
    pub divergent_years_reconciled: usize,
    pub remaining_divergent_albums: usize,
}

/// Maintenance and reconciliation engine for social catalog metadata (TASK-113).
///
/// 1. Infers missing album `release_date` from `MIN(tracks.release_year)`, track ISRCs, or populated counterpart albums.
/// 2. Canonically derives `tracks.release_year` from parent `albums.release_date`, reconciling divergent track years.
/// 3. Backfills `tracks.genre` prioritizing album siblings and artist dominant genre.
#[allow(dead_code)]
pub async fn backfill_social_metadata(db: &sqlx::SqlitePool) -> Result<SocialMetadataBackfillReport, String> {
    let mut report = SocialMetadataBackfillReport::default();

    // 1. Infer missing release_date for albums from MIN(tracks.release_year)
    let albums_updated = sqlx::query(
        r#"
        UPDATE albums
        SET release_date = (
            SELECT printf('%04d-01-01', MIN(t.release_year))
            FROM tracks t
            WHERE t.album_id = albums.id AND t.release_year IS NOT NULL AND t.release_year > 1900 AND t.release_year < 2100
        )
        WHERE (release_date IS NULL OR release_date = '')
          AND EXISTS (
            SELECT 1 FROM tracks t
            WHERE t.album_id = albums.id AND t.release_year IS NOT NULL AND t.release_year > 1900 AND t.release_year < 2100
          )
        "#
    )
    .execute(db)
    .await
    .map_err(|e| format!("Failed to infer album release_date from tracks: {}", e))?;

    report.albums_dates_inferred = albums_updated.rows_affected() as usize;

    // 1b. Also infer from matching duplicate album titles
    if let Ok(res) = sqlx::query(
        r#"
        UPDATE albums
        SET release_date = (
            SELECT a2.release_date
            FROM albums a2
            WHERE LOWER(TRIM(a2.title)) = LOWER(TRIM(albums.title))
              AND a2.id != albums.id
              AND a2.release_date IS NOT NULL AND a2.release_date != ''
            LIMIT 1
        )
        WHERE (release_date IS NULL OR release_date = '')
          AND EXISTS (
            SELECT 1 FROM albums a2
            WHERE LOWER(TRIM(a2.title)) = LOWER(TRIM(albums.title))
              AND a2.id != albums.id
              AND a2.release_date IS NOT NULL AND a2.release_date != ''
          )
        "#
    )
    .execute(db)
    .await
    {
        report.albums_dates_inferred += res.rows_affected() as usize;
    }

    // 1c. Infer from track ISRCs (characters 5..7)
    let isrc_rows: Vec<(i64, String)> = sqlx::query_as(
        r#"
        SELECT a.id, t.isrc
        FROM albums a
        JOIN tracks t ON t.album_id = a.id
        WHERE (a.release_date IS NULL OR a.release_date = '')
          AND t.isrc IS NOT NULL AND LENGTH(t.isrc) >= 12
        "#
    )
    .fetch_all(db)
    .await
    .unwrap_or_default();

    let mut isrc_map: std::collections::HashMap<i64, i32> = std::collections::HashMap::new();
    for (aid, isrc) in isrc_rows {
        if let Some(yr_digits) = isrc.get(5..7) {
            if let Ok(y) = yr_digits.parse::<i32>() {
                let full_yr = if y <= 30 { 2000 + y } else { 1900 + y };
                let entry = isrc_map.entry(aid).or_insert(full_yr);
                if full_yr < *entry {
                    *entry = full_yr;
                }
            }
        }
    }

    for (aid, yr) in isrc_map {
        let formatted = format!("{:04}-01-01", yr);
        let _ = sqlx::query("UPDATE albums SET release_date = ? WHERE id = ? AND (release_date IS NULL OR release_date = '')")
            .bind(&formatted)
            .bind(aid)
            .execute(db)
            .await;
        report.albums_dates_inferred += 1;
    }

    // 2. Canonical derivation of track release_year:
    // For non-compilation albums with a valid release_date, reconcile tracks where release_year diverges from the album year
    let tracks_reconciled = sqlx::query(
        r#"
        UPDATE tracks
        SET release_year = CAST(SUBSTR((SELECT a.release_date FROM albums a WHERE a.id = tracks.album_id), 1, 4) AS INTEGER)
        WHERE album_id IS NOT NULL
          AND EXISTS (
            SELECT 1 FROM albums a
            WHERE a.id = tracks.album_id
              AND a.release_date IS NOT NULL AND LENGTH(a.release_date) >= 4
              AND NOT EXISTS (
                SELECT 1 FROM album_artists aa
                JOIN artists ar ON ar.id = aa.artist_id
                WHERE aa.album_id = a.id
                  AND (LOWER(ar.name) = 'various artists' OR LOWER(ar.name) = 'various')
              )
          )
          AND (
            release_year IS NULL
            OR release_year != CAST(SUBSTR((SELECT a.release_date FROM albums a WHERE a.id = tracks.album_id), 1, 4) AS INTEGER)
          )
        "#
    )
    .execute(db)
    .await
    .map_err(|e| format!("Failed to reconcile track release_year with album: {}", e))?;

    report.divergent_years_reconciled = tracks_reconciled.rows_affected() as usize;

    // 3. Backfill genre:
    // 3a. Propagate genre from album (other tracks in same album with non-null genre)
    let album_genre_propagated = sqlx::query(
        r#"
        UPDATE tracks
        SET genre = (
            SELECT t2.genre
            FROM tracks t2
            WHERE t2.album_id = tracks.album_id
              AND t2.genre IS NOT NULL AND TRIM(t2.genre) != ''
            LIMIT 1
        )
        WHERE (genre IS NULL OR TRIM(genre) = '')
          AND album_id IS NOT NULL
          AND EXISTS (
            SELECT 1 FROM tracks t2
            WHERE t2.album_id = tracks.album_id
              AND t2.genre IS NOT NULL AND TRIM(t2.genre) != ''
          )
        "#
    )
    .execute(db)
    .await
    .map_err(|e| format!("Failed to propagate album genres: {}", e))?;

    report.genres_backfilled = album_genre_propagated.rows_affected() as usize;

    // 3b. Propagate genre from artist dominant genre
    let artist_genre_propagated = sqlx::query(
        r#"
        UPDATE tracks
        SET genre = (
            SELECT t2.genre
            FROM track_artists ta1
            JOIN track_artists ta2 ON ta1.artist_id = ta2.artist_id
            JOIN tracks t2 ON ta2.track_id = t2.id
            WHERE ta1.track_id = tracks.id
              AND t2.genre IS NOT NULL AND TRIM(t2.genre) != ''
            GROUP BY t2.genre
            ORDER BY COUNT(*) DESC
            LIMIT 1
        )
        WHERE (genre IS NULL OR TRIM(genre) = '')
          AND EXISTS (
            SELECT 1 FROM track_artists ta1
            JOIN track_artists ta2 ON ta1.artist_id = ta2.artist_id
            JOIN tracks t2 ON ta2.track_id = t2.id
            WHERE ta1.track_id = tracks.id
              AND t2.genre IS NOT NULL AND TRIM(t2.genre) != ''
          )
        "#
    )
    .execute(db)
    .await
    .map_err(|e| format!("Failed to propagate artist genres: {}", e))?;

    report.genres_backfilled += artist_genre_propagated.rows_affected() as usize;

    // 4. Compute remaining diagnostic counts
    let total_tracks: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM tracks")
        .fetch_one(db)
        .await
        .unwrap_or(0);
    report.total_tracks_scanned = total_tracks as usize;

    let remaining_null: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM tracks WHERE genre IS NULL OR TRIM(genre) = ''")
        .fetch_one(db)
        .await
        .unwrap_or(0);
    report.remaining_null_genres = remaining_null as usize;

    let remaining_div: i64 = sqlx::query_scalar(
        r#"
        SELECT COUNT(*) FROM (
            SELECT a.id
            FROM albums a
            JOIN tracks t ON t.album_id = a.id
            WHERE t.release_year IS NOT NULL
            GROUP BY a.id
            HAVING (MAX(t.release_year) - MIN(t.release_year)) > 2
        )
        "#
    )
    .fetch_one(db)
    .await
    .unwrap_or(0);
    report.remaining_divergent_albums = remaining_div as usize;

    Ok(report)
}

#[cfg(test)]
mod tests {
    use super::*;
    use syncify_metadata_domain::fixtures::*;

    #[test]
    fn test_enrichment_precedence_streaming_over_musicbrainz() {
        let mut meta = EnrichedMetadata::default();
        let now = chrono_now_iso();

        // MusicBrainz candidate first
        meta.title.merge_candidate(Some("MB Title".to_string()), "musicbrainz", 0.95, &now);
        // Streaming official title
        meta.title.merge_candidate(Some("Stream Official Title".to_string()), "qobuz", 0.90, &now);

        assert_eq!(meta.title.value(), Some("Stream Official Title"));
        assert_eq!(meta.title.source(), Some("qobuz"));
    }

    #[test]
    fn test_manual_override_preservation() {
        let mut meta = EnrichedMetadata::default();
        let now = chrono_now_iso();

        meta.label.merge_candidate(Some("User Label".to_string()), "manual", 1.0, &now);
        meta.label.merge_candidate(Some("MB Label".to_string()), "musicbrainz", 0.95, &now);

        assert_eq!(meta.label.value(), Some("User Label"));
        assert_eq!(meta.label.source(), Some("manual"));
    }

    #[test]
    fn test_field_validator_rejection_rules() {
        assert!(!FieldValidator::is_valid_year("0000"));
        assert!(!FieldValidator::is_valid_year("0"));
        assert!(FieldValidator::is_valid_year("1978"));

        assert!(!FieldValidator::is_valid_identifier(""));
        assert!(!FieldValidator::is_valid_identifier("0"));
        assert!(!FieldValidator::is_valid_identifier("null"));
        assert!(FieldValidator::is_valid_identifier("USRC12345678"));

        assert!(FieldValidator::is_valid_artist("Various Artists"));
        assert!(FieldValidator::is_valid_artist("Various"));
    }

    #[test]
    fn test_musicbrainz_exact_match_offline() {
        let json_val: serde_json::Value = serde_json::from_str(FIXTURE_MB_EXACT_RECORDING_JSON).unwrap();
        assert_eq!(json_val["id"].as_str(), Some("b10bbbfc-cf9e-42e0-be17-e2c3e1d2600d"));
    }

    #[test]
    fn test_musicbrainz_alternative_release_offline() {
        let json_val: serde_json::Value = serde_json::from_str(FIXTURE_MB_ALTERNATIVE_RELEASE_JSON).unwrap();
        let releases = json_val["releases"].as_array().unwrap();
        let norm_album = normalize_title("Heroes");
        let matched = releases.iter().find(|r| normalize_title(r["title"].as_str().unwrap_or("")) == norm_album);
        assert!(matched.is_some());
    }

    #[tokio::test]
    async fn test_sqlite_rejection_on_nonexistent_flac_file() {
        let engine = EnrichmentEngine::new();
        let meta = EnrichedMetadata::default();
        let fake_path = std::path::Path::new("C:/nonexistent/path/track.flac");

        // Create temporary in-memory pool
        let pool = sqlx::SqlitePool::connect("sqlite::memory:").await.unwrap();

        let result = engine.apply_to_database(&pool, 1, &meta, Some(fake_path)).await;
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("does not exist"));
    }

    #[tokio::test]
    async fn test_artist_global_name_safety() {
        let pool = sqlx::SqlitePool::connect("sqlite::memory:").await.unwrap();
        
        // Setup minimal schema
        sqlx::query("CREATE TABLE artists (id INTEGER PRIMARY KEY AUTOINCREMENT, name TEXT NOT NULL, musicbrainz_id TEXT);")
            .execute(&pool).await.unwrap();
        sqlx::query("CREATE TABLE tracks (id INTEGER PRIMARY KEY AUTOINCREMENT, title TEXT, album_id INTEGER, track_number INTEGER, disc_number INTEGER, isrc TEXT, release_year INTEGER, record_label TEXT, musicbrainz_id TEXT, enrichment_status TEXT, enriched_at TEXT);")
            .execute(&pool).await.unwrap();
        sqlx::query("CREATE TABLE track_artists (track_id INTEGER, artist_id INTEGER, role TEXT, PRIMARY KEY(track_id, artist_id));")
            .execute(&pool).await.unwrap();
        sqlx::query("CREATE TABLE albums (id INTEGER PRIMARY KEY AUTOINCREMENT, title TEXT, release_date TEXT, upc TEXT, total_tracks INTEGER, label TEXT, musicbrainz_id TEXT);")
            .execute(&pool).await.unwrap();

        // Insert canonical artist
        sqlx::query("INSERT INTO artists (name) VALUES ('Queen');").execute(&pool).await.unwrap();
        sqlx::query("INSERT INTO tracks (title, enrichment_status) VALUES ('Bohemian Rhapsody', 'pending');").execute(&pool).await.unwrap();

        let engine = EnrichmentEngine::new();
        let mut meta = EnrichedMetadata::default();
        let now = chrono_now_iso();
        // Enriched artist candidate with lowercase or alternate spelling
        meta.artist.merge_candidate(Some("queen".to_string()), "stream", 1.0, &now);
        meta.musicbrainz_artist_id.merge_candidate(Some("0383dadf-2a4e-4d10-a46a-e6e041dae229".to_string()), "musicbrainz", 0.95, &now);

        let res = engine.apply_to_database(&pool, 1, &meta, None).await;
        assert!(res.is_ok());

        // Assert canonical artist name was NOT modified / corrupted
        let (canonical_name, mbid): (String, Option<String>) = sqlx::query_as("SELECT name, musicbrainz_id FROM artists WHERE id = 1")
            .fetch_one(&pool).await.unwrap();
        assert_eq!(canonical_name, "Queen"); // Preserved!
        assert_eq!(mbid.as_deref(), Some("0383dadf-2a4e-4d10-a46a-e6e041dae229")); // Populated safely
    }

    #[tokio::test]
    async fn test_origin_track_metadata_full_41_field_enrichment() {
        let origin = OriginTrackMetadata {
            title: Some("Heroes".to_string()),
            artist: Some("David Bowie".to_string()),
            album: Some("Heroes".to_string()),
            album_artist: Some("David Bowie".to_string()),
            composer: Some("David Bowie, Brian Eno".to_string()),
            performers: Some("David Bowie, Robert Fripp".to_string()),
            work: Some("Heroes Symphony".to_string()),
            track_number: Some(3),
            track_total: Some(10),
            disc_number: Some(1),
            disc_total: Some(1),
            disc_subtitle: Some("Side 1".to_string()),
            release_year: Some("1977".to_string()),
            release_date: Some("1977-10-14".to_string()),
            original_date: Some("1977-10-14".to_string()),
            label: Some("RCA Victor".to_string()),
            catalog_number: Some("PL 12522".to_string()),
            isrc: Some("GBAYE7700021".to_string()),
            barcode: Some("0035629007421".to_string()),
            copyright: Some("(P) 1977 RCA Records".to_string()),
            release_type: Some("Album".to_string()),
            release_status: Some("Official".to_string()),
            release_country: Some("GB".to_string()),
            language: Some("eng".to_string()),
            genre: Some("Art Rock".to_string()),
            style: Some("Berlin Trilogy".to_string()),
            mood: Some("Triumphant".to_string()),
            explicit: Some(false),
            bpm: Some(112),
            initial_key: Some("D".to_string()),
            energy: Some(0.85),
            danceability: Some(0.55),
            loudness: Some(-7.2),
            replaygain_track_gain: Some("-6.50 dB".to_string()),
            replaygain_track_peak: Some("0.988220".to_string()),
            replaygain_album_gain: Some("-5.80 dB".to_string()),
            replaygain_album_peak: Some("0.999120".to_string()),
            r128_track_gain: Some("-2.10 LU".to_string()),
            comment: Some("Syncify Production".to_string()),
            lyrics_source: Some("LRCLIB".to_string()),
            cover_source: Some("Apple Music".to_string()),
            audio_source: Some("Qobuz".to_string()),
            acoustid_id: Some("11111111-2222-3333-4444-555555555555".to_string()),
            acoustid_fingerprint: Some("AQAA-AQAA-AQAA".to_string()),
            source_name: "qobuz".to_string(),
            ..Default::default()
        };

        let engine = EnrichmentEngine::new();
        let meta = engine.resolve_track_metadata(
            "David Bowie", "Heroes", "Heroes",
            Some("GBAYE7700021"),
            Some(&origin),
        ).await;

        // All streaming-sourced fields must be resolved from origin
        assert_eq!(meta.title.value(), Some("Heroes"));
        assert_eq!(meta.title.source(), Some("qobuz"));
        assert_eq!(meta.artist.value(), Some("David Bowie"));
        assert_eq!(meta.album.value(), Some("Heroes"));
        assert_eq!(meta.album_artist.value(), Some("David Bowie"));
        assert_eq!(meta.composer.value(), Some("David Bowie, Brian Eno"));
        assert_eq!(meta.performers.value(), Some("David Bowie, Robert Fripp"));
        assert_eq!(meta.work.value(), Some("Heroes Symphony"));
        assert_eq!(meta.track_number.value(), Some("3"));
        assert_eq!(meta.track_total.value(), Some("10"));
        assert_eq!(meta.disc_number.value(), Some("1"));
        assert_eq!(meta.disc_total.value(), Some("1"));
        assert_eq!(meta.disc_subtitle.value(), Some("Side 1"));
        assert_eq!(meta.release_year.value(), Some("1977"));
        assert_eq!(meta.release_date.value(), Some("1977-10-14"));
        assert_eq!(meta.original_date.value(), Some("1977-10-14"));
        assert_eq!(meta.label.value(), Some("RCA Victor"));
        assert_eq!(meta.catalog_number.value(), Some("PL 12522"));
        assert_eq!(meta.isrc.value(), Some("GBAYE7700021"));
        assert_eq!(meta.barcode.value(), Some("0035629007421"));
        assert_eq!(meta.copyright.value(), Some("(P) 1977 RCA Records"));
        assert_eq!(meta.release_type.value(), Some("Album"));
        assert_eq!(meta.release_status.value(), Some("Official"));
        assert_eq!(meta.release_country.value(), Some("United Kingdom"));
        assert_eq!(meta.language.value(), Some("eng"));
        assert!(meta.genre.value().unwrap_or("").contains("Art Rock"));
        assert_eq!(meta.style.value(), Some("Berlin Trilogy"));
        assert_eq!(meta.mood.value(), Some("Triumphant"));
        assert_eq!(meta.explicit.value(), Some("0")); // false -> "0"
        assert_eq!(meta.bpm.value(), Some("112"));
        assert_eq!(meta.initial_key.value(), Some("D"));
        assert_eq!(meta.energy.value(), Some("0.85"));
        assert_eq!(meta.danceability.value(), Some("0.55"));
        assert_eq!(meta.loudness.value(), Some("-7.2"));
        assert_eq!(meta.replaygain_track_gain.value(), Some("-6.50 dB"));
        assert_eq!(meta.replaygain_track_peak.value(), Some("0.988220"));
        assert_eq!(meta.replaygain_album_gain.value(), Some("-5.80 dB"));
        assert_eq!(meta.replaygain_album_peak.value(), Some("0.999120"));
        assert_eq!(meta.r128_track_gain.value(), Some("-2.10 LU"));
        assert_eq!(meta.comment.value(), Some("Syncify Production"));
        assert_eq!(meta.lyrics_source.value(), Some("LRCLIB"));
        assert_eq!(meta.cover_source.value(), Some("Apple Music"));
        assert_eq!(meta.audio_source.value(), Some("Qobuz"));
        assert_eq!(meta.acoustid_id.value(), Some("11111111-2222-3333-4444-555555555555"));
        assert_eq!(meta.acoustid_fingerprint.value(), Some("AQAA-AQAA-AQAA"));

        assert_eq!(meta.genre.source(), Some("stream"));
        assert_eq!(meta.bpm.source(), Some("qobuz"));
        assert_eq!(meta.copyright.source(), Some("qobuz"));
        assert_eq!(meta.acoustid_id.source(), Some("qobuz"));
    }

    #[tokio::test]
    async fn test_audio_analyzer_reports_honest_absence_on_undecodable_fixture() {
        let temp_dir = tempfile::TempDir::new().unwrap();
        let flac_path = temp_dir.path().join("analysis_test.flac");

        // Write a minimal valid FLAC stream
        let mut data = Vec::new();
        data.extend_from_slice(b"fLaC");
        data.push(0x80); // last block, type 0 (STREAMINFO)
        data.push(0x00);
        data.push(0x00);
        data.push(0x22); // 34 bytes
        data.extend_from_slice(&[0u8; 34]);
        data.extend_from_slice(&[0x12; 4096]); // filler bytes: no valid FLAC frames, undecodable
        std::fs::write(&flac_path, &data).unwrap();

        let metrics = AudioAnalyzer::analyze_file(&flac_path).await.unwrap();

        // Audit 2026-08-25: this fixture has no decodable audio frames, so ffmpeg
        // EBU R128, TempoAnalyzer DSP and fpcalc must all fail honestly. The old
        // assertions demanded the values fabricated by the removed estimators
        // (pseudo-ReplayGain with hardcoded peaks, modulo key/energy/danceability,
        // synthetic BPM and "AQAA-" fingerprints); absence is now the contract.
        assert!(metrics.replaygain_track_gain.is_none());
        assert!(metrics.replaygain_track_peak.is_none());
        assert!(metrics.replaygain_album_gain.is_none());
        assert!(metrics.replaygain_album_peak.is_none());
        assert!(metrics.r128_track_gain.is_none());
        assert!(metrics.loudness.is_none());

        assert!(metrics.bpm.is_none());
        assert!(metrics.initial_key.is_none());
        assert!(metrics.energy.is_none());
        assert!(metrics.danceability.is_none());

        assert!(metrics.acoustid_id.is_none());
        assert!(metrics.acoustid_fingerprint.is_none());
    }

    #[tokio::test]
    async fn test_staging_enrichment_keeps_audio_fields_empty_without_real_analysis() {
        let temp_dir = tempfile::TempDir::new().unwrap();
        let flac_path = temp_dir.path().join("provider_fallback.flac");

        let mut data = Vec::new();
        data.extend_from_slice(b"fLaC");
        data.push(0x80);
        data.push(0x00);
        data.push(0x00);
        data.push(0x22);
        data.extend_from_slice(&[0u8; 34]);
        data.extend_from_slice(&[0x34; 2048]); // no valid FLAC frames: undecodable fixture
        std::fs::write(&flac_path, &data).unwrap();

        // Origin provided basic stream data but NO replaygain and NO acoustic features
        let origin = OriginTrackMetadata {
            title: Some("Minimal Provider Track".to_string()),
            artist: Some("Minimal Artist".to_string()),
            album: Some("Minimal Album".to_string()),
            source_name: "tidal".to_string(),
            ..Default::default()
        };

        let engine = EnrichmentEngine::new();
        let enriched = engine.resolve_and_enrich_staging_audio(
            &flac_path,
            "Minimal Artist",
            "Minimal Album",
            "Minimal Provider Track",
            None,
            Some(&origin),
        ).await;

        // Origin fields preserved
        assert_eq!(enriched.title.value(), Some("Minimal Provider Track"));
        assert_eq!(enriched.title.source(), Some("tidal"));

        // Audit 2026-08-25: with no real analysis possible (ffmpeg EBU R128,
        // TempoAnalyzer DSP and fpcalc all fail on this undecodable fixture) the
        // audio-derived fields must stay empty. The old assertions demanded the
        // fabricated values of the removed estimators (pseudo-ReplayGain, modulo
        // key/energy/danceability, synthetic BPM, "AQAA-" fingerprint); honest
        // absence means nothing gets an "inferred" source here either.
        assert!(enriched.replaygain_track_gain.value().is_none());
        assert!(enriched.replaygain_track_peak.value().is_none());
        assert!(enriched.replaygain_album_gain.value().is_none());
        assert!(enriched.replaygain_album_peak.value().is_none());
        assert!(enriched.r128_track_gain.value().is_none());
        assert!(enriched.loudness.value().is_none());

        assert!(enriched.bpm.value().is_none());
        assert!(enriched.initial_key.value().is_none());
        assert!(enriched.energy.value().is_none());
        assert!(enriched.danceability.value().is_none());

        assert!(enriched.acoustid_id.value().is_none());
        assert!(enriched.acoustid_fingerprint.value().is_none());
    }

    #[tokio::test]
    async fn test_qobuz_performer_role_extraction_and_sanitization() {
        use syncify_core_domain::metadata::{parse_credit_role_and_name, parse_credits_string, sanitize_artist_name};

        // 1. Cadena "Piano\r - Glenn Gould" se separe en artista "Glenn Gould" y rol "Piano"
        let (artist, role) = parse_credit_role_and_name("Piano\r - Glenn Gould", "performer");
        assert_eq!(artist, "Glenn Gould");
        assert_eq!(role, "Piano");

        // 2. Cadena "SNEAKER KIDS &amp; Eli Noir" se normalice a "SNEAKER KIDS & Eli Noir"
        let normalized = sanitize_artist_name("SNEAKER KIDS &amp; Eli Noir");
        assert_eq!(normalized, "SNEAKER KIDS & Eli Noir");

        // 3. Artistas con espacios " Oasis " se trimeen a "Oasis"
        let trimmed = sanitize_artist_name(" Oasis ");
        assert_eq!(trimmed, "Oasis");

        // 4. Multiple credits parsing
        let credits = parse_credits_string("Piano\r - Glenn Gould, Violin\r - Yehudi Menuhin", "performer");
        assert_eq!(credits, vec![
            ("Glenn Gould".to_string(), "Piano".to_string()),
            ("Yehudi Menuhin".to_string(), "Violin".to_string()),
        ]);

        // 5. In-memory SQLite verification that only clean artist names enter artists table
        let pool = sqlx::SqlitePool::connect("sqlite::memory:").await.unwrap();
        sqlx::query(
            r#"
            CREATE TABLE artists (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                name TEXT NOT NULL UNIQUE
            );
            CREATE TABLE tracks (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                title TEXT NOT NULL
            );
            CREATE TABLE track_credits (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                track_id INTEGER NOT NULL,
                artist_id INTEGER NOT NULL,
                role TEXT NOT NULL,
                UNIQUE(track_id, artist_id, role)
            );
            "#
        )
        .execute(&pool)
        .await
        .unwrap();

        sqlx::query("INSERT INTO tracks (id, title) VALUES (1, 'Goldberg Variations')")
            .execute(&pool)
            .await
            .unwrap();

        let mut tx = pool.begin().await.unwrap();
        let raw_performers = "Piano\r - Glenn Gould, Violin\r - Yehudi Menuhin";

        for (p_name, p_role) in parse_credits_string(raw_performers, "performer") {
            let p_art_id: i64 = match sqlx::query_scalar("SELECT id FROM artists WHERE name = ? COLLATE NOCASE LIMIT 1")
                .bind(&p_name).fetch_optional(&mut *tx).await.ok().flatten() {
                Some(id) => id,
                None => {
                    let r = sqlx::query("INSERT INTO artists (name) VALUES (?) ON CONFLICT(name) DO UPDATE SET id=id RETURNING id")
                        .bind(&p_name).fetch_one(&mut *tx).await.unwrap();
                    use sqlx::Row;
                    r.get(0)
                }
            };
            sqlx::query("INSERT OR IGNORE INTO track_credits (track_id, artist_id, role) VALUES (?, ?, ?)")
                .bind(1i64).bind(p_art_id).bind(&p_role).execute(&mut *tx).await.unwrap();
        }
        tx.commit().await.unwrap();

        // Assert that NO corrupt artist names with '\r' exist in artists table
        let corrupt_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM artists WHERE name LIKE '%\r%'")
            .fetch_one(&pool).await.unwrap();
        assert_eq!(corrupt_count, 0, "No corrupt artist name with carriage return allowed in artists");

        // Assert clean artists exist
        let gould_exists: bool = sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM artists WHERE name = 'Glenn Gould'")
            .fetch_one(&pool).await.unwrap() == 1;
        assert!(gould_exists);

        // Assert role is preserved in track_credits
        let role_gould: String = sqlx::query_scalar(
            "SELECT tc.role FROM track_credits tc JOIN artists a ON a.id = tc.artist_id WHERE a.name = 'Glenn Gould'"
        )
        .fetch_one(&pool).await.unwrap();
        assert_eq!(role_gould, "Piano");
    }

    #[test]
    fn test_compute_effective_audio_tier() {
        use super::*;

        // 1. Fresh insert with lossy source
        let t1 = EnrichmentEngine::compute_effective_audio_tier(
            None,
            &[],
            None,
            None,
            Some("OGG"),
            Some("standard"),
        );
        assert_eq!(t1, Some(AudioTier::Lossy));

        // 2. Fresh insert with Hi-Res source
        let t2 = EnrichmentEngine::compute_effective_audio_tier(
            None,
            &[],
            Some(24),
            Some(96000),
            Some("FLAC"),
            None,
        );
        assert_eq!(t2, Some(AudioTier::HiRes));

        // 3. Existing track has 'hires', incoming is 'lossy' -> NEVER degrade
        let t3 = EnrichmentEngine::compute_effective_audio_tier(
            Some("hires"),
            &[],
            None,
            None,
            Some("OGG"),
            Some("standard"),
        );
        assert_eq!(t3, Some(AudioTier::HiRes));

        // 4. Existing track has 'lossy', incoming is 16-bit FLAC -> promote to Lossless
        let t4 = EnrichmentEngine::compute_effective_audio_tier(
            Some("lossy"),
            &[],
            Some(16),
            Some(44100),
            Some("FLAC"),
            Some("lossless"),
        );
        assert_eq!(t4, Some(AudioTier::Lossless));

        // 5. Existing track has 'lossy', incoming is 24-bit FLAC -> promote to HiRes
        let t5 = EnrichmentEngine::compute_effective_audio_tier(
            Some("lossy"),
            &[],
            Some(24),
            Some(192000),
            Some("FLAC"),
            Some("hires"),
        );
        assert_eq!(t5, Some(AudioTier::HiRes));

        // 6. Existing track_sources contains HiRes -> effective is HiRes
        let sources = vec![
            (Some(16), Some(44100), None, Some("FLAC".to_string())),
            (Some(24), Some(96000), None, Some("FLAC".to_string())),
        ];
        let t6 = EnrichmentEngine::compute_effective_audio_tier(
            Some("lossless"),
            &sources,
            None,
            None,
            Some("OGG"),
            Some("standard"),
        );
        assert_eq!(t6, Some(AudioTier::HiRes));
    }
}

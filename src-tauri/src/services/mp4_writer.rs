//! MP4 / M4A Metadata Tag Writer and Verifier for `src-tauri` using `mp4ameta`.
//! Applies standard iTunes-compatible metadata atoms (`©nam`, `©ART`, `aART`, `©alb`, `©day`, `trkn`, `disk`, `covr`, `©lyr`, etc.)
//! and performs post-write verification to ensure tags are physically present in the file.

use mp4ameta::{Data, Fourcc, FreeformIdent, Tag};
use serde::{Deserialize, Serialize};
use std::path::Path;
use tracing::{debug, info, warn};

/// Metadata DTO for MP4/M4A audio containers
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Mp4Metadata {
    pub title: String,
    pub artist: String,
    pub album: String,
    pub album_artist: Option<String>,
    pub composer: Option<String>,
    pub performer: Option<String>,
    pub genre: Option<String>,
    pub release_year: Option<String>,
    pub release_date: Option<String>,
    pub original_date: Option<String>,
    pub track_number: u32,
    pub track_total: u32,
    pub disc_number: u32,
    pub disc_total: u32,
    pub total_discs: Option<u32>,
    pub disc_track_total: Option<u32>,
    pub isrc: Option<String>,
    pub label: Option<String>,
    pub catalog_number: Option<String>,
    pub barcode: Option<String>,
    pub release_country: Option<String>,
    pub language: Option<String>,
    pub copyright: Option<String>,
    pub bpm: Option<u32>,
    pub comment: Option<String>,
    pub lyrics: Option<String>,
    pub cover_data: Option<Vec<u8>>,
    pub cover_mime: Option<String>,
    pub musicbrainz_track_id: Option<String>,
    pub musicbrainz_artist_id: Option<String>,
    pub musicbrainz_album_id: Option<String>,
    pub musicbrainz_albumartist_id: Option<String>,
    pub musicbrainz_release_group_id: Option<String>,
    pub replaygain_track_gain: Option<String>,
    pub replaygain_track_peak: Option<String>,
    pub replaygain_album_gain: Option<String>,
    pub replaygain_album_peak: Option<String>,
    pub r128_track_gain: Option<String>,
    pub audio_source: Option<String>,
    pub explicit: Option<bool>,
    pub compilation: Option<bool>,
    pub grouping: Option<String>,
    pub style: Option<String>,
    pub mood: Option<String>,
    pub tags: Option<String>,
    pub artist_tags: Option<Vec<String>>,
    pub media_type: Option<String>,
}

impl Mp4Metadata {
    /// Return the effective total tracks for the specific disc.
    /// In multidisc releases, track total must reflect the local disc track count,
    /// preferring `disc_track_total` if set, otherwise falling back to `track_total`.
    pub fn effective_track_total(&self) -> u32 {
        self.disc_track_total.filter(|&t| t > 0).unwrap_or(self.track_total)
    }

    /// Return the effective disc total for the release.
    /// Prefers `total_discs` if set, otherwise falling back to `disc_total`.
    pub fn effective_disc_total(&self) -> u32 {
        self.total_discs.filter(|&d| d > 0).unwrap_or(self.disc_total)
    }
}

/// Verification report after writing MP4/M4A tags
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct Mp4TagVerification {
    pub file_exists: bool,
    pub tags_match: bool,
    pub title_matches: bool,
    pub artist_matches: bool,
    pub album_matches: bool,
    pub album_artist_matches: bool,
    pub track_number_matches: bool,
    pub cover_present: bool,
    pub lyrics_present: bool,
    pub isrc_present: bool,
    pub musicbrainz_present: bool,
    pub mismatches: Vec<(String, String, String)>,
}

/// Apply metadata tags to an MP4/M4A file using `mp4ameta`
pub fn apply_mp4_tags(file_path: &Path, metadata: &Mp4Metadata) -> Result<(), String> {
    if !file_path.exists() {
        return Err(format!("File does not exist: {:?}", file_path));
    }

    let mut tag = Tag::read_from_path(file_path)
        .map_err(|e| format!("Failed to read MP4/M4A file for tagging {:?}: {}", file_path, e))?;

    // Title (©nam)
    if !metadata.title.trim().is_empty() {
        tag.set_title(metadata.title.trim());
    }

    // Artist (©ART)
    if !metadata.artist.trim().is_empty() {
        tag.set_artist(metadata.artist.trim());
    }

    // Album (©alb)
    if !metadata.album.trim().is_empty() {
        tag.set_album(metadata.album.trim());
    }

    // Album Artist (aART)
    if let Some(ref aa) = metadata.album_artist {
        if !aa.trim().is_empty() {
            tag.set_album_artist(aa.trim());
        }
    }

    // Composer (©wrt)
    if let Some(ref c) = metadata.composer {
        if !c.trim().is_empty() {
            tag.set_composer(c.trim());
        }
    }

    // Genre (standard ©gen atom).
    // Do NOT additionally write a freeform ----:com.apple.iTunes:GENRE atom: while both
    // exist, ffmpeg/ffprobe exports only "GENRE" and drops the standard lowercase "genre"
    // key regardless of atom order (verified empirically against Lavf63), which breaks
    // external-tool tag parity. `tag.genre()` reads ©gen for all internal consumers.
    if let Some(ref g) = metadata.genre {
        if !g.trim().is_empty() {
            let fused = syncify_metadata_domain::fuse_genres(&[g.as_str()]);
            let genre_str = if !fused.is_empty() { fused.join("; ") } else { g.trim().to_string() };
            tag.set_genre(&genre_str);
        }
    }

    // BPM / Tempo (tmpo)
    if let Some(bpm) = metadata.bpm {
        if bpm > 0 {
            tag.set_bpm(bpm as u16);
            let ident_bpm = FreeformIdent::new_static("com.apple.iTunes", "BPM");
            tag.set_data(ident_bpm, Data::Utf8(bpm.to_string()));
        }
    }

    // Year / Release Date (©day)
    if let Some(ref date) = metadata.release_date {
        if !date.trim().is_empty() {
            tag.set_year(date.trim());
        }
    } else if let Some(ref yr) = metadata.release_year {
        if !yr.trim().is_empty() {
            tag.set_year(yr.trim());
        }
    }

    // Track Number & Total (trkn)
    let effective_track_total = metadata.effective_track_total();
    if metadata.track_number > 0 {
        tag.set_track_number(metadata.track_number as u16);
        if effective_track_total > 0 {
            tag.set_total_tracks(effective_track_total as u16);
        }
    } else if effective_track_total > 0 {
        tag.set_total_tracks(effective_track_total as u16);
    }

    // Disc Number & Total (disk)
    let effective_disc_total = metadata.effective_disc_total();
    if metadata.disc_number > 0 || effective_disc_total > 0 {
        let disc_num = if metadata.disc_number > 0 { metadata.disc_number } else { 1 };
        tag.set_disc_number(disc_num as u16);
        if effective_disc_total > 0 {
            tag.set_total_discs(effective_disc_total as u16);
        }
    }

    // Comment (©cmt)
    if let Some(ref cmt) = metadata.comment {
        if !cmt.trim().is_empty() {
            tag.set_comment(cmt.trim());
        }
    }

    // Lyrics (©lyr)
    if let Some(ref lyr) = metadata.lyrics {
        if !lyr.trim().is_empty() {
            tag.set_lyrics(lyr.trim());
        }
    }

    // Freeform standard iTunes atoms (----:com.apple.iTunes:*)
    if let Some(ref isrc) = metadata.isrc {
        if !isrc.trim().is_empty() {
            let ident = FreeformIdent::new_static("com.apple.iTunes", "ISRC");
            tag.set_data(ident, Data::Utf8(isrc.trim().to_string()));
        }
    }

    if let Some(ref perf) = metadata.performer {
        if !perf.trim().is_empty() {
            let ident = FreeformIdent::new_static("com.apple.iTunes", "PERFORMER");
            tag.set_data(ident, Data::Utf8(perf.trim().to_string()));
        }
    }

    if let Some(ref od) = metadata.original_date {
        if !od.trim().is_empty() {
            let ident = FreeformIdent::new_static("com.apple.iTunes", "ORIGINALDATE");
            tag.set_data(ident, Data::Utf8(od.trim().to_string()));
        }
    }

    // Copyright (cprt)
    if let Some(ref cprt) = metadata.copyright {
        if !cprt.trim().is_empty() {
            tag.set_copyright(cprt.trim());
        }
    }

    if let Some(ref lbl) = metadata.label {
        if !lbl.trim().is_empty() {
            let labels = syncify_metadata_domain::fuse_labels(&[lbl.as_str()]);
            let label_str = if !labels.is_empty() { labels.join("; ") } else { lbl.trim().to_string() };
            let ident = FreeformIdent::new_static("com.apple.iTunes", "LABEL");
            tag.set_data(ident, Data::Utf8(label_str.clone()));
            let ident_rl = FreeformIdent::new_static("com.apple.iTunes", "RECORDLABEL");
            tag.set_data(ident_rl, Data::Utf8(label_str.clone()));
            let ident_org = FreeformIdent::new_static("com.apple.iTunes", "ORGANIZATION");
            tag.set_data(ident_org, Data::Utf8(label_str));
        }
    }

    if let Some(ref cat) = metadata.catalog_number {
        if !cat.trim().is_empty() {
            let ident = FreeformIdent::new_static("com.apple.iTunes", "CATALOGNUMBER");
            tag.set_data(ident, Data::Utf8(cat.trim().to_string()));
        }
    }

    if let Some(ref bc) = metadata.barcode {
        if !bc.trim().is_empty() {
            let ident = FreeformIdent::new_static("com.apple.iTunes", "BARCODE");
            tag.set_data(ident, Data::Utf8(bc.trim().to_string()));
            let ident_upc = FreeformIdent::new_static("com.apple.iTunes", "UPC");
            tag.set_data(ident_upc, Data::Utf8(bc.trim().to_string()));
        }
    }

    if let Some(ref cntry) = metadata.release_country {
        if !cntry.trim().is_empty() {
            // directiva del propietario 2026-08-24: nombres en el cable; anula contrato alpha-2 de S183.
            // Freeform COUNTRY / RELEASECOUNTRY atoms carry the canonical English name
            // when resolvable ("DE"/"Germany" -> "Germany"); regions keep their canonical
            // name and unrecognized values pass through verbatim (never invented). The
            // SAME domain helper (wire_country_value) backs verification below, so the
            // auditor always compares against exactly what was written.
            let norm_cntry = syncify_metadata_domain::wire_country_value(cntry.trim());
            let ident = FreeformIdent::new_static("com.apple.iTunes", "COUNTRY");
            tag.set_data(ident, Data::Utf8(norm_cntry.clone()));
            let ident_rc = FreeformIdent::new_static("com.apple.iTunes", "RELEASECOUNTRY");
            tag.set_data(ident_rc, Data::Utf8(norm_cntry.clone()));
            let ident_lc = FreeformIdent::new_static("com.apple.iTunes", "country");
            tag.set_data(ident_lc, Data::Utf8(norm_cntry));
        }
    }

    // Language (©lng) — standard iTunes atom.
    // NOTE (S184): the freeform ----:com.apple.iTunes:LANGUAGE atom must stay ABSENT per
    // the standing Symfonium contract (language_tag_roundtrip_test and four more suites
    // pin its absence), so the owner-directive name travels in ©lng. The shared
    // wire_language_value helper backs both this write and the verifier below.
    if let Some(ref lang) = metadata.language {
        if !lang.trim().is_empty() {
            // directiva del propietario 2026-08-24: nombres en el cable; anula contrato alpha-2 de S183.
            let norm_lang = syncify_metadata_domain::wire_language_value(lang.trim());
            tag.set_data(Fourcc(*b"\xa9lng"), Data::Utf8(norm_lang));
        }
    }

    // Compilation (cpil & TCMP)
    if let Some(comp) = metadata.compilation {
        if comp {
            let ident_cpil = FreeformIdent::new_static("com.apple.iTunes", "COMPILATION");
            tag.set_data(ident_cpil, Data::Utf8("1".to_string()));
            tag.set_data(Fourcc(*b"cpil"), Data::Reserved(vec![1]));
            tag.set_data(Fourcc(*b"TCMP"), Data::Utf8("1".to_string()));
        }
    }

    // Grouping (©grp)
    if let Some(ref grp) = metadata.grouping {
        if !grp.trim().is_empty() {
            tag.set_data(Fourcc(*b"\xa9grp"), Data::Utf8(grp.trim().to_string()));
            let ident_grp = FreeformIdent::new_static("com.apple.iTunes", "GROUPING");
            tag.set_data(ident_grp, Data::Utf8(grp.trim().to_string()));
        }
    }

    // Style, Mood & Freeform Tags
    if let Some(ref style) = metadata.style {
        if !style.trim().is_empty() {
            let ident_style = FreeformIdent::new_static("com.apple.iTunes", "STYLE");
            tag.set_data(ident_style, Data::Utf8(style.trim().to_string()));
        }
    }
    if let Some(ref mood) = metadata.mood {
        if !mood.trim().is_empty() {
            let ident_mood = FreeformIdent::new_static("com.apple.iTunes", "MOOD");
            tag.set_data(ident_mood, Data::Utf8(mood.trim().to_string()));
        }
    }
    if let Some(ref tags) = metadata.tags {
        if !tags.trim().is_empty() {
            let ident_tags = FreeformIdent::new_static("com.apple.iTunes", "TAGS");
            tag.set_data(ident_tags, Data::Utf8(tags.trim().to_string()));
        }
    }
    if let Some(ref artist_tags) = metadata.artist_tags {
        let valid_tags: Vec<String> = artist_tags
            .iter()
            .flat_map(|t| syncify_metadata_domain::fuse_genres(&[t.as_str()]))
            .filter(|t| !t.trim().is_empty())
            .collect();
        if !valid_tags.is_empty() {
            let ident_art_tags = FreeformIdent::new_static("com.apple.iTunes", "ARTISTS_TAGS");
            tag.set_data(ident_art_tags, Data::Utf8(valid_tags.join("; ")));
        }
    }
    if let Some(ref media_type) = metadata.media_type {
        if !media_type.trim().is_empty() {
            let ident_media = FreeformIdent::new_static("com.apple.iTunes", "MEDIA");
            tag.set_data(ident_media, Data::Utf8(media_type.trim().to_string()));
        }
    }

    if let Some(ref src) = metadata.audio_source {
        if !src.trim().is_empty() {
            let ident = FreeformIdent::new_static("com.apple.iTunes", "SOURCE");
            tag.set_data(ident, Data::Utf8(src.trim().to_string()));
            let ident_eng = FreeformIdent::new_static("com.apple.iTunes", "ENGINE");
            tag.set_data(ident_eng, Data::Utf8("Syncify Production".to_string()));
        }
    }

    // MusicBrainz Identifiers
    if let Some(ref mb_trk) = metadata.musicbrainz_track_id {
        if !mb_trk.trim().is_empty() {
            let ident = FreeformIdent::new_static("com.apple.iTunes", "MusicBrainz Track Id");
            tag.set_data(ident, Data::Utf8(mb_trk.trim().to_string()));
        }
    }

    if let Some(ref mb_art) = metadata.musicbrainz_artist_id {
        if !mb_art.trim().is_empty() {
            let ident = FreeformIdent::new_static("com.apple.iTunes", "MusicBrainz Artist Id");
            tag.set_data(ident, Data::Utf8(mb_art.trim().to_string()));
        }
    }

    if let Some(ref mb_alb) = metadata.musicbrainz_album_id {
        if !mb_alb.trim().is_empty() {
            let ident = FreeformIdent::new_static("com.apple.iTunes", "MusicBrainz Album Id");
            tag.set_data(ident, Data::Utf8(mb_alb.trim().to_string()));
        }
    }

    if let Some(ref mb_albart) = metadata.musicbrainz_albumartist_id {
        if !mb_albart.trim().is_empty() {
            let ident = FreeformIdent::new_static("com.apple.iTunes", "MusicBrainz Album Artist Id");
            tag.set_data(ident, Data::Utf8(mb_albart.trim().to_string()));
        }
    }

    if let Some(ref mb_rg) = metadata.musicbrainz_release_group_id {
        if !mb_rg.trim().is_empty() {
            let ident = FreeformIdent::new_static("com.apple.iTunes", "MusicBrainz Release Group Id");
            tag.set_data(ident, Data::Utf8(mb_rg.trim().to_string()));
        }
    }

    // ReplayGain
    if let Some(ref rgtg) = metadata.replaygain_track_gain {
        if !rgtg.trim().is_empty() {
            let ident = FreeformIdent::new_static("com.apple.iTunes", "replaygain_track_gain");
            tag.set_data(ident, Data::Utf8(rgtg.trim().to_string()));
        }
    }
    if let Some(ref rgtp) = metadata.replaygain_track_peak {
        if !rgtp.trim().is_empty() {
            let ident = FreeformIdent::new_static("com.apple.iTunes", "replaygain_track_peak");
            tag.set_data(ident, Data::Utf8(rgtp.trim().to_string()));
        }
    }
    if let Some(ref rgag) = metadata.replaygain_album_gain {
        if !rgag.trim().is_empty() {
            let ident = FreeformIdent::new_static("com.apple.iTunes", "replaygain_album_gain");
            tag.set_data(ident, Data::Utf8(rgag.trim().to_string()));
        }
    }
    if let Some(ref rgap) = metadata.replaygain_album_peak {
        if !rgap.trim().is_empty() {
            let ident = FreeformIdent::new_static("com.apple.iTunes", "replaygain_album_peak");
            tag.set_data(ident, Data::Utf8(rgap.trim().to_string()));
        }
    }

    // Artwork (covr)
    if let Some(ref cover_bytes) = metadata.cover_data {
        if !cover_bytes.is_empty() {
            let is_png = cover_bytes.starts_with(b"\x89PNG");
            let data = if is_png {
                Data::Png(cover_bytes.clone())
            } else {
                Data::Jpeg(cover_bytes.clone())
            };
            tag.set_data(Fourcc(*b"covr"), data);
            debug!("Embedded {} bytes cover art in MP4/M4A at {:?}", cover_bytes.len(), file_path);
        }
    }

    tag.write_to_path(file_path)
        .map_err(|e| format!("Failed to write MP4/M4A tags to {:?}: {}", file_path, e))?;

    info!("Successfully wrote MP4/M4A tags to {:?}", file_path);
    Ok(())
}

/// Verify that tags are physically present and match expectations in the MP4/M4A file
pub fn verify_mp4_tags(file_path: &Path, expected: &Mp4Metadata) -> Result<Mp4TagVerification, String> {
    if !file_path.exists() {
        return Err(format!("File does not exist: {:?}", file_path));
    }

    let tag = Tag::read_from_path(file_path)
        .map_err(|e| format!("Failed to read MP4/M4A file for verification {:?}: {}", file_path, e))?;

    let mut verification = Mp4TagVerification {
        file_exists: true,
        tags_match: true,
        ..Default::default()
    };

    let mut mismatches = Vec::new();

    // Check title
    if !expected.title.trim().is_empty() {
        match tag.title() {
            Some(t) if t.trim() == expected.title.trim() => {
                verification.title_matches = true;
            }
            Some(t) => {
                verification.tags_match = false;
                mismatches.push(("TITLE".to_string(), expected.title.clone(), t.to_string()));
            }
            None => {
                verification.tags_match = false;
                mismatches.push(("TITLE".to_string(), expected.title.clone(), "<missing>".to_string()));
            }
        }
    }

    // Check artist
    if !expected.artist.trim().is_empty() {
        match tag.artist() {
            Some(a) if a.trim() == expected.artist.trim() => {
                verification.artist_matches = true;
            }
            Some(a) => {
                verification.tags_match = false;
                mismatches.push(("ARTIST".to_string(), expected.artist.clone(), a.to_string()));
            }
            None => {
                verification.tags_match = false;
                mismatches.push(("ARTIST".to_string(), expected.artist.clone(), "<missing>".to_string()));
            }
        }
    }

    // Check album
    if !expected.album.trim().is_empty() {
        match tag.album() {
            Some(a) if a.trim() == expected.album.trim() => {
                verification.album_matches = true;
            }
            Some(a) => {
                verification.tags_match = false;
                mismatches.push(("ALBUM".to_string(), expected.album.clone(), a.to_string()));
            }
            None => {
                verification.tags_match = false;
                mismatches.push(("ALBUM".to_string(), expected.album.clone(), "<missing>".to_string()));
            }
        }
    }

    // Check album artist
    if let Some(ref exp_aa) = expected.album_artist {
        if !exp_aa.trim().is_empty() {
            match tag.album_artist() {
                Some(aa) if aa.trim() == exp_aa.trim() => {
                    verification.album_artist_matches = true;
                }
                Some(aa) => {
                    verification.tags_match = false;
                    mismatches.push(("ALBUMARTIST".to_string(), exp_aa.clone(), aa.to_string()));
                }
                None => {
                    verification.tags_match = false;
                    mismatches.push(("ALBUMARTIST".to_string(), exp_aa.clone(), "<missing>".to_string()));
                }
            }
        }
    }

    // Check track number
    if expected.track_number > 0 {
        match tag.track_number() {
            Some(tn) if tn == expected.track_number as u16 => {
                verification.track_number_matches = true;
            }
            Some(tn) => {
                verification.tags_match = false;
                mismatches.push(("TRACKNUMBER".to_string(), expected.track_number.to_string(), tn.to_string()));
            }
            None => {
                verification.tags_match = false;
                mismatches.push(("TRACKNUMBER".to_string(), expected.track_number.to_string(), "<missing>".to_string()));
            }
        }
    }

    // Check disc number & total discs
    if expected.disc_number > 0 {
        match tag.disc_number() {
            Some(dn) if dn == expected.disc_number as u16 => {}
            Some(dn) => {
                verification.tags_match = false;
                mismatches.push(("DISCNUMBER".to_string(), expected.disc_number.to_string(), dn.to_string()));
            }
            None => {
                verification.tags_match = false;
                mismatches.push(("DISCNUMBER".to_string(), expected.disc_number.to_string(), "<missing>".to_string()));
            }
        }
    }
    let effective_disc_total = expected.effective_disc_total();
    if effective_disc_total > 0 {
        match tag.total_discs() {
            Some(dt) if dt == effective_disc_total as u16 => {}
            Some(dt) => {
                verification.tags_match = false;
                mismatches.push(("DISCTOTAL".to_string(), effective_disc_total.to_string(), dt.to_string()));
            }
            None => {
                verification.tags_match = false;
                mismatches.push(("DISCTOTAL".to_string(), effective_disc_total.to_string(), "<missing>".to_string()));
            }
        }
    }

    // Check genre
    if let Some(ref exp_g) = expected.genre {
        if !exp_g.trim().is_empty() {
            let fused_exp = syncify_metadata_domain::format_fused_genres(&[exp_g.as_str()])
                .unwrap_or_else(|| exp_g.trim().to_string());
            match tag.genre() {
                Some(g) if g.trim() == exp_g.trim() || g.trim() == fused_exp.trim() => {}
                Some(g) => {
                    verification.tags_match = false;
                    mismatches.push(("GENRE".to_string(), exp_g.clone(), g.to_string()));
                }
                None => {
                    verification.tags_match = false;
                    mismatches.push(("GENRE".to_string(), exp_g.clone(), "<missing>".to_string()));
                }
            }
        }
    }

    // Check bpm
    if let Some(bpm) = expected.bpm {
        if bpm > 0 {
            match tag.bpm() {
                Some(b) if b == bpm as u16 => {}
                Some(b) => {
                    verification.tags_match = false;
                    mismatches.push(("BPM".to_string(), bpm.to_string(), b.to_string()));
                }
                None => {
                    verification.tags_match = false;
                    mismatches.push(("BPM".to_string(), bpm.to_string(), "<missing>".to_string()));
                }
            }
        }
    }

    // Check country
    if let Some(ref exp_cntry) = expected.release_country {
        if !exp_cntry.trim().is_empty() {
            // directiva del propietario 2026-08-24: nombres en el cable; anula contrato alpha-2 de S183.
            // Mirror of apply_mp4_tags via the SAME shared helper: canonical English
            // name for sovereign countries, region name for regions, unknown verbatim.
            let norm_cntry = syncify_metadata_domain::wire_country_value(exp_cntry.trim());
            let cntry_ident = FreeformIdent::new_static("com.apple.iTunes", "COUNTRY");
            let found_cntry = tag.strings_of(&cntry_ident).next().map(|s| s.to_string());
            match found_cntry {
                Some(c) if c.trim() == norm_cntry => {}
                Some(c) => {
                    verification.tags_match = false;
                    mismatches.push(("COUNTRY".to_string(), norm_cntry, c));
                }
                None => {
                    verification.tags_match = false;
                    mismatches.push(("COUNTRY".to_string(), norm_cntry, "<missing>".to_string()));
                }
            }
        }
    }

    // Check language (©lng standard atom)
    if let Some(ref exp_lang) = expected.language {
        if !exp_lang.trim().is_empty() {
            // directiva del propietario 2026-08-24: nombres en el cable; anula contrato alpha-2 de S183.
            let norm_lang = syncify_metadata_domain::wire_language_value(exp_lang.trim());
            let found_lang = tag.strings_of(&Fourcc(*b"\xa9lng")).next().map(|s| s.to_string());
            match found_lang {
                Some(l) if l.trim() == norm_lang => {}
                Some(l) => {
                    verification.tags_match = false;
                    mismatches.push(("LANGUAGE".to_string(), norm_lang, l));
                }
                None => {
                    verification.tags_match = false;
                    mismatches.push(("LANGUAGE".to_string(), norm_lang, "<missing>".to_string()));
                }
            }
        }
    }

    // Check artwork
    if tag.artwork().is_some() || tag.artworks().next().is_some() {
        verification.cover_present = true;
    }

    // Check lyrics
    if tag.lyrics().is_some() {
        verification.lyrics_present = true;
    }

    // Check ISRC
    if expected.isrc.is_some() {
        let isrc_ident = FreeformIdent::new_static("com.apple.iTunes", "ISRC");
        if tag.strings_of(&isrc_ident).next().is_some() {
            verification.isrc_present = true;
        }
    }

    // Check MBIDs
    if expected.musicbrainz_track_id.is_some() {
        let mb_ident = FreeformIdent::new_static("com.apple.iTunes", "MusicBrainz Track Id");
        if tag.strings_of(&mb_ident).next().is_some() {
            verification.musicbrainz_present = true;
        }
    }

    verification.mismatches = mismatches;
    Ok(verification)
}

/// Apply tags and immediately verify them in one step
pub fn apply_and_verify_mp4_tags(file_path: &Path, metadata: &Mp4Metadata) -> Result<Mp4TagVerification, String> {
    apply_mp4_tags(file_path, metadata)?;
    let verification = verify_mp4_tags(file_path, metadata)?;

    if !verification.tags_match {
        let mismatch_desc = verification
            .mismatches
            .iter()
            .map(|(k, exp, got)| format!("{}: expected '{}', got '{}'", k, exp, got))
            .collect::<Vec<_>>()
            .join("; ");
        warn!("[MP4TagVerification] Tag verification mismatches in {:?}: {}", file_path, mismatch_desc);
        return Err(format!("MP4 tag verification failed: {}", mismatch_desc));
    }

    Ok(verification)
}

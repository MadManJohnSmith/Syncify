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
    pub isrc: Option<String>,
    pub label: Option<String>,
    pub catalog_number: Option<String>,
    pub barcode: Option<String>,
    pub release_country: Option<String>,
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

    // Genre (©gen)
    if let Some(ref g) = metadata.genre {
        if !g.trim().is_empty() {
            tag.set_genre(g.trim());
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
    if metadata.track_number > 0 {
        tag.set_track_number(metadata.track_number as u16);
        if metadata.track_total > 0 {
            tag.set_total_tracks(metadata.track_total as u16);
        }
    }

    // Disc Number & Total (disk)
    if metadata.disc_number > 0 {
        tag.set_disc_number(metadata.disc_number as u16);
        if metadata.disc_total > 0 {
            tag.set_total_discs(metadata.disc_total as u16);
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

    if let Some(ref lbl) = metadata.label {
        if !lbl.trim().is_empty() {
            let ident = FreeformIdent::new_static("com.apple.iTunes", "LABEL");
            tag.set_data(ident, Data::Utf8(lbl.trim().to_string()));
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
        }
    }

    if let Some(ref cntry) = metadata.release_country {
        if !cntry.trim().is_empty() {
            let ident = FreeformIdent::new_static("com.apple.iTunes", "country");
            tag.set_data(ident, Data::Utf8(cntry.trim().to_string()));
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

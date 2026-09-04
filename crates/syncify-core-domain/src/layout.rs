//! Library Layout Engine & Path Sanitization for Syncify
//!
//! Provides deterministic folder structure generation, file naming template substitution,
//! Windows-safe sanitization, and sidecar path resolution.

use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

/// Sanitizes a string for use as a directory or file name across Windows, Linux, and macOS.
///
/// Rules applied:
/// 1. Replaces Windows forbidden characters: `< > : " / \ | ? *` with `_`
/// 2. Replaces ASCII control characters (0..31) with `_`
/// 3. Trims leading and trailing whitespace and dots (which Windows prohibits on directories/files)
/// 4. Protects against reserved Windows device names: `CON, PRN, AUX, NUL, COM1..9, LPT1..9`
pub fn sanitize_filename(name: &str) -> String {
    let s: String = name
        .chars()
        .map(|c| match c {
            '<' | '>' | ':' | '"' | '/' | '\\' | '|' | '?' | '*' => '_',
            c if c.is_control() => '_',
            c => c,
        })
        .collect();

    let trimmed = s.trim_matches(&[' ', '.'][..]);
    if trimmed.is_empty() {
        return "Unknown".to_string();
    }

    // Windows reserved device names check
    let upper = trimmed.to_ascii_uppercase();
    let is_reserved = matches!(
        upper.as_str(),
        "CON"
            | "PRN"
            | "AUX"
            | "NUL"
            | "COM1"
            | "COM2"
            | "COM3"
            | "COM4"
            | "COM5"
            | "COM6"
            | "COM7"
            | "COM8"
            | "COM9"
            | "LPT1"
            | "LPT2"
            | "LPT3"
            | "LPT4"
            | "LPT5"
            | "LPT6"
            | "LPT7"
            | "LPT8"
            | "LPT9"
    );

    if is_reserved {
        format!("{}_", trimmed)
    } else {
        trimmed.to_string()
    }
}

/// Configuration options for folder and file templates (matching `folder_settings` table in SQLite)
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct FolderFileTemplateConfig {
    pub folder_template: String,
    pub file_template: String,
    pub artist_separator: String,
    pub replace_spaces_with: Option<String>,
    pub max_path_length: usize,
}

impl Default for FolderFileTemplateConfig {
    fn default() -> Self {
        Self {
            folder_template: "{AlbumArtist}/[{Year}] {Album}".to_string(),
            file_template: "{TrackNumber:pad2} - {Title}".to_string(),
            artist_separator: ", ".to_string(),
            replace_spaces_with: None,
            max_path_length: 255,
        }
    }
}

/// Metadata context passed to template substitution
#[derive(Debug, Clone, Default)]
pub struct TrackLayoutContext<'a> {
    pub artist: &'a str,
    pub album_artist: Option<&'a str>,
    pub album: &'a str,
    pub title: &'a str,
    pub year: Option<i32>,
    pub original_date: Option<&'a str>,
    pub track_number: u32,
    pub track_total: Option<u32>,
    pub disc_number: u32,
    pub total_discs: u32,
    pub format: &'a str,
    pub bit_depth: Option<i32>,
    pub sample_rate: Option<f64>,
}

/// Structured Library Layout Engine
#[derive(Debug, Clone)]
pub struct LibraryLayout {
    pub base_dir: PathBuf,
    pub config: FolderFileTemplateConfig,
}

impl LibraryLayout {
    pub fn new(base_dir: impl AsRef<Path>) -> Self {
        Self {
            base_dir: base_dir.as_ref().to_path_buf(),
            config: FolderFileTemplateConfig::default(),
        }
    }

    pub fn with_config(base_dir: impl AsRef<Path>, config: FolderFileTemplateConfig) -> Self {
        Self {
            base_dir: base_dir.as_ref().to_path_buf(),
            config,
        }
    }

    /// Effective album artist (defaults to track artist if absent)
    pub fn effective_album_artist<'a>(&self, ctx: &TrackLayoutContext<'a>) -> &'a str {
        ctx.album_artist
            .filter(|s| !s.trim().is_empty())
            .unwrap_or(ctx.artist)
    }

    /// Path to Artist Directory: `{base_dir}/{AlbumArtist}`
    pub fn artist_dir(&self, artist: &str) -> PathBuf {
        let safe_artist = sanitize_filename(artist);
        let safe_artist = self.apply_space_replacement(&safe_artist);
        self.base_dir.join(safe_artist)
    }

    /// Path to Album Directory: `{base_dir}/{AlbumArtist}/[{Year}] {Album}`
    pub fn album_dir(&self, artist: &str, album: &str, year: Option<i32>) -> PathBuf {
        let safe_artist = sanitize_filename(artist);
        let safe_artist = self.apply_space_replacement(&safe_artist);

        let safe_album = sanitize_filename(album);
        let safe_album = self.apply_space_replacement(&safe_album);

        let folder_name = match year {
            Some(y) if (1900..=2100).contains(&y) => format!("[{}] {}", y, safe_album),
            _ => safe_album,
        };

        self.base_dir.join(safe_artist).join(folder_name)
    }

    /// Path to Album Directory dynamically resolved from template configuration (`folder_template`)
    pub fn format_album_dir(&self, album_artist: &str, album: &str, year: Option<i32>) -> PathBuf {
        let safe_album_artist = sanitize_filename(album_artist);
        let safe_album_artist = self.apply_space_replacement(&safe_album_artist);

        let safe_album = sanitize_filename(album);
        let safe_album = self.apply_space_replacement(&safe_album);

        let year_str = match year {
            Some(y) if (1900..=2100).contains(&y) => y.to_string(),
            _ => String::new(),
        };

        let mut folder_rel = self.config.folder_template.clone();
        folder_rel = folder_rel
            .replace("{AlbumArtist}", &safe_album_artist)
            .replace("{Artist}", &safe_album_artist)
            .replace("{Album}", &safe_album)
            .replace("{Year}", &year_str)
            .replace("{OriginalDate}", &year_str)
            .replace("{Title}", "")
            .replace("{DiscNumber:pad2}", "")
            .replace("{DiscNumber}", "");

        let folder_parts: Vec<String> = folder_rel
            .split('/')
            .map(|p| {
                let s = sanitize_filename(p);
                self.apply_space_replacement(&s)
            })
            .filter(|p| !p.is_empty())
            .collect();

        let mut target_dir = self.base_dir.clone();
        for part in folder_parts {
            target_dir.push(part);
        }
        target_dir
    }

    /// Path to Disc Directory (if multi-disc): `{AlbumDir}/Disc {DiscNumber}`
    pub fn disc_dir(
        &self,
        artist: &str,
        album: &str,
        year: Option<i32>,
        disc_number: u32,
        total_discs: u32,
    ) -> PathBuf {
        let alb_dir = self.album_dir(artist, album, year);
        if total_discs > 1 {
            alb_dir.join(format!("Disc {}", disc_number))
        } else {
            alb_dir
        }
    }

    /// Path to Track File using standard canonical layout:
    /// `{TargetDir}/{TrackNumber:02} - {Title}.{ext}`
    /// (For Various Artists: `{TrackNumber:02} - {TrackArtist} - {Title}.{ext}`)
    pub fn track_path(
        &self,
        album_artist: &str,
        track_artist: &str,
        album: &str,
        year: Option<i32>,
        disc_number: u32,
        total_discs: u32,
        track_number: u32,
        title: &str,
        ext: &str,
    ) -> PathBuf {
        let target_dir = self.disc_dir(album_artist, album, year, disc_number, total_discs);
        let safe_title = sanitize_filename(title);
        let safe_title = self.apply_space_replacement(&safe_title);
        let ext_clean = ext.trim_start_matches('.');

        let is_va = album_artist.eq_ignore_ascii_case("Various Artists")
            || album_artist.eq_ignore_ascii_case("VA")
            || album_artist.eq_ignore_ascii_case("Various");

        let file_name = if is_va {
            let safe_track_artist = sanitize_filename(track_artist);
            let safe_track_artist = self.apply_space_replacement(&safe_track_artist);
            format!(
                "{:02} - {} - {}.{}",
                track_number, safe_track_artist, safe_title, ext_clean
            )
        } else {
            format!("{:02} - {}.{}", track_number, safe_title, ext_clean)
        };

        target_dir.join(file_name)
    }

    /// Path to Track File resolved dynamically from template configuration
    pub fn resolve_track_path(&self, ctx: &TrackLayoutContext) -> PathBuf {
        let album_artist = self.effective_album_artist(ctx);
        let safe_artist = sanitize_filename(ctx.artist);
        let safe_album_artist = sanitize_filename(album_artist);
        let safe_album = sanitize_filename(ctx.album);
        let safe_title = sanitize_filename(ctx.title);
        let ext_clean = ctx.format.trim_start_matches('.');

        let year_str = ctx
            .year
            .map(|y| y.to_string())
            .or_else(|| ctx.original_date.and_then(|d| d.get(..4).map(|s| s.to_string())))
            .unwrap_or_default();

        let orig_date_str = ctx.original_date.unwrap_or(&year_str);

        // 1. Substitute Folder Template
        let mut folder_rel = self.config.folder_template.clone();
        folder_rel = folder_rel
            .replace("{AlbumArtist}", &safe_album_artist)
            .replace("{Artist}", &safe_artist)
            .replace("{Album}", &safe_album)
            .replace("{Year}", &year_str)
            .replace("{OriginalDate}", orig_date_str)
            .replace("{Title}", &safe_title)
            .replace("{DiscNumber:pad2}", &format!("{:02}", ctx.disc_number))
            .replace("{DiscNumber}", &ctx.disc_number.to_string());

        // Clean folder parts and apply space replacements
        let mut folder_parts: Vec<String> = folder_rel
            .split('/')
            .map(|p| {
                let s = sanitize_filename(p);
                self.apply_space_replacement(&s)
            })
            .filter(|p| !p.is_empty())
            .collect();

        let max_len = if self.config.max_path_length > 0 {
            self.config.max_path_length
        } else {
            255
        };

        let mut target_dir = self.base_dir.clone();
        let base_len = target_dir.to_string_lossy().len();
        let budget_for_folders_and_file = if max_len > base_len + 15 {
            max_len - base_len
        } else {
            30
        };
        let part_budget = (budget_for_folders_and_file / (folder_parts.len().max(1) + 1)).max(10);

        for part in &mut folder_parts {
            if part.len() > part_budget && max_len < 260 {
                let trimmed = part[..part_budget].trim_end_matches(&[' ', '.'][..]);
                *part = trimmed.to_string();
            }
            target_dir.push(&*part);
        }

        // Multi-disc subfolder if not already specified in folder_template
        if ctx.total_discs > 1 && !self.config.folder_template.contains("{DiscNumber") {
            target_dir.push(format!("Disc {}", ctx.disc_number));
        }

        // 2. Substitute File Template
        let is_va = album_artist.eq_ignore_ascii_case("Various Artists")
            || album_artist.eq_ignore_ascii_case("VA")
            || album_artist.eq_ignore_ascii_case("Various");

        let mut file_base = self.config.file_template.clone();

        // If template doesn't specify track artist for Various Artists, inject artist for clarity
        if is_va && !file_base.contains("{Artist}") && !file_base.contains("{AlbumArtist}") {
            file_base = format!("{{TrackNumber:pad2}} - {{Artist}} - {{Title}}");
        }

        file_base = file_base
            .replace("{TrackNumber:pad2}", &format!("{:02}", ctx.track_number))
            .replace("{TrackNumber}", &ctx.track_number.to_string())
            .replace("{Title}", &safe_title)
            .replace("{Artist}", &safe_artist)
            .replace("{AlbumArtist}", &safe_album_artist)
            .replace("{Album}", &safe_album)
            .replace("{Year}", &year_str)
            .replace("{DiscNumber:pad2}", &format!("{:02}", ctx.disc_number))
            .replace("{DiscNumber}", &ctx.disc_number.to_string())
            .replace("{Format:lower}", &ext_clean.to_lowercase())
            .replace("{Format}", ext_clean);

        let safe_file_base = sanitize_filename(&file_base);
        let safe_file_base = self.apply_space_replacement(&safe_file_base);

        let file_name = if safe_file_base.ends_with(&format!(".{}", ext_clean)) {
            safe_file_base
        } else {
            format!("{}.{}", safe_file_base, ext_clean)
        };

        let mut final_path = target_dir.join(&file_name);

        // Truncate stem if max_path_length is exceeded
        let path_len = final_path.to_string_lossy().len();
        if path_len > max_len {
            let overflow = path_len - max_len;
            let ext_suffix = format!(".{}", ext_clean);
            let stem = file_name.trim_end_matches(&ext_suffix);
            if stem.len() > overflow + 1 {
                let truncated_stem = &stem[..stem.len() - overflow];
                let truncated_file_name = format!(
                    "{}{}",
                    truncated_stem.trim_end_matches(&[' ', '.'][..]),
                    ext_suffix
                );
                final_path = target_dir.join(truncated_file_name);
            }
        }

        final_path
    }

    /// Path to Lyrics File (`.lrc`): matching the exact base name and folder of the track
    pub fn lyrics_path_for_track(&self, track_path: &Path) -> PathBuf {
        track_path.with_extension("lrc")
    }

    /// Path to Cover Image (`cover.jpg`) inside Album Directory
    pub fn cover_image_path(&self, artist: &str, album: &str, year: Option<i32>) -> PathBuf {
        self.album_dir(artist, album, year).join("cover.jpg")
    }

    /// Path to Animated Cover (`cover.webp`) inside Album Directory
    pub fn cover_webp_path(&self, artist: &str, album: &str, year: Option<i32>) -> PathBuf {
        self.album_dir(artist, album, year).join("cover.webp")
    }

    /// Path to Animated Folder Cover (`folder.webp`) inside Album Directory
    pub fn folder_webp_path(&self, artist: &str, album: &str, year: Option<i32>) -> PathBuf {
        self.album_dir(artist, album, year).join("folder.webp")
    }

    /// Path to Animated Cover Alias (`animated.webp`) inside Album Directory
    pub fn animated_webp_path(&self, artist: &str, album: &str, year: Option<i32>) -> PathBuf {
        self.album_dir(artist, album, year).join("animated.webp")
    }

    /// Path to Digital Booklet (`booklet.pdf`) inside Album Directory
    pub fn booklet_path(&self, artist: &str, album: &str, year: Option<i32>) -> PathBuf {
        self.album_dir(artist, album, year).join("booklet.pdf")
    }

    /// Path to Artist Profile Image (`artist.jpg`) inside Artist Directory
    pub fn artist_image_path(&self, artist: &str) -> PathBuf {
        self.artist_dir(artist).join("artist.jpg")
    }

    /// Path to Artist Fanart (`fanart.jpg`) inside Artist Directory
    pub fn artist_fanart_path(&self, artist: &str) -> PathBuf {
        self.artist_dir(artist).join("fanart.jpg")
    }

    /// Path to Artist Info XML (`artist.nfo`) inside Artist Directory
    pub fn artist_nfo_path(&self, artist: &str) -> PathBuf {
        self.artist_dir(artist).join("artist.nfo")
    }

    /// Path to Artist Biography text (`biography.txt`) inside Artist Directory
    pub fn artist_biography_path(&self, artist: &str) -> PathBuf {
        self.artist_dir(artist).join("biography.txt")
    }

    /// Helper to resolve collisions if destination file already exists
    pub fn resolve_unique_path(&self, target_path: &Path) -> PathBuf {
        if !target_path.exists() {
            return target_path.to_path_buf();
        }

        let stem = target_path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("file");
        let ext = target_path
            .extension()
            .and_then(|s| s.to_str())
            .unwrap_or("");
        let parent = target_path.parent().unwrap_or_else(|| Path::new(""));

        for idx in 1..1000 {
            let candidate_name = if ext.is_empty() {
                format!("{} ({})", stem, idx)
            } else {
                format!("{} ({}).{}", stem, idx, ext)
            };
            let candidate_path = parent.join(candidate_name);
            if !candidate_path.exists() {
                return candidate_path;
            }
        }

        target_path.to_path_buf()
    }

    /// Disambiguated path for collision resolution when two distinct tracks share title/artist:
    /// Injects source/edition context deterministically.
    pub fn resolve_disambiguated_track_path(
        &self,
        ctx: &TrackLayoutContext,
        disambiguator: Option<&str>,
    ) -> PathBuf {
        let base_path = self.resolve_track_path(ctx);
        if let Some(dis) = disambiguator {
            if !dis.trim().is_empty() {
                let parent = base_path.parent().unwrap_or_else(|| Path::new(""));
                let stem = base_path.file_stem().and_then(|s| s.to_str()).unwrap_or("track");
                let ext = ctx.format.trim_start_matches('.');
                let safe_dis = sanitize_filename(dis);
                let safe_dis = self.apply_space_replacement(&safe_dis);
                let new_filename = format!("{} [{}].{}", stem, safe_dis, ext);
                return parent.join(new_filename);
            }
        }
        self.resolve_unique_path(&base_path)
    }

    fn apply_space_replacement(&self, text: &str) -> String {
        if let Some(ref repl) = self.config.replace_spaces_with {
            if !repl.is_empty() {
                return text.replace(' ', repl);
            }
        }
        text.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sanitize_filename_windows_forbidden_chars() {
        assert_eq!(sanitize_filename("AC/DC"), "AC_DC");
        assert_eq!(sanitize_filename("What? Move!"), "What_ Move!");
        assert_eq!(sanitize_filename("Artist : Album <Deluxe>"), "Artist _ Album _Deluxe_");
        assert_eq!(sanitize_filename(r#"Song "Quotes" | Remix"#), "Song _Quotes_ _ Remix");
        assert_eq!(sanitize_filename("Asterisk*"), "Asterisk_");
    }

    #[test]
    fn test_sanitize_filename_windows_reserved_names() {
        assert_eq!(sanitize_filename("CON"), "CON_");
        assert_eq!(sanitize_filename("aux"), "aux_");
        assert_eq!(sanitize_filename("Nul"), "Nul_");
        assert_eq!(sanitize_filename("COM1"), "COM1_");
        assert_eq!(sanitize_filename("LPT9"), "LPT9_");
    }

    #[test]
    fn test_sanitize_filename_trim_dots_and_spaces() {
        assert_eq!(sanitize_filename("  Trim. "), "Trim");
        assert_eq!(sanitize_filename("...Hidden..."), "Hidden");
        assert_eq!(sanitize_filename("   "), "Unknown");
        assert_eq!(sanitize_filename(""), "Unknown");
    }

    #[test]
    fn test_cli_compatible_single_disc_layout() {
        let layout = LibraryLayout::new("/Music");
        let track_path = layout.track_path(
            "Queen",
            "Queen",
            "A Night at the Opera",
            Some(1975),
            1,
            1,
            4,
            "Bohemian Rhapsody",
            "flac",
        );

        let expected = PathBuf::from("/Music")
            .join("Queen")
            .join("[1975] A Night at the Opera")
            .join("04 - Bohemian Rhapsody.flac");

        assert_eq!(track_path, expected);
    }

    #[test]
    fn test_cli_compatible_multi_disc_layout() {
        let layout = LibraryLayout::new("/Music");
        let track_path = layout.track_path(
            "Pink Floyd",
            "Pink Floyd",
            "The Wall",
            Some(1979),
            2,
            2,
            1,
            "Hey You",
            "flac",
        );

        let expected = PathBuf::from("/Music")
            .join("Pink Floyd")
            .join("[1979] The Wall")
            .join("Disc 2")
            .join("01 - Hey You.flac");

        assert_eq!(track_path, expected);
    }

    #[test]
    fn test_cli_compatible_various_artists_layout() {
        let layout = LibraryLayout::new("/Music");
        let track_path = layout.track_path(
            "Various Artists",
            "Daft Punk",
            "Now That's What I Call Music!",
            Some(2024),
            1,
            1,
            1,
            "Get Lucky",
            "flac",
        );

        let expected = PathBuf::from("/Music")
            .join("Various Artists")
            .join("[2024] Now That's What I Call Music!")
            .join("01 - Daft Punk - Get Lucky.flac");

        assert_eq!(track_path, expected);
    }

    #[test]
    fn test_template_substitution_custom() {
        let config = FolderFileTemplateConfig {
            folder_template: "{Artist}/{Album}".to_string(),
            file_template: "{TrackNumber} - {Title}".to_string(),
            artist_separator: ", ".to_string(),
            replace_spaces_with: None,
            max_path_length: 255,
        };

        let layout = LibraryLayout::with_config("/Music", config);
        let ctx = TrackLayoutContext {
            artist: "David Bowie",
            album_artist: Some("David Bowie"),
            album: "Heroes",
            title: "Heroes",
            year: Some(1977),
            original_date: Some("1977-10-14"),
            track_number: 3,
            track_total: Some(10),
            disc_number: 1,
            total_discs: 1,
            format: "flac",
            bit_depth: Some(24),
            sample_rate: Some(96000.0),
        };

        let resolved = layout.resolve_track_path(&ctx);
        let expected = PathBuf::from("/Music")
            .join("David Bowie")
            .join("Heroes")
            .join("3 - Heroes.flac");

        assert_eq!(resolved, expected);
    }

    #[test]
    fn test_sidecar_paths() {
        let layout = LibraryLayout::new("/Music");
        let album_dir = layout.album_dir("Linkin Park", "From Zero", Some(2024));

        assert_eq!(
            layout.cover_image_path("Linkin Park", "From Zero", Some(2024)),
            album_dir.join("cover.jpg")
        );
        assert_eq!(
            layout.cover_webp_path("Linkin Park", "From Zero", Some(2024)),
            album_dir.join("cover.webp")
        );
        assert_eq!(
            layout.booklet_path("Linkin Park", "From Zero", Some(2024)),
            album_dir.join("booklet.pdf")
        );
    }

    #[test]
    fn test_format_album_dir() {
        let config = FolderFileTemplateConfig {
            folder_template: "{AlbumArtist}/{Album}".to_string(),
            file_template: "{TrackNumber:pad2} - {Title}".to_string(),
            artist_separator: ", ".to_string(),
            replace_spaces_with: None,
            max_path_length: 255,
        };
        let layout = LibraryLayout::with_config("/Music", config);
        let dir = layout.format_album_dir("Daft Punk", "Discovery", Some(2001));
        assert_eq!(dir, PathBuf::from("/Music").join("Daft Punk").join("Discovery"));
    }
}

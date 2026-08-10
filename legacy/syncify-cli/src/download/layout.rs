// Library Layout Engine for Syncify
// Enforces standard music library folder hierarchy and file naming for Symfonium, Kodi, Plex & Navidrome

use std::path::{Path, PathBuf};

/// Sanitizes a string for use as a directory or file name on Windows, Linux, and macOS.
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
        "Unknown".to_string()
    } else {
        trimmed.to_string()
    }
}

/// Structured Library Layout Engine for Syncify & Symfonium
#[derive(Debug, Clone)]
pub struct LibraryLayout {
    pub base_dir: PathBuf,
}

impl LibraryLayout {
    pub fn new(base_dir: impl AsRef<Path>) -> Self {
        Self {
            base_dir: base_dir.as_ref().to_path_buf(),
        }
    }

    /// Path to Artist Directory: `{base_dir}/{AlbumArtist}`
    pub fn artist_dir(&self, artist: &str) -> PathBuf {
        let safe_artist = sanitize_filename(artist);
        self.base_dir.join(safe_artist)
    }

    /// Path to Album Directory: `{base_dir}/{AlbumArtist}/[{Year}] {AlbumTitle}`
    pub fn album_dir(&self, artist: &str, album: &str, year: Option<i32>) -> PathBuf {
        let safe_album = sanitize_filename(album);
        let folder_name = match year {
            Some(y) if y > 1900 && y < 2100 => format!("[{}] {}", y, safe_album),
            _ => safe_album,
        };
        self.artist_dir(artist).join(folder_name)
    }

    /// Path to Disc Directory (if multi-disc): `{AlbumDir}/Disc {DiscNumber}`
    pub fn disc_dir(&self, artist: &str, album: &str, year: Option<i32>, disc_number: u32, total_discs: u32) -> PathBuf {
        let alb_dir = self.album_dir(artist, album, year);
        if total_discs > 1 {
            alb_dir.join(format!("Disc {}", disc_number))
        } else {
            alb_dir
        }
    }

    /// Path to Track File: `{TargetDir}/{TrackNumber:02} - {Title}.flac`
    /// (For Various Artists, incorporates track artist: `{TrackNumber:02} - {TrackArtist} - {Title}.flac`)
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
        let ext_clean = ext.trim_start_matches('.');

        let file_name = if album_artist.eq_ignore_ascii_case("Various Artists") || album_artist.eq_ignore_ascii_case("VA") {
            let safe_track_artist = sanitize_filename(track_artist);
            format!("{:02} - {} - {}.{}", track_number, safe_track_artist, safe_title, ext_clean)
        } else {
            format!("{:02} - {}.{}", track_number, safe_title, ext_clean)
        };

        target_dir.join(file_name)
    }

    /// Path to Lyrics File (`.lrc`): `{PistaBaseName}.lrc` in the same directory as the track
    pub fn lyrics_path(
        &self,
        album_artist: &str,
        track_artist: &str,
        album: &str,
        year: Option<i32>,
        disc_number: u32,
        total_discs: u32,
        track_number: u32,
        title: &str,
    ) -> PathBuf {
        let track_flac = self.track_path(
            album_artist,
            track_artist,
            album,
            year,
            disc_number,
            total_discs,
            track_number,
            title,
            "flac",
        );
        track_flac.with_extension("lrc")
    }

    /// Path to Cover Image (`cover.jpg`) inside Album Directory
    pub fn cover_image_path(&self, artist: &str, album: &str, year: Option<i32>) -> PathBuf {
        self.album_dir(artist, album, year).join("cover.jpg")
    }

    /// Path to Animated Cover (`cover.gif`) inside Album Directory
    pub fn animated_cover_path(&self, artist: &str, album: &str, year: Option<i32>) -> PathBuf {
        self.album_dir(artist, album, year).join("cover.gif")
    }

    /// Path to Booklet (`booklet.pdf`) inside Album Directory
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
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sanitize_filename() {
        assert_eq!(sanitize_filename("AC/DC"), "AC_DC");
        assert_eq!(sanitize_filename("What? Move!"), "What_ Move!");
        assert_eq!(sanitize_filename("  Trim. "), "Trim");
    }

    #[test]
    fn test_single_disc_layout() {
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
    fn test_multi_disc_layout() {
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
    fn test_various_artists_layout() {
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
}

// Staged Download Pipeline & Queue Memory State Manager for Syncify
// Provides isolated temporary downloads (.syncify_staging/), state persistence (state.json),
// and atomic commitment to final music library (downloads_syncify/)

use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs::File;
use std::path::{Path, PathBuf};
use std::sync::RwLock;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum DownloadStatus {
    Pending,
    AudioDownloaded,
    Enriched,
    Completed,
    Failed,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StagedTrack {
    pub track_key: String,
    pub title: String,
    pub artist: String,
    pub album: String,
    pub track_num: u32,
    pub disc_num: u32,
    pub status: DownloadStatus,
    pub audio_bytes: u64,
    pub temp_file_path: String,
    pub final_file_path: String,
    pub has_karaoke_lyrics: bool,
    pub error_msg: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StagedAlbum {
    pub album_id: String,
    pub album_title: String,
    pub artist_name: String,
    pub year: i32,
    pub total_tracks: u32,
    pub mb_release_id: Option<String>,
    pub discogs_release_id: Option<i64>,
    pub cover_saved: bool,
    pub animated_cover_saved: bool,
    pub tracks: HashMap<String, StagedTrack>,
    pub is_completed: bool,
}

#[derive(Debug, Serialize, Deserialize, Default)]
pub struct StagingState {
    pub albums: HashMap<String, StagedAlbum>,
}

pub struct DownloadStagingManager {
    pub staging_base_dir: PathBuf,
    pub target_base_dir: PathBuf,
    pub state_file_path: PathBuf,
    state: RwLock<StagingState>,
}

impl DownloadStagingManager {
    pub fn new<P: AsRef<Path>, Q: AsRef<Path>>(staging_dir: P, target_dir: Q) -> Self {
        let staging_base_dir = staging_dir.as_ref().to_path_buf();
        let target_base_dir = target_dir.as_ref().to_path_buf();
        let state_file_path = staging_base_dir.join("state.json");

        let _ = std::fs::create_dir_all(&staging_base_dir);
        let _ = std::fs::create_dir_all(&target_base_dir);

        let initial_state = if state_file_path.exists() {
            if let Ok(file) = File::open(&state_file_path) {
                serde_json::from_reader(file).unwrap_or_default()
            } else {
                StagingState::default()
            }
        } else {
            StagingState::default()
        };

        Self {
            staging_base_dir,
            target_base_dir,
            state_file_path,
            state: RwLock::new(initial_state),
        }
    }

    pub fn save_state(&self) -> Result<()> {
        let state_guard = self.state.read().unwrap();
        let file = File::create(&self.state_file_path)?;
        serde_json::to_writer_pretty(file, &*state_guard)?;
        Ok(())
    }

    pub fn staging_album_dir(&self, album_id: &str) -> PathBuf {
        self.staging_base_dir.join(album_id)
    }

    pub fn init_album(&self, album_id: &str, album_title: &str, artist_name: &str, year: i32, total_tracks: u32) {
        let mut state_guard = self.state.write().unwrap();
        if !state_guard.albums.contains_key(album_id) {
            let album_dir = self.staging_base_dir.join(album_id);
            let _ = std::fs::create_dir_all(&album_dir);

            state_guard.albums.insert(
                album_id.to_string(),
                StagedAlbum {
                    album_id: album_id.to_string(),
                    album_title: album_title.to_string(),
                    artist_name: artist_name.to_string(),
                    year,
                    total_tracks,
                    mb_release_id: None,
                    discogs_release_id: None,
                    cover_saved: false,
                    animated_cover_saved: false,
                    tracks: HashMap::new(),
                    is_completed: false,
                },
            );
        }
    }

    pub fn is_track_completed(&self, album_id: &str, track_key: &str) -> bool {
        let state_guard = self.state.read().unwrap();
        if let Some(album) = state_guard.albums.get(album_id) {
            if let Some(track) = album.tracks.get(track_key) {
                return track.status == DownloadStatus::Completed || track.status == DownloadStatus::Enriched;
            }
        }
        false
    }

    pub fn mark_audio_downloaded(
        &self,
        album_id: &str,
        track_key: &str,
        artist: &str,
        album: &str,
        title: &str,
        track_num: u32,
        disc_num: u32,
        audio_bytes: u64,
        temp_file_path: &Path,
        final_file_path: &Path,
    ) {
        let mut state_guard = self.state.write().unwrap();
        if let Some(staged_album) = state_guard.albums.get_mut(album_id) {
            staged_album.tracks.insert(
                track_key.to_string(),
                StagedTrack {
                    track_key: track_key.to_string(),
                    title: title.to_string(),
                    artist: artist.to_string(),
                    album: album.to_string(),
                    track_num,
                    disc_num,
                    status: DownloadStatus::AudioDownloaded,
                    audio_bytes,
                    temp_file_path: temp_file_path.to_string_lossy().to_string(),
                    final_file_path: final_file_path.to_string_lossy().to_string(),
                    has_karaoke_lyrics: false,
                    error_msg: None,
                },
            );
        }
        drop(state_guard);
        let _ = self.save_state();
    }

    pub fn mark_enriched(&self, album_id: &str, track_key: &str, has_karaoke: bool) {
        let mut state_guard = self.state.write().unwrap();
        if let Some(staged_album) = state_guard.albums.get_mut(album_id) {
            if let Some(track) = staged_album.tracks.get_mut(track_key) {
                track.status = DownloadStatus::Enriched;
                track.has_karaoke_lyrics = has_karaoke;
            }
        }
        drop(state_guard);
        let _ = self.save_state();
    }

    pub fn commit_album_to_library(&self, album_id: &str, target_album_dir: &Path) -> Result<()> {
        let mut state_guard = self.state.write().unwrap();
        let staged_album = state_guard
            .albums
            .get_mut(album_id)
            .ok_or_else(|| anyhow!("Staged album '{}' not found in state", album_id))?;

        let staging_album_dir = self.staging_base_dir.join(album_id);
        std::fs::create_dir_all(target_album_dir)?;

        // Move all files (FLAC, cover.jpg, cover.webp, booklet.pdf) from staging to target album dir
        if staging_album_dir.exists() {
            if let Ok(entries) = std::fs::read_dir(&staging_album_dir) {
                for entry in entries.flatten() {
                    let source_path = entry.path();
                    let file_name = entry.file_name();
                    let dest_path = target_album_dir.join(file_name);

                    if source_path.is_file() {
                        // Rename or copy+remove if across filesystems
                        if std::fs::rename(&source_path, &dest_path).is_err() {
                            std::fs::copy(&source_path, &dest_path)?;
                            let _ = std::fs::remove_file(&source_path);
                        }
                    }
                }
            }
        }

        staged_album.is_completed = true;
        for track in staged_album.tracks.values_mut() {
            track.status = DownloadStatus::Completed;
        }

        drop(state_guard);
        self.save_state()?;
        let _ = std::fs::remove_dir_all(&staging_album_dir);

        println!("✓ [Staging] Atomically committed album '{}' to library: {}", album_id, target_album_dir.display());
        Ok(())
    }
}

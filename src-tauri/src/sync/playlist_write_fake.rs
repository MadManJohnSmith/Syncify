use super::{
    PlaylistWriteCapabilities, PlaylistWriteError, PlaylistWriteErrorKind, PlaylistWriteOperation,
    PlaylistWriteProvider, PlaylistWriteRequest,
};
use std::collections::{HashMap, HashSet, VecDeque};
use std::sync::Mutex;
use std::time::Duration;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FakePlaylistItem {
    pub id: String,
    pub track_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FakePlaylistSnapshot {
    pub id: String,
    pub name: String,
    pub description: String,
    pub items: Vec<FakePlaylistItem>,
}

#[derive(Debug, Clone)]
struct FakePlaylist {
    name: String,
    description: String,
    items: Vec<FakePlaylistItem>,
}

#[derive(Default)]
struct FakeState {
    next_playlist: u64,
    next_item: u64,
    playlists: HashMap<String, FakePlaylist>,
    failures: VecDeque<(PlaylistWriteOperation, PlaylistWriteErrorKind)>,
    calls: Vec<PlaylistWriteRequest>,
}

impl FakeState {
    fn begin(&mut self, request: &PlaylistWriteRequest) -> Option<PlaylistWriteErrorKind> {
        self.calls.push(request.clone());
        let operation = request.operation();
        let position = self
            .failures
            .iter()
            .position(|(scheduled, _)| *scheduled == operation)?;
        Some(self.failures.remove(position)?.1)
    }

    fn allocate_playlist_id(&mut self) -> String {
        self.next_playlist += 1;
        format!("fake-playlist-{}", self.next_playlist)
    }

    fn allocate_item(&mut self, track_id: &str) -> FakePlaylistItem {
        self.next_item += 1;
        FakePlaylistItem {
            id: format!("fake-item-{}", self.next_item),
            track_id: track_id.to_string(),
        }
    }
}

/// Proveedor en proceso para el seam del gateway: conserva ocurrencias y registra
/// cada petición. Los fallos programados se consumen una sola vez.
pub struct FakePlaylistWriteProvider {
    capabilities: PlaylistWriteCapabilities,
    delay: Duration,
    state: Mutex<FakeState>,
}

impl Default for FakePlaylistWriteProvider {
    fn default() -> Self {
        Self {
            capabilities: PlaylistWriteCapabilities::ALL,
            delay: Duration::ZERO,
            state: Mutex::new(FakeState::default()),
        }
    }
}

impl FakePlaylistWriteProvider {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_capabilities(mut self, capabilities: PlaylistWriteCapabilities) -> Self {
        self.capabilities = capabilities;
        self
    }

    pub fn with_failure(
        self,
        operation: PlaylistWriteOperation,
        kind: PlaylistWriteErrorKind,
    ) -> Self {
        self.state
            .lock()
            .expect("fake playlist state")
            .failures
            .push_back((operation, kind));
        self
    }

    /// Retrasa la respuesta después de confirmar la mutación: permite probar que
    /// un timeout no demuestra que la escritura fallara en el proveedor.
    pub fn with_delay(mut self, delay: Duration) -> Self {
        self.delay = delay;
        self
    }

    pub fn calls(&self) -> Vec<PlaylistWriteRequest> {
        self.state
            .lock()
            .expect("fake playlist state")
            .calls
            .clone()
    }

    pub fn playlist(&self, playlist_id: &str) -> Option<FakePlaylistSnapshot> {
        self.state
            .lock()
            .expect("fake playlist state")
            .playlists
            .get(playlist_id)
            .map(|playlist| FakePlaylistSnapshot {
                id: playlist_id.to_string(),
                name: playlist.name.clone(),
                description: playlist.description.clone(),
                items: playlist.items.clone(),
            })
    }

    pub fn playlist_count(&self) -> usize {
        self.state
            .lock()
            .expect("fake playlist state")
            .playlists
            .len()
    }

    async fn complete<T: Send>(
        &self,
        result: Result<T, PlaylistWriteError>,
    ) -> Result<T, PlaylistWriteError> {
        if !self.delay.is_zero() {
            tokio::time::sleep(self.delay).await;
        }
        result
    }

    fn scheduled_error(
        &self,
        operation: PlaylistWriteOperation,
        kind: PlaylistWriteErrorKind,
    ) -> PlaylistWriteError {
        let mut error = PlaylistWriteError::new(
            "fake",
            operation,
            kind,
            format!("scheduled {} failure", kind.as_str()),
        );
        error.http_status = match kind {
            PlaylistWriteErrorKind::Unauthorized => Some(401),
            PlaylistWriteErrorKind::Forbidden => Some(403),
            PlaylistWriteErrorKind::RateLimited => Some(429),
            PlaylistWriteErrorKind::Validation => Some(400),
            PlaylistWriteErrorKind::NotFound => Some(404),
            PlaylistWriteErrorKind::Transient => Some(503),
            PlaylistWriteErrorKind::Timeout
            | PlaylistWriteErrorKind::Unsupported
            | PlaylistWriteErrorKind::Permanent => None,
        };
        error
    }

    fn missing_playlist(
        operation: PlaylistWriteOperation,
        playlist_id: &str,
    ) -> PlaylistWriteError {
        PlaylistWriteError::new(
            "fake",
            operation,
            PlaylistWriteErrorKind::NotFound,
            format!("playlist {playlist_id} does not exist"),
        )
    }
}

impl PlaylistWriteProvider for FakePlaylistWriteProvider {
    fn service_name(&self) -> &'static str {
        "fake"
    }

    fn capabilities(&self) -> PlaylistWriteCapabilities {
        self.capabilities
    }

    async fn create_playlist(
        &self,
        name: String,
        description: String,
    ) -> Result<String, PlaylistWriteError> {
        let request = PlaylistWriteRequest::Create {
            name: name.clone(),
            description: description.clone(),
        };
        let result = {
            let mut state = self.state.lock().expect("fake playlist state");
            if let Some(kind) = state.begin(&request) {
                return Err(self.scheduled_error(request.operation(), kind));
            }
            let id = state.allocate_playlist_id();
            state.playlists.insert(
                id.clone(),
                FakePlaylist {
                    name,
                    description,
                    items: Vec::new(),
                },
            );
            Ok(id)
        };
        self.complete(result).await
    }

    async fn add_tracks(
        &self,
        playlist_id: String,
        track_ids: Vec<String>,
    ) -> Result<(), PlaylistWriteError> {
        let request = PlaylistWriteRequest::AddTracks {
            playlist_id: playlist_id.clone(),
            track_ids: track_ids.clone(),
        };
        let result = {
            let mut state = self.state.lock().expect("fake playlist state");
            if let Some(kind) = state.begin(&request) {
                return Err(self.scheduled_error(request.operation(), kind));
            }
            if !state.playlists.contains_key(&playlist_id) {
                return Err(Self::missing_playlist(
                    PlaylistWriteOperation::AddTracks,
                    &playlist_id,
                ));
            }
            let new_items = track_ids
                .iter()
                .map(|track_id| state.allocate_item(track_id))
                .collect::<Vec<_>>();
            let playlist = state
                .playlists
                .get_mut(&playlist_id)
                .expect("playlist checked above");
            playlist.items.extend(new_items);
            Ok(())
        };
        self.complete(result).await
    }

    async fn update_playlist(
        &self,
        playlist_id: String,
        name: Option<String>,
        description: Option<String>,
    ) -> Result<(), PlaylistWriteError> {
        let request = PlaylistWriteRequest::Update {
            playlist_id: playlist_id.clone(),
            name: name.clone(),
            description: description.clone(),
        };
        let result = {
            let mut state = self.state.lock().expect("fake playlist state");
            if let Some(kind) = state.begin(&request) {
                return Err(self.scheduled_error(request.operation(), kind));
            }
            let playlist = state.playlists.get_mut(&playlist_id).ok_or_else(|| {
                Self::missing_playlist(PlaylistWriteOperation::Update, &playlist_id)
            })?;
            if let Some(name) = name {
                playlist.name = name;
            }
            if let Some(description) = description {
                playlist.description = description;
            }
            Ok(())
        };
        self.complete(result).await
    }

    async fn remove_items(
        &self,
        playlist_id: String,
        item_ids: Vec<String>,
    ) -> Result<(), PlaylistWriteError> {
        let request = PlaylistWriteRequest::RemoveItems {
            playlist_id: playlist_id.clone(),
            item_ids: item_ids.clone(),
        };
        let result = {
            let mut state = self.state.lock().expect("fake playlist state");
            if let Some(kind) = state.begin(&request) {
                return Err(self.scheduled_error(request.operation(), kind));
            }
            let playlist = state.playlists.get_mut(&playlist_id).ok_or_else(|| {
                Self::missing_playlist(PlaylistWriteOperation::RemoveItems, &playlist_id)
            })?;
            let existing: HashSet<&str> =
                playlist.items.iter().map(|item| item.id.as_str()).collect();
            if item_ids.iter().any(|id| !existing.contains(id.as_str())) {
                return Err(PlaylistWriteError::new(
                    "fake",
                    PlaylistWriteOperation::RemoveItems,
                    PlaylistWriteErrorKind::NotFound,
                    "one or more playlist occurrences do not exist",
                ));
            }
            let removed: HashSet<&str> = item_ids.iter().map(String::as_str).collect();
            playlist
                .items
                .retain(|item| !removed.contains(item.id.as_str()));
            Ok(())
        };
        self.complete(result).await
    }

    async fn reorder_items(
        &self,
        playlist_id: String,
        item_ids: Vec<String>,
    ) -> Result<(), PlaylistWriteError> {
        let request = PlaylistWriteRequest::ReorderItems {
            playlist_id: playlist_id.clone(),
            item_ids: item_ids.clone(),
        };
        let result = {
            let mut state = self.state.lock().expect("fake playlist state");
            if let Some(kind) = state.begin(&request) {
                return Err(self.scheduled_error(request.operation(), kind));
            }
            let playlist = state.playlists.get_mut(&playlist_id).ok_or_else(|| {
                Self::missing_playlist(PlaylistWriteOperation::ReorderItems, &playlist_id)
            })?;
            if item_ids.len() != playlist.items.len() {
                return Err(PlaylistWriteError::validation(
                    "fake",
                    PlaylistWriteOperation::ReorderItems,
                    "reorder requires the complete occurrence list",
                ));
            }
            let positions: HashMap<&str, usize> = item_ids
                .iter()
                .enumerate()
                .map(|(position, id)| (id.as_str(), position))
                .collect();
            if playlist
                .items
                .iter()
                .any(|item| !positions.contains_key(item.id.as_str()))
            {
                return Err(PlaylistWriteError::validation(
                    "fake",
                    PlaylistWriteOperation::ReorderItems,
                    "reorder must be a permutation of the existing occurrences",
                ));
            }
            playlist
                .items
                .sort_by_key(|item| positions[item.id.as_str()]);
            Ok(())
        };
        self.complete(result).await
    }
}

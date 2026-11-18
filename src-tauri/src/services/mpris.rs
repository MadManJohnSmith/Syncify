//! Integración MPRIS nativa (Linux) del motor de reproducción local.
//!
//! La ruta antigua (`<audio>` + WebKit MediaSession) heredaba MPRIS del motor
//! media de WebKitGTK. Con el motor nativo (`syncify-player`, rodio + symphonia)
//! esa herencia desaparece: este servicio la reconstruye con `zbus` sobre el bus
//! de sesión y expone `org.mpris.MediaPlayer2` + `.Player` en
//! `/org/mpris/MediaPlayer2` (nombre de bus `org.mpris.MediaPlayer2.syncify`).
//!
//! Desde P02 (#133) MPRIS lee y manda sobre el BACKEND, no sobre la webview:
//! - El ticker alimenta transporte, posición y la identidad de la pista
//!   cargada; los metadatos (título, artista, álbum, carátula) se resuelven
//!   desde la biblioteca por `track_id`, sin que la UI los empuje.
//! - Play/Pause/Next/Previous/Stop/Seek/SetPosition se ejecutan con
//!   `services::player_control` sobre `PlayerState` y el agregado persistido
//!   de cola. Las propiedades `Shuffle` y `LoopStatus` son escribibles y se
//!   persisten en `player_state`.
//! - La UI es una vista: recibe `player:advanced` y `player:modes-changed`.
//!
//! Sin bus de sesión disponible (o fuera de Linux) el servicio degrada a no-op:
//! la app arranca igual y los comandos no fallan.

// El Mutex solo lo tocan el estado compartido y lock_shared, plumbing del
// hilo D-Bus de Linux; fuera de Linux el servicio es un stub.
#[cfg(target_os = "linux")]
use std::sync::{Mutex, MutexGuard};
use std::time::Duration;

use tauri::AppHandle;
#[cfg(target_os = "linux")]
use tauri::Manager;

/// Estado de reproducción que ve MPRIS (`PlaybackStatus`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum PlaybackStatus {
    #[default]
    Stopped,
    Playing,
    Paused,
}

impl PlaybackStatus {
    // Solo el hilo D-Bus de Linux lo traduce a string; en el resto de
    // plataformas el servicio es un stub y quedaria como codigo muerto.
    #[cfg(target_os = "linux")]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Stopped => "Stopped",
            Self::Playing => "Playing",
            Self::Paused => "Paused",
        }
    }
}

/// Identidad y duración de la pista que el motor tiene cargada. La publica el
/// ticker; los metadatos textuales los completa el servicio desde SQLite.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NowPlaying {
    pub track_id: i64,
    pub duration: Option<Duration>,
}

/// `LoopStatus` de MPRIS ("None" | "Track" | "Playlist").
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[cfg(target_os = "linux")]
enum LoopStatus {
    #[default]
    None,
    Track,
    Playlist,
}

#[cfg(target_os = "linux")]
impl LoopStatus {
    fn as_str(self) -> &'static str {
        match self {
            Self::None => "None",
            Self::Track => "Track",
            Self::Playlist => "Playlist",
        }
    }

    fn from_repeat(value: &str) -> Self {
        match value {
            "one" => Self::Track,
            "all" => Self::Playlist,
            _ => Self::None,
        }
    }
}

/// Copia interna (normalizada) de los metadatos.
// El plumbing siguiente (MprisMetadata, Shared, Snapshot, lock_shared,
// LoopStatus) solo lo consume el hilo D-Bus de Linux; en el resto de
// plataformas el servicio es un stub y quedaria como codigo muerto.
#[derive(Debug, Clone, Default, PartialEq)]
#[cfg(target_os = "linux")]
struct MprisMetadata {
    track_id: Option<i64>,
    title: Option<String>,
    artist: Option<String>,
    album: Option<String>,
    duration_us: Option<i64>,
    art_url: Option<String>,
}

/// Estado compartido entre el ticker/acciones y el hilo D-Bus.
#[derive(Debug, Clone, Default)]
#[cfg(target_os = "linux")]
struct Shared {
    playback: PlaybackStatus,
    position_us: i64,
    metadata: MprisMetadata,
    loop_status: LoopStatus,
    shuffle: bool,
}

/// Subconjunto que decide si hay que emitir `PropertiesChanged`.
#[derive(Debug, Clone, Default, PartialEq)]
#[cfg(target_os = "linux")]
struct Snapshot {
    playback: PlaybackStatus,
    metadata: MprisMetadata,
    loop_status: LoopStatus,
    shuffle: bool,
}

#[cfg(target_os = "linux")]
impl From<&Shared> for Snapshot {
    fn from(shared: &Shared) -> Self {
        Self {
            playback: shared.playback,
            metadata: shared.metadata.clone(),
            loop_status: shared.loop_status,
            shuffle: shared.shuffle,
        }
    }
}

#[cfg(target_os = "linux")]
fn lock_shared(shared: &Mutex<Shared>) -> MutexGuard<'_, Shared> {
    shared
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

/// Servicio MPRIS. En Linux arranca el hilo D-Bus; en el resto es un no-op.
pub struct MprisService {
    #[cfg(target_os = "linux")]
    shared: std::sync::Arc<Mutex<Shared>>,
    #[cfg(target_os = "linux")]
    stop: std::sync::Arc<std::sync::atomic::AtomicBool>,
}

impl MprisService {
    /// Arranca el servicio (Linux) o devuelve el no-op (resto de plataformas).
    /// No falla nunca: sin bus de sesión sólo se registra un aviso.
    pub fn start(app: &AppHandle) -> Self {
        #[cfg(target_os = "linux")]
        {
            use std::sync::atomic::AtomicBool;
            use std::sync::Arc;

            let shared = Arc::new(Mutex::new(Shared::default()));
            let stop = Arc::new(AtomicBool::new(false));
            let service = Self {
                shared: Arc::clone(&shared),
                stop: Arc::clone(&stop),
            };
            // Modos persistidos desde el arranque: MPRIS refleja la cola real
            // antes de que la webview restaure nada.
            if let Some(db) = app
                .try_state::<crate::AppState>()
                .map(|state| state.db.clone())
            {
                let shared = Arc::clone(&shared);
                tauri::async_runtime::spawn(async move {
                    match crate::commands::playback_queue::perform_get_playback_queue(&db).await {
                        Ok(snapshot) => {
                            let mut shared = lock_shared(&shared);
                            shared.shuffle = snapshot.shuffle;
                            shared.loop_status = LoopStatus::from_repeat(&snapshot.repeat);
                        }
                        Err(e) => tracing::warn!("mpris: no se pudieron leer los modos: {e}"),
                    }
                });
            }
            let app = app.clone();
            let spawn = std::thread::Builder::new()
                .name("syncify-mpris".into())
                .spawn(move || {
                    if let Err(err) = linux::serve(&app, &shared, &stop) {
                        tracing::warn!("mpris: servicio no disponible: {err}");
                    }
                });
            if let Err(err) = spawn {
                tracing::warn!("mpris: no se pudo arrancar el hilo: {err}");
            }
            service
        }
        #[cfg(not(target_os = "linux"))]
        {
            let _ = app;
            Self {}
        }
    }

    /// Publica el estado del motor (lo llama el ticker cada 250 ms). Si la
    /// pista cambió, resuelve sus metadatos desde la biblioteca en segundo
    /// plano; la UI no participa.
    pub fn update_playback<R: tauri::Runtime>(
        &self,
        app: &AppHandle<R>,
        status: PlaybackStatus,
        position: Duration,
        now_playing: Option<NowPlaying>,
    ) {
        #[cfg(target_os = "linux")]
        {
            let mut fetch_track = None;
            {
                let mut shared = lock_shared(&self.shared);
                shared.playback = status;
                shared.position_us = i64::try_from(position.as_micros()).unwrap_or(i64::MAX);
                let track_id = now_playing.map(|now| now.track_id);
                if shared.metadata.track_id != track_id {
                    shared.metadata = MprisMetadata {
                        track_id,
                        duration_us: now_playing
                            .and_then(|now| now.duration)
                            .and_then(|duration| i64::try_from(duration.as_micros()).ok()),
                        ..MprisMetadata::default()
                    };
                    if let Some(track_id) = track_id {
                        fetch_track = Some(track_id);
                    }
                } else if let Some(duration) = now_playing.and_then(|now| now.duration) {
                    shared.metadata.duration_us =
                        Some(i64::try_from(duration.as_micros()).unwrap_or(i64::MAX));
                }
            }
            if let Some(track_id) = fetch_track {
                spawn_metadata_fetch(app.clone(), track_id);
            }
        }
        #[cfg(not(target_os = "linux"))]
        {
            let _ = (app, status, position, now_playing);
        }
    }

    /// Publica shuffle/repeat persistidos. Lo llaman las acciones del backend
    /// (`player_control`) y el comando `playback_state_set_modes`.
    pub fn update_modes(&self, shuffle: bool, repeat: &str) {
        #[cfg(target_os = "linux")]
        {
            let mut shared = lock_shared(&self.shared);
            shared.shuffle = shuffle;
            shared.loop_status = LoopStatus::from_repeat(repeat);
        }
        #[cfg(not(target_os = "linux"))]
        {
            let _ = (shuffle, repeat);
        }
    }

    /// Rellena los metadatos textuales de la pista actual si sigue vigente.
    /// Una consulta lenta de una pista ya sustituida no pisa la nueva.
    #[cfg(target_os = "linux")]
    fn apply_track_metadata(
        &self,
        track_id: i64,
        metadata: &crate::commands::library::TrackMetadata,
    ) {
        let mut shared = lock_shared(&self.shared);
        if shared.metadata.track_id != Some(track_id) {
            return;
        }
        shared.metadata.title = Some(metadata.title.clone());
        shared.metadata.artist = metadata.artist_name.clone();
        shared.metadata.album = metadata.album_name.clone();
        shared.metadata.art_url = metadata.cover_art_url.clone();
        if shared.metadata.duration_us.is_none() {
            shared.metadata.duration_us = metadata
                .duration_ms
                .and_then(|ms| i64::try_from(ms).ok())
                .map(|ms| ms * 1_000);
        }
    }
}

#[cfg(target_os = "linux")]
fn spawn_metadata_fetch<R: tauri::Runtime + 'static>(app: AppHandle<R>, track_id: i64) {
    tauri::async_runtime::spawn(async move {
        let Some(db) = app
            .try_state::<crate::AppState>()
            .map(|state| state.db.clone())
        else {
            return;
        };
        match crate::commands::library::fetch_track_metadata(&db, track_id).await {
            Ok(metadata) => {
                if let Some(service) = app.try_state::<MprisService>() {
                    service.apply_track_metadata(track_id, &metadata);
                }
            }
            Err(e) => tracing::debug!(track_id, "mpris: metadatos no disponibles: {e}"),
        }
    });
}

impl Drop for MprisService {
    fn drop(&mut self) {
        #[cfg(target_os = "linux")]
        self.stop.store(true, std::sync::atomic::Ordering::Relaxed);
    }
}

#[cfg(target_os = "linux")]
mod linux {
    use std::borrow::Cow;
    use std::collections::HashMap;
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::{Arc, Mutex};
    use std::time::Duration;

    use tauri::{AppHandle, Manager};
    use zbus::blocking::object_server::InterfaceRef;
    use zbus::blocking::Connection;
    use zbus::fdo::Properties;
    use zbus::interface;
    use zbus::names::InterfaceName;
    use zbus::zvariant::{ObjectPath, OwnedObjectPath, OwnedValue, Value};

    use super::{lock_shared, MprisMetadata, Shared, Snapshot};
    use crate::services::player_control::{self, BackendAction, RepeatMode};

    const MPRIS_PATH: &str = "/org/mpris/MediaPlayer2";
    const BUS_NAME: &str = "org.mpris.MediaPlayer2.syncify";
    const PLAYER_INTERFACE: &str = "org.mpris.MediaPlayer2.Player";

    /// Sirve ambos objetos D-Bus y bombea `PropertiesChanged` hasta que se alce
    /// la bandera `stop` (al soltar el servicio, al cerrar la app).
    pub(super) fn serve(
        app: &AppHandle,
        shared: &Arc<Mutex<Shared>>,
        stop: &Arc<AtomicBool>,
    ) -> zbus::Result<()> {
        let connection = Connection::session()?;
        connection.request_name(BUS_NAME)?;
        let object_server = connection.object_server();
        object_server.at(MPRIS_PATH, MprisRoot { app: app.clone() })?;
        object_server.at(
            MPRIS_PATH,
            MprisPlayer {
                app: app.clone(),
                shared: Arc::clone(shared),
            },
        )?;
        let player = object_server.interface::<_, MprisPlayer>(MPRIS_PATH)?;

        let mut last = Snapshot::from(&*lock_shared(shared));
        while !stop.load(Ordering::Relaxed) {
            std::thread::sleep(Duration::from_millis(250));
            let current = Snapshot::from(&*lock_shared(shared));
            if current != last {
                emit_properties_changed(&player, &current);
                last = current;
            }
        }
        Ok(())
    }

    fn emit_properties_changed(player: &InterfaceRef<MprisPlayer>, snapshot: &Snapshot) {
        let Ok(interface_name) = InterfaceName::try_from(PLAYER_INTERFACE) else {
            tracing::warn!("mpris: nombre de interfaz inválido: {PLAYER_INTERFACE}");
            return;
        };
        let mut changed: HashMap<&str, Value<'_>> = HashMap::new();
        changed.insert("PlaybackStatus", Value::from(snapshot.playback.as_str()));
        changed.insert("Metadata", Value::from(metadata_dict(&snapshot.metadata)));
        changed.insert("LoopStatus", Value::from(snapshot.loop_status.as_str()));
        changed.insert("Shuffle", Value::from(snapshot.shuffle));
        let invalidated: Cow<'_, [&str]> = Cow::Owned(Vec::new());

        if let Err(err) = zbus::block_on(Properties::properties_changed(
            player.signal_emitter(),
            interface_name,
            changed,
            invalidated,
        )) {
            tracing::warn!("mpris: no se pudo emitir PropertiesChanged: {err}");
        }
    }

    /// Dict `a{sv}` con los metadatos en el vocabulario MPRIS.
    fn metadata_dict(meta: &MprisMetadata) -> HashMap<String, OwnedValue> {
        let mut dict: HashMap<String, OwnedValue> = HashMap::new();
        let path_str = track_path(meta.track_id);
        if let Ok(path) = ObjectPath::try_from(path_str.as_str()) {
            if let Ok(value) = OwnedValue::try_from(Value::from(path)) {
                dict.insert("mpris:trackid".to_string(), value);
            }
        }
        if let Some(length) = meta.duration_us {
            dict.insert("mpris:length".to_string(), OwnedValue::from(length));
        }
        if let Some(title) = &meta.title {
            if let Ok(value) = OwnedValue::try_from(Value::from(title.clone())) {
                dict.insert("xesam:title".to_string(), value);
            }
        }
        if let Some(artist) = &meta.artist {
            if let Ok(value) = OwnedValue::try_from(Value::from(vec![artist.clone()])) {
                dict.insert("xesam:artist".to_string(), value);
            }
        }
        if let Some(album) = &meta.album {
            if let Ok(value) = OwnedValue::try_from(Value::from(album.clone())) {
                dict.insert("xesam:album".to_string(), value);
            }
        }
        if let Some(art_url) = &meta.art_url {
            if let Ok(value) = OwnedValue::try_from(Value::from(art_url.clone())) {
                dict.insert("mpris:artUrl".to_string(), value);
            }
        }
        dict
    }

    /// `mpris:trackid` estable por pista (mismo formato en `SetPosition`).
    fn track_path(track_id: Option<i64>) -> String {
        format!("/org/syncify/track/{}", track_id.unwrap_or(0))
    }

    struct MprisRoot {
        app: AppHandle,
    }

    #[interface(name = "org.mpris.MediaPlayer2")]
    impl MprisRoot {
        /// Trae la ventana principal al frente.
        fn raise(&self) {
            if let Some(window) = self.app.get_webview_window("main") {
                let _ = window.unminimize();
                let _ = window.show();
                let _ = window.set_focus();
            }
        }

        /// Cierra la aplicación (como el Quit de cualquier reproductor MPRIS).
        fn quit(&self) {
            self.app.exit(0);
        }

        #[zbus(property)]
        fn can_quit(&self) -> bool {
            true
        }

        #[zbus(property)]
        fn can_raise(&self) -> bool {
            true
        }

        #[zbus(property)]
        fn has_track_list(&self) -> bool {
            false
        }

        #[zbus(property)]
        fn identity(&self) -> String {
            "Syncify".to_string()
        }

        #[zbus(property)]
        fn desktop_entry(&self) -> String {
            "syncify".to_string()
        }

        #[zbus(property)]
        fn supported_uri_schemes(&self) -> Vec<String> {
            Vec::new()
        }

        #[zbus(property)]
        fn supported_mime_types(&self) -> Vec<String> {
            vec![
                "audio/flac".to_string(),
                "audio/mpeg".to_string(),
                "audio/mp4".to_string(),
                "audio/ogg".to_string(),
                "audio/wav".to_string(),
            ]
        }
    }

    struct MprisPlayer {
        app: AppHandle,
        shared: Arc<Mutex<Shared>>,
    }

    #[interface(name = "org.mpris.MediaPlayer2.Player")]
    impl MprisPlayer {
        fn next(&self) {
            player_control::spawn_action(self.app.clone(), BackendAction::Next);
        }

        fn previous(&self) {
            player_control::spawn_action(self.app.clone(), BackendAction::Previous);
        }

        fn pause(&self) {
            player_control::spawn_action(self.app.clone(), BackendAction::Pause);
        }

        fn play_pause(&self) {
            player_control::spawn_action(self.app.clone(), BackendAction::PlayPause);
        }

        fn stop(&self) {
            player_control::spawn_action(self.app.clone(), BackendAction::Stop);
        }

        fn play(&self) {
            player_control::spawn_action(self.app.clone(), BackendAction::Play);
        }

        /// Offset relativo en microsegundos (puede ser negativo).
        fn seek(&self, offset: i64) {
            player_control::spawn_action(self.app.clone(), BackendAction::SeekByMicros(offset));
        }

        /// Posición absoluta; se ignora si el trackid no es el actual (spec MPRIS).
        fn set_position(&self, track_id: OwnedObjectPath, position: i64) {
            let is_current = {
                let shared = lock_shared(&self.shared);
                track_id.as_str() == track_path(shared.metadata.track_id)
            };
            if is_current {
                player_control::spawn_action(
                    self.app.clone(),
                    BackendAction::SetPositionMicros {
                        track_id: lock_shared(&self.shared).metadata.track_id.unwrap_or(0),
                        position_us: position,
                    },
                );
            }
        }

        fn open_uri(&self, _uri: String) {
            // Syncify no resuelve URIs externas: no-op deliberado.
        }

        #[zbus(property)]
        fn playback_status(&self) -> String {
            lock_shared(&self.shared).playback.as_str().to_string()
        }

        #[zbus(property)]
        fn loop_status(&self) -> String {
            lock_shared(&self.shared).loop_status.as_str().to_string()
        }

        #[zbus(property)]
        fn set_loop_status(&self, value: String) {
            if let Some(repeat) = RepeatMode::from_mpris_loop_status(&value) {
                player_control::spawn_set_repeat(self.app.clone(), repeat);
            }
        }

        #[zbus(property)]
        fn rate(&self) -> f64 {
            1.0
        }

        #[zbus(property)]
        fn shuffle(&self) -> bool {
            lock_shared(&self.shared).shuffle
        }

        #[zbus(property)]
        fn set_shuffle(&self, value: bool) {
            player_control::spawn_set_shuffle(self.app.clone(), value);
        }

        #[zbus(property)]
        fn metadata(&self) -> HashMap<String, OwnedValue> {
            metadata_dict(&lock_shared(&self.shared).metadata)
        }

        /// Sin sync inverso de volumen por ahora: el panel no escribe el nuestro.
        #[zbus(property)]
        fn volume(&self) -> f64 {
            1.0
        }

        #[zbus(property)]
        fn position(&self) -> i64 {
            lock_shared(&self.shared).position_us
        }

        #[zbus(property)]
        fn minimum_rate(&self) -> f64 {
            1.0
        }

        #[zbus(property)]
        fn maximum_rate(&self) -> f64 {
            1.0
        }

        #[zbus(property)]
        fn can_go_next(&self) -> bool {
            true
        }

        #[zbus(property)]
        fn can_go_previous(&self) -> bool {
            true
        }

        #[zbus(property)]
        fn can_play(&self) -> bool {
            true
        }

        #[zbus(property)]
        fn can_pause(&self) -> bool {
            true
        }

        #[zbus(property)]
        fn can_seek(&self) -> bool {
            true
        }

        #[zbus(property)]
        fn can_control(&self) -> bool {
            true
        }
    }
}

#[cfg(test)]
#[path = "mpris_tests.rs"]
mod tests;

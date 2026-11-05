//! Integración MPRIS nativa (Linux) del motor de reproducción local.
//!
//! La ruta antigua (`<audio>` + WebKit MediaSession) heredaba MPRIS del motor
//! media de WebKitGTK. Con el motor nativo (`syncify-player`, rodio + symphonia)
//! esa herencia desaparece: este servicio la reconstruye con `zbus` sobre el bus
//! de sesión y expone `org.mpris.MediaPlayer2` + `.Player` en
//! `/org/mpris/MediaPlayer2` (nombre de bus `org.mpris.MediaPlayer2.syncify`).
//!
//! Reparto de responsabilidades:
//! - El hilo MPRIS sólo LEE estado compartido y emite `PropertiesChanged`
//!   cuando algo cambia (polling ligero, 250 ms). La posición queda fuera del
//!   diff: los clientes MPRIS la consultan bajo demanda (como dicta la spec).
//! - Las acciones de los clientes (PlayPause, Next, Seek…) NO tocan el motor:
//!   se reenvían a la UI con el evento `player:mpris-action`; la UI es la dueña
//!   de la cola, los modos y el crédito de escucha — el mismo reparto que tenía
//!   `navigator.mediaSession`.
//! - El estado lo alimentan el ticker del player (`update_playback`, cada
//!   250 ms) y la UI (`player_mpris_update`: metadatos, shuffle, repeat).
//!
//! Sin bus de sesión disponible (o fuera de Linux) el servicio degrada a no-op:
//! la app arranca igual y los comandos no fallan.

use std::sync::{Mutex, MutexGuard};
use std::time::Duration;

use serde::Deserialize;
use tauri::AppHandle;

/// Estado de reproducción que ve MPRIS (`PlaybackStatus`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum PlaybackStatus {
    #[default]
    Stopped,
    Playing,
    Paused,
}

impl PlaybackStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Stopped => "Stopped",
            Self::Playing => "Playing",
            Self::Paused => "Paused",
        }
    }
}

/// `LoopStatus` de MPRIS ("None" | "Track" | "Playlist").
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
enum LoopStatus {
    #[default]
    None,
    Track,
    Playlist,
}

impl LoopStatus {
    fn as_str(self) -> &'static str {
        match self {
            Self::None => "None",
            Self::Track => "Track",
            Self::Playlist => "Playlist",
        }
    }

    fn from_wire(value: Option<&str>) -> Self {
        match value {
            Some("Track") => Self::Track,
            Some("Playlist") => Self::Playlist,
            _ => Self::None,
        }
    }
}

/// Metadatos y modos que la UI envía para el panel MPRIS del escritorio.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct MprisMetadataInput {
    pub track_id: Option<i64>,
    pub title: Option<String>,
    pub artist: Option<String>,
    pub album: Option<String>,
    pub duration_sec: Option<f64>,
    pub art_url: Option<String>,
    #[serde(default)]
    pub shuffle: bool,
    #[serde(default)]
    pub loop_status: Option<String>,
}

/// Copia interna (normalizada) de los metadatos.
#[derive(Debug, Clone, Default, PartialEq)]
struct MprisMetadata {
    track_id: Option<i64>,
    title: Option<String>,
    artist: Option<String>,
    album: Option<String>,
    duration_us: Option<i64>,
    art_url: Option<String>,
}

impl From<&MprisMetadataInput> for MprisMetadata {
    fn from(input: &MprisMetadataInput) -> Self {
        Self {
            track_id: input.track_id,
            title: input.title.clone(),
            artist: input.artist.clone(),
            album: input.album.clone(),
            duration_us: input
                .duration_sec
                .filter(|s| s.is_finite() && *s > 0.0)
                .map(|s| (s * 1_000_000.0) as i64),
            art_url: input.art_url.clone(),
        }
    }
}

/// Estado compartido entre el ticker/UI y el hilo D-Bus.
#[derive(Debug, Clone, Default)]
struct Shared {
    playback: PlaybackStatus,
    position_us: i64,
    metadata: MprisMetadata,
    loop_status: LoopStatus,
    shuffle: bool,
}

/// Subconjunto que decide si hay que emitir `PropertiesChanged`.
#[derive(Debug, Clone, Default, PartialEq)]
struct Snapshot {
    playback: PlaybackStatus,
    metadata: MprisMetadata,
    loop_status: LoopStatus,
    shuffle: bool,
}

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

    /// Publica el estado del motor (lo llama el ticker cada 250 ms).
    pub fn update_playback(&self, status: PlaybackStatus, position: Duration) {
        #[cfg(target_os = "linux")]
        {
            let mut shared = lock_shared(&self.shared);
            shared.playback = status;
            shared.position_us = i64::try_from(position.as_micros()).unwrap_or(i64::MAX);
        }
        #[cfg(not(target_os = "linux"))]
        {
            let _ = (status, position);
        }
    }

    /// Publica metadatos y modos (lo llama `player_mpris_update`).
    pub fn update_metadata(&self, input: &MprisMetadataInput) {
        #[cfg(target_os = "linux")]
        {
            let mut shared = lock_shared(&self.shared);
            shared.metadata = MprisMetadata::from(input);
            shared.loop_status = LoopStatus::from_wire(input.loop_status.as_deref());
            shared.shuffle = input.shuffle;
        }
        #[cfg(not(target_os = "linux"))]
        {
            let _ = input;
        }
    }
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

    use tauri::{AppHandle, Emitter, Manager};
    use zbus::blocking::object_server::InterfaceRef;
    use zbus::blocking::Connection;
    use zbus::fdo::Properties;
    use zbus::interface;
    use zbus::names::InterfaceName;
    use zbus::zvariant::{ObjectPath, OwnedObjectPath, OwnedValue, Value};

    use super::{lock_shared, MprisMetadata, Shared, Snapshot};

    const MPRIS_PATH: &str = "/org/mpris/MediaPlayer2";
    const BUS_NAME: &str = "org.mpris.MediaPlayer2.syncify";
    const PLAYER_INTERFACE: &str = "org.mpris.MediaPlayer2.Player";
    /// Evento Tauri con la acción MPRIS que debe ejecutar la UI.
    const ACTION_EVENT: &str = "player:mpris-action";

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
            self.emit_action("next", None, None);
        }

        fn previous(&self) {
            self.emit_action("previous", None, None);
        }

        fn pause(&self) {
            self.emit_action("pause", None, None);
        }

        fn play_pause(&self) {
            self.emit_action("playPause", None, None);
        }

        fn stop(&self) {
            self.emit_action("stop", None, None);
        }

        fn play(&self) {
            self.emit_action("play", None, None);
        }

        /// Offset relativo en microsegundos (puede ser negativo).
        fn seek(&self, offset: i64) {
            self.emit_action("seek", Some(offset), None);
        }

        /// Posición absoluta; se ignora si el trackid no es el actual (spec MPRIS).
        fn set_position(&self, track_id: OwnedObjectPath, position: i64) {
            let is_current = {
                let shared = lock_shared(&self.shared);
                track_id.as_str() == track_path(shared.metadata.track_id)
            };
            if is_current {
                self.emit_action("setPosition", None, Some(position));
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
        fn rate(&self) -> f64 {
            1.0
        }

        #[zbus(property)]
        fn shuffle(&self) -> bool {
            lock_shared(&self.shared).shuffle
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

    impl MprisPlayer {
        /// Reenvía la acción a la UI: allí viven cola, modos y crédito de escucha.
        fn emit_action(
            &self,
            action: &'static str,
            offset_us: Option<i64>,
            position_us: Option<i64>,
        ) {
            let payload = MprisActionPayload {
                action,
                offset_us,
                position_us,
            };
            if let Err(err) = self.app.emit(ACTION_EVENT, payload) {
                tracing::warn!("mpris: no se pudo reenviar la acción {action}: {err}");
            }
        }
    }

    #[derive(Clone, serde::Serialize)]
    #[serde(rename_all = "camelCase")]
    struct MprisActionPayload {
        action: &'static str,
        #[serde(skip_serializing_if = "Option::is_none")]
        offset_us: Option<i64>,
        #[serde(skip_serializing_if = "Option::is_none")]
        position_us: Option<i64>,
    }
}

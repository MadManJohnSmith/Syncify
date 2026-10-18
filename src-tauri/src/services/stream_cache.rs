//! Fase 2 — Caché y sesión de streaming progresivo.
//!
//! El motor nativo (`syncify-player`, rodio+symphonia) reproduce ficheros
//! locales. Para streaming, la descarga progresiva escribe un fichero en la
//! caché de la app y el motor lo reproduce mientras crece:
//!
//! 1. `commands::player::player_load_track` resuelve la URL
//!    (`services::streaming`), hace el *prime* (buffer inicial) y carga el
//!    motor.
//! 2. El resto de la descarga continúa en segundo plano escribiendo el mismo
//!    fichero.
//! 3. El ticker de `commands::player` vigila la frontera descargada: si la
//!    reproducción se acerca al final de lo descargado, pausa y espera
//!    ("buffering"); los seeks fuera del rango descargado quedan pendientes
//!    hasta que la descarga los alcanza.
//!
//! La URL y la sesión viven sólo en backend; al webview le llega una etiqueta
//! honesta de calidad (`StreamSourceInfo`) y eventos de estado.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime};

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, Manager};

use super::streaming::StreamSourceInfo;

/// Presupuesto máximo de la caché de streaming (se limpia la más antigua).
const CACHE_BUDGET_BYTES: u64 = 3 * 1024 * 1024 * 1024; // 3 GiB
/// Los parciales abandonados (crash) se limpian al arrancar la siguiente sesión.
const PARTIAL_MAX_AGE: Duration = Duration::from_secs(24 * 60 * 60);

/// Payload de `player:stream-state` (camelCase para el webview).
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StreamStatePayload {
    pub seq: u64,
    pub track_id: i64,
    pub buffering: bool,
    pub downloaded_bytes: u64,
    pub total_bytes: Option<u64>,
    pub complete: bool,
    pub pending_seek_sec: Option<f64>,
    pub service_name: String,
    pub quality_label: String,
    pub quality_tier: String,
    pub error: Option<String>,
}

/// Sesión de streaming activa (o cacheada) para UNA carga del motor.
pub struct StreamSession {
    /// `seq` del motor al que pertenece esta sesión.
    pub seq: u64,
    pub track_id: i64,
    pub info: StreamSourceInfo,
    pub path: PathBuf,
    /// `0` = desconocido (servidor sin Content-Length).
    pub total_bytes: AtomicU64,
    pub downloaded_bytes: AtomicU64,
    pub complete: AtomicBool,
    pub canceled: AtomicBool,
    /// El motor ya está cargado con este fichero.
    pub started: AtomicBool,
    /// El ticker pausó el motor por quedarse sin buffer.
    pub paused_for_buffer: AtomicBool,
    /// Seek pedido más allá de la frontera descargada (se completa al llegar).
    pub pending_seek_sec: Mutex<Option<f64>>,
    /// Tras completar un seek pendiente, reanudar (el usuario buscaba mientras sonaba).
    resume_after_seek: AtomicBool,
}

impl StreamSession {
    pub fn new(
        seq: u64,
        track_id: i64,
        info: StreamSourceInfo,
        path: PathBuf,
    ) -> Arc<StreamSession> {
        Arc::new(StreamSession {
            seq,
            track_id,
            info,
            path,
            total_bytes: AtomicU64::new(0),
            downloaded_bytes: AtomicU64::new(0),
            complete: AtomicBool::new(false),
            canceled: AtomicBool::new(false),
            started: AtomicBool::new(false),
            paused_for_buffer: AtomicBool::new(false),
            pending_seek_sec: Mutex::new(None),
            resume_after_seek: AtomicBool::new(false),
        })
    }

    pub fn is_canceled(&self) -> bool {
        self.canceled.load(Ordering::SeqCst)
    }

    /// Segundos de audio disponibles según los bytes descargados.
    /// `None` cuando aún no se conoce el tamaño total.
    pub fn frontier_secs(&self, duration_secs: f64) -> Option<f64> {
        let total = self.total_bytes.load(Ordering::SeqCst);
        if total == 0 || !duration_secs.is_finite() || duration_secs <= 0.0 {
            return None;
        }
        let downloaded = self.downloaded_bytes.load(Ordering::SeqCst).min(total);
        Some(downloaded as f64 / total as f64 * duration_secs)
    }

    pub fn take_pending_seek(&self) -> Option<f64> {
        self.pending_seek_sec.lock().ok().and_then(|mut p| p.take())
    }

    pub fn set_pending_seek(&self, sec: f64) {
        if let Ok(mut p) = self.pending_seek_sec.lock() {
            *p = Some(sec);
        }
    }

    pub fn set_resume_after_seek(&self, resume: bool) {
        self.resume_after_seek.store(resume, Ordering::SeqCst);
    }

    pub fn take_resume_after_seek(&self) -> bool {
        self.resume_after_seek.swap(false, Ordering::SeqCst)
    }
}

/// Estado gestionado (`app.manage`) con la sesión de streaming actual.
#[derive(Default)]
pub struct StreamCacheState {
    current: Mutex<Option<Arc<StreamSession>>>,
}

impl StreamCacheState {
    pub fn install(&self, session: Arc<StreamSession>) {
        if let Ok(mut guard) = self.current.lock() {
            *guard = Some(session);
        }
    }

    pub fn current(&self) -> Option<Arc<StreamSession>> {
        self.current.lock().ok().and_then(|guard| guard.clone())
    }

    /// Sesión cuya `seq` coincide con la carga actual del motor.
    pub fn for_seq(&self, seq: u64) -> Option<Arc<StreamSession>> {
        self.current().filter(|session| session.seq == seq)
    }

    /// Cancela y olvida la sesión actual (la descarga en curso lo detecta y
    /// limpia su parcial). Devuelve la sesión si existía.
    pub fn cancel_current(&self) -> Option<Arc<StreamSession>> {
        if let Ok(mut guard) = self.current.lock() {
            if let Some(session) = guard.take() {
                session.canceled.store(true, Ordering::SeqCst);
                return Some(session);
            }
        }
        None
    }
}

/// Emite `player:stream-state` para la sesión.
pub fn emit_state(app: &AppHandle, session: &StreamSession, error: Option<String>) {
    let total = session.total_bytes.load(Ordering::SeqCst);
    let payload = StreamStatePayload {
        seq: session.seq,
        track_id: session.track_id,
        buffering: session.paused_for_buffer.load(Ordering::SeqCst),
        downloaded_bytes: session.downloaded_bytes.load(Ordering::SeqCst),
        total_bytes: (total > 0).then_some(total),
        complete: session.complete.load(Ordering::SeqCst),
        pending_seek_sec: session.pending_seek_sec.lock().ok().and_then(|p| *p),
        service_name: session.info.service_name.clone(),
        quality_label: session.info.quality_label.clone(),
        quality_tier: session.info.quality_tier.clone(),
        error,
    };
    let _ = app.emit("player:stream-state", payload);
}

// ---------------------------------------------------------------------------
// Caché en disco
// ---------------------------------------------------------------------------

/// Directorio de caché de streaming (`app_cache_dir/stream`).
pub fn cache_dir(app: &AppHandle) -> Result<PathBuf, String> {
    let dir = app
        .path()
        .app_cache_dir()
        .map_err(|e| format!("No se pudo obtener la caché de la app: {}", e))?
        .join("stream");
    std::fs::create_dir_all(&dir)
        .map_err(|e| format!("No se pudo crear la caché de streaming: {}", e))?;
    Ok(dir)
}

/// Nombre estable por pista+calidad: permite reutilizar una descarga completa
/// aunque la URL firmada haya caducado.
pub fn cache_file_path(
    app: &AppHandle,
    track_id: i64,
    info: &StreamSourceInfo,
) -> Result<PathBuf, String> {
    let name = format!(
        "track-{}-{}-{}bit-{}hz.{}",
        track_id,
        info.service_name,
        info.bit_depth.unwrap_or(0),
        info.sample_rate.unwrap_or(0),
        info.extension
    );
    Ok(cache_dir(app)?.join(name))
}

fn marker_path(path: &Path) -> PathBuf {
    let mut name = path
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| "stream".to_string());
    name.push_str(".done");
    path.with_file_name(name)
}

#[derive(Serialize, Deserialize)]
struct CacheMarker {
    bytes: u64,
    info: StreamSourceInfo,
}

/// Escribe el marcador `.done` al completar una descarga (con la info real de
/// calidad obtenida, para que la reutilización no mienta).
pub fn write_done_marker(session: &StreamSession) -> Result<(), String> {
    let marker = CacheMarker {
        bytes: session.downloaded_bytes.load(Ordering::SeqCst),
        info: session.info.clone(),
    };
    let json = serde_json::to_vec(&marker)
        .map_err(|e| format!("No se pudo serializar el marcador de caché: {}", e))?;
    std::fs::write(marker_path(&session.path), json)
        .map_err(|e| format!("No se pudo escribir el marcador de caché: {}", e))
}

/// Comprueba si el fichero cacheado está completo (marcador + tamaño real).
pub fn read_complete_cache(path: &Path) -> Option<StreamSourceInfo> {
    let raw = std::fs::read(marker_path(path)).ok()?;
    let marker: CacheMarker = serde_json::from_slice(&raw).ok()?;
    let len = std::fs::metadata(path).ok()?.len();
    if len == marker.bytes && marker.bytes > 0 {
        Some(marker.info)
    } else {
        None
    }
}

/// Limpia parciales abandonados y recorta la caché al presupuesto (borra los
/// más antiguos). Nunca toca el fichero de la sesión recién creada.
pub fn cleanup_cache(app: &AppHandle, keep: Option<&Path>) {
    let Ok(dir) = cache_dir(app) else {
        return;
    };
    let Ok(entries) = std::fs::read_dir(&dir) else {
        return;
    };

    let now = SystemTime::now();
    let mut files: Vec<(PathBuf, u64, SystemTime)> = Vec::new();
    for entry in entries.flatten() {
        let path = entry.path();
        let Ok(meta) = entry.metadata() else { continue };
        if !meta.is_file() {
            continue;
        }
        let is_keep = keep.map(|k| k == path).unwrap_or(false);
        if is_keep {
            continue;
        }
        let modified = meta.modified().unwrap_or(SystemTime::UNIX_EPOCH);
        // Parciales viejos: descartar sin piedad.
        if path.extension().map(|e| e == "part").unwrap_or(false)
            && now
                .duration_since(modified)
                .map(|d| d > PARTIAL_MAX_AGE)
                .unwrap_or(false)
        {
            let _ = std::fs::remove_file(&path);
            let _ = std::fs::remove_file(marker_path(&path));
            continue;
        }
        files.push((path, meta.len(), modified));
    }

    let mut total: u64 = files.iter().map(|(_, len, _)| *len).sum();
    if total <= CACHE_BUDGET_BYTES {
        return;
    }
    files.sort_by_key(|(_, _, modified)| *modified);
    for (path, len, _) in files {
        if total <= CACHE_BUDGET_BYTES {
            break;
        }
        if std::fs::remove_file(&path).is_ok() {
            let _ = std::fs::remove_file(marker_path(&path));
            total = total.saturating_sub(len);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::services::streaming::StreamSourceInfo;

    fn sample_info() -> StreamSourceInfo {
        StreamSourceInfo {
            service_name: "qobuz".to_string(),
            service_track_id: "1".to_string(),
            format: "FLAC".to_string(),
            codec: Some("flac".to_string()),
            container: Some("FLAC".to_string()),
            extension: "flac".to_string(),
            bit_depth: Some(24),
            sample_rate: Some(192000),
            quality_label: "FLAC 24 bits / 192 kHz".to_string(),
            quality_tier: "hires".to_string(),
            source_name: "Qobuz API oficial".to_string(),
        }
    }

    #[test]
    fn frontier_scales_with_downloaded_bytes() {
        let session = StreamSession::new(1, 42, sample_info(), PathBuf::from("/tmp/x.flac"));
        session.total_bytes.store(100, Ordering::SeqCst);
        session.downloaded_bytes.store(25, Ordering::SeqCst);
        assert_eq!(session.frontier_secs(200.0), Some(50.0));
        // Sin total conocido no hay frontera.
        session.total_bytes.store(0, Ordering::SeqCst);
        assert_eq!(session.frontier_secs(200.0), None);
    }

    #[test]
    fn pending_seek_is_taken_once() {
        let session = StreamSession::new(1, 42, sample_info(), PathBuf::from("/tmp/x.flac"));
        session.set_pending_seek(12.5);
        assert_eq!(session.take_pending_seek(), Some(12.5));
        assert_eq!(session.take_pending_seek(), None);
    }

    #[test]
    fn cancel_marks_session() {
        let state = StreamCacheState::default();
        let session = StreamSession::new(3, 7, sample_info(), PathBuf::from("/tmp/y.flac"));
        assert!(state.for_seq(3).is_none());
        state.install(session.clone());
        assert!(state.for_seq(3).is_some());
        assert!(state.cancel_current().is_some());
        assert!(session.is_canceled());
        assert!(state.current().is_none());
    }
}

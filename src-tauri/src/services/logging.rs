//! Native System Logging Service (S170 & S170A)
//!
//! Provides:
//! 1. Development mode detection (`cfg!(debug_assertions)` or `SYNCIFY_ENV=development`).
//! 2. Rotating file logging active by default in development (`syncify-dev.log`).
//! 3. 50 MB size-based file rotation and 30-day retention cleanup.
//! 4. In-memory thread-safe circular log buffer.
//! 5. Secret and credential sanitization (tokens, passwords, signed URLs).
//! 6. Tracing Subscriber integration and Tauri IPC status reporting.

use chrono::Utc;
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;
use std::path::{Path, PathBuf};
use std::sync::{Arc, OnceLock, RwLock};
use tauri::Emitter;
use tracing::field::{Field, Visit};
use tracing::Level;

pub const DEFAULT_BUFFER_CAPACITY: usize = 2000;
pub const MAX_LOG_FILE_SIZE_BYTES: u64 = 50 * 1024 * 1024; // 50 MB
pub const LOG_RETENTION_DAYS: i64 = 30; // 30 days

/// Structured log entry for system audit and UI inspection
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SystemLogEntry {
    pub id: String,
    pub timestamp: String,
    pub level: String,      // "info", "warn", "error", "debug", "trace", "success"
    pub target: String,
    pub module: String,     // "Qobuz", "Tidal", "Spotify", "Worker", "Enrichment", "Database", "Filesystem", "System", etc.
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fields: Option<serde_json::Value>,
}

/// Status report DTO for UI & diagnostics
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct LoggingStatusDto {
    pub is_development: bool,
    pub file_logging_active: bool,
    pub active_log_file_path: Option<String>,
    pub log_dir: String,
    pub log_level: String,
    pub buffer_count: usize,
    pub retention_days: i64,
    pub max_file_size_mb: u64,
}

/// Global circular log buffer
pub struct LogBuffer {
    capacity: usize,
    entries: RwLock<VecDeque<SystemLogEntry>>,
    app_handle: RwLock<Option<tauri::AppHandle>>,
    counter: std::sync::atomic::AtomicU64,
}

impl LogBuffer {
    pub fn new(capacity: usize) -> Self {
        Self {
            capacity: capacity.max(1),
            entries: RwLock::new(VecDeque::with_capacity(capacity.min(500))),
            app_handle: RwLock::new(None),
            counter: std::sync::atomic::AtomicU64::new(1),
        }
    }

    /// Set Tauri AppHandle for real-time log event emission
    pub fn set_app_handle(&self, handle: tauri::AppHandle) {
        if let Ok(mut h) = self.app_handle.write() {
            *h = Some(handle);
        }
    }

    /// Add a new structured log entry into the ring buffer
    pub fn push(&self, mut entry: SystemLogEntry) {
        // Sanitize message before persisting
        entry.message = sanitize_log_message(&entry.message);

        // Assign incremental monotonic ID if missing
        if entry.id.is_empty() {
            let next_id = self.counter.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            entry.id = format!("log-{}", next_id);
        }

        // Emit live event to Tauri frontend if connected
        if let Ok(guard) = self.app_handle.read() {
            if let Some(ref handle) = *guard {
                let _ = handle.emit("syncify:log_event", &entry);
            }
        }

        // Store into ring buffer
        if let Ok(mut lock) = self.entries.write() {
            if lock.len() >= self.capacity {
                lock.pop_front();
            }
            lock.push_back(entry);
        }
    }

    /// Record a manual log entry directly
    #[allow(dead_code)] // usado por commands/logging.rs y tests del módulo; el lint lo marca muerto por análisis de unidad
    pub fn log(&self, level: &str, target: &str, module: &str, message: &str) {
        let entry = SystemLogEntry {
            id: String::new(),
            timestamp: Utc::now().to_rfc3339(),
            level: level.to_lowercase(),
            target: target.to_string(),
            module: if module.is_empty() {
                normalize_target_module(target)
            } else {
                module.to_string()
            },
            message: message.to_string(),
            fields: None,
        };
        self.push(entry);
    }

    /// Query logs with optional limit and filters
    pub fn get_logs(
        &self,
        limit: Option<usize>,
        level_filter: Option<&str>,
        module_filter: Option<&str>,
        search: Option<&str>,
    ) -> Vec<SystemLogEntry> {
        let max_limit = limit.unwrap_or(500).min(self.capacity);
        let guard = match self.entries.read() {
            Ok(g) => g,
            Err(_) => return Vec::new(),
        };

        let level_norm = level_filter.map(|l| l.trim().to_lowercase());
        let module_norm = module_filter.map(|m| m.trim().to_lowercase());
        let search_norm = search.map(|s| s.trim().to_lowercase());

        guard
            .iter()
            .rev() // Newest first
            .filter(|entry| {
                // Filter by level
                if let Some(ref lvl) = level_norm {
                    if lvl != "all" && !entry.level.eq_ignore_ascii_case(lvl) {
                        return false;
                    }
                }

                // Filter by module/provider
                if let Some(ref mod_filter) = module_norm {
                    if mod_filter != "all" {
                        let entry_mod = entry.module.to_lowercase();
                        let entry_target = entry.target.to_lowercase();
                        if !entry_mod.contains(mod_filter) && !entry_target.contains(mod_filter) {
                            return false;
                        }
                    }
                }

                // Search query
                if let Some(ref q) = search_norm {
                    if !q.is_empty() {
                        let matches_msg = entry.message.to_lowercase().contains(q);
                        let matches_mod = entry.module.to_lowercase().contains(q);
                        let matches_target = entry.target.to_lowercase().contains(q);
                        if !matches_msg && !matches_mod && !matches_target {
                            return false;
                        }
                    }
                }

                true
            })
            .take(max_limit)
            .cloned()
            .collect()
    }

    /// Clear all logs in the buffer
    pub fn clear(&self) {
        if let Ok(mut lock) = self.entries.write() {
            lock.clear();
        }
    }

    /// Export logs as plain text dump
    pub fn export_text(&self) -> String {
        let logs = self.get_logs(Some(self.capacity), None, None, None);
        let mut out = String::with_capacity(logs.len() * 128);
        out.push_str(&format!(
            "# Syncify System Log Export - Generated {}\n",
            Utc::now().to_rfc3339()
        ));
        out.push_str("# --------------------------------------------------\n");

        for l in logs.into_iter().rev() {
            out.push_str(&format!(
                "[{}] [{}] [{}] [{}] {}\n",
                l.timestamp,
                l.level.to_uppercase(),
                l.module,
                l.target,
                l.message
            ));
        }

        out
    }

    /// Total count of logs currently in buffer
    pub fn count(&self) -> usize {
        self.entries.read().map(|g| g.len()).unwrap_or(0)
    }
}

/// Thread-safe rotating file writer supporting 50 MB limits and 30-day retention
pub struct RotatingFileWriter {
    pub log_dir: PathBuf,
    pub active_filename: String,
    pub active_path: PathBuf,
    pub max_file_size: u64,
    pub retention_days: i64,
    file: std::sync::Mutex<Option<std::fs::File>>,
    is_active: std::sync::atomic::AtomicBool,
    last_cleanup: std::sync::Mutex<chrono::DateTime<Utc>>,
}

impl RotatingFileWriter {
    pub fn new(log_dir: PathBuf, active_filename: String) -> Self {
        let active_path = log_dir.join(&active_filename);
        let writer = Self {
            log_dir,
            active_filename,
            active_path,
            max_file_size: MAX_LOG_FILE_SIZE_BYTES,
            retention_days: LOG_RETENTION_DAYS,
            file: std::sync::Mutex::new(None),
            is_active: std::sync::atomic::AtomicBool::new(false),
            last_cleanup: std::sync::Mutex::new(chrono::DateTime::<Utc>::MIN_UTC),
        };
        writer.init_or_open();
        writer
    }

    pub fn is_active(&self) -> bool {
        self.is_active.load(std::sync::atomic::Ordering::Relaxed)
    }

    pub fn active_path(&self) -> &Path {
        &self.active_path
    }

    fn init_or_open(&self) {
        if let Err(e) = std::fs::create_dir_all(&self.log_dir) {
            eprintln!("[WARN] [Syncify Logging] Failed to create log directory {:?}: {}", self.log_dir, e);
            self.is_active.store(false, std::sync::atomic::Ordering::Relaxed);
            return;
        }

        match std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.active_path)
        {
            Ok(f) => {
                if let Ok(mut guard) = self.file.lock() {
                    *guard = Some(f);
                }
                self.is_active.store(true, std::sync::atomic::Ordering::Relaxed);
                self.cleanup_old_logs();
            }
            Err(e) => {
                eprintln!("[WARN] [Syncify Logging] Failed to open active log file {:?}: {}", self.active_path, e);
                self.is_active.store(false, std::sync::atomic::Ordering::Relaxed);
            }
        }
    }

    pub fn write_line(&self, line: &str) {
        if !self.is_active() {
            return;
        }

        let mut guard = match self.file.lock() {
            Ok(g) => g,
            Err(_) => return,
        };

        // Check if rotation needed
        let current_size = std::fs::metadata(&self.active_path).map(|m| m.len()).unwrap_or(0);
        let line_len = line.len() as u64 + 1; // including newline

        if current_size + line_len >= self.max_file_size {
            // Rotate current file
            drop(guard);
            self.rotate_current_file();
            guard = match self.file.lock() {
                Ok(g) => g,
                Err(_) => return,
            };
        }

        if let Some(ref mut f) = *guard {
            use std::io::Write;
            if let Err(e) = writeln!(f, "{}", line) {
                eprintln!("[WARN] [Syncify Logging] Write to logfile failed: {}", e);
            } else {
                let _ = f.flush();
            }
        }
    }

    pub fn rotate_current_file(&self) {
        let mut guard = match self.file.lock() {
            Ok(g) => g,
            Err(_) => return,
        };
        // Close active file
        *guard = None;

        let timestamp = Utc::now().format("%Y%m%dT%H%M%S").to_string();
        let stem = Path::new(&self.active_filename)
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("syncify");
        let ext = Path::new(&self.active_filename)
            .extension()
            .and_then(|s| s.to_str())
            .unwrap_or("log");
        let rotated_filename = format!("{}.{}.{}", stem, timestamp, ext);
        let rotated_path = self.log_dir.join(rotated_filename);

        if self.active_path.exists() {
            let _ = std::fs::rename(&self.active_path, &rotated_path);
        }

        // Open new active file
        match std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.active_path)
        {
            Ok(f) => {
                *guard = Some(f);
                self.is_active.store(true, std::sync::atomic::Ordering::Relaxed);
            }
            Err(e) => {
                eprintln!("[WARN] [Syncify Logging] Failed to open new logfile after rotation: {}", e);
                self.is_active.store(false, std::sync::atomic::Ordering::Relaxed);
            }
        }

        drop(guard);
        self.cleanup_old_logs();
    }

    pub fn cleanup_old_logs(&self) {
        let now = Utc::now();
        if let Ok(mut last_cleanup) = self.last_cleanup.lock() {
            if now.signed_duration_since(*last_cleanup).num_hours() < 1 && *last_cleanup != chrono::DateTime::<Utc>::MIN_UTC {
                return;
            }
            *last_cleanup = now;
        }

        let cutoff = now - chrono::Duration::days(self.retention_days);
        let entries = match std::fs::read_dir(&self.log_dir) {
            Ok(e) => e,
            Err(_) => return,
        };

        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_file() {
                // NEVER delete the active log file
                if path == self.active_path {
                    continue;
                }
                if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
                    if ext.eq_ignore_ascii_case("log") {
                        if let Ok(metadata) = std::fs::metadata(&path) {
                            if let Ok(modified) = metadata.modified() {
                                let modified_dt: chrono::DateTime<Utc> = modified.into();
                                if modified_dt < cutoff {
                                    let _ = std::fs::remove_file(&path);
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}

/// Detects development mode via debug assertions or SYNCIFY_ENV
pub fn is_development_mode() -> bool {
    if cfg!(debug_assertions) {
        return true;
    }
    match std::env::var("SYNCIFY_ENV") {
        Ok(val) => {
            let lower = val.trim().to_lowercase();
            lower == "development" || lower == "dev"
        }
        Err(_) => false,
    }
}

/// Resolves OS-native app log directory
pub fn resolve_app_log_dir() -> PathBuf {
    if let Ok(custom_dir) = std::env::var("SYNCIFY_LOG_DIR") {
        if !custom_dir.trim().is_empty() {
            return PathBuf::from(custom_dir);
        }
    }

    if let Some(base_data_dir) = dirs::data_local_dir().or_else(dirs::data_dir) {
        return base_data_dir.join("com.syncify.app").join("logs");
    }

    if let Some(home) = dirs::home_dir() {
        return home.join(".syncify").join("logs");
    }

    std::env::temp_dir().join("syncify_logs")
}

/// Parses level string into tracing::Level
pub fn parse_level_from_str(s: &str) -> Option<Level> {
    match s.trim().to_lowercase().as_str() {
        "error" => Some(Level::ERROR),
        "warn" | "warning" => Some(Level::WARN),
        "info" => Some(Level::INFO),
        "debug" => Some(Level::DEBUG),
        "trace" => Some(Level::TRACE),
        _ => None,
    }
}

/// Effective configuration model
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EffectiveLogConfig {
    pub is_development: bool,
    pub log_to_file: bool,
    pub log_level: Level,
    pub log_dir: PathBuf,
    pub active_log_path: PathBuf,
}

/// Resolves effective configuration obeying precedence rules
pub fn resolve_effective_log_config(
    persisted_log_to_file: Option<bool>,
    persisted_log_level: Option<&str>,
) -> EffectiveLogConfig {
    let is_dev = is_development_mode();

    // In development mode: log_to_file = true by default.
    // In production mode: respect persisted setting (default false).
    let log_to_file = if is_dev {
        true
    } else {
        persisted_log_to_file.unwrap_or(false)
    };

    // Precedence: RUST_LOG override > if dev: DEBUG > if prod: persisted level or INFO
    let log_level = if let Ok(env_filter) = std::env::var("RUST_LOG") {
        parse_level_from_str(&env_filter).unwrap_or(if is_dev { Level::DEBUG } else { Level::INFO })
    } else if is_dev {
        Level::DEBUG
    } else {
        persisted_log_level
            .and_then(parse_level_from_str)
            .unwrap_or(Level::INFO)
    };

    let log_dir = resolve_app_log_dir();
    let filename = if is_dev { "syncify-dev.log" } else { "syncify.log" };
    let active_log_path = log_dir.join(filename);

    EffectiveLogConfig {
        is_development: is_dev,
        log_to_file,
        log_level,
        log_dir,
        active_log_path,
    }
}

static GLOBAL_EFFECTIVE_CONFIG: RwLock<Option<EffectiveLogConfig>> = RwLock::new(None);
static GLOBAL_FILE_WRITER: RwLock<Option<Arc<RotatingFileWriter>>> = RwLock::new(None);

pub fn get_effective_log_config() -> EffectiveLogConfig {
    if let Ok(guard) = GLOBAL_EFFECTIVE_CONFIG.read() {
        if let Some(ref c) = *guard {
            return c.clone();
        }
    }
    resolve_effective_log_config(None, None)
}

pub fn get_global_file_writer() -> Option<Arc<RotatingFileWriter>> {
    GLOBAL_FILE_WRITER.read().ok().and_then(|g| g.clone())
}

/// Tracing Subscriber Layer that pipes tracing events into `LogBuffer`
pub struct LogBufferLayer {
    buffer: Arc<LogBuffer>,
}

impl LogBufferLayer {
    pub fn new(buffer: Arc<LogBuffer>) -> Self {
        Self { buffer }
    }
}

impl<S> tracing_subscriber::Layer<S> for LogBufferLayer
where
    S: tracing::Subscriber,
{
    fn on_event(
        &self,
        event: &tracing::Event<'_>,
        _ctx: tracing_subscriber::layer::Context<'_, S>,
    ) {
        let metadata = event.metadata();
        let target = metadata.target();
        let level = match *metadata.level() {
            Level::ERROR => "error",
            Level::WARN => "warn",
            Level::INFO => "info",
            Level::DEBUG => "debug",
            Level::TRACE => "trace",
        };

        let mut visitor = FieldVisitor::new();
        event.record(&mut visitor);

        let message = visitor.message.unwrap_or_default();
        if message.is_empty() {
            return;
        }

        let module = normalize_target_module(target);
        let fields = if visitor.extra_fields.is_empty() {
            None
        } else {
            Some(serde_json::Value::Object(visitor.extra_fields))
        };

        let entry = SystemLogEntry {
            id: String::new(),
            timestamp: Utc::now().to_rfc3339(),
            level: level.to_string(),
            target: target.to_string(),
            module,
            message,
            fields,
        };

        self.buffer.push(entry);
    }
}

/// Tracing Subscriber Layer that writes formatted, sanitized events to the active log file
pub struct FileLogLayer {
    writer: Arc<RotatingFileWriter>,
}

impl FileLogLayer {
    pub fn new(writer: Arc<RotatingFileWriter>) -> Self {
        Self { writer }
    }
}

impl<S> tracing_subscriber::Layer<S> for FileLogLayer
where
    S: tracing::Subscriber,
{
    fn on_event(
        &self,
        event: &tracing::Event<'_>,
        _ctx: tracing_subscriber::layer::Context<'_, S>,
    ) {
        if !self.writer.is_active() {
            return;
        }

        let metadata = event.metadata();
        let target = metadata.target();
        let level = match *metadata.level() {
            Level::ERROR => "ERROR",
            Level::WARN => "WARN",
            Level::INFO => "INFO",
            Level::DEBUG => "DEBUG",
            Level::TRACE => "TRACE",
        };

        let mut visitor = FieldVisitor::new();
        event.record(&mut visitor);

        let message = visitor.message.unwrap_or_default();
        if message.is_empty() {
            return;
        }

        let sanitized_msg = sanitize_log_message(&message);
        let module = normalize_target_module(target);
        let timestamp = Utc::now().to_rfc3339();

        let formatted_line = if visitor.extra_fields.is_empty() {
            format!("[{}] [{}] [{}] [{}] {}", timestamp, level, module, target, sanitized_msg)
        } else {
            let fields_json = serde_json::Value::Object(visitor.extra_fields).to_string();
            let sanitized_fields = sanitize_log_message(&fields_json);
            format!("[{}] [{}] [{}] [{}] {} {}", timestamp, level, module, target, sanitized_msg, sanitized_fields)
        };

        self.writer.write_line(&formatted_line);
    }
}

/// Global singleton log buffer accessor
pub fn get_global_log_buffer() -> &'static Arc<LogBuffer> {
    static BUFFER: OnceLock<Arc<LogBuffer>> = OnceLock::new();
    BUFFER.get_or_init(|| Arc::new(LogBuffer::new(DEFAULT_BUFFER_CAPACITY)))
}

/// Query current system logging status for IPC and UI
pub fn get_logging_status() -> LoggingStatusDto {
    let config = get_effective_log_config();
    let is_dev = config.is_development;
    let file_writer = get_global_file_writer();
    let file_active = file_writer.as_ref().map(|w| w.is_active()).unwrap_or(false);

    let active_path_opt = if is_dev && file_active {
        file_writer.as_ref().map(|w| w.active_path().to_string_lossy().to_string())
    } else {
        None
    };

    LoggingStatusDto {
        is_development: is_dev,
        file_logging_active: file_active,
        active_log_file_path: active_path_opt,
        log_dir: config.log_dir.to_string_lossy().to_string(),
        log_level: config.log_level.to_string(),
        buffer_count: get_global_log_buffer().count(),
        retention_days: LOG_RETENTION_DAYS,
        max_file_size_mb: MAX_LOG_FILE_SIZE_BYTES / (1024 * 1024),
    }
}

/// Initialize tracing subscriber with unified console, in-memory ring buffer, and rotating file logging
pub fn init_logging_system(
    persisted_to_file: Option<bool>,
    persisted_level: Option<&str>,
) -> EffectiveLogConfig {
    static INIT_ONCE: std::sync::Once = std::sync::Once::new();
    let config = resolve_effective_log_config(persisted_to_file, persisted_level);

    INIT_ONCE.call_once(|| {
        use tracing_subscriber::layer::SubscriberExt;
        use tracing_subscriber::util::SubscriberInitExt;
        use tracing_subscriber::Layer;

        if let Ok(mut cfg_guard) = GLOBAL_EFFECTIVE_CONFIG.write() {
            *cfg_guard = Some(config.clone());
        }

        let log_buffer = get_global_log_buffer();
        let buffer_layer = LogBufferLayer::new(log_buffer.clone());

        let filter = tracing_subscriber::filter::LevelFilter::from_level(config.log_level);

        let registry = tracing_subscriber::registry()
            .with(tracing_subscriber::fmt::layer().with_filter(filter))
            .with(buffer_layer.with_filter(filter));

        if config.log_to_file {
            let filename = if config.is_development { "syncify-dev.log" } else { "syncify.log" };
            let writer = Arc::new(RotatingFileWriter::new(config.log_dir.clone(), filename.to_string()));
            if let Ok(mut w_guard) = GLOBAL_FILE_WRITER.write() {
                *w_guard = Some(writer.clone());
            }
            let file_layer = FileLogLayer::new(writer).with_filter(filter);
            let _ = registry.with(file_layer).try_init();
        } else {
            let _ = registry.try_init();
        }
    });

    config
}

/// Sanitizes sensitive secrets such as Bearer tokens, JSON credentials, cookies, and passwords
pub fn sanitize_log_message(msg: &str) -> String {
    static SENSITIVE_REGEX: OnceLock<Vec<Regex>> = OnceLock::new();
    let regexes = SENSITIVE_REGEX.get_or_init(|| {
        vec![
            // Bearer tokens
            Regex::new(r"(?i)Bearer\s+[A-Za-z0-9_\-\.]{12,}").unwrap(),
            // Generic token assignments, session cookies (sp_dc, sp_key, arl), and secrets
            Regex::new(r#"(?i)(auth_token|user_auth_token|access_token|refresh_token|token|api_key|client_secret|password|secret|app_secret|user_token|sp_dc|sp_key|arl|session_token|session_id)["']?\s*[:=]\s*["']?[A-Za-z0-9_\-\.%+=]{8,}["']?"#).unwrap(),
            // Credentials JSON blocks
            Regex::new(r#"(?i)credentials_json\s*=\s*["'][^"']+["']"#).unwrap(),
            // Basic Authorization
            Regex::new(r"(?i)Authorization:\s*Basic\s+[A-Za-z0-9+/=]{8,}").unwrap(),
            // Signed URL and query parameters (Signature, Expires, Key-Pair-Id, token, sp_dc, sp_key, arl)
            Regex::new(r"(?i)(Signature|Expires|Key-Pair-Id|token|api_key|code|sp_dc|sp_key|arl)=[A-Za-z0-9_\-\.%+=]+").unwrap(),
            // Cookie and Set-Cookie headers
            Regex::new(r#"(?i)["']?\b(?:set-)?cookie["']?\s*:\s*["']?[^"'\r\n]+["']?"#).unwrap(),
        ]
    });

    let mut result = msg.to_string();
    for re in regexes {
        result = re.replace_all(&result, "[REDACTED]").to_string();
    }
    result
}

/// Normalizes target/module paths to clean high-level categories
pub fn normalize_target_module(target: &str) -> String {
    let lower = target.to_lowercase();
    if lower.contains("qobuz") {
        "Qobuz".to_string()
    } else if lower.contains("tidal") {
        "Tidal".to_string()
    } else if lower.contains("spotify") {
        "Spotify".to_string()
    } else if lower.contains("deezer") {
        "Deezer".to_string()
    } else if lower.contains("apple") {
        "Apple Music".to_string()
    } else if lower.contains("soundcloud") {
        "SoundCloud".to_string()
    } else if lower.contains("musicbrainz") {
        "MusicBrainz".to_string()
    } else if lower.contains("lastfm") {
        "Last.fm".to_string()
    } else if lower.contains("worker") {
        "Worker".to_string()
    } else if lower.contains("downloader") || lower.contains("download") {
        "Downloads".to_string()
    } else if lower.contains("enrichment") || lower.contains("metadata") {
        "Enrichment".to_string()
    } else if lower.contains("db") || lower.contains("sqlx") || lower.contains("database") {
        "Database".to_string()
    } else if lower.contains("crypto") || lower.contains("keychain") || lower.contains("auth") {
        "Security".to_string()
    } else if lower.contains("scanner") || lower.contains("organize") || lower.contains("filesystem") {
        "Filesystem".to_string()
    } else if lower.contains("lyrics") {
        "Lyrics".to_string()
    } else {
        "System".to_string()
    }
}

/// Visitor for tracing fields
struct FieldVisitor {
    message: Option<String>,
    extra_fields: serde_json::Map<String, serde_json::Value>,
}

impl FieldVisitor {
    fn new() -> Self {
        Self {
            message: None,
            extra_fields: serde_json::Map::new(),
        }
    }
}

impl Visit for FieldVisitor {
    fn record_debug(&mut self, field: &Field, value: &dyn std::fmt::Debug) {
        let name = field.name();
        let val_str = format!("{:?}", value);
        if name == "message" {
            self.message = Some(val_str);
        } else {
            self.extra_fields.insert(name.to_string(), serde_json::Value::String(val_str));
        }
    }

    fn record_str(&mut self, field: &Field, value: &str) {
        let name = field.name();
        if name == "message" {
            self.message = Some(value.to_string());
        } else {
            self.extra_fields.insert(name.to_string(), serde_json::Value::String(value.to_string()));
        }
    }

    fn record_i64(&mut self, field: &Field, value: i64) {
        let name = field.name();
        self.extra_fields.insert(name.to_string(), serde_json::json!(value));
    }

    fn record_u64(&mut self, field: &Field, value: u64) {
        let name = field.name();
        self.extra_fields.insert(name.to_string(), serde_json::json!(value));
    }

    fn record_bool(&mut self, field: &Field, value: bool) {
        let name = field.name();
        self.extra_fields.insert(name.to_string(), serde_json::json!(value));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_log_buffer_push_and_query() {
        let buffer = LogBuffer::new(10);
        buffer.log("info", "syncify::qobuz", "Qobuz", "Connected to Qobuz API");
        buffer.log("error", "syncify::worker", "Worker", "Download failed for item 42");
        buffer.log("warn", "syncify::spotify", "Spotify", "Rate limit approaching");

        let logs = buffer.get_logs(None, None, None, None);
        assert_eq!(logs.len(), 3);
        assert_eq!(logs[0].message, "Rate limit approaching");
        assert_eq!(logs[1].message, "Download failed for item 42");
        assert_eq!(logs[2].message, "Connected to Qobuz API");

        // Filter by level
        let errors = buffer.get_logs(None, Some("error"), None, None);
        assert_eq!(errors.len(), 1);
        assert_eq!(errors[0].module, "Worker");

        // Filter by module
        let qobuz = buffer.get_logs(None, None, Some("qobuz"), None);
        assert_eq!(qobuz.len(), 1);
        assert_eq!(qobuz[0].level, "info");

        // Search text
        let search = buffer.get_logs(None, None, None, Some("item 42"));
        assert_eq!(search.len(), 1);
        assert_eq!(search[0].level, "error");
    }

    #[test]
    fn test_log_buffer_capacity_eviction() {
        let buffer = LogBuffer::new(3);
        buffer.log("info", "sys", "System", "Log 1");
        buffer.log("info", "sys", "System", "Log 2");
        buffer.log("info", "sys", "System", "Log 3");
        buffer.log("info", "sys", "System", "Log 4");

        let logs = buffer.get_logs(None, None, None, None);
        assert_eq!(logs.len(), 3);
        assert_eq!(logs[0].message, "Log 4");
        assert_eq!(logs[1].message, "Log 3");
        assert_eq!(logs[2].message, "Log 2");
    }

    #[test]
    fn test_secret_sanitizer() {
        let raw = "Auth failed for user with Bearer eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9 and access_token=secret_123456789";
        let sanitized = sanitize_log_message(raw);
        assert!(!sanitized.contains("eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9"));
        assert!(!sanitized.contains("secret_123456789"));
        assert!(sanitized.contains("[REDACTED]"));

        let signed_url = "https://audio.tidal.com/stream?Signature=abc123xyz789&Expires=1789000000&Key-Pair-Id=K12345";
        let sanitized_url = sanitize_log_message(signed_url);
        assert!(!sanitized_url.contains("abc123xyz789"));
        assert!(!sanitized_url.contains("1789000000"));
        assert!(!sanitized_url.contains("K12345"));
        assert!(sanitized_url.contains("[REDACTED]"));

        // Session cookies & Cookie headers (TASK-100 / SEC-016)
        let cookie_log = "Request sent with Cookie: sp_dc=AQBAEPG...; sp_key=12345678; other=val";
        let sanitized_cookie = sanitize_log_message(cookie_log);
        assert!(!sanitized_cookie.contains("AQBAEPG"));
        assert!(!sanitized_cookie.contains("12345678"));
        assert!(sanitized_cookie.contains("[REDACTED]"));

        let arl_log = "Deezer user token: arl=1234567890abcdef1234567890abcdef";
        let sanitized_arl = sanitize_log_message(arl_log);
        assert!(!sanitized_arl.contains("1234567890abcdef1234567890abcdef"));
        assert!(sanitized_arl.contains("[REDACTED]"));

        let sp_dc_log = "Spotify session cookie: sp_dc=AQB_secret_session_token_value";
        let sanitized_sp_dc = sanitize_log_message(sp_dc_log);
        assert!(!sanitized_sp_dc.contains("AQB_secret_session_token_value"));
        assert!(sanitized_sp_dc.contains("[REDACTED]"));
    }

    #[test]
    fn test_export_text() {
        let buffer = LogBuffer::new(10);
        buffer.log("info", "sys", "System", "Export test log");
        let export = buffer.export_text();
        assert!(export.contains("Syncify System Log Export"));
        assert!(export.contains("Export test log"));
    }

    #[test]
    fn test_rotating_file_writer_creation_and_write() {
        let temp_dir = std::env::temp_dir().join(format!("syncify_log_test_{}", uuid::Uuid::new_v4()));
        let writer = RotatingFileWriter::new(temp_dir.clone(), "test-syncify.log".to_string());
        assert!(writer.is_active());
        assert!(writer.active_path().exists());

        writer.write_line("[2026-08-23T16:00:00Z] [INFO] [System] [syncify] Hello file logging");
        let content = std::fs::read_to_string(writer.active_path()).unwrap();
        assert!(content.contains("Hello file logging"));

        let _ = std::fs::remove_dir_all(&temp_dir);
    }
}

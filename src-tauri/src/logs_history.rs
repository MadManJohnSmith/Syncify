//! Log history from disk (R15 — round trip of the log)
//!
//! The app writes logs to rotating files under the app log dir
//! (`services::logging::FileLogLayer` + `RotatingFileWriter`) but there was no
//! way to read them back. This module adds two Tauri commands:
//!
//! - `read_log_history(from, to, level, query, offset, limit)`: parses the
//!   rotating log files (`syncify[-dev].log` + `*.log` rotations) and returns a
//!   filtered, paginated page of `SystemLogEntry`.
//! - `export_log_range(from, to, dest_path[, level, query])`: writes a range of
//!   the history to a user-chosen file (destination picked via the native
//!   dialog in the UI).
//!
//! Line format produced by `FileLogLayer`:
//! `[RFC3339] [LEVEL] [module] [target] message [{optional fields JSON}]`

use chrono::{DateTime, Utc};
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::io::{BufRead, BufWriter};
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

/// Hard cap on parsed entries per query: the history scan must stay bounded
/// even if retention has not pruned old files yet.
pub const MAX_HISTORY_SCAN_ENTRIES: usize = 200_000;

/// Page size cap, aligned with the in-memory ring buffer capacity.
pub const MAX_HISTORY_PAGE_LIMIT: usize = 2000;

const DEFAULT_PAGE_LIMIT: usize = 500;

/// One page of the on-disk log history, newest entries first.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct LogHistoryPage {
    pub entries: Vec<crate::services::logging::SystemLogEntry>,
    pub total: usize,
    pub offset: usize,
    pub limit: usize,
}

/// Result of `export_log_range`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct LogHistoryExportResult {
    pub path: String,
    pub exported_count: usize,
}

/// Filters shared by `read_log_history` and `export_log_range`.
#[derive(Debug, Default, Clone)]
struct HistoryFilters {
    from: Option<DateTime<Utc>>,
    to: Option<DateTime<Utc>>,
    level: Option<String>,
    query: Option<String>,
}

/// Parse a range bound that can be either a full RFC3339 timestamp or a plain
/// `YYYY-MM-DD` date. For the `to` bound a bare date is expanded to the last
/// millisecond of that day so the whole day is included.
fn parse_range_bound(raw: Option<&str>, end_of_day: bool) -> Result<Option<DateTime<Utc>>, String> {
    let Some(raw) = raw.map(str::trim).filter(|v| !v.is_empty()) else {
        return Ok(None);
    };

    if let Ok(dt) = DateTime::parse_from_rfc3339(raw) {
        return Ok(Some(dt.with_timezone(&Utc)));
    }

    let date = chrono::NaiveDate::parse_from_str(raw, "%Y-%m-%d")
        .map_err(|e| format!("Invalid date '{raw}' (expected RFC3339 or YYYY-MM-DD): {e}"))?;

    let dt = if end_of_day {
        date.and_hms_milli_opt(23, 59, 59, 999)
    } else {
        date.and_hms_milli_opt(0, 0, 0, 0)
    }
    .ok_or_else(|| format!("Invalid date '{raw}'"))?;

    Ok(Some(dt.and_utc()))
}

impl HistoryFilters {
    fn build(
        from: Option<&str>,
        to: Option<&str>,
        level: Option<&str>,
        query: Option<&str>,
    ) -> Result<Self, String> {
        let level = level
            .map(str::trim)
            .filter(|l| !l.is_empty() && !l.eq_ignore_ascii_case("all"))
            .map(|l| {
                // Acepta el alias 'warning' igual que el ring buffer en memoria.
                if l.eq_ignore_ascii_case("warning") {
                    "warn".to_string()
                } else {
                    l.to_lowercase()
                }
            });
        let query = query
            .map(str::trim)
            .filter(|q| !q.is_empty())
            .map(str::to_lowercase);

        Ok(Self {
            from: parse_range_bound(from, false)?,
            to: parse_range_bound(to, true)?,
            level,
            query,
        })
    }

    /// Whether a parsed entry passes every configured filter.
    fn matches(&self, ts: DateTime<Utc>, entry: &crate::services::logging::SystemLogEntry) -> bool {
        if let Some(from) = self.from {
            if ts < from {
                return false;
            }
        }
        if let Some(to) = self.to {
            if ts > to {
                return false;
            }
        }
        if let Some(level) = &self.level {
            if !entry.level.eq_ignore_ascii_case(level) {
                return false;
            }
        }
        if let Some(query) = &self.query {
            let matches = entry.message.to_lowercase().contains(query)
                || entry.module.to_lowercase().contains(query)
                || entry.target.to_lowercase().contains(query);
            if !matches {
                return false;
            }
        }
        true
    }
}

fn line_regex() -> &'static Regex {
    static LINE_RE: OnceLock<Regex> = OnceLock::new();
    LINE_RE.get_or_init(|| {
        // [timestamp] [LEVEL] [module] [target] message [fields JSON]
        Regex::new(r"^\[([^\]]+)\]\s+\[([A-Za-z]+)\]\s+\[([^\]]*)\]\s+\[([^\]]*)\]\s?(.*)$")
            .expect("log history line regex must compile")
    })
}

/// Splits the trailing optional fields JSON blob (`{...}` appended by
/// `FileLogLayer` after the message) from the plain message text.
fn split_message_fields(rest: &str) -> (String, Option<serde_json::Value>) {
    let trimmed = rest.trim_end();
    if !trimmed.ends_with('}') {
        return (trimmed.to_string(), None);
    }

    // FileLogLayer joins message and fields with a single space; try the last
    // few ` {"` separators (the message itself may contain JSON braces).
    let mut search_end = trimmed.len();
    for _ in 0..3 {
        let window = &trimmed[..search_end];
        let Some(rel) = window.rfind(" {\"") else {
            break;
        };
        let candidate = &trimmed[rel + 1..];
        match serde_json::from_str::<serde_json::Value>(candidate) {
            Ok(value) if value.is_object() => {
                return (trimmed[..rel].trim_end().to_string(), Some(value));
            }
            _ => {
                search_end = rel;
            }
        }
    }

    (trimmed.to_string(), None)
}

/// Parses one `FileLogLayer` line into `(timestamp, SystemLogEntry)`.
/// Returns `None` for lines that do not follow the format.
pub fn parse_log_line(
    line: &str,
) -> Option<(DateTime<Utc>, crate::services::logging::SystemLogEntry)> {
    let caps = line_regex().captures(line.trim_end())?;

    let ts = DateTime::parse_from_rfc3339(caps.get(1)?.as_str().trim())
        .ok()?
        .with_timezone(&Utc);
    let level = caps.get(2)?.as_str().to_lowercase();
    let module = caps.get(3)?.as_str().to_string();
    let target = caps.get(4)?.as_str().to_string();
    let (message, fields) = split_message_fields(caps.get(5)?.as_str());

    if message.is_empty() {
        return None;
    }

    Some((
        ts,
        crate::services::logging::SystemLogEntry {
            // Stable id: the md5 of the raw line dedupes when the UI pages
            // through or reloads the same history slice.
            id: format!("hist-{:x}", md5::compute(line.trim_end())),
            timestamp: ts.to_rfc3339(),
            level,
            target,
            module,
            message,
            fields,
        },
    ))
}

/// Lists the rotating log files of the app log dir: the active
/// `syncify[-dev].log` plus every `*.log` rotation of it.
pub fn list_history_files(log_dir: &Path) -> Vec<PathBuf> {
    let Ok(entries) = std::fs::read_dir(log_dir) else {
        return Vec::new();
    };

    let mut files: Vec<PathBuf> = entries
        .flatten()
        .map(|e| e.path())
        .filter(|p| {
            p.is_file()
                && p.file_name()
                    .and_then(|n| n.to_str())
                    .map(|n| n.starts_with("syncify") && n.ends_with(".log"))
                    .unwrap_or(false)
        })
        .collect();

    // Rotated files first, active file last: reading in this order approximates
    // the on-disk chronological order before the final sort.
    files.sort_by_key(|p| {
        std::fs::metadata(p)
            .and_then(|m| m.modified())
            .map(std::time::SystemTime::into)
            .unwrap_or(std::time::SystemTime::UNIX_EPOCH)
    });
    files
}

/// Walks every log file, parses and filters entries, and returns them sorted
/// newest first. Bounded by [`MAX_HISTORY_SCAN_ENTRIES`].
fn collect_history(
    log_dir: &Path,
    filters: &HistoryFilters,
) -> Vec<(DateTime<Utc>, crate::services::logging::SystemLogEntry)> {
    let mut collected: Vec<(DateTime<Utc>, crate::services::logging::SystemLogEntry)> = Vec::new();

    'files: for path in list_history_files(log_dir) {
        let Ok(file) = std::fs::File::open(&path) else {
            continue;
        };
        let reader = std::io::BufReader::new(file);
        for line in reader.lines().map_while(Result::ok) {
            if collected.len() >= MAX_HISTORY_SCAN_ENTRIES {
                tracing::warn!(
                    "Log history scan reached the {} entry cap; older lines are not included",
                    MAX_HISTORY_SCAN_ENTRIES
                );
                break 'files;
            }
            if let Some((ts, entry)) = parse_log_line(&line) {
                if filters.matches(ts, &entry) {
                    collected.push((ts, entry));
                }
            }
        }
    }

    collected.sort_by_key(|a| std::cmp::Reverse(a.0)); // Newest first
    collected
}

fn history_log_dir() -> PathBuf {
    crate::services::logging::get_effective_log_config().log_dir
}

/// Synchronous implementation of `read_log_history` (runs on the blocking pool).
fn read_history_blocking(
    log_dir: PathBuf,
    filters: HistoryFilters,
    offset: usize,
    limit: usize,
) -> Result<LogHistoryPage, String> {
    let all = collect_history(&log_dir, &filters);
    let total = all.len();

    let page_limit = limit.clamp(1, MAX_HISTORY_PAGE_LIMIT);
    let entries = all
        .into_iter()
        .skip(offset)
        .take(page_limit)
        .map(|(_, entry)| entry)
        .collect();

    Ok(LogHistoryPage {
        entries,
        total,
        offset,
        limit: page_limit,
    })
}

/// Tauri command: reads the rotating log files from disk (R15 round trip).
///
/// Params: `from`, `to` (RFC3339 or YYYY-MM-DD), `level` ("all" or a level),
/// `query` (case-insensitive substring), `offset`/`limit` for pagination.
#[tauri::command]
pub async fn read_log_history(
    from: Option<String>,
    to: Option<String>,
    level: Option<String>,
    query: Option<String>,
    offset: Option<usize>,
    limit: Option<usize>,
) -> Result<LogHistoryPage, String> {
    let log_dir = history_log_dir();
    let filters = HistoryFilters::build(
        from.as_deref(),
        to.as_deref(),
        level.as_deref(),
        query.as_deref(),
    )?;
    let offset = offset.unwrap_or(0);

    // File scans can move tens of MB: keep them off the async runtime threads.
    // Sin límite explícito se usa la página por defecto (read_history_blocking
    // re-acota a 1..=MAX_HISTORY_PAGE_LIMIT).
    tauri::async_runtime::spawn_blocking(move || {
        read_history_blocking(
            log_dir,
            filters,
            offset,
            limit.unwrap_or(DEFAULT_PAGE_LIMIT),
        )
    })
    .await
    .map_err(|e| format!("Log history task failed: {e}"))?
}

/// Synchronous implementation of `export_log_range`.
fn export_history_blocking(
    log_dir: PathBuf,
    filters: HistoryFilters,
    dest_path: PathBuf,
) -> Result<LogHistoryExportResult, String> {
    let mut all = collect_history(&log_dir, &filters);
    // An export reads chronologically (oldest first), opposite to the page view.
    all.sort_by_key(|a| a.0);

    if let Some(parent) = dest_path.parent() {
        if !parent.as_os_str().is_empty() {
            std::fs::create_dir_all(parent)
                .map_err(|e| format!("Failed to create {}: {e}", parent.display()))?;
        }
    }

    let file = std::fs::File::create(&dest_path)
        .map_err(|e| format!("Failed to create {}: {e}", dest_path.display()))?;
    let mut writer = BufWriter::new(file);

    use std::io::Write;
    writeln!(
        writer,
        "# Syncify Log History Export - Generated {}",
        Utc::now().to_rfc3339()
    )
    .and_then(|_| {
        writeln!(
            writer,
            "# Range: {} .. {}",
            filters
                .from
                .map(|d| d.to_rfc3339())
                .unwrap_or_else(|| "(beginning)".to_string()),
            filters
                .to
                .map(|d| d.to_rfc3339())
                .unwrap_or_else(|| "(now)".to_string()),
        )
    })
    .and_then(|_| {
        writeln!(
            writer,
            "# --------------------------------------------------"
        )
    })
    .map_err(|e| format!("Failed to write {}: {e}", dest_path.display()))?;

    let count = all.len();
    for (_, entry) in all {
        writeln!(
            writer,
            "[{}] [{}] [{}] [{}] {}",
            entry.timestamp,
            entry.level.to_uppercase(),
            entry.module,
            entry.target,
            entry.message
        )
        .map_err(|e| format!("Failed to write {}: {e}", dest_path.display()))?;
    }

    let path = dest_path.to_string_lossy().to_string();
    tracing::info!(exported = count, path = %path, "Log history exported");
    Ok(LogHistoryExportResult {
        path,
        exported_count: count,
    })
}

/// Tauri command: exports a range of the on-disk log history to a file chosen
/// by the user through the native dialog.
///
/// `level`/`query` are optional so the UI can export exactly the filtered
/// slice it is looking at.
#[tauri::command]
pub async fn export_log_range(
    from: Option<String>,
    to: Option<String>,
    dest_path: String,
    level: Option<String>,
    query: Option<String>,
) -> Result<LogHistoryExportResult, String> {
    let dest = PathBuf::from(dest_path.trim());
    if dest.as_os_str().is_empty() {
        return Err("Destination path is empty".to_string());
    }

    let log_dir = history_log_dir();
    let filters = HistoryFilters::build(
        from.as_deref(),
        to.as_deref(),
        level.as_deref(),
        query.as_deref(),
    )?;

    tauri::async_runtime::spawn_blocking(move || export_history_blocking(log_dir, filters, dest))
        .await
        .map_err(|e| format!("Log export task failed: {e}"))?
}

#[cfg(test)]
mod tests {
    use super::*;

    // Formato FileLogLayer: `mensaje {opcional fields JSON}`; el token es
    // inventado y viaja en fields, nunca dentro del mensaje.
    const SAMPLE_ACTIVE: &str = concat!(
        "[2026-10-05T10:00:00+00:00] [INFO] [System] [syncify] Syncify starting...\n",
        "[2026-10-05T10:00:01+00:00] [WARN] [Worker] [syncify_tauri::worker] Slow download {\"speed_kbps\": 12} 200 OK\n",
        "[2026-10-05T10:00:02+00:00] [ERROR] [Tidal] [syncify_tauri::services::tidal] Auth failed {\"error\": 401, \"token\": \"Bearer abcdefghijklmn\"}\n",
    );
    const SAMPLE_ROTATED: &str = concat!(
        "[2026-10-04T09:00:00+00:00] [INFO] [Database] [sqlx] Vacuum finished\n",
        "[2026-10-04T09:05:00+00:00] [INFO] [System] [syncify] Old session message\n",
    );

    fn write_sample_files(dir: &Path) {
        std::fs::write(dir.join("syncify-dev.log"), SAMPLE_ACTIVE).unwrap();
        std::fs::write(dir.join("syncify-dev.20261004T090000.log"), SAMPLE_ROTATED).unwrap();
    }

    #[test]
    fn parses_file_log_layer_lines_and_fields() {
        let (ts, entry) = parse_log_line(
            "[2026-10-05T10:00:01+00:00] [WARN] [Worker] [syncify_tauri::worker] Slow download {\"speed_kbps\": 12} 200 OK",
        )
        .expect("line must parse");

        assert_eq!(ts.to_rfc3339(), "2026-10-05T10:00:01+00:00");
        assert_eq!(entry.level, "warn");
        assert_eq!(entry.module, "Worker");
        // El mensaje puede contener llaves: solo el objeto JSON final es fields.
        assert_eq!(entry.message, "Slow download {\"speed_kbps\": 12} 200 OK");
        assert!(entry.fields.is_none());
        assert!(entry.id.starts_with("hist-"));
    }

    #[test]
    fn splits_trailing_fields_json_from_message() {
        let (_, entry) = parse_log_line(
            "[2026-10-05T10:00:02+00:00] [ERROR] [Tidal] [syncify_tauri::services::tidal] Auth failed {\"error\": 401}",
        )
        .expect("line must parse");
        assert_eq!(entry.message, "Auth failed");
        assert_eq!(
            entry.fields.as_ref().and_then(|f| f.get("error")).cloned(),
            Some(serde_json::json!(401))
        );
    }

    #[test]
    fn rejects_malformed_lines() {
        assert!(parse_log_line("not a log line").is_none());
        assert!(parse_log_line("[garbage] [INFO] no target").is_none());
    }

    #[test]
    fn history_is_paginated_newest_first_and_filtered() {
        let dir = tempfile::tempdir().unwrap();
        write_sample_files(dir.path());

        let filters = HistoryFilters::build(None, None, None, None).unwrap();
        let page = read_history_blocking(dir.path().to_path_buf(), filters, 0, 10).unwrap();
        assert_eq!(page.total, 5);
        assert_eq!(page.entries.len(), 5);
        // Newest first: la entrada del fichero activo de las 10:00:02.
        assert_eq!(page.entries[0].message, "Auth failed");
        // La rotación del 04-10 cierra la página: la entrada más antigua
        // (09:00) es la última y la de las 09:05 va justo antes.
        assert_eq!(page.entries[4].message, "Vacuum finished");
        assert_eq!(page.entries[3].message, "Old session message");

        // Paginación
        let filters = HistoryFilters::build(None, None, None, None).unwrap();
        let page2 = read_history_blocking(dir.path().to_path_buf(), filters, 3, 2).unwrap();
        assert_eq!(page2.entries.len(), 2);
        assert_eq!(page2.total, 5);
        assert_eq!(page2.entries[0].message, "Old session message");

        // Filtro por nivel
        let filters = HistoryFilters::build(None, None, Some("error"), None).unwrap();
        let page = read_history_blocking(dir.path().to_path_buf(), filters, 0, 10).unwrap();
        assert_eq!(page.total, 1);
        assert_eq!(page.entries[0].level, "error");

        // Búsqueda de texto
        let filters = HistoryFilters::build(None, None, None, Some("vacuum")).unwrap();
        let page = read_history_blocking(dir.path().to_path_buf(), filters, 0, 10).unwrap();
        assert_eq!(page.total, 1);
        assert_eq!(page.entries[0].module, "Database");

        // Rango de fechas (solo el 04-10)
        let filters =
            HistoryFilters::build(Some("2026-10-04"), Some("2026-10-04"), None, None).unwrap();
        let page = read_history_blocking(dir.path().to_path_buf(), filters, 0, 10).unwrap();
        assert_eq!(page.total, 2);
    }

    #[test]
    fn export_writes_chronological_text_file() {
        let dir = tempfile::tempdir().unwrap();
        write_sample_files(dir.path());
        let dest = dir.path().join("export").join("history.txt");

        let filters = HistoryFilters::build(None, None, None, None).unwrap();
        let result =
            export_history_blocking(dir.path().to_path_buf(), filters, dest.clone()).unwrap();

        assert_eq!(result.exported_count, 5);
        assert!(dest.exists());
        let content = std::fs::read_to_string(&dest).unwrap();
        let lines: Vec<&str> = content.lines().collect();
        // Cronológico: lo más antiguo (04-10 09:00) primero tras la cabecera.
        assert!(lines[3].contains("2026-10-04T09:00:00+00:00"));
        assert!(content.contains("Syncify Log History Export"));
        assert!(content.contains("Auth failed"));
    }

    #[test]
    fn page_limit_is_clamped_to_valid_bounds() {
        let dir = tempfile::tempdir().unwrap();
        write_sample_files(dir.path());

        // Límite 0: read_history_blocking re-acota al mínimo (página de 1).
        let filters = HistoryFilters::build(None, None, None, None).unwrap();
        let page = read_history_blocking(dir.path().to_path_buf(), filters, 0, 0).unwrap();
        assert_eq!(page.limit, 1);
        assert_eq!(page.entries.len(), 1);

        // Límite por encima del tope: se re-acota a MAX_HISTORY_PAGE_LIMIT.
        let filters = HistoryFilters::build(None, None, None, None).unwrap();
        let page = read_history_blocking(dir.path().to_path_buf(), filters, 0, 10_000).unwrap();
        assert_eq!(page.limit, MAX_HISTORY_PAGE_LIMIT);
    }

    #[test]
    fn range_bounds_accept_dates_and_rfc3339() {
        let from = parse_range_bound(Some("2026-10-04"), false)
            .unwrap()
            .unwrap();
        assert_eq!(
            from.format("%Y-%m-%dT%H:%M:%S").to_string(),
            "2026-10-04T00:00:00"
        );
        let to = parse_range_bound(Some("2026-10-04"), true)
            .unwrap()
            .unwrap();
        assert_eq!(
            to.format("%Y-%m-%dT%H:%M:%S").to_string(),
            "2026-10-04T23:59:59"
        );
        let rfc = parse_range_bound(Some("2026-10-04T09:00:00Z"), false)
            .unwrap()
            .unwrap();
        assert_eq!(rfc.to_rfc3339(), "2026-10-04T09:00:00+00:00");
        assert!(parse_range_bound(Some("nope"), false).is_err());
        assert!(parse_range_bound(None, false).unwrap().is_none());
        assert!(parse_range_bound(Some(""), false).unwrap().is_none());
    }
}

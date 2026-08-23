// System Logging Tauri Commands (S170)

use crate::services::logging::{get_global_log_buffer, SystemLogEntry};

/// Get buffered system logs with optional filtering and pagination
#[tauri::command]
pub async fn get_system_logs(
    limit: Option<usize>,
    level_filter: Option<String>,
    module_filter: Option<String>,
    search: Option<String>,
) -> Result<Vec<SystemLogEntry>, String> {
    let buffer = get_global_log_buffer();
    let logs = buffer.get_logs(
        limit,
        level_filter.as_deref(),
        module_filter.as_deref(),
        search.as_deref(),
    );
    Ok(logs)
}

/// Clear in-memory system log buffer
#[tauri::command]
pub async fn clear_system_logs() -> Result<(), String> {
    let buffer = get_global_log_buffer();
    buffer.clear();
    tracing::info!("System logs cleared by user request");
    Ok(())
}

/// Export system logs as text dump with sanitized secrets
#[tauri::command]
pub async fn export_system_logs() -> Result<String, String> {
    let buffer = get_global_log_buffer();
    Ok(buffer.export_text())
}

/// Record a client-side or system log entry into the backend ring buffer
#[tauri::command]
pub async fn record_system_log(
    level: String,
    target: Option<String>,
    module: Option<String>,
    message: String,
) -> Result<SystemLogEntry, String> {
    let buffer = get_global_log_buffer();
    let t = target.unwrap_or_else(|| "syncify::ui".to_string());
    let m = module.unwrap_or_else(|| "UI".to_string());
    
    let entry = SystemLogEntry {
        id: String::new(),
        timestamp: chrono::Utc::now().to_rfc3339(),
        level: level.to_lowercase(),
        target: t,
        module: m,
        message,
        fields: None,
    };
    
    buffer.push(entry.clone());
    Ok(entry)
}

/// Query system logging status (active level, file logging status, path in dev)
#[tauri::command]
pub async fn get_logging_status() -> Result<crate::services::logging::LoggingStatusDto, String> {
    Ok(crate::services::logging::get_logging_status())
}

#[cfg(test)]
mod logging_commands_tests {
    use super::*;

    #[tokio::test]
    async fn test_get_and_clear_system_logs() {
        let buffer = get_global_log_buffer();
        buffer.clear();

        buffer.log("info", "syncify::test", "Test", "Test message 1");
        buffer.log("error", "syncify::test", "Test", "Error message 2");

        let logs = get_system_logs(Some(10), None, None, None).await.unwrap();
        assert_eq!(logs.len(), 2);

        let filtered = get_system_logs(Some(10), Some("error".to_string()), None, None).await.unwrap();
        assert_eq!(filtered.len(), 1);
        assert_eq!(filtered[0].message, "Error message 2");

        let exported = export_system_logs().await.unwrap();
        assert!(exported.contains("Test message 1"));
        assert!(exported.contains("Error message 2"));

        clear_system_logs().await.unwrap();
        let logs_after_clear = get_system_logs(None, None, None, None).await.unwrap();
        // clear_system_logs itself logs "System logs cleared by user request"
        assert!(logs_after_clear.iter().all(|l| l.message.contains("System logs cleared")));
    }
}

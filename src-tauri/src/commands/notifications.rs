// Notification system commands and push helpers

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum NotificationKind {
    Info,
    Success,
    Warning,
    Error,
    Progress,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum NotificationCategory {
    Download,
    Enrichment,
    Sync,
    System,
    Backup,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppNotification {
    pub id: String,
    pub kind: NotificationKind,
    pub title: String,
    pub message: String,
    pub timestamp: String,
    pub category: NotificationCategory,
    pub metadata: Option<serde_json::Value>,
}

impl AppNotification {
    pub fn new(
        kind: NotificationKind,
        title: impl Into<String>,
        message: impl Into<String>,
        category: NotificationCategory,
        metadata: Option<serde_json::Value>,
    ) -> Self {
        let id = format!(
            "notif_{}_{}",
            chrono::Utc::now().timestamp_millis(),
            &uuid::Uuid::new_v4().simple().to_string()[..8]
        );
        let timestamp = chrono::Utc::now().to_rfc3339();
        Self {
            id,
            kind,
            title: title.into(),
            message: message.into(),
            timestamp,
            category,
            metadata,
        }
    }
}

/// Helper to emit typed notifications to all windows
pub fn emit_app_notification(app: &tauri::AppHandle, notification: &AppNotification) -> Result<(), tauri::Error> {
    app.emit("syncify:notification", notification)
}

/// Tauri command to emit a notification from frontend or for testing
#[tauri::command]
pub async fn emit_test_notification(
    app: tauri::AppHandle,
    kind: String,
    title: String,
    message: String,
    category: String,
    metadata: Option<serde_json::Value>,
) -> Result<AppNotification, String> {
    let kind_enum = match kind.to_lowercase().as_str() {
        "success" => NotificationKind::Success,
        "warning" => NotificationKind::Warning,
        "error" => NotificationKind::Error,
        "progress" => NotificationKind::Progress,
        _ => NotificationKind::Info,
    };

    let cat_enum = match category.to_lowercase().as_str() {
        "download" => NotificationCategory::Download,
        "enrichment" => NotificationCategory::Enrichment,
        "sync" => NotificationCategory::Sync,
        "backup" => NotificationCategory::Backup,
        _ => NotificationCategory::System,
    };

    let notification = AppNotification::new(kind_enum, title, message, cat_enum, metadata);
    let _ = emit_app_notification(&app, &notification);
    Ok(notification)
}

//! User Report Service (FE-8)
//!
//! Persists user-submitted reports (bug reports, general feedback and help
//! article ratings) as JSON files inside a `reports/` folder under the
//! application log directory, following the same directory conventions as
//! [`crate::services::logging`] (`resolve_app_log_dir`). Reports are purely
//! local: nothing is uploaded anywhere; the file path is returned so the UI
//! can show a visible confirmation.

use crate::services::logging::{get_global_log_buffer, resolve_app_log_dir};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::path::Path;

/// Sub-directory of the log dir where reports are stored
pub const REPORTS_DIR_NAME: &str = "reports";

/// Maximum persisted length for user free-text fields (guards against
/// accidental multi-hundred-MB pastes; not a security boundary).
pub const MAX_TEXT_FIELD_LEN: usize = 20_000;
/// Maximum persisted length for short metadata fields (titles, types).
pub const MAX_SHORT_FIELD_LEN: usize = 300;

const KNOWN_KINDS: [&str; 3] = ["bug_report", "feedback", "article_feedback"];

/// Report payload submitted from the Help panel
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct UserReportInput {
    /// One of: "bug_report", "feedback", "article_feedback"
    pub kind: String,
    /// Main free text (bug description / feedback message)
    #[serde(default)]
    pub message: String,
    /// Bug reports only: reproduction steps
    #[serde(default)]
    pub steps_to_reproduce: String,
    /// Feedback only: "Bug Report" | "Feature Request" | "General Feedback"
    #[serde(default)]
    pub feedback_type: String,
    /// Article feedback only: title of the rated article
    #[serde(default)]
    pub article_title: String,
    /// Article feedback only: thumbs up / thumbs down
    #[serde(default)]
    pub helpful: Option<bool>,
    /// Bug reports only: attach the sanitized in-memory system log dump
    #[serde(default)]
    pub attach_logs: bool,
}

/// Confirmation returned to the UI after a report is persisted
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct UserReportSaved {
    pub file_path: String,
    pub created_at: String,
}

fn clamp_text(value: &str, max: usize) -> String {
    value.trim().chars().take(max).collect()
}

/// Persist a report under `dir/reports/` as a timestamped JSON file.
///
/// The `dir` parameter is injected so the behavior is testable with temp
/// directories; [`save_user_report`] resolves the real app log directory.
pub fn save_report_in_dir(dir: &Path, report: &UserReportInput) -> Result<UserReportSaved, String> {
    let kind = report.kind.trim().to_lowercase();
    if !KNOWN_KINDS.contains(&kind.as_str()) {
        return Err(format!(
            "Unknown report kind '{}'; expected one of: {}",
            kind,
            KNOWN_KINDS.join(", ")
        ));
    }

    let message = clamp_text(&report.message, MAX_TEXT_FIELD_LEN);
    // A bug report / feedback without any text is not useful: reject it so the
    // UI can ask the user for content instead of persisting an empty file.
    if (kind == "bug_report" || kind == "feedback") && message.is_empty() {
        return Err("Report message must not be empty".to_string());
    }

    let reports_dir = dir.join(REPORTS_DIR_NAME);
    std::fs::create_dir_all(&reports_dir).map_err(|e| {
        format!(
            "Failed to create reports directory {:?}: {}",
            reports_dir, e
        )
    })?;

    let created_at = Utc::now().to_rfc3339();

    let mut payload = serde_json::json!({
        "kind": kind,
        "message": message,
        "steps_to_reproduce": clamp_text(&report.steps_to_reproduce, MAX_TEXT_FIELD_LEN),
        "feedback_type": clamp_text(&report.feedback_type, MAX_SHORT_FIELD_LEN),
        "article_title": clamp_text(&report.article_title, MAX_SHORT_FIELD_LEN),
        "helpful": report.helpful,
        "attach_logs": report.attach_logs,
        "created_at": created_at,
    });

    if report.attach_logs {
        // The export is already secret-sanitized by the logging service.
        payload["system_logs"] = serde_json::Value::String(get_global_log_buffer().export_text());
    }

    let file_stem = format!(
        "user-report-{}-{}-{}",
        kind,
        Utc::now().format("%Y%m%dT%H%M%S%3f"),
        uuid::Uuid::new_v4().simple()
    );
    let file_path = reports_dir.join(format!("{}.json", file_stem));
    let contents = serde_json::to_string_pretty(&payload)
        .map_err(|e| format!("Failed to serialize user report: {}", e))?;
    std::fs::write(&file_path, contents)
        .map_err(|e| format!("Failed to write user report {:?}: {}", file_path, e))?;

    tracing::info!("User report '{}' saved to {:?}", kind, file_path);

    Ok(UserReportSaved {
        file_path: file_path.to_string_lossy().to_string(),
        created_at,
    })
}

/// Persist a report in the application log directory (see
/// [`crate::services::logging::resolve_app_log_dir`]).
pub fn save_user_report(report: UserReportInput) -> Result<UserReportSaved, String> {
    save_report_in_dir(&resolve_app_log_dir(), &report)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_reports_root() -> std::path::PathBuf {
        std::env::temp_dir().join(format!("syncify_reports_test_{}", uuid::Uuid::new_v4()))
    }

    fn bug_report() -> UserReportInput {
        UserReportInput {
            kind: "bug_report".to_string(),
            message: "App crashes when syncing favorites".to_string(),
            steps_to_reproduce: "1. Open favorites\n2. Sync".to_string(),
            feedback_type: String::new(),
            article_title: String::new(),
            helpful: None,
            attach_logs: false,
        }
    }

    #[test]
    fn test_saves_bug_report_as_json_file() {
        let root = temp_reports_root();
        let saved = save_report_in_dir(&root, &bug_report()).expect("save must succeed");

        let contents = std::fs::read_to_string(&saved.file_path).expect("file must exist");
        let parsed: serde_json::Value = serde_json::from_str(&contents).expect("valid JSON");
        assert_eq!(parsed["kind"], "bug_report");
        assert_eq!(parsed["message"], "App crashes when syncing favorites");
        assert_eq!(parsed["steps_to_reproduce"], "1. Open favorites\n2. Sync");
        assert_eq!(parsed["attach_logs"], false);
        assert!(parsed["created_at"].as_str().is_some());
        // No log attachment requested → no logs persisted
        assert!(parsed.get("system_logs").is_none());
        // Stored under the reports sub-directory of the given root
        assert!(saved.file_path.contains(REPORTS_DIR_NAME));

        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn test_attach_logs_includes_sanitized_export() {
        let root = temp_reports_root();
        get_global_log_buffer().log(
            "info",
            "syncify::user_reports_test",
            "Test",
            "REPORT_MARKER_unique_log_line_12345",
        );

        let mut report = bug_report();
        report.attach_logs = true;
        let saved = save_report_in_dir(&root, &report).expect("save must succeed");

        let contents = std::fs::read_to_string(&saved.file_path).expect("file must exist");
        assert!(contents.contains("REPORT_MARKER_unique_log_line_12345"));

        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn test_rejects_unknown_kind_and_empty_message() {
        let root = temp_reports_root();

        let mut report = bug_report();
        report.kind = "wall_of_shame".to_string();
        assert!(save_report_in_dir(&root, &report).is_err());

        let mut report = bug_report();
        report.message = "   ".to_string();
        assert!(save_report_in_dir(&root, &report).is_err());

        // No file must have been created for rejected reports
        let reports_dir = root.join(REPORTS_DIR_NAME);
        if reports_dir.exists() {
            assert_eq!(std::fs::read_dir(&reports_dir).unwrap().count(), 0);
        }

        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn test_article_feedback_allows_empty_message_with_rating() {
        let root = temp_reports_root();
        let report = UserReportInput {
            kind: "article_feedback".to_string(),
            message: String::new(),
            steps_to_reproduce: String::new(),
            feedback_type: String::new(),
            article_title: "Managing playlists".to_string(),
            helpful: Some(false),
            attach_logs: false,
        };

        let saved = save_report_in_dir(&root, &report).expect("article rating must save");
        let contents = std::fs::read_to_string(&saved.file_path).expect("file must exist");
        let parsed: serde_json::Value = serde_json::from_str(&contents).expect("valid JSON");
        assert_eq!(parsed["article_title"], "Managing playlists");
        assert_eq!(parsed["helpful"], false);

        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn test_text_fields_are_trimmed_and_clamped() {
        let root = temp_reports_root();
        let mut report = bug_report();
        report.message = format!("  {}  ", "x".repeat(MAX_TEXT_FIELD_LEN + 500));
        report.article_title = "  ".repeat(0) + &"t".repeat(MAX_SHORT_FIELD_LEN + 100);

        let saved = save_report_in_dir(&root, &report).expect("save must succeed");
        let contents = std::fs::read_to_string(&saved.file_path).expect("file must exist");
        let parsed: serde_json::Value = serde_json::from_str(&contents).expect("valid JSON");
        assert_eq!(
            parsed["message"].as_str().unwrap().len(),
            MAX_TEXT_FIELD_LEN
        );
        assert_eq!(
            parsed["article_title"].as_str().unwrap().len(),
            MAX_SHORT_FIELD_LEN
        );

        let _ = std::fs::remove_dir_all(&root);
    }
}

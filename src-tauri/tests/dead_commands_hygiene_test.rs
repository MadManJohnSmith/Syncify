//! dead_commands_hygiene_test.rs
//!
//! Regression test for [TASK-119]: "Cablear o Purgar Comandos Backend Muertos (detail endpoints y utilitarios)".
//!
//! Asserts that:
//! 1. Dead/inert commands have been removed from `generate_handler!`:
//!    - `get_album_detail`, `get_artist_detail`, `get_album_tracks`, `get_artist_albums`, `get_artist_tracks`
//!    - `list_playlists`, `toggle_track_favorite`, `organize_files`, `preview_organization`, `convert_audio`, `get_audio_info`
//! 2. No duplicate queue or handler registrations exist in `generate_handler!`.
//! 3. Canonical commands remain registered and intact:
//!    - `get_album`, `get_artist`, `toggle_favorite`, `retry_failed`, `retry_all_failed`, `clear_completed`.
//! 4. Notification pipeline types and deduplication logic are active and functional.

#[test]
fn test_generate_handler_does_not_contain_dead_commands() {
    let main_rs = include_str!("../src/main.rs");

    // Extract the generate_handler! invocation block
    let handler_start = main_rs
        .find("tauri::generate_handler![")
        .expect("tauri::generate_handler! must exist in main.rs");
    let handler_end = main_rs[handler_start..]
        .find("])")
        .expect("Closing delimiter for generate_handler! must exist");
    let handler_block = &main_rs[handler_start..handler_start + handler_end];

    let dead_commands = [
        "commands::get_album_detail",
        "commands::get_artist_detail",
        "commands::get_album_tracks",
        "commands::get_artist_albums",
        "commands::get_artist_tracks",
        "commands::list_playlists",
        "commands::toggle_track_favorite",
        "commands::organize_files",
        "commands::preview_organization",
        "commands::convert_audio",
        "commands::get_audio_info",
    ];

    for cmd in &dead_commands {
        assert!(
            !handler_block.contains(cmd),
            "generate_handler! must not contain dead command: {}",
            cmd
        );
    }
}

#[test]
fn test_generate_handler_retains_canonical_commands() {
    let main_rs = include_str!("../src/main.rs");

    let handler_start = main_rs
        .find("tauri::generate_handler![")
        .expect("tauri::generate_handler! must exist in main.rs");
    let handler_end = main_rs[handler_start..]
        .find("])")
        .expect("Closing delimiter for generate_handler! must exist");
    let handler_block = &main_rs[handler_start..handler_start + handler_end];

    let canonical_commands = [
        "commands::get_album",
        "commands::get_artist",
        "commands::toggle_favorite",
        "commands::retry_failed",
        "commands::retry_all_failed",
        "commands::clear_completed",
        "commands::download_tidal_single_track",
    ];

    for cmd in &canonical_commands {
        assert!(
            handler_block.contains(cmd),
            "generate_handler! must retain canonical command: {}",
            cmd
        );
    }
}

#[test]
fn test_generate_handler_has_no_duplicate_registrations() {
    let main_rs = include_str!("../src/main.rs");

    let handler_start = main_rs
        .find("tauri::generate_handler![")
        .expect("tauri::generate_handler! must exist in main.rs");
    let handler_end = main_rs[handler_start..]
        .find("])")
        .expect("Closing delimiter for generate_handler! must exist");
    let handler_block = &main_rs[handler_start..handler_start + handler_end];

    let mut seen = std::collections::HashSet::new();
    let mut duplicates = Vec::new();

    for line in handler_block.lines() {
        let trimmed = line.trim().trim_end_matches(',');
        if trimmed.starts_with("commands::") || trimmed.starts_with("tray::") {
            if !seen.insert(trimmed.to_string()) {
                duplicates.push(trimmed.to_string());
            }
        }
    }

    assert!(
        duplicates.is_empty(),
        "generate_handler! contains duplicate command registrations: {:?}",
        duplicates
    );
}

#[tokio::test]
async fn test_notification_deduplication_and_payload_contract() {
    use syncify_tauri_lib::services::notification::{
        clear_notification_cache, create_service_notification, should_emit_notification,
    };

    clear_notification_cache();

    let notif1 = create_service_notification(
        "tidal",
        None,
        "download",
        "completed",
        "info",
        "Download finished: Track A",
    );
    assert_eq!(notif1.service, "tidal");
    assert_eq!(notif1.severity, "info");
    assert_eq!(notif1.operation, "download");
    assert_eq!(notif1.kind, "completed");
    assert_eq!(notif1.message, "Download finished: Track A");
    assert!(!notif1.occurred_at.is_empty());

    // First emission should be allowed
    assert!(should_emit_notification(&notif1));

    // Duplicate within window should be suppressed
    assert!(!should_emit_notification(&notif1));

    // Different message should be allowed
    let notif2 = create_service_notification(
        "tidal",
        None,
        "download",
        "completed",
        "info",
        "Download finished: Track B",
    );
    assert!(should_emit_notification(&notif2));

    // Different kind should be allowed
    let notif3 = create_service_notification(
        "tidal",
        None,
        "download",
        "network",
        "error",
        "Download finished: Track A",
    );
    assert!(should_emit_notification(&notif3));

    // After clearing cache, same notification can be emitted again
    clear_notification_cache();
    assert!(should_emit_notification(&notif1));
}

//! dead_commands_hygiene_test.rs
//!
//! Regression test for [TASK-119] (unregister dead commands) and the post-audit
//! triage items [BE-3]/[IN-5] (purge dead command source and prune registered
//! commands without any caller).
//!
//! Freezes the NEW state:
//! 1. Zero dead commands: no `#[tauri::command]` may carry `#[allow(dead_code)]`
//!    (the 11 commands of BE-3 — get_album_detail, get_artist_detail,
//!    get_album_tracks, get_artist_albums, get_artist_tracks, list_playlists,
//!    toggle_track_favorite, organize_files, preview_organization, convert_audio,
//!    get_audio_info — were removed from the source, not just unregistered).
//! 2. The commands removed by the IN-5 triage (aliases, duplicates and inert
//!    wrappers) are gone from both the source and `generate_handler!`:
//!    download_track, batch_download_tracks, fetch_lyrics, get_artist_appearances,
//!    merge_level2_3_duplicates, get_effective_download_paths, get_sidecar_settings,
//!    update_sidecar_settings, reset_download_history, enrich_before_download,
//!    update_tray_icon_command, retry_all_failed.
//! 3. Every command registered in `generate_handler!` is reachable from the
//!    frontend: the UI production sources invoke it by name (the IN-5 end state —
//!    zero registered commands without a caller).
//! 4. No duplicate queue or handler registrations exist in `generate_handler!`
//!    and every registered command resolves to a declared function.
//! 5. Canonical commands remain registered and intact:
//!    - `get_album`, `get_artist`, `toggle_favorite`, `retry_failed`, `clear_completed`.
//! 6. The `sync_playlist` phantom command (invoked by the removed
//!    `syncPlaylist` UI wrapper) does not exist anywhere in the backend.
//! 7. Notification pipeline types and deduplication logic are active and functional.

use std::collections::HashSet;
use std::fs;
use std::path::Path;

/// Matches `pub fn name(`, `pub async fn name(` and generic variants `pub async fn name<R: Runtime>(`.
fn declares_function(backend: &str, fn_name: &str) -> bool {
    backend.lines().any(|l| {
        let t = l.trim();
        if !(t.starts_with("pub async fn ") || t.starts_with("pub fn ")) {
            return false;
        }
        let rest = &t[t.find("fn ").unwrap() + 3..];
        rest.starts_with(&format!("{}(", fn_name)) || rest.starts_with(&format!("{}<", fn_name))
    })
}

/// Reads a single file relative to the crate root.
fn read_crate_source(rel: &str) -> String {
    let manifest_dir = env!("CARGO_MANIFEST_DIR");
    let path = Path::new(manifest_dir).join(rel);
    fs::read_to_string(&path).unwrap_or_else(|e| panic!("failed to read {}: {}", path.display(), e))
}

/// Concatenates every `.rs` file under a directory tree (relative to the crate root).
fn read_crate_source_tree(rel: &str) -> String {
    let mut buffer = String::new();
    collect_source_tree(
        &Path::new(env!("CARGO_MANIFEST_DIR")).join(rel),
        &mut buffer,
    );
    buffer
}

fn collect_source_tree(root: &Path, buffer: &mut String) {
    let entries = match fs::read_dir(root) {
        Ok(entries) => entries,
        Err(_) => return,
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect_source_tree(&path, buffer);
        } else if path.extension().and_then(|e| e.to_str()) == Some("rs") {
            if let Ok(source) = fs::read_to_string(&path) {
                buffer.push_str(&source);
                buffer.push('\n');
            }
        }
    }
}

/// Scans every `.rs` file under a directory for `#[tauri::command]` functions
/// annotated with `#[allow(dead_code)]`.
fn find_dead_tauri_commands(root: &Path, found: &mut Vec<String>) {
    let entries = match fs::read_dir(root) {
        Ok(entries) => entries,
        Err(_) => return,
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            find_dead_tauri_commands(&path, found);
        } else if path.extension().and_then(|e| e.to_str()) == Some("rs") {
            let source = match fs::read_to_string(&path) {
                Ok(s) => s,
                Err(_) => continue,
            };
            let mut remaining = source.as_str();
            while let Some(idx) = remaining.find("#[tauri::command]") {
                let after = &remaining[idx + "#[tauri::command]".len()..];
                let trimmed = after.trim_start();
                if trimmed.starts_with("#[allow(dead_code") {
                    let fn_name = trimmed
                        .split("fn ")
                        .nth(1)
                        .and_then(|rest| {
                            rest.chars()
                                .take_while(|c| c.is_alphanumeric() || *c == '_')
                                .collect::<String>()
                                .split_whitespace()
                                .next()
                                .map(|s| s.to_string())
                        })
                        .unwrap_or_else(|| "<unknown>".to_string());
                    found.push(format!("{}: {}", path.display(), fn_name));
                }
                remaining = after;
            }
        }
    }
}

#[test]
fn test_no_tauri_command_is_marked_dead_code() {
    let commands_dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("src/commands");
    let mut dead = Vec::new();
    find_dead_tauri_commands(&commands_dir, &mut dead);
    // tray.rs and any other src/*.rs module are covered too.
    find_dead_tauri_commands(
        &Path::new(env!("CARGO_MANIFEST_DIR")).join("src"),
        &mut dead,
    );

    assert!(
        dead.is_empty(),
        "zero dead commands expected, found #[tauri::command] + #[allow(dead_code)] at: {:?}",
        dead
    );
}

#[test]
fn test_purged_in5_commands_are_gone_from_source_and_handler() {
    // Commands removed by the IN-5 triage: alias/duplicate/inert wrappers.
    let purged = [
        "download_track",
        "batch_download_tracks",
        "fetch_lyrics",
        "get_artist_appearances",
        "merge_level2_3_duplicates",
        "get_effective_download_paths",
        "get_sidecar_settings",
        "update_sidecar_settings",
        "reset_download_history",
        "enrich_before_download",
        "update_tray_icon_command",
        // Exact duplicate of `retry_failed(None)` — both call `perform_retry_all_failed`.
        "retry_all_failed",
    ];

    let handler_block = extract_handler_block();
    let commands_src = read_crate_source_tree("src/commands");
    let tray_src = read_crate_source("src/tray.rs");
    let backend = format!("{}{}", commands_src, tray_src);

    for cmd in &purged {
        assert!(
            !handler_block.contains(&format!("::{}", cmd)),
            "generate_handler! must not register purged command: {}",
            cmd
        );
        // The command definition must be gone too: a `#[tauri::command]` fn with
        // that exact name must no longer exist in the backend source.
        for backend_chunk in [commands_src.as_str(), tray_src.as_str()] {
            if declares_function(backend_chunk, cmd) {
                panic!("purged command {} still defined in backend source", cmd);
            }
        }
    }
    // Silence unused warning for `backend` when assertions above already used chunks.
    let _ = backend;
}

#[test]
fn test_sync_playlist_phantom_command_does_not_exist() {
    // FE-11: the UI wrapper invoking `sync_playlist` was removed; the backend
    // must not define such a command either.
    let backend = format!(
        "{}{}",
        read_crate_source_tree("src/commands"),
        read_crate_source("src/main.rs")
    );
    assert!(
        !declares_function(&backend, "sync_playlist"),
        "phantom command sync_playlist must not exist in the backend"
    );
}

fn extract_handler_block() -> String {
    let main_rs = read_crate_source("src/main.rs");
    let handler_start = main_rs
        .find("tauri::generate_handler![")
        .expect("tauri::generate_handler! must exist in main.rs");
    let handler_end = main_rs[handler_start..]
        .find("])")
        .expect("Closing delimiter for generate_handler! must exist");
    main_rs[handler_start..handler_start + handler_end].to_string()
}

/// Every `commands::name` / `tray::name` entry listed in `generate_handler!`.
fn extract_registered_commands() -> Vec<String> {
    extract_handler_block()
        .lines()
        .map(str::trim)
        .filter_map(|line| {
            line.trim_end_matches(',')
                .strip_prefix("commands::")
                .or_else(|| line.trim_end_matches(',').strip_prefix("tray::"))
                .map(str::to_string)
        })
        .collect()
}

fn is_ident_byte(b: u8) -> bool {
    b.is_ascii_alphanumeric() || b == b'_'
}

fn is_command_literal(literal: &str) -> bool {
    let mut chars = literal.chars();
    matches!(chars.next(), Some(c) if c.is_ascii_lowercase())
        && chars.all(|c| c.is_ascii_alphanumeric() || c == '_')
}

/// Collects the command names the frontend passes as first string argument to
/// `invoke(...)` / `invokeCommand(...)`, mirroring the `invokeCommand` helper in
/// `ui/src/api/tauri.ts`. Generic arguments (`invokeCommand<Record<string,
/// string>>(...)`) are skipped with a nesting-aware scan.
fn collect_invoke_command_names(source: &str, out: &mut HashSet<String>) {
    let bytes = source.as_bytes();
    let mut cursor = 0usize;

    while cursor < bytes.len() {
        let Some(rel) = source[cursor..].find("invoke") else {
            break;
        };
        let start = cursor + rel;
        let mut i = start + "invoke".len();
        cursor = i;
        if start > 0 && is_ident_byte(bytes[start - 1]) {
            continue; // part of a longer identifier (e.g. `myInvoke`)
        }
        while i < bytes.len() && is_ident_byte(bytes[i]) {
            i += 1; // `invokeCommand` -> `invoke`
        }
        let mut j = i;
        while j < bytes.len() && bytes[j].is_ascii_whitespace() {
            j += 1;
        }
        if j >= bytes.len() || (bytes[j] != b'(' && bytes[j] != b'<') {
            continue;
        }
        let mut pos = j;
        if bytes[pos] == b'<' {
            let mut depth = 0usize;
            while pos < bytes.len() {
                match bytes[pos] {
                    b'<' => depth += 1,
                    b'>' => {
                        depth -= 1;
                        if depth == 0 {
                            pos += 1;
                            break;
                        }
                    }
                    _ => {}
                }
                pos += 1;
            }
            while pos < bytes.len() && bytes[pos].is_ascii_whitespace() {
                pos += 1;
            }
            if pos >= bytes.len() || bytes[pos] != b'(' {
                continue;
            }
        }
        pos += 1; // step past the opening paren of the call

        let mut depth = 0i32;
        let mut is_first_argument = true;
        while pos < bytes.len() {
            match bytes[pos] {
                b'(' | b'[' | b'{' => depth += 1,
                b')' | b']' | b'}' => {
                    if depth == 0 {
                        break; // end of the invoke() call
                    }
                    depth -= 1;
                }
                b'\'' | b'"' if depth == 0 && is_first_argument => {
                    let quote = bytes[pos];
                    let literal_start = pos + 1;
                    let mut end = literal_start;
                    while end < bytes.len() && bytes[end] != quote {
                        end += 1;
                    }
                    if end >= bytes.len() {
                        break;
                    }
                    let literal = &source[literal_start..end];
                    if is_command_literal(literal) {
                        out.insert(literal.to_string());
                    }
                    is_first_argument = false;
                    pos = end + 1;
                    continue;
                }
                _ => {}
            }
            pos += 1;
        }
    }
}

/// Command names invoked by the frontend production sources (`__tests__` excluded:
/// a mock is not a caller).
fn ui_invoked_commands() -> HashSet<String> {
    let ui_src = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("repo root")
        .join("ui")
        .join("src");

    let mut names = HashSet::new();
    if !ui_src.exists() {
        return names;
    }
    for entry in walkdir::WalkDir::new(&ui_src)
        .into_iter()
        .filter_entry(|e| e.path().file_name().is_none_or(|n| n != "__tests__"))
        .filter_map(|e| e.ok())
        .filter(|e| {
            e.path().is_file()
                && e.path().extension().is_some_and(|ext| {
                    matches!(ext.to_str(), Some("ts" | "tsx" | "js" | "jsx" | "vue"))
                })
        })
    {
        if let Ok(source) = fs::read_to_string(entry.path()) {
            collect_invoke_command_names(&source, &mut names);
        }
    }
    names
}

/// IN-5 end state: a command registered in `generate_handler!` that nothing in
/// the frontend invokes is dead IPC surface — it can only ever fail at runtime.
#[test]
fn test_every_registered_command_is_invoked_by_the_frontend() {
    let invoked = ui_invoked_commands();
    assert!(
        !invoked.is_empty(),
        "no frontend invocation could be parsed from ui/src; the scan is broken, not the app"
    );
    // Sanity anchors: if these are missing the parser stopped understanding the
    // frontend call style and the orphan assertion below would pass vacuously.
    for anchor in [
        "get_album",       // plain `invokeCommand('x')`
        "get_kv_settings", // `invokeCommand<Record<string, string>>('x')`
        "read_track_tags", // multi-line payload
    ] {
        assert!(
            invoked.contains(anchor),
            "ui_invoked_commands() must resolve {}, otherwise the scan is broken",
            anchor
        );
    }

    let orphans: Vec<String> = extract_registered_commands()
        .into_iter()
        .filter(|cmd| !invoked.contains(cmd))
        .collect();

    assert!(
        orphans.is_empty(),
        "every command in generate_handler! must be invoked from ui/src (IN-5); orphans: {:?}",
        orphans
    );
}

#[test]
fn test_generate_handler_retains_canonical_commands() {
    let handler_block = extract_handler_block();

    let canonical_commands = [
        "commands::get_album",
        "commands::get_artist",
        "commands::toggle_favorite",
        "commands::retry_failed",
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
    let handler_block = extract_handler_block();

    let mut seen = HashSet::new();
    let mut duplicates = Vec::new();

    for line in handler_block.lines() {
        let trimmed = line.trim().trim_end_matches(',');
        if (trimmed.starts_with("commands::") || trimmed.starts_with("tray::"))
            && !seen.insert(trimmed.to_string())
        {
            duplicates.push(trimmed.to_string());
        }
    }

    assert!(
        duplicates.is_empty(),
        "generate_handler! contains duplicate command registrations: {:?}",
        duplicates
    );
}

#[test]
fn test_every_registered_command_is_declared_in_backend_source() {
    // Freeze: no registered command may point to a function that no longer
    // exists (the BE-3/TASK-119 half-registration failure mode).
    let handler_block = extract_handler_block();
    let backend = format!(
        "{}{}",
        read_crate_source_tree("src/commands"),
        read_crate_source("src/tray.rs")
    );

    let mut missing = Vec::new();
    for line in handler_block.lines() {
        let trimmed = line.trim().trim_end_matches(',');
        if !(trimmed.starts_with("commands::") || trimmed.starts_with("tray::")) {
            continue;
        }
        let fn_name = trimmed.rsplit("::").next().unwrap_or_default();
        if fn_name.is_empty() {
            continue;
        }
        if !declares_function(&backend, fn_name) {
            missing.push(trimmed.to_string());
        }
    }

    assert!(
        missing.is_empty(),
        "generate_handler! registers commands that are not declared in the backend: {:?}",
        missing
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

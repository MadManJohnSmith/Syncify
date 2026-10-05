//! Regression suite for the startup preferences that Settings > General exposes.
//!
//! `start_on_boot` used to be persisted and then never applied to the operating
//! system, so the toggle was a lie; these tests pin the artifact generators and
//! the executable resolution that make it real.

use std::path::PathBuf;
use syncify_tauri_lib::tray::{
    linux_autostart_desktop_entry, macos_autostart_plist, pick_autostart_exe,
    windows_autostart_command, LINUX_AUTOSTART_FILE, MACOS_AUTOSTART_FILE,
    WINDOWS_AUTOSTART_RUN_KEY, WINDOWS_AUTOSTART_VALUE,
};

#[test]
fn linux_autostart_entry_points_at_the_executable() {
    let entry = linux_autostart_desktop_entry(std::path::Path::new("/opt/syncify/syncify"));

    assert!(entry.starts_with("[Desktop Entry]\n"));
    assert!(entry.contains("Type=Application"));
    assert!(entry.contains("Exec=/opt/syncify/syncify"));
    assert!(entry.contains("Terminal=false"));
    // Autostart managers skip the entry entirely when this key is absent/false.
    assert!(entry.contains("X-GNOME-Autostart-enabled=true"));
}

#[test]
fn linux_autostart_entry_keeps_spaces_in_the_path() {
    let entry =
        linux_autostart_desktop_entry(std::path::Path::new("/home/user/My Apps/Syncify/syncify"));

    assert!(entry.contains("Exec=/home/user/My Apps/Syncify/syncify"));
}

#[test]
fn macos_autostart_plist_is_a_valid_launch_agent() {
    let plist = macos_autostart_plist(std::path::Path::new("/Applications/Syncify.app"));

    assert!(plist.contains("<plist version=\"1.0\">"));
    assert!(plist.contains("<key>Label</key>"));
    assert!(plist.contains("<string>com.syncify.app</string>"));
    assert!(plist.contains("<string>/Applications/Syncify.app</string>"));
    assert!(plist.contains("<key>RunAtLoad</key>"));
    assert!(plist.contains("<true/>"));
    assert!(plist.trim_end().ends_with("</plist>"));
}

#[test]
fn windows_autostart_command_quotes_the_install_path() {
    let command = windows_autostart_command(std::path::Path::new(
        r"C:\Program Files\Syncify\syncify.exe",
    ));

    assert_eq!(
        command, r#""C:\Program Files\Syncify\syncify.exe""#,
        "a Run value with spaces must stay quoted or the entry silently fails"
    );
}

#[test]
fn windows_autostart_command_does_not_double_quote() {
    let command = windows_autostart_command(std::path::Path::new(r#""C:\Syncify\syncify.exe""#));

    assert_eq!(command, r#""C:\Syncify\syncify.exe""#);
}

#[test]
fn autostart_targets_are_the_ones_each_os_actually_reads() {
    // Renaming any of these orphans the entries written by an earlier version.
    assert_eq!(LINUX_AUTOSTART_FILE, "syncify.desktop");
    assert_eq!(MACOS_AUTOSTART_FILE, "com.syncify.app.plist");
    assert_eq!(WINDOWS_AUTOSTART_VALUE, "Syncify");
    assert!(WINDOWS_AUTOSTART_RUN_KEY.contains("CurrentVersion\\Run"));
}

#[test]
fn appimage_path_wins_over_the_ephemeral_mount_point() {
    let mounted = Some(PathBuf::from(
        "/tmp/.mount_Syncifysomething/usr/bin/syncify",
    ));
    let appimage = PathBuf::from("/home/user/apps/Syncify.AppImage");

    let chosen = pick_autostart_exe(Some(appimage.clone()), mounted).expect("a path must resolve");

    assert_eq!(
        chosen, appimage,
        "current_exe() inside an AppImage points at a mount that no longer exists at login"
    );
}

#[test]
fn executable_resolution_falls_back_to_current_exe() {
    let chosen = pick_autostart_exe(None, Some(PathBuf::from("/usr/bin/syncify")))
        .expect("current_exe must be used when APPIMAGE is absent");

    assert_eq!(chosen, PathBuf::from("/usr/bin/syncify"));
}

#[test]
fn executable_resolution_reports_when_nothing_resolves() {
    assert!(
        pick_autostart_exe(None, None).is_err(),
        "a silent Ok would let update_tray_settings claim success while registering nothing"
    );
}

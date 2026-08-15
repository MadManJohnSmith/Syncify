//! Tests for real-time push notifications system
use serde_json::json;
use syncify_tauri_lib::commands::{
    AppNotification, NotificationCategory, NotificationKind,
};

#[test]
fn test_notification_creation_and_fields() {
    let notif = AppNotification::new(
        NotificationKind::Success,
        "Download Complete",
        "Pink Floyd - Comfortably Numb",
        NotificationCategory::Download,
        Some(json!({ "queue_id": 42, "track_id": 101 })),
    );

    assert!(notif.id.starts_with("notif_"));
    assert_eq!(notif.kind, NotificationKind::Success);
    assert_eq!(notif.title, "Download Complete");
    assert_eq!(notif.message, "Pink Floyd - Comfortably Numb");
    assert_eq!(notif.category, NotificationCategory::Download);
    assert!(!notif.timestamp.is_empty());
    assert!(notif.metadata.is_some());
}

#[test]
fn test_notification_serialization_roundtrip() {
    let notif = AppNotification::new(
        NotificationKind::Warning,
        "Rate Limit Warning",
        "MusicBrainz quota reached, throttling requests",
        NotificationCategory::Enrichment,
        None,
    );

    let json_str = serde_json::to_string(&notif).expect("Must serialize to JSON");
    assert!(json_str.contains("\"kind\":\"warning\""));
    assert!(json_str.contains("\"category\":\"enrichment\""));

    let deserialized: AppNotification =
        serde_json::from_str(&json_str).expect("Must deserialize from JSON");
    assert_eq!(deserialized.id, notif.id);
    assert_eq!(deserialized.kind, NotificationKind::Warning);
    assert_eq!(deserialized.category, NotificationCategory::Enrichment);
    assert_eq!(deserialized.title, "Rate Limit Warning");
}

#[test]
fn test_notification_categories_and_kinds_coverage() {
    let kinds = vec![
        (NotificationKind::Info, "info"),
        (NotificationKind::Success, "success"),
        (NotificationKind::Warning, "warning"),
        (NotificationKind::Error, "error"),
        (NotificationKind::Progress, "progress"),
    ];

    for (kind, expected_str) in kinds {
        let serialized = serde_json::to_string(&kind).unwrap();
        assert_eq!(serialized, format!("\"{}\"", expected_str));
    }

    let categories = vec![
        (NotificationCategory::Download, "download"),
        (NotificationCategory::Enrichment, "enrichment"),
        (NotificationCategory::Sync, "sync"),
        (NotificationCategory::System, "system"),
        (NotificationCategory::Backup, "backup"),
    ];

    for (category, expected_str) in categories {
        let serialized = serde_json::to_string(&category).unwrap();
        assert_eq!(serialized, format!("\"{}\"", expected_str));
    }
}

#[test]
fn test_notification_metadata_payload() {
    let meta = json!({
        "track_id": 99,
        "service": "tidal",
        "bit_depth": 24,
        "sample_rate": 96000
    });

    let notif = AppNotification::new(
        NotificationKind::Info,
        "Sync Event",
        "Playlist synced successfully",
        NotificationCategory::Sync,
        Some(meta.clone()),
    );

    assert_eq!(notif.metadata.unwrap()["bit_depth"], 24);
}

#[test]
fn test_notification_unique_identifiers() {
    let notif1 = AppNotification::new(
        NotificationKind::Info,
        "Test 1",
        "Message 1",
        NotificationCategory::System,
        None,
    );
    let notif2 = AppNotification::new(
        NotificationKind::Info,
        "Test 2",
        "Message 2",
        NotificationCategory::System,
        None,
    );

    assert_ne!(notif1.id, notif2.id, "Notification IDs must be globally unique");
}

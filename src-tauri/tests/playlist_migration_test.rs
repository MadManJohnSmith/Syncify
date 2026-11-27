//! M08 (#173): reconstrucción de playlists en el destino.
//!
//! Las pruebas usan el proveedor falso del gateway de PL06 (#159): no abren
//! sockets ni consultan servicios reales.

use syncify_tauri_lib::services::playlist_migration::{
    mirror_playlist, ExistingPlaylist, PlaylistMirrorOptions, PlaylistMirrorRequest,
    PlaylistMirrorStatus, PlaylistMirrorWarning,
};
use syncify_tauri_lib::sync::playlist_write::{
    FakePlaylistWriteProvider, PlaylistWriteCapabilities, PlaylistWriteGateway,
    PlaylistWriteOperation, PlaylistWriteOutcome, PlaylistWriteRequest,
};

fn mirror_request(name: &str, track_ids: &[&str]) -> PlaylistMirrorRequest {
    PlaylistMirrorRequest {
        name: name.to_string(),
        description: String::new(),
        track_ids: track_ids.iter().map(|id| id.to_string()).collect(),
    }
}

async fn seed_playlist(provider: &FakePlaylistWriteProvider, name: &str) -> String {
    let gateway = PlaylistWriteGateway::new(provider);
    match gateway
        .execute(PlaylistWriteRequest::Create {
            name: name.to_string(),
            description: String::new(),
        })
        .await
        .expect("seed playlist")
    {
        PlaylistWriteOutcome::Created { playlist_id } => playlist_id,
        other => panic!("unexpected seed outcome: {other:?}"),
    }
}

#[tokio::test]
async fn a_new_playlist_is_created_with_its_tracks_in_order() {
    let provider = FakePlaylistWriteProvider::new();
    let gateway = PlaylistWriteGateway::new(&provider);

    let report = mirror_playlist(
        &gateway,
        &mirror_request("Morning", &["101", "102", "103"]),
        &[],
        PlaylistMirrorOptions::default(),
    )
    .await;

    assert_eq!(report.status, PlaylistMirrorStatus::Created);
    assert_eq!(report.requested_tracks, 3);
    assert_eq!(report.added_tracks, 3);
    assert!(report.error.is_none());

    let playlist = provider
        .playlist(report.playlist_id.as_deref().expect("created id"))
        .expect("created playlist");
    assert_eq!(
        playlist
            .items
            .iter()
            .map(|item| item.track_id.as_str())
            .collect::<Vec<_>>(),
        vec!["101", "102", "103"]
    );
    assert_eq!(
        provider
            .calls()
            .iter()
            .map(PlaylistWriteRequest::operation)
            .collect::<Vec<_>>(),
        vec![
            PlaylistWriteOperation::Create,
            PlaylistWriteOperation::AddTracks,
        ]
    );
}

#[tokio::test]
async fn merging_by_name_does_not_duplicate_the_destination_list() {
    let provider = FakePlaylistWriteProvider::new();
    let existing_id = seed_playlist(&provider, "Favoritas").await;
    let gateway = PlaylistWriteGateway::new(&provider);

    let report = mirror_playlist(
        &gateway,
        &mirror_request(" favoritas ", &["7", "8"]),
        &[ExistingPlaylist {
            playlist_id: existing_id.clone(),
            name: "Favoritas".into(),
        }],
        PlaylistMirrorOptions {
            merge_by_name: true,
            ..PlaylistMirrorOptions::default()
        },
    )
    .await;

    assert_eq!(report.status, PlaylistMirrorStatus::Merged);
    assert_eq!(report.playlist_id.as_deref(), Some(existing_id.as_str()));
    assert_eq!(provider.playlist_count(), 1);
    assert_eq!(
        provider
            .playlist(&existing_id)
            .expect("merged playlist")
            .items
            .iter()
            .map(|item| item.track_id.as_str())
            .collect::<Vec<_>>(),
        vec!["7", "8"]
    );
    assert_eq!(
        provider
            .calls()
            .iter()
            .filter(|call| call.operation() == PlaylistWriteOperation::Create)
            .count(),
        1,
        "the seed is the only Create; the merge must not issue a second one"
    );
}

#[tokio::test]
async fn repeated_tracks_are_sent_and_the_collapse_risk_is_reported() {
    let provider = FakePlaylistWriteProvider::new();
    let gateway = PlaylistWriteGateway::new(&provider);

    let report = mirror_playlist(
        &gateway,
        &mirror_request("Loop", &["9", "9", "10"]),
        &[],
        PlaylistMirrorOptions::default(),
    )
    .await;

    assert!(report.succeeded());
    assert_eq!(report.added_tracks, 3);
    assert_eq!(
        report.warnings,
        vec![PlaylistMirrorWarning::RepeatedTracksMayCollapse {
            repeated_occurrences: 1,
        }]
    );
    assert_eq!(
        provider
            .playlist(report.playlist_id.as_deref().expect("created id"))
            .expect("created playlist")
            .items
            .len(),
        3
    );
}

#[tokio::test]
async fn a_destination_that_cannot_add_tracks_is_not_written() {
    let provider = FakePlaylistWriteProvider::new().with_capabilities(PlaylistWriteCapabilities {
        add_tracks: false,
        ..PlaylistWriteCapabilities::ALL
    });
    let gateway = PlaylistWriteGateway::new(&provider);

    let report = mirror_playlist(
        &gateway,
        &mirror_request("Unsupported", &["1"]),
        &[],
        PlaylistMirrorOptions::default(),
    )
    .await;

    assert_eq!(report.status, PlaylistMirrorStatus::Failed);
    assert!(provider.calls().is_empty());
    assert_eq!(provider.playlist_count(), 0);
}

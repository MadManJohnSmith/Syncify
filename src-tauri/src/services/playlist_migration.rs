//! Reconstrucción de playlists en el destino durante una migración (M08, #173).
//!
//! La migración ya sabe qué pistas del origen se emparejaron en el destino
//! (`migration_items.destination_track_id`). Esta capa convierte esa lista
//! ordenada en una playlist remota usando el gateway de escritura de PL06
//! ([`crate::sync::playlist_write::PlaylistWriteGateway`]): nunca habla con un
//! cliente de proveedor por su cuenta.
//!
//! ## Reglas
//!
//! - Solo se intenta en destinos que declaran `create` y `add_tracks`. Una
//!   capacidad ausente se reporta como fallo; no se simula con una playlist
//!   local.
//! - Con `merge_by_name` activo, si el destino ya tiene una playlist con el
//!   mismo nombre (normalizado: sin espacios en los extremos y sin distinguir
//!   mayúsculas) se añaden las pistas a esa lista en vez de crear otra. Si hay
//!   varias con ese nombre, se usa la primera del directorio y se avisa: elegir
//!   una arbitrariamente es preferible a añadir un tercer duplicado.
//! - Las pistas se envían en el orden del origen, en lotes del tamaño
//!   configurado, y las repeticiones no se deduplican antes de enviar. Es
//!   best-effort: el gateway no expone reordenación ni lectura de contenido en
//!   los adaptadores Qobuz/Tidal, así que no puede confirmar que el proveedor
//!   conservó cada ocurrencia. Cuando la petición repite pistas se emite un
//!   aviso explícito ([`PlaylistMirrorWarning::RepeatedTracksMayCollapse`])
//!   para que el informe no prometa una fidelidad que el destino no garantiza.
//!
//! ## Pruebas
//!
//! Las pruebas de este módulo usan `FakePlaylistWriteProvider`, el proveedor en
//! memoria del gateway: no abren sockets ni consultan servicios reales.

use crate::sync::playlist_write::{
    PlaylistWriteError, PlaylistWriteErrorKind, PlaylistWriteGateway, PlaylistWriteOperation,
    PlaylistWriteOutcome, PlaylistWriteProvider, PlaylistWriteRequest,
};

/// Tamaño de lote por defecto para `addTracks`, el mismo que usaba el motor
/// antes de M08. Mantiene las peticiones pequeñas sin alterar el orden.
pub const DEFAULT_PLAYLIST_MIRROR_CHUNK: usize = 100;

/// Una playlist del origen que se quiere reconstruir en el destino.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlaylistMirrorRequest {
    pub name: String,
    /// Descripción para la playlist nueva. La fusión no la reescribe: la lista
    /// existente conserva la suya.
    pub description: String,
    /// Ids de pista del destino en orden de reproducción, con repeticiones.
    pub track_ids: Vec<String>,
}

/// Playlist que ya existe en el destino, tal como la conoce el directorio.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExistingPlaylist {
    pub playlist_id: String,
    pub name: String,
}

/// Opciones del espejo de playlists.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PlaylistMirrorOptions {
    /// Fusiona por nombre en vez de crear una lista nueva.
    pub merge_by_name: bool,
    /// Pistas por llamada de `addTracks`.
    pub chunk_size: usize,
}

impl Default for PlaylistMirrorOptions {
    fn default() -> Self {
        Self {
            merge_by_name: false,
            chunk_size: DEFAULT_PLAYLIST_MIRROR_CHUNK,
        }
    }
}

/// Avisos que acompañan a un espejo correcto pero no fiel al origen.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PlaylistMirrorWarning {
    /// La petición contiene ocurrencias repetidas y el destino puede
    /// colapsarlas en una sola. El gateway no lee el contenido final.
    RepeatedTracksMayCollapse { repeated_occurrences: usize },
    /// Varias playlists del destino comparten el nombre de fusión; se usó la
    /// primera del directorio para no crear otro duplicado.
    AmbiguousMergeTarget {
        matches: usize,
        chosen_playlist_id: String,
    },
}

impl std::fmt::Display for PlaylistMirrorWarning {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::RepeatedTracksMayCollapse {
                repeated_occurrences,
            } => write!(
                formatter,
                "{repeated_occurrences} repeated track occurrence(s) are best effort: the destination may keep only one"
            ),
            Self::AmbiguousMergeTarget {
                matches,
                chosen_playlist_id,
            } => write!(
                formatter,
                "{matches} destination playlists share the merge name; using {chosen_playlist_id} to avoid another duplicate"
            ),
        }
    }
}

/// Desenlace del espejo de una playlist.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlaylistMirrorStatus {
    Created,
    Merged,
    Failed,
}

/// Resultado de intentar reconstruir una playlist. Un fallo conserva el
/// progreso parcial (`playlist_id`, `added_tracks`) para que el llamador pueda
/// registrarlo; el error tipado dice si merece reintento.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlaylistMirrorReport {
    pub status: PlaylistMirrorStatus,
    pub playlist_id: Option<String>,
    pub requested_tracks: usize,
    /// Ocurrencias que el gateway confirmó como enviadas. No demuestra que el
    /// proveedor las conservara todas.
    pub added_tracks: usize,
    pub warnings: Vec<PlaylistMirrorWarning>,
    pub error: Option<PlaylistWriteError>,
}

impl PlaylistMirrorReport {
    pub fn succeeded(&self) -> bool {
        self.status != PlaylistMirrorStatus::Failed
    }
}

/// Normaliza un nombre para la fusión: recorta extremos y no distingue
/// mayúsculas. No elimina tildes ni signos: dos nombres que solo difieren en
/// eso siguen siendo listas distintas.
fn normalized_playlist_name(name: &str) -> String {
    name.trim().to_lowercase()
}

fn merge_target<'a>(
    existing: &'a [ExistingPlaylist],
    name: &str,
) -> Option<(&'a ExistingPlaylist, usize)> {
    let wanted = normalized_playlist_name(name);
    let mut matches = existing
        .iter()
        .filter(|playlist| normalized_playlist_name(&playlist.name) == wanted);
    let first = matches.next()?;
    Some((first, 1 + matches.count()))
}

fn repeated_occurrences(track_ids: &[String]) -> usize {
    let unique: std::collections::HashSet<&str> = track_ids.iter().map(String::as_str).collect();
    track_ids.len().saturating_sub(unique.len())
}

fn unexpected_outcome(
    service: &str,
    operation: PlaylistWriteOperation,
    outcome: &PlaylistWriteOutcome,
) -> PlaylistWriteError {
    PlaylistWriteError {
        service: service.to_string(),
        operation,
        kind: PlaylistWriteErrorKind::Permanent,
        http_status: None,
        message: format!(
            "playlist gateway returned an unexpected outcome for {}: {:?}",
            operation.as_str(),
            outcome
        ),
    }
}

fn failed_report(
    requested_tracks: usize,
    added_tracks: usize,
    playlist_id: Option<String>,
    warnings: Vec<PlaylistMirrorWarning>,
    error: PlaylistWriteError,
) -> PlaylistMirrorReport {
    PlaylistMirrorReport {
        status: PlaylistMirrorStatus::Failed,
        playlist_id,
        requested_tracks,
        added_tracks,
        warnings,
        error: Some(error),
    }
}

/// Reconstruye una playlist del origen en el destino a través del gateway.
///
/// No devuelve `Err`: una playlist que falla no debe abortar el resto del
/// espejo. El error tipado viaja en el informe con el progreso parcial.
pub async fn mirror_playlist<P: PlaylistWriteProvider>(
    gateway: &PlaylistWriteGateway<'_, P>,
    request: &PlaylistMirrorRequest,
    existing: &[ExistingPlaylist],
    options: PlaylistMirrorOptions,
) -> PlaylistMirrorReport {
    let requested_tracks = request.track_ids.len();
    let mut warnings = Vec::new();
    let repeated = repeated_occurrences(&request.track_ids);
    if repeated > 0 {
        warnings.push(PlaylistMirrorWarning::RepeatedTracksMayCollapse {
            repeated_occurrences: repeated,
        });
    }

    let capabilities = gateway.capabilities();
    let service = gateway.service_name();
    if !capabilities.create {
        return failed_report(
            requested_tracks,
            0,
            None,
            warnings,
            PlaylistWriteError::unsupported(service, PlaylistWriteOperation::Create),
        );
    }
    if !capabilities.add_tracks {
        return failed_report(
            requested_tracks,
            0,
            None,
            warnings,
            PlaylistWriteError::unsupported(service, PlaylistWriteOperation::AddTracks),
        );
    }

    let (playlist_id, status) = if options.merge_by_name {
        match merge_target(existing, &request.name) {
            Some((target, matches)) => {
                if matches > 1 {
                    warnings.push(PlaylistMirrorWarning::AmbiguousMergeTarget {
                        matches,
                        chosen_playlist_id: target.playlist_id.clone(),
                    });
                }
                (target.playlist_id.clone(), PlaylistMirrorStatus::Merged)
            }
            None => match create_destination_playlist(gateway, request).await {
                Ok(playlist_id) => (playlist_id, PlaylistMirrorStatus::Created),
                Err(error) => {
                    return failed_report(requested_tracks, 0, None, warnings, error);
                }
            },
        }
    } else {
        match create_destination_playlist(gateway, request).await {
            Ok(playlist_id) => (playlist_id, PlaylistMirrorStatus::Created),
            Err(error) => return failed_report(requested_tracks, 0, None, warnings, error),
        }
    };

    let chunk_size = options.chunk_size.max(1);
    let mut added_tracks = 0usize;
    for chunk in request.track_ids.chunks(chunk_size) {
        let outcome = gateway
            .execute(PlaylistWriteRequest::AddTracks {
                playlist_id: playlist_id.clone(),
                track_ids: chunk.to_vec(),
            })
            .await;
        match outcome {
            Ok(PlaylistWriteOutcome::TracksAdded { count }) => added_tracks += count,
            Ok(other) => {
                let error = unexpected_outcome(service, PlaylistWriteOperation::AddTracks, &other);
                return failed_report(
                    requested_tracks,
                    added_tracks,
                    Some(playlist_id),
                    warnings,
                    error,
                );
            }
            Err(error) => {
                return failed_report(
                    requested_tracks,
                    added_tracks,
                    Some(playlist_id),
                    warnings,
                    error,
                );
            }
        }
    }

    PlaylistMirrorReport {
        status,
        playlist_id: Some(playlist_id),
        requested_tracks,
        added_tracks,
        warnings,
        error: None,
    }
}

async fn create_destination_playlist<P: PlaylistWriteProvider>(
    gateway: &PlaylistWriteGateway<'_, P>,
    request: &PlaylistMirrorRequest,
) -> Result<String, PlaylistWriteError> {
    match gateway
        .execute(PlaylistWriteRequest::Create {
            name: request.name.clone(),
            description: request.description.clone(),
        })
        .await
    {
        Ok(PlaylistWriteOutcome::Created { playlist_id }) => Ok(playlist_id),
        Ok(other) => Err(unexpected_outcome(
            gateway.service_name(),
            PlaylistWriteOperation::Create,
            &other,
        )),
        Err(error) => Err(error),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sync::playlist_write::{FakePlaylistWriteProvider, PlaylistWriteCapabilities};

    fn request(name: &str, track_ids: &[&str]) -> PlaylistMirrorRequest {
        PlaylistMirrorRequest {
            name: name.to_string(),
            description: String::new(),
            track_ids: track_ids.iter().map(|id| id.to_string()).collect(),
        }
    }

    async fn seeded_playlist(provider: &FakePlaylistWriteProvider, name: &str) -> String {
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
            other => panic!("unexpected outcome: {other:?}"),
        }
    }

    #[tokio::test]
    async fn creates_a_new_playlist_with_tracks_in_order() {
        let provider = FakePlaylistWriteProvider::new();
        let gateway = PlaylistWriteGateway::new(&provider);

        let report = mirror_playlist(
            &gateway,
            &request("Road trip", &["10", "11", "12"]),
            &[],
            PlaylistMirrorOptions::default(),
        )
        .await;

        assert_eq!(report.status, PlaylistMirrorStatus::Created);
        assert!(report.succeeded());
        assert_eq!(report.requested_tracks, 3);
        assert_eq!(report.added_tracks, 3);
        assert!(report.error.is_none());
        assert!(report.warnings.is_empty());
        assert_eq!(
            provider.calls(),
            vec![
                PlaylistWriteRequest::Create {
                    name: "Road trip".into(),
                    description: String::new(),
                },
                PlaylistWriteRequest::AddTracks {
                    playlist_id: report.playlist_id.clone().unwrap(),
                    track_ids: vec!["10".into(), "11".into(), "12".into()],
                },
            ]
        );
        let playlist = provider
            .playlist(report.playlist_id.as_deref().unwrap())
            .expect("created playlist");
        assert_eq!(
            playlist
                .items
                .iter()
                .map(|item| item.track_id.as_str())
                .collect::<Vec<_>>(),
            vec!["10", "11", "12"]
        );
    }

    #[tokio::test]
    async fn chunks_preserve_the_source_order_and_repetitions() {
        let provider = FakePlaylistWriteProvider::new();
        let gateway = PlaylistWriteGateway::new(&provider);

        let report = mirror_playlist(
            &gateway,
            &request("Chunked", &["a", "b", "c", "d", "e"]),
            &[],
            PlaylistMirrorOptions {
                merge_by_name: false,
                chunk_size: 2,
            },
        )
        .await;

        assert_eq!(report.status, PlaylistMirrorStatus::Created);
        assert_eq!(report.added_tracks, 5);
        let added: Vec<Vec<String>> = provider
            .calls()
            .into_iter()
            .filter_map(|call| match call {
                PlaylistWriteRequest::AddTracks { track_ids, .. } => Some(track_ids),
                _ => None,
            })
            .collect();
        assert_eq!(
            added,
            vec![
                vec!["a".to_string(), "b".to_string()],
                vec!["c".to_string(), "d".to_string()],
                vec!["e".to_string()],
            ]
        );
        let playlist = provider
            .playlist(report.playlist_id.as_deref().unwrap())
            .expect("created playlist");
        assert_eq!(
            playlist
                .items
                .iter()
                .map(|item| item.track_id.as_str())
                .collect::<Vec<_>>(),
            vec!["a", "b", "c", "d", "e"]
        );
    }

    #[tokio::test]
    async fn merge_by_name_adds_to_the_existing_playlist_without_creating_another() {
        let provider = FakePlaylistWriteProvider::new();
        let existing_id = seeded_playlist(&provider, "Focus").await;
        let gateway = PlaylistWriteGateway::new(&provider);

        let report = mirror_playlist(
            &gateway,
            &request("  focus ", &["20", "21"]),
            &[ExistingPlaylist {
                playlist_id: existing_id.clone(),
                name: "Focus".into(),
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
                .calls()
                .iter()
                .map(PlaylistWriteRequest::operation)
                .collect::<Vec<_>>(),
            vec![
                PlaylistWriteOperation::Create,
                PlaylistWriteOperation::AddTracks
            ]
        );
        assert_eq!(
            provider
                .playlist(&existing_id)
                .expect("existing playlist")
                .items
                .iter()
                .map(|item| item.track_id.as_str())
                .collect::<Vec<_>>(),
            vec!["20", "21"]
        );
    }

    #[tokio::test]
    async fn merge_disabled_creates_a_playlist_even_when_the_name_exists() {
        let provider = FakePlaylistWriteProvider::new();
        let existing_id = seeded_playlist(&provider, "Focus").await;
        let gateway = PlaylistWriteGateway::new(&provider);

        let report = mirror_playlist(
            &gateway,
            &request("Focus", &["20"]),
            &[ExistingPlaylist {
                playlist_id: existing_id,
                name: "Focus".into(),
            }],
            PlaylistMirrorOptions::default(),
        )
        .await;

        assert_eq!(report.status, PlaylistMirrorStatus::Created);
        assert_eq!(provider.playlist_count(), 2);
    }

    #[tokio::test]
    async fn an_ambiguous_merge_name_uses_the_first_and_warns() {
        let provider = FakePlaylistWriteProvider::new();
        let first = seeded_playlist(&provider, "Mix").await;
        let second = seeded_playlist(&provider, "mix").await;
        let gateway = PlaylistWriteGateway::new(&provider);

        let report = mirror_playlist(
            &gateway,
            &request("MIX", &["1"]),
            &[
                ExistingPlaylist {
                    playlist_id: first.clone(),
                    name: "Mix".into(),
                },
                ExistingPlaylist {
                    playlist_id: second.clone(),
                    name: "mix".into(),
                },
            ],
            PlaylistMirrorOptions {
                merge_by_name: true,
                ..PlaylistMirrorOptions::default()
            },
        )
        .await;

        assert_eq!(report.status, PlaylistMirrorStatus::Merged);
        assert_eq!(report.playlist_id.as_deref(), Some(first.as_str()));
        assert_eq!(provider.playlist_count(), 2);
        assert_eq!(
            report.warnings,
            vec![PlaylistMirrorWarning::AmbiguousMergeTarget {
                matches: 2,
                chosen_playlist_id: first.clone(),
            }]
        );
        assert_eq!(
            provider
                .playlist(&first)
                .expect("first playlist")
                .items
                .iter()
                .map(|item| item.track_id.as_str())
                .collect::<Vec<_>>(),
            vec!["1"]
        );
        assert!(provider
            .playlist(&second)
            .expect("second playlist")
            .items
            .is_empty());
    }

    #[tokio::test]
    async fn repeated_tracks_are_sent_whole_and_reported_as_best_effort() {
        let provider = FakePlaylistWriteProvider::new();
        let gateway = PlaylistWriteGateway::new(&provider);

        let report = mirror_playlist(
            &gateway,
            &request("Repeated", &["10", "10", "11", "10"]),
            &[],
            PlaylistMirrorOptions::default(),
        )
        .await;

        assert!(report.succeeded());
        assert_eq!(report.requested_tracks, 4);
        assert_eq!(report.added_tracks, 4);
        assert_eq!(
            report.warnings,
            vec![PlaylistMirrorWarning::RepeatedTracksMayCollapse {
                repeated_occurrences: 2,
            }]
        );
        assert_eq!(
            provider
                .playlist(report.playlist_id.as_deref().unwrap())
                .expect("created playlist")
                .items
                .len(),
            4
        );
    }

    #[tokio::test]
    async fn an_absent_add_capability_fails_before_writing() {
        let provider =
            FakePlaylistWriteProvider::new().with_capabilities(PlaylistWriteCapabilities {
                add_tracks: false,
                ..PlaylistWriteCapabilities::ALL
            });
        let gateway = PlaylistWriteGateway::new(&provider);

        let report = mirror_playlist(
            &gateway,
            &request("Nope", &["1"]),
            &[],
            PlaylistMirrorOptions::default(),
        )
        .await;

        assert_eq!(report.status, PlaylistMirrorStatus::Failed);
        assert!(!report.succeeded());
        assert_eq!(
            report.error.as_ref().map(|error| error.kind),
            Some(PlaylistWriteErrorKind::Unsupported)
        );
        assert_eq!(
            report.error.as_ref().map(|error| error.operation),
            Some(PlaylistWriteOperation::AddTracks)
        );
        assert!(provider.calls().is_empty());
        assert_eq!(provider.playlist_count(), 0);
    }

    #[tokio::test]
    async fn a_create_failure_is_reported_without_partial_state() {
        let provider = FakePlaylistWriteProvider::new().with_failure(
            PlaylistWriteOperation::Create,
            PlaylistWriteErrorKind::Forbidden,
        );
        let gateway = PlaylistWriteGateway::new(&provider);

        let report = mirror_playlist(
            &gateway,
            &request("Forbidden", &["1"]),
            &[],
            PlaylistMirrorOptions::default(),
        )
        .await;

        assert_eq!(report.status, PlaylistMirrorStatus::Failed);
        assert!(report.playlist_id.is_none());
        assert_eq!(report.added_tracks, 0);
        assert_eq!(
            report.error.as_ref().map(|error| error.kind),
            Some(PlaylistWriteErrorKind::Forbidden)
        );
        assert_eq!(provider.playlist_count(), 0);
    }
}

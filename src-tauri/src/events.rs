//! Contratos de eventos compartidos entre el backend y la UI.
//!
//! La UI no debería reconstruir la forma de un evento leyendo al productor:
//! cada evento vive aquí con su nombre canónico y su tipo serializable.

use crate::services::incremental_enrichment::EnrichmentJobSummary;
use serde::Serialize;

/// Nombre canónico del canal de progreso de enriquecimiento.
pub const ENRICHMENT_PROGRESS_EVENT: &str = "enrichment-progress";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum MusicBrainzStatus {
    Started,
    Processing,
    Completed,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MusicBrainzProgress {
    pub status: MusicBrainzStatus,
    pub total: usize,
    pub current: usize,
    pub enriched: usize,
    pub failed: usize,
    pub current_track: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum EnrichmentEvent {
    #[serde(rename = "musicbrainz")]
    MusicBrainz(MusicBrainzProgress),
    #[serde(rename = "incrementalSummary")]
    IncrementalSummary(EnrichmentJobSummary),
}

/// Emite un evento de enriquecimiento. Un fallo de emisión no debe tumbar la
/// operación que lo produce: se registra y se sigue.
pub fn emit_enrichment<R: tauri::Runtime>(app: &tauri::AppHandle<R>, event: &EnrichmentEvent) {
    use tauri::Emitter;

    if let Err(error) = app.emit(ENRICHMENT_PROGRESS_EVENT, event) {
        tracing::warn!(%error, "no se pudo emitir enrichment-progress");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn musicbrainz_event_uses_kind_tag_and_camel_case() {
        let event = EnrichmentEvent::MusicBrainz(MusicBrainzProgress {
            status: MusicBrainzStatus::Processing,
            total: 120,
            current: 40,
            enriched: 38,
            failed: 2,
            current_track: Some("Artist - Title".to_string()),
        });

        let json = serde_json::to_value(&event).expect("serialize");
        assert_eq!(json["kind"], "musicbrainz");
        assert_eq!(json["status"], "processing");
        assert_eq!(json["total"], 120);
        assert_eq!(json["current"], 40);
        assert_eq!(json["enriched"], 38);
        assert_eq!(json["failed"], 2);
        assert_eq!(json["currentTrack"], "Artist - Title");
    }

    #[test]
    fn completed_event_serializes_absent_track_as_null() {
        let event = EnrichmentEvent::MusicBrainz(MusicBrainzProgress {
            status: MusicBrainzStatus::Completed,
            total: 10,
            current: 10,
            enriched: 9,
            failed: 1,
            current_track: None,
        });

        let json = serde_json::to_value(&event).expect("serialize");
        assert_eq!(json["status"], "completed");
        assert!(json["currentTrack"].is_null());
    }

    #[test]
    fn incremental_summary_event_uses_kind_tag_and_keeps_camel_case() {
        use crate::services::incremental_enrichment::{EnrichmentJobSummary, JobStatus};

        let summary = EnrichmentJobSummary {
            job_id: "job-1".to_string(),
            mode: Default::default(),
            status: JobStatus::Running,
            total_tracks: 900,
            processed_tracks: 3,
            modified_tracks: 0,
            skipped_complete_tracks: 0,
            skipped_precedence_tracks: 0,
            failed_tracks: 0,
            current_track: Some("Otra - Pista".to_string()),
            current_phase: Some("Enriching".to_string()),
            items: Vec::new(),
            available_sources: Vec::new(),
            started_at: "2026-11-05T00:00:00Z".to_string(),
            completed_at: None,
        };

        let event = EnrichmentEvent::IncrementalSummary(summary);
        let json = serde_json::to_value(&event).expect("serialize");
        assert_eq!(json["kind"], "incrementalSummary");
        assert_eq!(json["totalTracks"], 900);
        assert_eq!(json["processedTracks"], 3);
        assert_eq!(json["status"], "running");
    }
}

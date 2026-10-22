//! Apariciones de una pista en el ledger de ediciones (ED-04b).
//!
//! Una aparición no es una fuente: dice que el ítem se vio y a qué Track se
//! asignó, sin mover el álbum canónico ni escribir evidencia nueva.

use crate::db::DbPool;
use serde::Serialize;
use tauri::State;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct EdicionDePista {
    pub service: String,
    pub service_track_id: String,
    pub album: Option<String>,
    pub validation_status: String,
}

/// «qobuz: album=Lucky (Soundtrack)» → `Some("Lucky (Soundtrack)")`.
fn album_de_procedencia(procedencia: &str) -> Option<String> {
    procedencia
        .split_once("album=")
        .map(|(_, album)| album.trim().to_string())
        .filter(|album| !album.is_empty())
}

pub async fn ediciones_de_pista_en(
    pool: &DbPool,
    track_id: i64,
) -> Result<Vec<EdicionDePista>, String> {
    let filas: Vec<(String, String, String, String)> = sqlx::query_as(
        "SELECT s.name, i.service_track_id, i.provenance, link.validation_status
           FROM edition_provider_items i
           JOIN edition_item_tracks link ON link.item_id = i.id
           JOIN services s ON s.id = i.service_id
          WHERE link.track_id = ?
          ORDER BY s.name, i.service_track_id",
    )
    .bind(track_id)
    .fetch_all(pool)
    .await
    .map_err(|e| format!("ediciones_de_pista: {e}"))?;

    Ok(filas
        .into_iter()
        .map(
            |(service, service_track_id, provenance, validation_status)| EdicionDePista {
                service,
                service_track_id,
                album: album_de_procedencia(&provenance),
                validation_status,
            },
        )
        .collect())
}

#[tauri::command]
pub async fn ediciones_de_pista(
    state: State<'_, crate::AppState>,
    track_id: i64,
) -> Result<Vec<EdicionDePista>, String> {
    ediciones_de_pista_en(&state.db, track_id).await
}

#[cfg(test)]
mod tests {
    use super::*;

    async fn pool() -> DbPool {
        let pool = sqlx::SqlitePool::connect("sqlite::memory:").await.unwrap();
        sqlx::migrate!("./migrations").run(&pool).await.unwrap();
        pool
    }

    #[test]
    fn la_procedencia_sin_album_no_inventa_uno() {
        assert_eq!(
            album_de_procedencia("tidal: album=Storyteller"),
            Some("Storyteller".to_string())
        );
        assert_eq!(album_de_procedencia("legacy_track_sources"), None);
        assert_eq!(album_de_procedencia("qobuz: album=  "), None);
    }

    #[tokio::test]
    async fn devuelve_las_apariciones_de_la_pista_con_su_album() {
        let pool = pool().await;
        let servicio: i64 = sqlx::query_scalar("SELECT id FROM services WHERE name = 'tidal'")
            .fetch_one(&pool)
            .await
            .unwrap();
        let track: i64 =
            sqlx::query_scalar("INSERT INTO tracks (title) VALUES ('Lucky') RETURNING id")
                .fetch_one(&pool)
                .await
                .unwrap();
        let item: i64 = sqlx::query_scalar(
            "INSERT INTO edition_provider_items(service_id, service_track_id, evidence_status, provenance) VALUES (?, 'item-1', 'unknown', 'tidal: album=Lucky (Soundtrack)') RETURNING id",
        )
        .bind(servicio)
        .fetch_one(&pool)
        .await
        .unwrap();
        sqlx::query(
            "INSERT INTO edition_item_tracks(item_id, track_id, validation_status, provenance) VALUES (?, ?, 'needs_review', 'ed04:album-family')",
        )
        .bind(item)
        .bind(track)
        .execute(&pool)
        .await
        .unwrap();

        let apariciones = ediciones_de_pista_en(&pool, track).await.unwrap();
        assert_eq!(
            apariciones,
            vec![EdicionDePista {
                service: "tidal".to_string(),
                service_track_id: "item-1".to_string(),
                album: Some("Lucky (Soundtrack)".to_string()),
                validation_status: "needs_review".to_string(),
            }]
        );
    }
}

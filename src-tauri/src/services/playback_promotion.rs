//! Promoción de entradas efímeras de la cola (#134, ADR-0010 decisión 3).
//!
//! El núcleo recibe la identidad ya resuelta por el proveedor. Así las pruebas
//! ejercitan la misma transacción y el mismo matcher sin red; el adaptador de
//! producción es responsable sólo de obtener esos metadatos.
//!
//! Invariantes:
//! * encolar nunca materializa biblioteca;
//! * la primera escucha que cruza 30 s congela su `listen_session_id` en la
//!   entrada, incluso si la identidad falla;
//! * Track/TrackSource los persiste el motor unificado de enriquecimiento;
//! * el crédito usa el núcleo transaccional idempotente por sesión.

use syncify_core_domain::metadata::{
    is_placeholder_artist, is_placeholder_title, ProviderTrackIdentity,
};

use crate::commands::playback::{normalize_listen_session_id, record_local_play_in_tx};
use crate::download::qobuz::QobuzDownloader;
use crate::services::enrichment::{
    EnrichmentEngine, OriginTrackMetadata, SyncTrackInput, SyncTrackResult,
};
use crate::services::qobuz::{QobuzClient, QOBUZ_APP_ID, QOBUZ_APP_SECRET};
use crate::services::tidal::TidalClient;
use crate::services::track_identity::{resolve_identity, IdentityOutcome, IdentityRequest};
use crate::DbPool;

/// Identidad de proveedor resuelta fuera del núcleo, junto con la cuenta que
/// permitió resolverla. El motor la usa para persistir la pertenencia viva.
#[derive(Debug, Clone)]
pub struct ResolvedProviderIdentity {
    pub identity: ProviderTrackIdentity,
    pub account_id: i64,
}

/// Resultado observable de una promoción.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PromotionOutcome {
    /// La efímera se convirtió y esta llamada insertó la escucha.
    Promoted { track_id: i64, counted: bool },
    /// La entrada ya era de biblioteca; el token congelado sigue deduplicando.
    AlreadyLibrary { track_id: i64, counted: bool },
    /// No hubo identidad suficiente; la entrada continúa efímera.
    StillEphemeral { reason: String },
}

fn bounded_reason(reason: &str) -> String {
    reason.chars().take(500).collect()
}

async fn record_unresolved(
    db: &DbPool,
    entry_id: &str,
    reason: &str,
) -> Result<PromotionOutcome, String> {
    let reason = bounded_reason(reason);
    sqlx::query(
        "UPDATE player_queue_entries SET promotion_error = ? \
         WHERE entry_id = ? AND track_id IS NULL",
    )
    .bind(&reason)
    .bind(entry_id)
    .execute(db)
    .await
    .map_err(|e| e.to_string())?;
    Ok(PromotionOutcome::StillEphemeral { reason })
}

async fn credit_and_clear_error(
    db: &DbPool,
    entry_id: &str,
    track_id: i64,
    listen_session_id: &str,
) -> Result<PromotionOutcome, String> {
    let mut tx = db.begin().await.map_err(|e| e.to_string())?;
    match record_local_play_in_tx(&mut tx, track_id, listen_session_id).await {
        Ok(stats) => {
            sqlx::query(
                "UPDATE player_queue_entries SET promotion_error = NULL WHERE entry_id = ?",
            )
            .bind(entry_id)
            .execute(&mut *tx)
            .await
            .map_err(|e| e.to_string())?;
            tx.commit().await.map_err(|e| e.to_string())?;
            Ok(PromotionOutcome::AlreadyLibrary {
                track_id,
                counted: stats.counted,
            })
        }
        Err(error) => {
            drop(tx);
            let reason = bounded_reason(&format!("escucha no acreditada: {error}"));
            sqlx::query("UPDATE player_queue_entries SET promotion_error = ? WHERE entry_id = ?")
                .bind(&reason)
                .bind(entry_id)
                .execute(db)
                .await
                .map_err(|e| e.to_string())?;
            Err(error)
        }
    }
}

async fn validate_resolved_identity(
    db: &DbPool,
    entry: &QueueIdentity,
    resolved: &ResolvedProviderIdentity,
) -> Result<(), String> {
    if resolved.identity.service_id != entry.service_id {
        return Err(format!(
            "la identidad resuelta pertenece al servicio {}, no al {}",
            resolved.identity.service_id, entry.service_id
        ));
    }
    if resolved.identity.service_track_id.trim() != entry.service_track_id.trim() {
        return Err("la identidad resuelta no corresponde al ítem de la cola".to_string());
    }
    if resolved.account_id <= 0 {
        return Err("la identidad resuelta no tiene una cuenta válida".to_string());
    }
    let account_service: Option<i64> = sqlx::query_scalar(
        "SELECT service_id FROM accounts \
         WHERE id = ? AND service_id = ? AND state = 'enabled' \
           AND COALESCE(credentials_invalid, 0) = 0 \
           AND COALESCE(streaming_entitled, -1) != 0 \
           AND COALESCE(invalid_reason, '') <> 'logged_out'",
    )
    .bind(resolved.account_id)
    .bind(entry.service_id)
    .fetch_optional(db)
    .await
    .map_err(|e| e.to_string())?;
    if account_service.is_none() {
        return Err(
            "la cuenta resuelta no está habilitada para el servicio de la entrada".to_string(),
        );
    }
    let title = resolved.identity.title.as_deref().unwrap_or_default();
    let artist = resolved.identity.artist.as_deref().unwrap_or_default();
    if is_placeholder_title(title) || is_placeholder_artist(artist) {
        return Err("metadatos incompletos: título o artista no identificable".to_string());
    }
    Ok(())
}

struct QueueIdentity {
    track_id: Option<i64>,
    service_id: i64,
    service_track_id: String,
    listen_session_id: String,
}

async fn load_queue_identity(db: &DbPool, entry_id: &str) -> Result<QueueIdentity, String> {
    let row: Option<(Option<i64>, Option<i64>, Option<String>, Option<String>)> = sqlx::query_as(
        "SELECT track_id, service_id, service_track_id, listen_session_id \
         FROM player_queue_entries WHERE entry_id = ?",
    )
    .bind(entry_id)
    .fetch_optional(db)
    .await
    .map_err(|e| e.to_string())?;
    let (track_id, service_id, service_track_id, listen_session_id) =
        row.ok_or_else(|| format!("La entrada {entry_id} no existe"))?;
    let listen_session_id = listen_session_id
        .ok_or_else(|| format!("La entrada {entry_id} no tiene sesión de escucha"))?;
    if track_id.is_none() {
        let service_id = service_id.ok_or_else(|| {
            format!("La entrada {entry_id} no tiene track ni identidad de proveedor")
        })?;
        let service_track_id = service_track_id
            .ok_or_else(|| format!("La entrada {entry_id} no tiene service_track_id"))?;
        Ok(QueueIdentity {
            track_id,
            service_id,
            service_track_id,
            listen_session_id,
        })
    } else {
        Ok(QueueIdentity {
            track_id,
            service_id: service_id.unwrap_or(0),
            service_track_id: service_track_id.unwrap_or_default(),
            listen_session_id,
        })
    }
}

/// El motor histórico importa una huella ambigua como Track propio. La cola no
/// puede hacer eso: antes de llamarlo, aplica la misma decisión estricta y deja
/// la entrada efímera cuando la evidencia no elige una grabación.
async fn reject_ambiguous_identity(
    db: &DbPool,
    resolved: &ResolvedProviderIdentity,
) -> Result<(), String> {
    let identity = &resolved.identity;
    let request = IdentityRequest {
        service_id: identity.service_id,
        service_track_id: &identity.service_track_id,
        account_id: Some(resolved.account_id),
        title: identity.title.as_deref().unwrap_or_default(),
        artist: identity.artist.as_deref().unwrap_or_default(),
        album: identity.album.as_deref(),
        duration_ms: identity.duration_ms,
        isrc: identity.isrc.as_deref(),
        explicit: identity.explicit,
    };
    let mut tx = db.begin().await.map_err(|e| e.to_string())?;
    match resolve_identity(&mut tx, &request).await? {
        IdentityOutcome::Ambiguous { .. } => Err(
            "needs_review: la huella de identidad tiene varios candidatos; la entrada sigue efímera"
                .to_string(),
        ),
        IdentityOutcome::Linked { .. } | IdentityOutcome::New => Ok(()),
    }
}

async fn persist_identity_via_engine(
    db: &DbPool,
    resolved: &ResolvedProviderIdentity,
) -> Result<SyncTrackResult, String> {
    let identity = &resolved.identity;
    let artist = identity.artist.clone().unwrap_or_default();
    let input = SyncTrackInput {
        origin_meta: OriginTrackMetadata {
            title: identity.title.clone(),
            artist: Some(artist),
            album: identity.album.clone(),
            isrc: identity.sanitized_isrc(),
            track_number: identity.track_number.map(|value| value.max(0) as u32),
            disc_number: identity.disc_number.map(|value| value.max(0) as u32),
            explicit: identity.explicit,
            source_name: identity.service_name.clone(),
            ..Default::default()
        },
        service_track_id: identity.service_track_id.clone(),
        service_name: identity.service_name.clone(),
        service_id: identity.service_id,
        account_id: resolved.account_id,
        duration_ms: identity.duration_ms,
        album_provider_track_id: identity.provider_album_id.clone(),
        query_musicbrainz: false,
        ..Default::default()
    };
    let engine = EnrichmentEngine::with_db(db.clone());
    crate::commands::service::enrich_persist_with_locked_retry(&engine, db, input).await
}

async fn candidate_accounts(db: &DbPool, service_name: &str) -> Result<Vec<i64>, String> {
    let preferred: Option<i64> = sqlx::query_scalar(
        "SELECT account_id FROM service_preferred_accounts WHERE service_name = ?",
    )
    .bind(service_name)
    .fetch_optional(db)
    .await
    .map_err(|e| e.to_string())?;
    let mut accounts =
        crate::commands::accounts::get_service_fallback_account_ids(db, service_name, preferred)
            .await?;
    let eligible =
        crate::services::streaming::eligible_streaming_account_ids(db, service_name).await?;
    accounts.retain(|account_id| eligible.contains(account_id));
    Ok(accounts)
}

fn qobuz_identity(
    account_id: i64,
    service_id: i64,
    service_track_id: &str,
    track: &crate::services::qobuz::QobuzTrack,
) -> ResolvedProviderIdentity {
    let artist = track
        .performer
        .as_ref()
        .and_then(|artist| artist.name.clone())
        .or_else(|| {
            track
                .composer
                .as_ref()
                .and_then(|artist| artist.name.clone())
        });
    ResolvedProviderIdentity {
        identity: ProviderTrackIdentity {
            service_id,
            service_name: "qobuz".into(),
            service_track_id: service_track_id.into(),
            isrc: track.isrc.clone(),
            provider_album_id: track.album.as_ref().map(|album| album.id.clone()),
            provider_artist_id: track
                .performer
                .as_ref()
                .and_then(|artist| artist.id)
                .or_else(|| track.composer.as_ref().and_then(|artist| artist.id))
                .map(|id| id.to_string()),
            title: track.title.clone(),
            artist,
            album: track.album.as_ref().and_then(|album| album.title.clone()),
            duration_ms: Some(track.duration * 1000),
            track_number: track.track_number,
            disc_number: track.media_number,
            explicit: None,
        },
        account_id,
    }
}

fn tidal_identity(
    account_id: i64,
    service_id: i64,
    service_track_id: &str,
    track: &crate::services::tidal::TidalTrack,
) -> ResolvedProviderIdentity {
    ResolvedProviderIdentity {
        identity: ProviderTrackIdentity {
            service_id,
            service_name: "tidal".into(),
            service_track_id: service_track_id.into(),
            isrc: track.isrc.clone(),
            provider_album_id: track.album.as_ref().map(|album| album.tidal_id.to_string()),
            provider_artist_id: track.artist.as_ref().map(|artist| artist.id.to_string()),
            title: Some(track.title.clone()),
            artist: track.artist.as_ref().map(|artist| artist.name.clone()),
            album: track.album.as_ref().map(|album| album.title.clone()),
            duration_ms: Some(track.duration * 1000),
            track_number: track.track_number,
            disc_number: track.disc_number,
            explicit: track.explicit,
        },
        account_id,
    }
}

/// Adaptador de producción: obtiene metadatos del proveedor sin escribir en
/// biblioteca. El núcleo de promoción recibe el resultado ya resuelto.
pub async fn resolve_provider_identity(
    db: &DbPool,
    service_id: i64,
    service_track_id: &str,
) -> Result<ResolvedProviderIdentity, String> {
    let service_name: String = sqlx::query_scalar("SELECT name FROM services WHERE id = ?")
        .bind(service_id)
        .fetch_optional(db)
        .await
        .map_err(|e| e.to_string())?
        .ok_or_else(|| format!("El servicio {service_id} no existe"))?;
    let accounts = candidate_accounts(db, &service_name).await?;
    if accounts.is_empty() {
        return Err(format!(
            "El servicio {service_name} no tiene una cuenta habilitada para resolver el ítem"
        ));
    }
    let mut failures = Vec::new();
    match service_name.to_ascii_lowercase().as_str() {
        "qobuz" => {
            let downloader = QobuzDownloader::new();
            let app_id = std::env::var("QOBUZ_APP_ID").unwrap_or_else(|_| QOBUZ_APP_ID.to_string());
            let app_secret =
                std::env::var("QOBUZ_APP_SECRET").unwrap_or_else(|_| QOBUZ_APP_SECRET.to_string());
            for account_id in accounts {
                let token = match downloader
                    .resolve_token_for_account(db, Some(account_id))
                    .await
                {
                    Ok(token) => token,
                    Err(crate::download::qobuz::QobuzAuthStatus::RequiresAuth(message)) => {
                        failures.push(message);
                        continue;
                    }
                };
                let client = QobuzClient::new_with_token(app_id.clone(), app_secret.clone(), token);
                match client
                    .get_track(
                        service_track_id.parse::<i64>().map_err(|_| {
                            "id de Qobuz inválido: se esperaba un entero".to_string()
                        })?,
                    )
                    .await
                {
                    Ok(track) => {
                        return Ok(qobuz_identity(
                            account_id,
                            service_id,
                            service_track_id,
                            &track,
                        ))
                    }
                    Err(error) => failures.push(error),
                }
            }
        }
        "tidal" => {
            let http = reqwest::Client::new();
            for account_id in accounts {
                let (Some(creds), _) =
                    crate::services::tidal_pipeline::resolve_and_refresh_gui_credentials_for_account(
                        db,
                        &http,
                        Some(account_id),
                        false,
                    )
                    .await
                else {
                    failures.push(format!("cuenta Tidal {account_id} sin credenciales válidas"));
                    continue;
                };
                let user_id = creds.user_id.as_ref().and_then(|value| {
                    value
                        .as_str()
                        .map(str::to_string)
                        .or_else(|| value.as_i64().map(|id| id.to_string()))
                });
                let country = creds.country_code.clone().unwrap_or_else(|| "US".into());
                let mut client = TidalClient::new(creds.access_token.clone());
                if let Some(user_id) = user_id {
                    client = client.with_user(user_id, country);
                }
                match client
                    .get_track(
                        service_track_id.parse::<i64>().map_err(|_| {
                            "id de Tidal inválido: se esperaba un entero".to_string()
                        })?,
                    )
                    .await
                {
                    Ok(track) => {
                        return Ok(tidal_identity(
                            account_id,
                            service_id,
                            service_track_id,
                            &track,
                        ))
                    }
                    Err(error) => failures.push(error),
                }
            }
        }
        other => return Err(format!("Proveedor sin resolución de identidad: {other}")),
    }
    Err(format!(
        "No se pudo resolver identidad en {}: {}",
        service_name,
        failures.join(" · ")
    ))
}

/// Congela la primera sesión que cruzó el umbral, antes de cualquier trabajo de
/// red. Un reintento con otra sesión nunca puede producir un segundo crédito.
pub async fn freeze_queue_listen_session(
    db: &DbPool,
    entry_id: &str,
    listen_session_id: &str,
) -> Result<String, String> {
    if entry_id.trim().is_empty() {
        return Err("entry_id vacío".to_string());
    }
    let session_id = normalize_listen_session_id(listen_session_id)?;
    let updated = sqlx::query(
        "UPDATE player_queue_entries \
         SET listen_session_id = COALESCE(listen_session_id, ?) WHERE entry_id = ?",
    )
    .bind(&session_id)
    .bind(entry_id)
    .execute(db)
    .await
    .map_err(|e| e.to_string())?
    .rows_affected();
    if updated != 1 {
        return Err(format!("La entrada {entry_id} no existe"));
    }
    sqlx::query_scalar("SELECT listen_session_id FROM player_queue_entries WHERE entry_id = ?")
        .bind(entry_id)
        .fetch_one(db)
        .await
        .map_err(|e| e.to_string())
}

/// Promueve una entrada tras el umbral de escucha. `resolved` es `Err` cuando
/// el proveedor no pudo identificar el ítem: se persiste el motivo y la entrada
/// sigue efímera. Los errores de BD o de crédito sí se devuelven como `Err`.
pub async fn promote_queue_entry(
    db: &DbPool,
    entry_id: &str,
    listen_session_id: &str,
    resolved: Result<ResolvedProviderIdentity, String>,
) -> Result<PromotionOutcome, String> {
    freeze_queue_listen_session(db, entry_id, listen_session_id).await?;
    let entry = load_queue_identity(db, entry_id).await?;

    if let Some(track_id) = entry.track_id {
        return credit_and_clear_error(db, entry_id, track_id, &entry.listen_session_id).await;
    }

    // Si una caída anterior ya persistió la fuente, enlazar no necesita volver
    // a consultar al proveedor ni crear una pertenencia duplicada.
    let existing_track_id: Option<i64> = sqlx::query_scalar(
        "SELECT track_id FROM track_sources \
         WHERE service_id = ? AND service_track_id = ? LIMIT 1",
    )
    .bind(entry.service_id)
    .bind(&entry.service_track_id)
    .fetch_optional(db)
    .await
    .map_err(|e| e.to_string())?;

    let track_id = if let Some(track_id) = existing_track_id {
        track_id
    } else {
        let resolved = match resolved {
            Ok(resolved) => resolved,
            Err(reason) => return record_unresolved(db, entry_id, &reason).await,
        };
        if let Err(reason) = validate_resolved_identity(db, &entry, &resolved).await {
            return record_unresolved(db, entry_id, &reason).await;
        }
        if let Err(reason) = reject_ambiguous_identity(db, &resolved).await {
            return record_unresolved(db, entry_id, &reason).await;
        }
        match persist_identity_via_engine(db, &resolved).await {
            Ok(result) if result.availability_status != "needs_review" => result.track_id,
            Ok(_) => {
                return record_unresolved(
                    db,
                    entry_id,
                    "needs_review: la identidad existe, pero la edición requiere revisión",
                )
                .await
            }
            Err(reason) => return record_unresolved(db, entry_id, &reason).await,
        }
    };

    // Conversión + crédito en la misma transacción: una caída no puede dejar
    // una entrada de biblioteca sin su play_count. El CAS protege dos
    // promociones concurrentes; ambas usan el token congelado de la entrada.
    let mut tx = db
        .begin_with("BEGIN IMMEDIATE")
        .await
        .map_err(|e| e.to_string())?;
    let converted = sqlx::query(
        "UPDATE player_queue_entries \
         SET track_id = ?, service_id = NULL, service_track_id = NULL, \
             promotion_error = NULL \
         WHERE entry_id = ? AND track_id IS NULL",
    )
    .bind(track_id)
    .bind(entry_id)
    .execute(&mut *tx)
    .await
    .map_err(|e| e.to_string())?
    .rows_affected();
    let converted_now = converted == 1;
    let track_id = if converted_now {
        track_id
    } else {
        sqlx::query_scalar("SELECT track_id FROM player_queue_entries WHERE entry_id = ?")
            .bind(entry_id)
            .fetch_one(&mut *tx)
            .await
            .map_err(|e| e.to_string())?
    };
    let stats = match record_local_play_in_tx(&mut tx, track_id, &entry.listen_session_id).await {
        Ok(stats) => stats,
        Err(error) => {
            drop(tx);
            let reason = bounded_reason(&format!("escucha no acreditada: {error}"));
            sqlx::query("UPDATE player_queue_entries SET promotion_error = ? WHERE entry_id = ?")
                .bind(&reason)
                .bind(entry_id)
                .execute(db)
                .await
                .map_err(|e| e.to_string())?;
            return Err(error);
        }
    };
    tx.commit().await.map_err(|e| e.to_string())?;
    if converted_now {
        Ok(PromotionOutcome::Promoted {
            track_id,
            counted: stats.counted,
        })
    } else {
        Ok(PromotionOutcome::AlreadyLibrary {
            track_id,
            counted: stats.counted,
        })
    }
}

/// Recupera estados durables dejados por una caída: conversión sin crédito o
/// fuente persistida antes de convertir la entrada. Es idempotente porque
/// `playback_listens` vuelve a decidir el conteo.
pub async fn reconcile_queue_promotions(db: &DbPool) -> Result<usize, String> {
    let pending: Vec<(String, i64, String)> = sqlx::query_as(
        "SELECT e.entry_id, COALESCE(e.track_id, 0), e.listen_session_id \
         FROM player_queue_entries e \
         WHERE e.listen_session_id IS NOT NULL \
           AND (e.track_id IS NOT NULL OR EXISTS ( \
               SELECT 1 FROM track_sources ts \
               WHERE ts.service_id = e.service_id \
                 AND ts.service_track_id = e.service_track_id \
           )) \
           AND NOT EXISTS ( \
               SELECT 1 FROM playback_listens l \
               WHERE l.listen_session_id = e.listen_session_id \
           )",
    )
    .fetch_all(db)
    .await
    .map_err(|e| e.to_string())?;
    let mut recovered = 0;
    for (entry_id, _track_id, listen_session_id) in pending {
        match promote_queue_entry(
            db,
            &entry_id,
            &listen_session_id,
            Err("recuperación de crédito pendiente".into()),
        )
        .await
        {
            Ok(_) => recovered += 1,
            Err(error) => tracing::warn!(
                entry_id = %entry_id,
                "no se pudo recuperar el crédito de promoción: {error}"
            ),
        }
    }
    Ok(recovered)
}

/// Reintenta al arrancar las efímeras cuyo token se congeló antes de una caída
/// y que todavía no tienen `TrackSource`. La resolución puede usar red; las
/// pruebas del núcleo llaman sólo a `reconcile_queue_promotions`.
pub async fn retry_unresolved_queue_promotions(db: &DbPool) -> Result<usize, String> {
    let pending: Vec<(String, i64, String, String)> = sqlx::query_as(
        "SELECT e.entry_id, e.service_id, e.service_track_id, e.listen_session_id \
         FROM player_queue_entries e \
         WHERE e.track_id IS NULL AND e.listen_session_id IS NOT NULL \
           AND NOT EXISTS ( \
               SELECT 1 FROM track_sources ts \
               WHERE ts.service_id = e.service_id \
                 AND ts.service_track_id = e.service_track_id \
           )",
    )
    .fetch_all(db)
    .await
    .map_err(|e| e.to_string())?;
    let mut retried = 0;
    for (entry_id, service_id, service_track_id, listen_session_id) in pending {
        let resolved = resolve_provider_identity(db, service_id, &service_track_id).await;
        match promote_queue_entry(db, &entry_id, &listen_session_id, resolved).await {
            Ok(_) => retried += 1,
            Err(error) => tracing::warn!(
                entry_id = %entry_id,
                "no se pudo reintentar la promoción tras el arranque: {error}"
            ),
        }
    }
    Ok(retried)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tidal_conserva_el_flag_explicit_al_resolver_identidad() {
        let track: crate::services::tidal::TidalTrack = serde_json::from_str(
            r#"{
                "id": 7,
                "title": "Canción explícita",
                "duration": 180,
                "explicit": true,
                "artist": { "id": 9, "name": "Artista" }
            }"#,
        )
        .expect("DTO Tidal");
        let resolved = tidal_identity(1, 2, "7", &track);
        assert_eq!(resolved.identity.explicit, Some(true));
        assert_eq!(resolved.identity.service_track_id, "7");
    }
}

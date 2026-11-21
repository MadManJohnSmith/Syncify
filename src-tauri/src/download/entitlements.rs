//! Evidencia de entrega por cuenta (D04, spec #111).
//!
//! `download_entitlements` es la memoria del enrutado: una entrega correcta deja
//! un `granted` reutilizable durante 30 días; un 401/403 o un rechazo de calidad
//! deja un `denied` de una hora que impide volver a probar esa cuenta para esa
//! pista. La evidencia caducada pasa a `stale` y no se usa a ciegas.
//!
//! Este módulo solo persiste y lee hechos. El orden de candidatos vive en
//! `resolver.rs` y el registro de la entrega en `worker.rs`.

use std::collections::HashMap;

use sqlx::sqlite::SqlitePool;

/// Vigencia de una entrega correcta (ADR-0005).
pub const GRANTED_TTL_DAYS: i64 = 30;
/// Vigencia de una denegación por auth o rechazo de calidad (ADR-0005).
pub const DENIED_TTL_HOURS: i64 = 1;

/// Estados del vocabulario de `download_entitlements` (migración 0102).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EntitlementState {
    Granted,
    Denied,
    Unknown,
    Stale,
}

impl EntitlementState {
    pub fn from_db(value: &str) -> Option<Self> {
        match value {
            "granted" => Some(Self::Granted),
            "denied" => Some(Self::Denied),
            "unknown" => Some(Self::Unknown),
            "stale" => Some(Self::Stale),
            _ => None,
        }
    }
}

/// Fila de evidencia vigente leída por el resolutor (solo tests/auditoría).
#[cfg(test)]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EntitlementEvidence {
    pub account_id: i64,
    pub service_name: String,
    pub state: EntitlementState,
    pub quality: Option<String>,
}

/// Evidencia vigente de una pista, indexada por servicio y cuenta.
#[derive(Debug, Clone, Default)]
pub struct TrackEvidence {
    /// Cuentas con `granted` vigente por servicio (nombre en minúsculas).
    pub granted: HashMap<String, Vec<i64>>,
    /// Cuentas con `denied` vigente por servicio (nombre en minúsculas).
    pub denied: HashMap<String, Vec<i64>>,
}

impl TrackEvidence {
    pub fn has_granted_service(&self, service_name: &str) -> bool {
        self.granted
            .contains_key(&service_name.trim().to_ascii_lowercase())
    }

    pub fn is_granted(&self, service_name: &str, account_id: i64) -> bool {
        self.granted
            .get(&service_name.trim().to_ascii_lowercase())
            .is_some_and(|accounts| accounts.contains(&account_id))
    }

    pub fn is_denied(&self, service_name: &str, account_id: i64) -> bool {
        self.denied
            .get(&service_name.trim().to_ascii_lowercase())
            .is_some_and(|accounts| accounts.contains(&account_id))
    }
}

fn normalize_service(service_name: &str) -> String {
    service_name.trim().to_ascii_lowercase()
}

/// Escribe/refresca un `granted` con la calidad física entregada y caducidad de
/// 30 días. Una entrega correcta invalida cualquier denegación anterior.
pub async fn record_granted(
    db: &SqlitePool,
    track_id: i64,
    service_name: &str,
    account_id: i64,
    quality: &str,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        r#"
        INSERT INTO download_entitlements (
            track_id, service_name, account_id, state, quality, source,
            last_delivered_at, checked_at, expires_at, error_class
        )
        VALUES (?, ?, ?, 'granted', ?, 'delivered',
                CURRENT_TIMESTAMP, CURRENT_TIMESTAMP, datetime('now', ?), NULL)
        ON CONFLICT(track_id, service_name, account_id) DO UPDATE SET
            state = 'granted',
            quality = excluded.quality,
            source = 'delivered',
            last_delivered_at = CURRENT_TIMESTAMP,
            checked_at = CURRENT_TIMESTAMP,
            expires_at = excluded.expires_at,
            error_class = NULL
        "#,
    )
    .bind(track_id)
    .bind(normalize_service(service_name))
    .bind(account_id)
    .bind(quality.trim())
    .bind(format!("+{GRANTED_TTL_DAYS} days"))
    .execute(db)
    .await?;
    Ok(())
}

/// Escribe/refresca un `denied` de una hora. `error_class` distingue auth de
/// rechazo de calidad sin guardar el mensaje crudo del proveedor.
pub async fn record_denied(
    db: &SqlitePool,
    track_id: i64,
    service_name: &str,
    account_id: i64,
    error_class: &str,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        r#"
        INSERT INTO download_entitlements (
            track_id, service_name, account_id, state, quality, source,
            checked_at, expires_at, error_class
        )
        VALUES (?, ?, ?, 'denied', NULL, 'probe',
                CURRENT_TIMESTAMP, datetime('now', ?), ?)
        ON CONFLICT(track_id, service_name, account_id) DO UPDATE SET
            state = 'denied',
            quality = NULL,
            source = 'probe',
            checked_at = CURRENT_TIMESTAMP,
            expires_at = excluded.expires_at,
            error_class = excluded.error_class
        "#,
    )
    .bind(track_id)
    .bind(normalize_service(service_name))
    .bind(account_id)
    .bind(format!("+{DENIED_TTL_HOURS} hours"))
    .bind(error_class.trim())
    .execute(db)
    .await?;
    Ok(())
}

/// Lee la evidencia vigente de una pista. Las filas caducadas se devuelven como
/// `stale` (nunca como hechos utilizables) y se persisten con ese estado.
pub async fn load_track_evidence(
    db: &SqlitePool,
    track_id: i64,
) -> Result<TrackEvidence, sqlx::Error> {
    expire_stale(db).await?;

    let rows: Vec<(i64, String, String, Option<String>)> = sqlx::query_as(
        r#"
        SELECT account_id, service_name, state, quality
        FROM download_entitlements
        WHERE track_id = ?
          AND state IN ('granted', 'denied')
          AND expires_at IS NOT NULL
          AND datetime(expires_at) > datetime('now')
        "#,
    )
    .bind(track_id)
    .fetch_all(db)
    .await?;

    let mut evidence = TrackEvidence::default();
    for (account_id, service_name, state, quality) in rows {
        let service = normalize_service(&service_name);
        match EntitlementState::from_db(&state) {
            Some(EntitlementState::Granted) => {
                evidence
                    .granted
                    .entry(service)
                    .or_default()
                    .push(account_id);
            }
            Some(EntitlementState::Denied) => {
                evidence.denied.entry(service).or_default().push(account_id);
            }
            _ => {}
        }
        let _ = quality;
    }
    Ok(evidence)
}

/// Marca como `stale` toda evidencia que ya caducó. No borra evidencia vigente
/// ni la denegación que aún no ha expirado.
pub async fn expire_stale(db: &SqlitePool) -> Result<u64, sqlx::Error> {
    let result = sqlx::query(
        r#"
        UPDATE download_entitlements
        SET state = 'stale'
        WHERE state IN ('granted', 'denied')
          AND expires_at IS NOT NULL
          AND datetime(expires_at) <= datetime('now')
        "#,
    )
    .execute(db)
    .await?;
    Ok(result.rows_affected())
}

/// Lee una evidencia concreta (sin filtrar caducidad), para tests y auditoría.
#[cfg(test)]
pub async fn get_evidence(
    db: &SqlitePool,
    track_id: i64,
    service_name: &str,
    account_id: i64,
) -> Result<Option<EntitlementEvidence>, sqlx::Error> {
    let row: Option<(i64, String, String, Option<String>)> = sqlx::query_as(
        "SELECT account_id, service_name, state, quality FROM download_entitlements \
         WHERE track_id = ? AND service_name = ? AND account_id = ?",
    )
    .bind(track_id)
    .bind(normalize_service(service_name))
    .bind(account_id)
    .fetch_optional(db)
    .await?;

    Ok(row.and_then(|(account_id, service_name, state, quality)| {
        EntitlementState::from_db(&state).map(|state| EntitlementEvidence {
            account_id,
            service_name,
            state,
            quality,
        })
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use sqlx::sqlite::SqlitePoolOptions;

    async fn db() -> SqlitePool {
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .unwrap();
        sqlx::migrate!("./migrations").run(&pool).await.unwrap();
        pool
    }

    async fn track(pool: &SqlitePool) -> i64 {
        sqlx::query_scalar("INSERT INTO tracks (title) VALUES ('T') RETURNING id")
            .fetch_one(pool)
            .await
            .unwrap()
    }

    async fn account(pool: &SqlitePool, id: i64) -> i64 {
        sqlx::query(
            "INSERT INTO accounts (id, service_id, display_name, email, is_active) \
             VALUES (?, 2, 'a', 'a@test.invalid', 1)",
        )
        .bind(id)
        .execute(pool)
        .await
        .unwrap();
        id
    }

    #[tokio::test]
    async fn granted_keeps_physical_quality_and_thirty_days() {
        let pool = db().await;
        let track_id = track(&pool).await;
        let account_id = account(&pool, 11).await;

        record_granted(
            &pool,
            track_id,
            "qobuz",
            account_id,
            "FLAC 24-bit / 96.0kHz",
        )
        .await
        .unwrap();
        let evidence = get_evidence(&pool, track_id, "qobuz", account_id)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(evidence.state, EntitlementState::Granted);
        assert_eq!(evidence.quality.as_deref(), Some("FLAC 24-bit / 96.0kHz"));

        let days: f64 = sqlx::query_scalar(
            "SELECT (julianday(expires_at) - julianday('now')) FROM download_entitlements \
             WHERE track_id = ? AND account_id = ?",
        )
        .bind(track_id)
        .bind(account_id)
        .fetch_one(&pool)
        .await
        .unwrap();
        assert!((days - 30.0).abs() < 0.01, "granted lasts 30 days: {days}");
    }

    #[tokio::test]
    async fn denial_lasts_one_hour_and_a_delivery_overwrites_it() {
        let pool = db().await;
        let track_id = track(&pool).await;
        let account_id = account(&pool, 12).await;

        record_denied(&pool, track_id, "tidal", account_id, "auth")
            .await
            .unwrap();
        let hours: f64 = sqlx::query_scalar(
            "SELECT (julianday(expires_at) - julianday('now')) * 24.0 FROM download_entitlements",
        )
        .fetch_one(&pool)
        .await
        .unwrap();
        assert!((hours - 1.0).abs() < 0.01, "denied lasts one hour: {hours}");

        record_granted(
            &pool,
            track_id,
            "tidal",
            account_id,
            "FLAC 16-bit / 44.1kHz",
        )
        .await
        .unwrap();
        let evidence = get_evidence(&pool, track_id, "tidal", account_id)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(evidence.state, EntitlementState::Granted);
    }

    #[tokio::test]
    async fn expired_evidence_becomes_stale_and_is_not_loaded() {
        let pool = db().await;
        let track_id = track(&pool).await;
        let account_id = account(&pool, 13).await;

        record_granted(&pool, track_id, "qobuz", account_id, "FLAC")
            .await
            .unwrap();
        sqlx::query("UPDATE download_entitlements SET expires_at = datetime('now', '-1 minute')")
            .execute(&pool)
            .await
            .unwrap();

        let evidence = load_track_evidence(&pool, track_id).await.unwrap();
        assert!(!evidence.is_granted("qobuz", account_id));
        let stored = get_evidence(&pool, track_id, "qobuz", account_id)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(stored.state, EntitlementState::Stale);
    }
}

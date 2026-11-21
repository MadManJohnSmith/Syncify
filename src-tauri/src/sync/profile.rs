//! Perfil de sincronización resuelto (issue #69, ADR-0006 decisiones 3-5, 7 y 30).
//!
//! Una sola función pura, [`resolve`], combina el default del servicio con el
//! override de la cuenta. El job, el scheduler y la UI consumen el
//! [`SyncProfile`] resultante; nadie mezcla las dos fuentes por su cuenta.
//!
//! Contrato de este corte:
//!
//! - El modo por defecto es `import_only` y el intervalo 24 h (decisiones 1 y
//!   30). El bidireccional existe en el modelo pero no es seleccionable: su
//!   motivo es [`BIDIRECTIONAL_UNAVAILABLE_REASON`] (decisión 18).
//! - El toggle de participación es de la cuenta y gobierna el auto-sync y
//!   «sincronizar todo»; un «sync ahora» por cuenta lo ignora (decisión 4).
//! - El override de cuenta gana al default del servicio en modo, colecciones e
//!   intervalo. Se persiste en la KV de ajustes (`settings`) bajo
//!   `sync_profile:account:{id}` — este carril no abre migración nueva.
//! - Los defaults de colecciones salen de `service_sync_settings`, la misma
//!   superficie que ya edita la ficha del servicio; modo e intervalo de servicio
//!   son constantes del contrato en este corte.

use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

/// Intervalo propuesto por el ADR-0006 (decisión 30), en horas.
pub const DEFAULT_INTERVAL_HOURS: i64 = 24;

/// Cota inferior del intervalo editable: menos de una hora castiga cuotas.
pub const MIN_INTERVAL_HOURS: i64 = 1;

/// Cota superior defensiva: el contrato no propone cadencias mayores a 30 días.
pub const MAX_INTERVAL_HOURS: i64 = 720;

/// Prefijo de la clave de la KV de ajustes donde vive el override de una cuenta.
pub const ACCOUNT_PROFILE_KEY_PREFIX: &str = "sync_profile:account:";

/// Motivo estable que la UI muestra junto al modo no disponible (decisión 18).
pub const BIDIRECTIONAL_UNAVAILABLE_REASON: &str = "write_back_not_available";

/// Modos canónicos del contrato de sincronización (ADR-0006, decisión 1).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SyncMode {
    ImportOnly,
    MirrorQuarantine,
    StrictMirror,
    Bidirectional,
}

impl SyncMode {
    /// Orden en el que la UI ofrece los modos.
    pub const ALL: [SyncMode; 4] = [
        SyncMode::ImportOnly,
        SyncMode::MirrorQuarantine,
        SyncMode::StrictMirror,
        SyncMode::Bidirectional,
    ];

    pub fn as_str(self) -> &'static str {
        match self {
            SyncMode::ImportOnly => "import_only",
            SyncMode::MirrorQuarantine => "mirror_quarantine",
            SyncMode::StrictMirror => "strict_mirror",
            SyncMode::Bidirectional => "bidirectional",
        }
    }

    /// Acepta el literal del contrato; cualquier otra cosa es inválida.
    pub fn parse(raw: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|mode| mode.as_str() == raw)
    }

    /// `false` solo para el modo modelado y pendiente de post-beta (decisión 18).
    pub fn is_selectable(self) -> bool {
        self != SyncMode::Bidirectional
    }

    /// Motivo por el que el modo no se puede elegir, si lo hay.
    pub fn unavailable_reason(self) -> Option<&'static str> {
        match self {
            SyncMode::Bidirectional => Some(BIDIRECTIONAL_UNAVAILABLE_REASON),
            _ => None,
        }
    }
}

/// Colecciones del contrato que el perfil activa por cuenta (decisión 7).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct SyncCollections {
    pub favorites: bool,
    pub playlists: bool,
    pub albums: bool,
    pub artists: bool,
}

impl SyncCollections {
    /// Default de fábrica de las cuatro colecciones del perfil.
    pub const fn all() -> Self {
        Self {
            favorites: true,
            playlists: true,
            albums: true,
            artists: true,
        }
    }
}

/// Origen del valor resuelto: el default del servicio o el override de la cuenta.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ProfileSource {
    Service,
    Account,
}

/// Defaults del servicio que la cuenta hereda.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ServiceSyncDefaults {
    pub service_name: String,
    pub mode: SyncMode,
    pub interval_hours: i64,
    pub collections: SyncCollections,
}

/// Override persistido de una cuenta. Los `None` significan «hereda».
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AccountSyncOverride {
    #[serde(default = "enabled_by_default")]
    pub enabled: bool,
    #[serde(default)]
    pub mode: Option<SyncMode>,
    #[serde(default)]
    pub interval_hours: Option<i64>,
    #[serde(default)]
    pub collections: Option<SyncCollections>,
}

fn enabled_by_default() -> bool {
    true
}

impl Default for AccountSyncOverride {
    fn default() -> Self {
        Self {
            enabled: true,
            mode: None,
            interval_hours: None,
            collections: None,
        }
    }
}

/// Una opción de modo tal como la ofrece la UI, con su disponibilidad y motivo.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SyncModeOption {
    pub value: SyncMode,
    pub enabled: bool,
    pub reason: Option<&'static str>,
}

/// Perfil efectivo que consumen el job, el scheduler y la ficha de la cuenta.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SyncProfile {
    pub account_id: i64,
    pub service_name: String,
    pub enabled: bool,
    pub mode: SyncMode,
    pub mode_source: ProfileSource,
    pub interval_hours: i64,
    pub interval_source: ProfileSource,
    pub collections: SyncCollections,
    pub collections_source: ProfileSource,
    pub modes: Vec<SyncModeOption>,
}

/// Payload de `update_sync_profile`: `None` en un campo significa «hereda del
/// servicio». `enabled` es siempre explícito porque el toggle no tiene default
/// por servicio: la cuenta participa salvo que se apague.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SyncProfileUpdate {
    #[serde(default = "enabled_by_default")]
    pub enabled: bool,
    #[serde(default)]
    pub mode: Option<String>,
    #[serde(default)]
    pub interval_hours: Option<i64>,
    #[serde(default)]
    pub collections: Option<SyncCollections>,
}

fn mode_options() -> Vec<SyncModeOption> {
    SyncMode::ALL
        .into_iter()
        .map(|mode| SyncModeOption {
            value: mode,
            enabled: mode.is_selectable(),
            reason: mode.unavailable_reason(),
        })
        .collect()
}

/// Resolución pura del perfil: el override de la cuenta gana al default del
/// servicio; lo ausente se hereda. Sin I/O.
pub fn resolve(
    account_id: i64,
    service: &ServiceSyncDefaults,
    account: Option<&AccountSyncOverride>,
) -> SyncProfile {
    let mode = account.and_then(|o| o.mode).unwrap_or(service.mode);
    let interval_hours = account
        .and_then(|o| o.interval_hours)
        .unwrap_or(service.interval_hours);
    let collections = account
        .and_then(|o| o.collections)
        .unwrap_or(service.collections);

    SyncProfile {
        account_id,
        service_name: service.service_name.clone(),
        enabled: account.map(|o| o.enabled).unwrap_or(true),
        mode,
        mode_source: if account.and_then(|o| o.mode).is_some() {
            ProfileSource::Account
        } else {
            ProfileSource::Service
        },
        interval_hours,
        interval_source: if account.and_then(|o| o.interval_hours).is_some() {
            ProfileSource::Account
        } else {
            ProfileSource::Service
        },
        collections,
        collections_source: if account.and_then(|o| o.collections).is_some() {
            ProfileSource::Account
        } else {
            ProfileSource::Service
        },
        modes: mode_options(),
    }
}

/// Clave de la KV de ajustes para el override de una cuenta.
pub fn account_profile_key(account_id: i64) -> String {
    format!("{}{}", ACCOUNT_PROFILE_KEY_PREFIX, account_id)
}

async fn account_context(db: &sqlx::SqlitePool, account_id: i64) -> Result<String, String> {
    let row: Option<(String,)> = sqlx::query_as(
        "SELECT LOWER(s.name) FROM accounts a JOIN services s ON s.id = a.service_id WHERE a.id = ?",
    )
    .bind(account_id)
    .fetch_optional(db)
    .await
    .map_err(|e| format!("SyncProfileLookup: {}", e))?;

    row.map(|(service,)| service)
        .ok_or_else(|| format!("SyncProfileNotFound: account {} does not exist", account_id))
}

/// Defaults del servicio: colecciones de `service_sync_settings` y las
/// constantes del contrato para modo e intervalo.
pub async fn load_service_defaults(
    db: &sqlx::SqlitePool,
    service_name: &str,
) -> Result<ServiceSyncDefaults, String> {
    let row: Option<(bool, bool, bool, bool)> = sqlx::query_as(
        r#"SELECT IFNULL(sync_favorites, 1),
                  IFNULL(sync_playlists, 1),
                  IFNULL(sync_albums, 1),
                  IFNULL(sync_favorite_artists, 1)
           FROM service_sync_settings
           WHERE LOWER(service_name) = LOWER(?)"#,
    )
    .bind(service_name)
    .fetch_optional(db)
    .await
    .map_err(|e| format!("SyncProfileLookup: {}", e))?;

    let collections = match row {
        Some((favorites, playlists, albums, artists)) => SyncCollections {
            favorites,
            playlists,
            albums,
            artists,
        },
        // Servicio sin fila (Apple Music): hereda el Default del contrato de
        // importación, que ya activa las cuatro colecciones.
        None => SyncCollections::all(),
    };

    Ok(ServiceSyncDefaults {
        service_name: service_name.to_ascii_lowercase(),
        mode: SyncMode::ImportOnly,
        interval_hours: DEFAULT_INTERVAL_HOURS,
        collections,
    })
}

/// Override persistido de la cuenta, si lo hay. Un JSON corrupto se propaga:
/// nunca se sustituye en silencio por los defaults.
pub async fn load_account_override(
    db: &sqlx::SqlitePool,
    account_id: i64,
) -> Result<Option<AccountSyncOverride>, String> {
    let key = account_profile_key(account_id);
    let row: Option<(String,)> = sqlx::query_as("SELECT value FROM settings WHERE key = ?")
        .bind(&key)
        .fetch_optional(db)
        .await
        .map_err(|e| format!("SyncProfileLookup: {}", e))?;

    match row {
        Some((raw,)) => serde_json::from_str(&raw).map(Some).map_err(|e| {
            format!(
                "SyncProfileInvalid: account {} override is not valid JSON: {}",
                account_id, e
            )
        }),
        None => Ok(None),
    }
}

/// Perfil efectivo de la cuenta.
pub async fn load_sync_profile(
    db: &sqlx::SqlitePool,
    account_id: i64,
) -> Result<SyncProfile, String> {
    let service_name = account_context(db, account_id).await?;
    let service = load_service_defaults(db, &service_name).await?;
    let account = load_account_override(db, account_id).await?;
    Ok(resolve(account_id, &service, account.as_ref()))
}

/// `true` si la cuenta participa en auto-sync y «sincronizar todo». Una cuenta
/// sin override participa (decisión 4).
pub async fn is_account_participating(
    db: &sqlx::SqlitePool,
    account_id: i64,
) -> Result<bool, String> {
    Ok(load_account_override(db, account_id)
        .await?
        .map(|account| account.enabled)
        .unwrap_or(true))
}

/// Servicios con al menos una cuenta activa que participa. Es el universo de
/// «sincronizar todo» y del auto-sync; un «sync ahora» no pasa por aquí.
pub async fn active_participating_services(db: &sqlx::SqlitePool) -> Result<Vec<String>, String> {
    let rows: Vec<(i64, String)> = sqlx::query_as(
        r#"SELECT a.id, LOWER(s.name)
           FROM accounts a
           JOIN services s ON s.id = a.service_id
           WHERE a.is_active = 1
           ORDER BY 2, 1"#,
    )
    .fetch_all(db)
    .await
    .map_err(|e| format!("SyncProfileLookup: {}", e))?;

    let mut services = BTreeSet::new();
    for (account_id, service_name) in rows {
        if is_account_participating(db, account_id).await? {
            services.insert(service_name);
        }
    }
    Ok(services.into_iter().collect())
}

/// Convierte el payload de la UI en el override a persistir. Valida el modo y
/// el intervalo, rechaza el bidireccional y colapsa a «hereda» lo que ya es
/// igual al default del servicio (para que la fuente del valor siga siendo
/// visible en la ficha).
fn normalize_update(
    service: &ServiceSyncDefaults,
    update: &SyncProfileUpdate,
) -> Result<AccountSyncOverride, String> {
    let mode = match update.mode.as_deref() {
        None => None,
        Some(raw) => {
            let parsed = SyncMode::parse(raw)
                .ok_or_else(|| format!("SyncProfileInvalid: unknown sync mode '{}'", raw))?;
            if let Some(reason) = parsed.unavailable_reason() {
                return Err(format!(
                    "SyncProfileUnsupported: mode '{}' is not available ({})",
                    raw, reason
                ));
            }
            (parsed != service.mode).then_some(parsed)
        }
    };

    let interval_hours = match update.interval_hours {
        None => None,
        Some(hours) if (MIN_INTERVAL_HOURS..=MAX_INTERVAL_HOURS).contains(&hours) => {
            (hours != service.interval_hours).then_some(hours)
        }
        Some(hours) => {
            return Err(format!(
                "SyncProfileInvalid: interval_hours {} is outside {}..={}",
                hours, MIN_INTERVAL_HOURS, MAX_INTERVAL_HOURS
            ))
        }
    };

    let collections = match update.collections {
        None => None,
        Some(collections) if collections == service.collections => None,
        Some(collections) => Some(collections),
    };

    Ok(AccountSyncOverride {
        enabled: update.enabled,
        mode,
        interval_hours,
        collections,
    })
}

/// Persiste el override y devuelve el perfil ya resuelto. Si todo vuelve a los
/// defaults del servicio y la cuenta participa, la fila se elimina: herencia
/// limpia es ausencia de override.
pub async fn save_account_profile(
    db: &sqlx::SqlitePool,
    account_id: i64,
    update: &SyncProfileUpdate,
) -> Result<SyncProfile, String> {
    let service_name = account_context(db, account_id).await?;
    let service = load_service_defaults(db, &service_name).await?;
    let account = normalize_update(&service, update)?;
    let key = account_profile_key(account_id);

    let inherits_everything = account.enabled
        && account.mode.is_none()
        && account.interval_hours.is_none()
        && account.collections.is_none();

    if inherits_everything {
        sqlx::query("DELETE FROM settings WHERE key = ?")
            .bind(&key)
            .execute(db)
            .await
            .map_err(|e| format!("SyncProfilePersist: {}", e))?;
    } else {
        let value =
            serde_json::to_string(&account).map_err(|e| format!("SyncProfilePersist: {}", e))?;
        sqlx::query(
            "INSERT INTO settings (key, value, updated_at)
             VALUES (?, ?, datetime('now'))
             ON CONFLICT(key) DO UPDATE SET value = excluded.value, updated_at = excluded.updated_at",
        )
        .bind(&key)
        .bind(&value)
        .execute(db)
        .await
        .map_err(|e| format!("SyncProfilePersist: {}", e))?;
    }

    load_sync_profile(db, account_id).await
}

/// Borra el override al eliminar la cuenta: la KV no tiene FK que lo haga.
pub async fn delete_account_profile<'e, E>(executor: E, account_id: i64) -> Result<(), String>
where
    E: sqlx::Executor<'e, Database = sqlx::Sqlite>,
{
    sqlx::query("DELETE FROM settings WHERE key = ?")
        .bind(account_profile_key(account_id))
        .execute(executor)
        .await
        .map_err(|e| format!("SyncProfilePersist: {}", e))?;
    Ok(())
}

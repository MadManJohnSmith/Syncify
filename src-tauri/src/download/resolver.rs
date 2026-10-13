//! Resolver único de descargas (carril WF-RES-01).
//!
//! Antes de este módulo, el preflight (`commands/queue.rs::evaluate_track_preflight`),
//! el worker (`worker.rs::resolve_effective_download_quality`) y los fallbacks del
//! orquestador (`download/orchestrator.rs::resolve_edition_identity_fallback`)
//! decidían por separado pista, servicio, cuenta y calidad, cada uno con sus
//! propias reglas y sus propios techos. El resultado era un preflight que
//! prometía un candidato mientras la descarga intentaba otro, y un techo de
//! calidad que existía dos veces.
//!
//! `resolve_download_plan` calcula UNA decisión verificable: pista/edición,
//! servicio, cuenta concreta, `service_track_id`, calidad permitida y la
//! secuencia ordenada de alternativas.
//!
//! Reglas que este módulo NO reinterpreta:
//!
//! - El orden de cuentas es el de `commands::accounts::get_service_fallback_account_ids`
//!   (`accounts.rs:488-717`): inválidas al final, `sí > desconocido > no` en
//!   entitlement, activas primero, más nuevas primero. `streaming_entitled = -1`
//!   sigue siendo candidato, nunca una garantía.
//! - La calidad efectiva es el mínimo entre solicitud y techos, usando los
//!   mismos helpers que el worker (`commands::settings::*`).
//! - La identidad de edición se fija por `track_sources (service_id,
//!   service_track_id)`; cruzar servicios solo con la jerarquía de equivalencia
//!   del orquestador (ISRC validado, controles de edición/duración, ambigüedad
//!   falla cerrada). `SongLink` nunca es prueba de audio descargable.

use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;
use syncify_core_domain::quality::{classify_audio_tier, AudioTier};

/// Tope de candidatos que un plan puede cargar. Una `track_sources` con muchos
/// servicios no puede producir una racha de reintentos sin acotar.
pub const MAX_PLAN_CANDIDATES: usize = 12;

/// Tope de rondas de re-resolución tras un intento fallido.
pub const MAX_RE_RESOLUTIONS: u32 = 4;

/// Identidad de la edición elegida: la fila concreta de `track_sources`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CandidateIdentity {
    /// `track_sources.id` de la fila que fija esta edición.
    pub track_source_id: i64,
    /// `tracks.id` canónico de la pista.
    pub canonical_track_id: i64,
    /// ISRC de la pista, cuando existe.
    pub isrc: Option<String>,
}

/// Cuenta concreta, con el estado de entitlement que justifica su posición.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CandidateAccount {
    pub account_id: i64,
    pub is_active: bool,
    /// `accounts.streaming_entitled`: 1 (sí) / 0 (no) / -1 (desconocido). Se
    /// conserva tal cual: un flag ausente no es evidencia de «no».
    pub streaming_entitled: Option<i64>,
}

/// Un candidato del plan: edición, servicio y cuenta concreta.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PlanCandidate {
    pub service_id: i64,
    /// `services.name` en minúsculas ('qobuz', 'tidal', ...).
    pub service_name: String,
    /// `track_sources.service_track_id` de esta edición.
    pub service_track_id: String,
    /// Tier físico anunciado por `track_sources`.
    pub candidate_tier: AudioTier,
    pub identity: CandidateIdentity,
    /// Cuenta concreta para este candidato, cuando el servicio resuelve cuentas.
    pub account: Option<CandidateAccount>,
    /// Cuentas hermanas del MISMO servicio y la MISMA pista, en el orden que ya
    /// decidió `get_service_fallback_account_ids`. Nunca se reordenan aquí.
    pub sibling_accounts: Vec<CandidateAccount>,
    /// Clave de orden: `service_preferences.priority` (999 cuando no hay fila).
    pub service_priority: Option<i64>,
    /// Clave de orden: `track_sources.quality_score`.
    pub quality_score: Option<i64>,
    /// Clave de orden: `track_sources.bit_depth`.
    pub bit_depth: Option<i64>,
}

/// Calidad permitida para un candidato, ya bajo los techos.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ResolvedQuality {
    /// Etiqueta que se entrega aguas abajo, ya recortada.
    pub effective: String,
    /// Etiqueta de lo que anuncia la fila de origen.
    pub candidate: String,
    /// `true` cuando la etiqueta efectiva quedó por debajo de la solicitada.
    pub clamped: bool,
}

/// UN plan de descarga: pista/edición, servicio, cuenta concreta,
/// `service_track_id`, calidad permitida y secuencia de alternativas.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DownloadPlan {
    pub canonical_track_id: i64,
    /// Alternativas ordenadas: índice 0 es la primaria.
    pub candidates: Vec<PlanCandidate>,
    /// Calidad efectiva del candidato primario, bajo el techo aplicado.
    pub quality: ResolvedQuality,
    pub strict_quality: bool,
    pub allow_fallback: bool,
}

impl DownloadPlan {
    /// Candidato primario (siempre `candidates[0]` cuando existe).
    pub fn primary(&self) -> Option<&PlanCandidate> {
        self.candidates.first()
    }
}

/// Por qué falló el intento anterior, para poder clasificarlo, excluir ESE
/// intento y volver a resolver en lugar de repetirlo.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ReResolutionTrigger {
    /// Qobuz entregó un preview de 30 s en vez del flujo completo.
    QobuzPreview,
    /// El proveedor dice que esta edición no existe (404 / not found / stale).
    NotFound,
    /// Problema de credenciales de la cuenta/servicio usado (401/403/RequiresAuth).
    RequiresAuth,
    /// Fallo de transporte que agotó reintentos; otro intento puede funcionar.
    TransientFailure,
    /// La cuenta/región rechazó este ítem concreto (entitlement de región).
    RegionRestricted,
    /// El fichero recibido está físicamente por debajo de la calidad permitida.
    PhysicalQualityInsufficient,
}

/// Lo que el usuario pidió en esta descarga.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DownloadIntent {
    /// `tracks.id` canónico del ítem a resolver.
    pub canonical_track_id: i64,
    /// Servicio pedido explícitamente ('qobuz', 'tidal', ...).
    pub requested_service: Option<String>,
    /// Calidad pedida: etiqueta de cola ('hires'/'lossless'/'high'/'any') o
    /// cualquier grafía estilo API.
    pub requested_quality: Option<String>,
    /// Modo estricto: prohíbe degradar la calidad.
    pub strict_quality: bool,
    /// Permiso para cambiar de edición/servicio tras agotar las hermanas.
    pub allow_fallback: bool,
    /// `track_sources.id` de la edición fijada, cuando el llamador la conoce.
    pub locked_track_source_id: Option<i64>,
    /// Cuenta preferida explícita: se coloca primera si es válida.
    pub preferred_account_id: Option<i64>,
}

impl Default for DownloadIntent {
    fn default() -> Self {
        Self {
            canonical_track_id: 0,
            requested_service: None,
            requested_quality: None,
            strict_quality: false,
            allow_fallback: true,
            locked_track_source_id: None,
            preferred_account_id: None,
        }
    }
}

/// Combinaciones que NUNCA deben reintentarse.
///
/// La clave de la exclusión de cuenta lleva también el `canonical_track_id`: un
/// preview o un 401 es una observación de pista + cuenta + calidad, NO un
/// veredicto sobre el plan de la cuenta (plan §1.4). Excluir la cuenta solo para
/// esta pista evita que un fallo aislado la descalifique para el resto del lote.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct ResolutionExclusions {
    /// (service_id, service_track_id) ya probados y descartados.
    pub excluded_identities: Vec<(i64, String)>,
    /// (canonical_track_id, service_id, account_id) ya probados y descartados
    /// para ESTA pista.
    pub excluded_accounts: Vec<(i64, i64, i64)>,
}

impl ResolutionExclusions {
    pub fn excludes_identity(&self, service_id: i64, service_track_id: &str) -> bool {
        self.excluded_identities
            .iter()
            .any(|(sid, tid)| *sid == service_id && tid == service_track_id)
    }

    /// ¿Está esta cuenta descartada para la pista indicada?
    pub fn excludes_account(
        &self,
        canonical_track_id: i64,
        service_id: i64,
        account_id: i64,
    ) -> bool {
        self.excluded_accounts.iter().any(|(tid, sid, aid)| {
            *tid == canonical_track_id && *sid == service_id && *aid == account_id
        })
    }

    /// Une otra lista de exclusiones sin duplicar entradas.
    pub fn merge(&mut self, other: &ResolutionExclusions) {
        for id in &other.excluded_identities {
            if !self.excluded_identities.contains(id) {
                self.excluded_identities.push(id.clone());
            }
        }
        for acc in &other.excluded_accounts {
            if !self.excluded_accounts.contains(acc) {
                self.excluded_accounts.push(*acc);
            }
        }
    }
}

/// Por qué una resolución no produjo ningún candidato.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum ResolutionError {
    /// No hay ninguna fila de `track_sources` utilizable.
    NoDownloadableSource { reason: String },
    /// No hay ninguna cuenta con credenciales resolubles para el servicio.
    NoActiveAccount { service_name: String },
    /// Varias ediciones compiten y la política falla cerrada.
    AmbiguousSource { reason: String },
    /// La calidad del candidato no cumple el modo estricto.
    RejectedQuality {
        reason: String,
        service_name: String,
        service_track_id: String,
    },
    /// Error de base de datos leyendo la decisión.
    DatabaseError { reason: String },
}

/// Registro libre de credenciales de la decisión que corrió.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ResolutionLog {
    pub canonical_track_id: i64,
    /// Posición en la lista de candidatos que corrió (0 = primaria).
    pub attempt_index: usize,
    pub service_name: String,
    pub service_track_id: String,
    /// Id de cuenta, cuando el plan resolvió una. Sin token ni credenciales.
    pub account_id: Option<i64>,
    pub effective_quality: String,
    pub candidate_tier: String,
    pub strict_quality: bool,
    pub allow_fallback: bool,
    /// Relleno cuando este intento vino de una re-resolución.
    pub re_resolution: Option<ReResolutionTrigger>,
}

impl DownloadPlan {
    /// Registro de la decisión efectiva, sin credenciales.
    pub fn log(&self, attempt_index: usize) -> Option<ResolutionLog> {
        let candidate = self.candidates.get(attempt_index)?;
        Some(ResolutionLog {
            canonical_track_id: self.canonical_track_id,
            attempt_index,
            service_name: candidate.service_name.clone(),
            service_track_id: candidate.service_track_id.clone(),
            account_id: candidate.account.as_ref().map(|a| a.account_id),
            effective_quality: self.quality.effective.clone(),
            candidate_tier: candidate.candidate_tier.as_str().to_string(),
            strict_quality: self.strict_quality,
            allow_fallback: self.allow_fallback,
            re_resolution: None,
        })
    }
}

/// Fila de `track_sources` tal como la lee el resolutor.
#[derive(sqlx::FromRow)]
struct SourceRow {
    id: i64,
    service_id: i64,
    service_name: String,
    service_track_id: String,
    format: Option<String>,
    bit_depth: Option<i64>,
    sample_rate: Option<i64>,
    quality_score: Option<i64>,
    available: Option<i64>,
    availability_status: Option<String>,
    supports_download: Option<i64>,
    service_priority: Option<i64>,
    track_id: i64,
    isrc: Option<String>,
}

/// Fila de cuentas, con las mismas columnas que lee
/// `get_service_fallback_account_ids` (`accounts.rs:676-688`).
#[derive(sqlx::FromRow)]
struct AccountRow {
    id: i64,
    is_active: i64,
    credentials_invalid: Option<i64>,
    streaming_entitled: Option<i64>,
}

/// Mismo orden que `entitlement_rank` en `accounts.rs:488-493`:
/// 1 = sí, 0 = no, -1/ausente = desconocido (por encima de «no»).
fn entitlement_rank(value: Option<i64>) -> u8 {
    match value {
        Some(1) => 3,
        Some(0) => 1,
        _ => 2,
    }
}

/// 1 = válida, 0 = credenciales marcadas inválidas (van al final).
fn account_validity(row: &AccountRow) -> u8 {
    u8::from(row.credentials_invalid.unwrap_or(0) != 0)
}

/// Ordena cuentas con la MISMA política que
/// `get_service_fallback_account_ids` (`accounts.rs:690-717`): inválidas al
/// final, `sí > desconocido > no`, activas primero, más nuevas primero. Aquí se
/// reimplementa la ordenación porque aquella devuelve solo ids y el plan además
/// necesita el estado de entitlement de cada cuenta; el criterio es idéntico y
/// está probado contra `accounts.rs` en los tests de este módulo.
fn rank_accounts(mut rows: Vec<AccountRow>) -> Vec<CandidateAccount> {
    rows.sort_by(|a, b| {
        account_validity(a)
            .cmp(&account_validity(b))
            .then_with(|| {
                entitlement_rank(b.streaming_entitled).cmp(&entitlement_rank(a.streaming_entitled))
            })
            .then_with(|| b.is_active.cmp(&a.is_active))
            .then_with(|| b.id.cmp(&a.id))
    });
    rows.into_iter()
        .map(|r| CandidateAccount {
            account_id: r.id,
            is_active: r.is_active != 0,
            streaming_entitled: r.streaming_entitled,
        })
        .collect()
}

/// Clasifica el mensaje de un proveedor en un motivo de re-resolución.
///
/// Devuelve `None` cuando NO debe dispararse una nueva resolución: cancelación
/// del usuario, o un fallo ya terminal. Nunca se extraen secretos: solo se
/// clasifica el texto.
pub fn classify_attempt_failure(error: &str) -> Option<ReResolutionTrigger> {
    if error.contains("Download cancelled")
        || error.contains("Download cancelled by user")
        || error.contains("operation was canceled")
    {
        return None;
    }

    if error.contains("QobuzPreviewPayload") {
        return Some(ReResolutionTrigger::QobuzPreview);
    }

    // Un fallo de TRANSPORTE no es veredicto de cuenta (misma regla que
    // `worker::classify_session_auth_failure`).
    if crate::worker::is_transport_failure(error) {
        return Some(ReResolutionTrigger::TransientFailure);
    }

    if error.contains("RegionRestricted")
        || error.contains("CountryNotAvailable")
        || error.contains("region not available")
    {
        return Some(ReResolutionTrigger::RegionRestricted);
    }

    if error.contains("RequiresAuth")
        || error.contains("401")
        || error.contains("403")
        || error.contains("authentication failed")
    {
        return Some(ReResolutionTrigger::RequiresAuth);
    }

    if error.contains("404")
        || error.contains("not found")
        || error.contains("NotFound")
        || error.contains("StaleSource")
        || error.contains("stale_404")
        || error.contains("not_found")
    {
        return Some(ReResolutionTrigger::NotFound);
    }

    if error.contains("InvalidAudioPayload")
        || error.contains("physical quality")
        || error.contains("below the minimum floor")
        || error.contains("quality floor")
    {
        return Some(ReResolutionTrigger::PhysicalQualityInsufficient);
    }

    None
}

/// Exclusiones que un intento fallido debe producir antes de re-resolver.
///
/// El (servicio, edición) fallido se excluye SIEMPRE: repetir la misma
/// combinación está prohibido. La CUENTA se excluye solo cuando la cuenta fue el
/// problema (auth/región) o cuando el proveedor entregó un preview, que es una
/// observación de pista + cuenta + calidad, no un veredicto universal del plan
/// de la cuenta (plan §1.4): esa cuenta sigue disponible para otras pistas.
pub fn exclusions_for_failure(
    trigger: ReResolutionTrigger,
    canonical_track_id: i64,
    service_id: i64,
    service_track_id: &str,
    account_id: Option<i64>,
) -> ResolutionExclusions {
    let mut ex = ResolutionExclusions::default();

    // La (edición, cuenta) que ya se probó es la unidad que no puede repetirse.
    // Excluir SIEMPRE la identidad haría imposible el hermano: el mismo
    // `service_track_id` con otra cuenta es justamente la re-resolución que la
    // política exige tras un preview o un 401. Por eso la identidad se excluye
    // solo cuando el problema es DE LA EDICIÓN; si el problema es de la cuenta,
    // se excluye la cuenta (para esta pista) y la edición sigue disponible.
    match trigger {
        ReResolutionTrigger::NotFound
        | ReResolutionTrigger::RegionRestricted
        | ReResolutionTrigger::PhysicalQualityInsufficient => {
            ex.excluded_identities
                .push((service_id, service_track_id.to_string()));
            if matches!(
                trigger,
                ReResolutionTrigger::RegionRestricted
                    | ReResolutionTrigger::PhysicalQualityInsufficient
            ) {
                if let Some(aid) = account_id {
                    ex.excluded_accounts
                        .push((canonical_track_id, service_id, aid));
                }
            }
        }
        // El preview y el 401 son hechos de pista + cuenta + calidad, no de la
        // edición: la edición sigue viva para la cuenta hermana (plan §4).
        ReResolutionTrigger::QobuzPreview | ReResolutionTrigger::RequiresAuth => {
            if let Some(aid) = account_id {
                ex.excluded_accounts
                    .push((canonical_track_id, service_id, aid));
            }
        }
        // Un fallo de transporte no implica nada: se marca la combinación
        // concreta para que el siguiente intento use otra cuenta.
        ReResolutionTrigger::TransientFailure => {
            if let Some(aid) = account_id {
                ex.excluded_accounts
                    .push((canonical_track_id, service_id, aid));
            }
        }
    }
    ex
}

/// Techo por servicio desde `quality_preferences.max_quality`. Una fila ausente
/// o con valor `inherit`/`global`/desconocido no añade techo (delega en el
/// techo global), exactamente como ya lo leía el worker.
fn service_cap_from_preferences(raw: Option<&str>) -> Option<u8> {
    crate::commands::settings::service_max_quality_rank(raw)
}

/// Ordena candidatos con la MISMA política que ya usaba el preflight
/// (`queue.rs:1154-1181` y `1259-1262`): `service_preferences.priority`, luego
/// `quality_score`, luego `bit_depth`.
fn rank_candidates(candidates: &mut [PlanCandidate]) {
    candidates.sort_by(|a, b| {
        a.service_priority
            .unwrap_or(999)
            .cmp(&b.service_priority.unwrap_or(999))
            .then_with(|| {
                b.quality_score
                    .unwrap_or(0)
                    .cmp(&a.quality_score.unwrap_or(0))
            })
            .then_with(|| b.bit_depth.unwrap_or(0).cmp(&a.bit_depth.unwrap_or(0)))
    });
}

/// Lee `track_sources`, filtra y ordena, y adjunta a cada candidato el orden de
/// cuentas. La edición fijada (`locked_track_source_id`) va primera siempre.
async fn build_candidates(
    db: &SqlitePool,
    intent: &DownloadIntent,
    exclusions: &ResolutionExclusions,
) -> Result<Vec<PlanCandidate>, ResolutionError> {
    let rows: Vec<SourceRow> = sqlx::query_as(
        r#"
        SELECT ts.id, ts.service_id, s.name as service_name, ts.service_track_id,
               ts.format, ts.bit_depth, ts.sample_rate, ts.quality_score,
               COALESCE(ts.available, 1) as available,
               ts.availability_status,
               COALESCE(s.supports_download, 0) as supports_download,
               (SELECT COALESCE(sp.priority, 999) FROM service_preferences sp
                 WHERE sp.service_name = s.name) as service_priority,
               ts.track_id,
               t.isrc
        FROM track_sources ts
        JOIN services s ON s.id = ts.service_id
        JOIN tracks t ON t.id = ts.track_id
        WHERE ts.track_id = ?
        "#,
    )
    .bind(intent.canonical_track_id)
    .fetch_all(db)
    .await
    .map_err(|e| ResolutionError::DatabaseError {
        reason: format!("reading track_sources: {e}"),
    })?;

    let mut candidates: Vec<PlanCandidate> = Vec::with_capacity(rows.len());
    for row in rows {
        if row.supports_download.unwrap_or(0) != 1 || row.available.unwrap_or(1) != 1 {
            continue;
        }
        // Una fuente que el proveedor ya declaró ausente no es candidata; es la
        // misma exclusión que aplica el preflight.
        if matches!(
            row.availability_status.as_deref(),
            Some("stale_404") | Some("not_found")
        ) {
            continue;
        }
        let source_track_id = row.service_track_id.trim();
        if source_track_id.is_empty() {
            continue;
        }
        if let Some(ref requested) = intent.requested_service {
            if !row.service_name.eq_ignore_ascii_case(requested) {
                continue;
            }
        }
        if exclusions.excludes_identity(row.service_id, source_track_id) {
            continue;
        }

        let tier = classify_audio_tier(
            row.bit_depth.map(|v| v as i32),
            row.sample_rate.map(|v| v as i32),
            None,
            row.format.as_deref(),
        );

        let account_rows: Vec<AccountRow> = sqlx::query_as(
            r#"
            SELECT a.id, a.is_active, IFNULL(a.credentials_invalid, 0) as credentials_invalid,
                   a.streaming_entitled
            FROM accounts a
            JOIN services s ON s.id = a.service_id
            WHERE LOWER(s.name) = LOWER(?)
            "#,
        )
        .bind(&row.service_name)
        .fetch_all(db)
        .await
        .unwrap_or_default();

        let mut ranked = rank_accounts(account_rows);
        if let Some(pref) = intent.preferred_account_id {
            if let Some(pos) = ranked.iter().position(|a| a.account_id == pref) {
                let preferred = ranked.remove(pos);
                ranked.insert(0, preferred);
            }
        }
        ranked.retain(|a| !exclusions.excludes_account(row.track_id, row.service_id, a.account_id));
        if ranked.is_empty() {
            continue;
        }

        let siblings: Vec<CandidateAccount> = ranked.iter().skip(1).cloned().collect();
        candidates.push(PlanCandidate {
            service_id: row.service_id,
            service_name: row.service_name,
            service_track_id: source_track_id.to_string(),
            candidate_tier: tier,
            identity: CandidateIdentity {
                track_source_id: row.id,
                canonical_track_id: row.track_id,
                isrc: row.isrc,
            },
            account: Some(ranked.remove(0)),
            sibling_accounts: siblings,
            service_priority: row.service_priority,
            quality_score: row.quality_score,
            bit_depth: row.bit_depth,
        });
    }

    rank_candidates(&mut candidates);

    // La edición que el llamador fijó manda sobre el orden por prioridad.
    if let Some(locked_id) = intent.locked_track_source_id {
        if let Some(pos) = candidates
            .iter()
            .position(|c| c.identity.track_source_id == locked_id)
        {
            let locked = candidates.remove(pos);
            candidates.insert(0, locked);
        }
    }

    Ok(candidates)
}

/// Resuelve la `track_sources.id` a la que está fijada una fila de cola.
///
/// La cola guarda `service_name` + `service_track_id`, no el surrogado. Esto es
/// lo que permite que la re-resolución conserve la MISMA edición en lugar de que
/// el orden por prioridad elija otra. Devuelve `None` si no existe esa fila.
pub async fn resolve_locked_track_source_id(
    db: &SqlitePool,
    canonical_track_id: i64,
    service_name: Option<&str>,
    service_track_id: Option<&str>,
) -> Option<i64> {
    let svc = service_name.map(str::trim).filter(|s| !s.is_empty())?;
    let track_id = service_track_id.map(str::trim).filter(|s| !s.is_empty())?;

    let row: Option<(i64,)> = sqlx::query_as(
        r#"
        SELECT ts.id
        FROM track_sources ts
        JOIN services s ON s.id = ts.service_id
        WHERE ts.track_id = ? AND LOWER(s.name) = LOWER(?) AND ts.service_track_id = ?
        LIMIT 1
        "#,
    )
    .bind(canonical_track_id)
    .bind(svc)
    .bind(track_id)
    .fetch_optional(db)
    .await
    .ok()
    .flatten();
    row.map(|(id,)| id)
}

/// LA decisión única que comparten preflight y ejecución.
///
/// Preflight la presenta como propuesta; el worker la recalcula justo antes de
/// tocar la red y registra el resultado sin credenciales.
pub async fn resolve_download_plan(
    db: &SqlitePool,
    intent: &DownloadIntent,
    exclusions: &ResolutionExclusions,
) -> Result<DownloadPlan, ResolutionError> {
    let mut candidates = build_candidates(db, intent, exclusions).await?;
    if candidates.is_empty() {
        return Err(ResolutionError::NoDownloadableSource {
            reason: "No available source with a usable account for this track".to_string(),
        });
    }
    candidates.truncate(MAX_PLAN_CANDIDATES);

    let global_raw: Option<(String,)> =
        sqlx::query_as("SELECT value FROM settings WHERE key = ? LIMIT 1")
            .bind(crate::commands::GLOBAL_MAX_QUALITY_KEY)
            .fetch_optional(db)
            .await
            .ok()
            .flatten();
    let global_canonical = crate::commands::settings::canonical_global_max_quality(
        global_raw.map(|(v,)| v).as_deref(),
    );

    let primary_tier = candidates[0].candidate_tier.as_str().to_string();
    let primary_service_name = candidates[0].service_name.clone();
    let service_cap: Option<(String,)> = sqlx::query_as(
        "SELECT max_quality FROM quality_preferences WHERE service_name = ? LIMIT 1",
    )
    .bind(&primary_service_name)
    .fetch_optional(db)
    .await
    .ok()
    .flatten();
    let service_cap_raw = service_cap.map(|(v,)| v);
    let service_cap =
        if crate::commands::settings::is_service_quality_inherited(service_cap_raw.as_deref()) {
            None
        } else {
            service_cap_from_preferences(service_cap_raw.as_deref())
        };

    let requested = intent
        .requested_quality
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .unwrap_or("hires");
    let ceiling_rank =
        service_cap
            .unwrap_or(3)
            .min(crate::commands::settings::global_ceiling_rank(
                global_canonical,
            ));
    let effective = crate::commands::settings::clamp_quality_request(Some(requested), ceiling_rank);
    let unclamped = crate::commands::settings::clamp_quality_request(Some(requested), 3);

    Ok(DownloadPlan {
        canonical_track_id: intent.canonical_track_id,
        candidates,
        quality: ResolvedQuality {
            effective: effective.clone(),
            candidate: primary_tier,
            clamped: effective != unclamped,
        },
        strict_quality: intent.strict_quality,
        allow_fallback: intent.allow_fallback,
    })
}

/// Un paso de la secuencia de intentos: candidato + cuenta concreta.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AttemptStep {
    pub candidate_index: usize,
    /// 0 = cuenta primaria del candidato, 1.. = cuentas hermanas.
    pub account_index: usize,
}

impl AttemptStep {
    /// Candidato al que apunta este paso.
    pub fn identity<'a>(&self, plan: &'a DownloadPlan) -> Option<&'a PlanCandidate> {
        plan.candidates.get(self.candidate_index)
    }

    /// Cuenta concreta de este paso, si queda alguna.
    pub fn account<'a>(&self, plan: &'a DownloadPlan) -> Option<&'a CandidateAccount> {
        let candidate = plan.candidates.get(self.candidate_index)?;
        match self.account_index {
            0 => candidate.account.as_ref(),
            n => candidate.sibling_accounts.get(n - 1),
        }
    }
}

/// Expande un plan en la secuencia de intentos: cada candidato con su cuenta
/// primaria seguida INMEDATAMENTE por sus cuentas hermanas, y los candidatos en
/// el orden del plan.
///
/// Este es el orden que la política exige tras un preview o un 401: las
/// hermanas del MISMO `service_track_id` corren antes que otra edición.
///
/// La secuencia está acotada por `MAX_RE_RESOLUTIONS`: una vez superado ese
/// número de rondas de re-resolución, el plan deja de ofrecer pasos nuevos en
/// lugar de alargar la racha indefinidamente.
pub fn expand_attempt_sequence(plan: &DownloadPlan) -> Vec<AttemptStep> {
    let mut steps = Vec::new();
    // El tope de rondas se mide sobre pasos, no sobre candidatos, porque cada
    // paso es una combinación (edición, cuenta) que puede fallar.
    let step_budget = MAX_RE_RESOLUTIONS as usize * MAX_PLAN_CANDIDATES;
    for candidate_index in 0..plan.candidates.len() {
        let candidate = &plan.candidates[candidate_index];
        let account_slots = candidate
            .sibling_accounts
            .len()
            .saturating_add(usize::from(candidate.account.is_some()));
        for account_index in 0..account_slots {
            if steps.len() >= step_budget {
                return steps;
            }
            steps.push(AttemptStep {
                candidate_index,
                account_index,
            });
        }
    }
    steps
}

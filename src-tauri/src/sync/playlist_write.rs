//! Gateway de escritura de playlists para Qobuz y Tidal (PL06, #159).
//!
//! El seam público es [`PlaylistWriteGateway`]: valida la petición, consulta las
//! capacidades declaradas por el proveedor y delega en [`PlaylistWriteProvider`].
//! Los ids son cadenas opacas dentro de un proveedor; el gateway no los mezcla
//! con ids locales ni con ids de otro servicio.
//!
//! ## Capacidades verificadas en el árbol
//!
//! Los clientes existentes ya escriben `playlist/create` y `playlist/addTracks`
//! en Qobuz, y `POST /users/{id}/playlists` y `POST /playlists/{id}/items` en
//! Tidal. Son las únicas operaciones que este gateway promete hoy. Los endpoints
//! de actualización, retirada de ocurrencias y reordenación no están cableados en
//! esos clientes: se declaran [`PlaylistWriteErrorKind::Unsupported`] en vez de
//! simularlos con escrituras locales.
//!
//! Tidal añade con `onDuplicateChoices=SKIP`; el adaptador no promete conservar
//! ocurrencias repetidas. El proveedor falso sí las conserva y las identifica con
//! ids de ocurrencia distintos.
//!
//! ## Errores, timeouts y reintentos
//!
//! 401, 403, 429, timeout y validación son clases distintas. Cada operación tiene
//! un presupuesto de 30 segundos, también en Tidal, cuyo cliente no fija uno.
//! Clasificar un fallo no autoriza a reintentarlo: crear y añadir no son
//! idempotentes, por lo que un timeout puede haber dejado una escritura parcial
//! en el proveedor. La cola de PL07 decidirá la reconciliación antes de repetir
//! esas operaciones.
//!
//! ## Pruebas
//!
//! `FakePlaylistWriteProvider` mantiene el estado en memoria y programa fallos
//! por operación. Las pruebas del gateway no abren sockets ni consultan servicios
//! reales.

use crate::services::{QobuzClient, TidalClient};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::fmt;
use std::future::{ready, Future};
use std::time::Duration;

/// Operaciones del contrato de escritura. `RemoveItems` y `ReorderItems` citan
/// ocurrencias remotas, no ids de pista: una pista puede aparecer varias veces.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PlaylistWriteOperation {
    Create,
    AddTracks,
    Update,
    RemoveItems,
    ReorderItems,
}

impl PlaylistWriteOperation {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Create => "create",
            Self::AddTracks => "add_tracks",
            Self::Update => "update",
            Self::RemoveItems => "remove_items",
            Self::ReorderItems => "reorder_items",
        }
    }
}

/// Petición serializable para que PL07 pueda encolarla sin reconstruir su
/// semántica. Los ids pertenecen al espacio del proveedor seleccionado.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "operation", content = "arguments", rename_all = "snake_case")]
pub enum PlaylistWriteRequest {
    Create {
        name: String,
        description: String,
    },
    AddTracks {
        playlist_id: String,
        track_ids: Vec<String>,
    },
    Update {
        playlist_id: String,
        name: Option<String>,
        description: Option<String>,
    },
    RemoveItems {
        playlist_id: String,
        item_ids: Vec<String>,
    },
    ReorderItems {
        playlist_id: String,
        item_ids: Vec<String>,
    },
}

impl PlaylistWriteRequest {
    pub fn operation(&self) -> PlaylistWriteOperation {
        match self {
            Self::Create { .. } => PlaylistWriteOperation::Create,
            Self::AddTracks { .. } => PlaylistWriteOperation::AddTracks,
            Self::Update { .. } => PlaylistWriteOperation::Update,
            Self::RemoveItems { .. } => PlaylistWriteOperation::RemoveItems,
            Self::ReorderItems { .. } => PlaylistWriteOperation::ReorderItems,
        }
    }

    fn validate(&self, service: &str) -> Result<(), PlaylistWriteError> {
        let operation = self.operation();
        match self {
            Self::Create { name, .. } => validate_nonempty(service, operation, "name", name)?,
            Self::AddTracks {
                playlist_id,
                track_ids,
            } => {
                validate_nonempty(service, operation, "playlist_id", playlist_id)?;
                validate_ids(service, operation, "track_ids", track_ids, true)?;
            }
            Self::Update {
                playlist_id,
                name,
                description,
            } => {
                validate_nonempty(service, operation, "playlist_id", playlist_id)?;
                if name.is_none() && description.is_none() {
                    return Err(PlaylistWriteError::validation(
                        service,
                        operation,
                        "update requires a name or a description",
                    ));
                }
                if let Some(name) = name {
                    validate_nonempty(service, operation, "name", name)?;
                }
            }
            Self::RemoveItems {
                playlist_id,
                item_ids,
            } => {
                validate_nonempty(service, operation, "playlist_id", playlist_id)?;
                validate_ids(service, operation, "item_ids", item_ids, false)?;
            }
            Self::ReorderItems {
                playlist_id,
                item_ids,
            } => {
                validate_nonempty(service, operation, "playlist_id", playlist_id)?;
                validate_ids(service, operation, "item_ids", item_ids, false)?;
            }
        }
        Ok(())
    }
}

fn validate_nonempty(
    service: &str,
    operation: PlaylistWriteOperation,
    field: &str,
    value: &str,
) -> Result<(), PlaylistWriteError> {
    if value.trim().is_empty() {
        return Err(PlaylistWriteError::validation(
            service,
            operation,
            format!("{field} must not be empty"),
        ));
    }
    Ok(())
}

fn validate_ids(
    service: &str,
    operation: PlaylistWriteOperation,
    field: &str,
    ids: &[String],
    allow_duplicates: bool,
) -> Result<(), PlaylistWriteError> {
    if ids.is_empty() {
        return Err(PlaylistWriteError::validation(
            service,
            operation,
            format!("{field} must not be empty"),
        ));
    }
    for id in ids {
        validate_nonempty(service, operation, field, id)?;
    }
    if !allow_duplicates {
        let unique: HashSet<&str> = ids.iter().map(String::as_str).collect();
        if unique.len() != ids.len() {
            return Err(PlaylistWriteError::validation(
                service,
                operation,
                format!("{field} must not contain duplicate occurrence ids"),
            ));
        }
    }
    Ok(())
}

/// Resultado observable de una escritura. Los contadores son de ocurrencias
/// solicitadas; no inventan un estado remoto que el proveedor no haya confirmado.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PlaylistWriteOutcome {
    Created { playlist_id: String },
    TracksAdded { count: usize },
    Updated,
    ItemsRemoved { count: usize },
    ItemsReordered { count: usize },
}

/// Capacidades que el adaptador puede ejecutar de principio a fin. Una capacidad
/// ausente se reporta antes de llamar al proveedor.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PlaylistWriteCapabilities {
    pub create: bool,
    pub add_tracks: bool,
    pub update: bool,
    pub remove_items: bool,
    pub reorder_items: bool,
}

impl PlaylistWriteCapabilities {
    pub const NONE: Self = Self {
        create: false,
        add_tracks: false,
        update: false,
        remove_items: false,
        reorder_items: false,
    };

    pub const ALL: Self = Self {
        create: true,
        add_tracks: true,
        update: true,
        remove_items: true,
        reorder_items: true,
    };

    /// Superficie ya cableada en los clientes Qobuz y Tidal de este árbol.
    pub const CREATE_ADD: Self = Self {
        create: true,
        add_tracks: true,
        ..Self::NONE
    };

    pub const fn supports(self, operation: PlaylistWriteOperation) -> bool {
        match operation {
            PlaylistWriteOperation::Create => self.create,
            PlaylistWriteOperation::AddTracks => self.add_tracks,
            PlaylistWriteOperation::Update => self.update,
            PlaylistWriteOperation::RemoveItems => self.remove_items,
            PlaylistWriteOperation::ReorderItems => self.reorder_items,
        }
    }
}

impl Default for PlaylistWriteCapabilities {
    fn default() -> Self {
        Self::NONE
    }
}

/// Taxonomía estable para la cola y la interfaz de PL07.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlaylistWriteErrorKind {
    Unsupported,
    Unauthorized,
    Forbidden,
    RateLimited,
    Timeout,
    Validation,
    NotFound,
    Transient,
    Permanent,
}

impl PlaylistWriteErrorKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Unsupported => "unsupported",
            Self::Unauthorized => "unauthorized",
            Self::Forbidden => "forbidden",
            Self::RateLimited => "rate_limited",
            Self::Timeout => "timeout",
            Self::Validation => "validation",
            Self::NotFound => "not_found",
            Self::Transient => "transient",
            Self::Permanent => "permanent",
        }
    }
}

/// Error con contexto suficiente para decidir reautenticación, espera o reparación.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlaylistWriteError {
    pub service: String,
    pub operation: PlaylistWriteOperation,
    pub kind: PlaylistWriteErrorKind,
    pub http_status: Option<u16>,
    pub message: String,
}

impl PlaylistWriteError {
    fn new(
        service: &str,
        operation: PlaylistWriteOperation,
        kind: PlaylistWriteErrorKind,
        message: impl Into<String>,
    ) -> Self {
        Self {
            service: service.to_string(),
            operation,
            kind,
            http_status: None,
            message: message.into(),
        }
    }

    pub fn validation(
        service: &str,
        operation: PlaylistWriteOperation,
        message: impl Into<String>,
    ) -> Self {
        Self::new(
            service,
            operation,
            PlaylistWriteErrorKind::Validation,
            message,
        )
    }

    pub fn unsupported(service: &str, operation: PlaylistWriteOperation) -> Self {
        Self::new(
            service,
            operation,
            PlaylistWriteErrorKind::Unsupported,
            format!(
                "{} does not support playlist {}",
                service,
                operation.as_str()
            ),
        )
    }

    /// Convierte el error textual de los clientes existentes sin buscar estados
    /// arbitrarios en el cuerpo: solo prefijos estructurados de Qobuz/Tidal/HTTP.
    pub fn from_provider_message(
        service: &str,
        operation: PlaylistWriteOperation,
        message: &str,
    ) -> Self {
        let message = crate::commands::redact_auth_payload(message);
        let (kind, http_status) = classify_provider_message(&message);
        Self {
            service: service.to_string(),
            operation,
            kind,
            http_status,
            message,
        }
    }
}

impl fmt::Display for PlaylistWriteError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{} playlist {} failed: {}",
            self.service,
            self.operation.as_str(),
            self.message
        )
    }
}

impl std::error::Error for PlaylistWriteError {}

fn classify_provider_message(message: &str) -> (PlaylistWriteErrorKind, Option<u16>) {
    if let Some(status) = provider_http_status(message) {
        let kind = match status {
            401 => PlaylistWriteErrorKind::Unauthorized,
            403 => PlaylistWriteErrorKind::Forbidden,
            429 => PlaylistWriteErrorKind::RateLimited,
            408 | 504 => PlaylistWriteErrorKind::Timeout,
            400 | 422 => PlaylistWriteErrorKind::Validation,
            404 => PlaylistWriteErrorKind::NotFound,
            (500..=599) => PlaylistWriteErrorKind::Transient,
            _ => PlaylistWriteErrorKind::Permanent,
        };
        return (kind, Some(status));
    }

    let lower = message.to_ascii_lowercase();
    let kind = if lower.contains("requiresauth")
        || lower.contains("authentication required")
        || lower.contains("user authentication is required")
        || lower.contains("login rejected")
        || lower.contains("unauthorized")
    {
        PlaylistWriteErrorKind::Unauthorized
    } else if lower.contains("forbidden") {
        PlaylistWriteErrorKind::Forbidden
    } else if lower.contains("timeout") || lower.contains("timed out") {
        PlaylistWriteErrorKind::Timeout
    } else if lower.contains("validation")
        || lower.contains("invalid request")
        || lower.contains("bad request")
        || lower.contains("unprocessable")
    {
        PlaylistWriteErrorKind::Validation
    } else if lower.contains("not found") {
        PlaylistWriteErrorKind::NotFound
    } else if lower.contains("failed to parse")
        || lower.contains("no playlist id")
        || lower.contains("user id not set")
    {
        PlaylistWriteErrorKind::Permanent
    } else {
        PlaylistWriteErrorKind::Transient
    };
    (kind, None)
}

fn provider_http_status(message: &str) -> Option<u16> {
    for marker in [
        "Qobuz API error (",
        "Tidal API error ",
        "HTTP status ",
        "HTTP ",
    ] {
        if let Some(rest) = message.split_once(marker) {
            let token: String = rest.1.chars().take_while(char::is_ascii_digit).collect();
            if let Ok(status) = token.parse() {
                return Some(status);
            }
        }
    }
    None
}

/// Seam de proveedor. Cada adaptador envuelve un cliente ya autenticado;
/// resolver credenciales y persistirlas no es trabajo de PL06.
pub trait PlaylistWriteProvider: Send + Sync {
    fn service_name(&self) -> &'static str;
    fn capabilities(&self) -> PlaylistWriteCapabilities;

    fn create_playlist(
        &self,
        name: String,
        description: String,
    ) -> impl Future<Output = Result<String, PlaylistWriteError>> + Send;

    fn add_tracks(
        &self,
        playlist_id: String,
        track_ids: Vec<String>,
    ) -> impl Future<Output = Result<(), PlaylistWriteError>> + Send;

    fn update_playlist(
        &self,
        _playlist_id: String,
        _name: Option<String>,
        _description: Option<String>,
    ) -> impl Future<Output = Result<(), PlaylistWriteError>> + Send {
        ready(Err(PlaylistWriteError::unsupported(
            self.service_name(),
            PlaylistWriteOperation::Update,
        )))
    }

    fn remove_items(
        &self,
        _playlist_id: String,
        _item_ids: Vec<String>,
    ) -> impl Future<Output = Result<(), PlaylistWriteError>> + Send {
        ready(Err(PlaylistWriteError::unsupported(
            self.service_name(),
            PlaylistWriteOperation::RemoveItems,
        )))
    }

    fn reorder_items(
        &self,
        _playlist_id: String,
        _item_ids: Vec<String>,
    ) -> impl Future<Output = Result<(), PlaylistWriteError>> + Send {
        ready(Err(PlaylistWriteError::unsupported(
            self.service_name(),
            PlaylistWriteOperation::ReorderItems,
        )))
    }
}

/// Presupuesto por operación. Cubre también los POST de Tidal, que hoy no fijan
/// timeout en su cliente HTTP.
pub const DEFAULT_PLAYLIST_WRITE_TIMEOUT: Duration = Duration::from_secs(30);

/// Fachada pública: una sola operación por llamada, con validación, capacidades y
/// timeout antes de exponer el resultado a la cola de PL07.
pub struct PlaylistWriteGateway<'a, P: PlaylistWriteProvider> {
    provider: &'a P,
    timeout: Duration,
}

impl<'a, P: PlaylistWriteProvider> PlaylistWriteGateway<'a, P> {
    pub fn new(provider: &'a P) -> Self {
        Self {
            provider,
            timeout: DEFAULT_PLAYLIST_WRITE_TIMEOUT,
        }
    }

    pub fn with_timeout(mut self, timeout: Duration) -> Self {
        self.timeout = timeout;
        self
    }

    pub fn service_name(&self) -> &'static str {
        self.provider.service_name()
    }

    pub fn capabilities(&self) -> PlaylistWriteCapabilities {
        self.provider.capabilities()
    }

    pub async fn execute(
        &self,
        request: PlaylistWriteRequest,
    ) -> Result<PlaylistWriteOutcome, PlaylistWriteError> {
        let operation = request.operation();
        let service = self.provider.service_name();
        request.validate(service)?;
        if !self.provider.capabilities().supports(operation) {
            return Err(PlaylistWriteError::unsupported(service, operation));
        }

        match tokio::time::timeout(self.timeout, self.dispatch(request)).await {
            Ok(result) => result,
            Err(_) => Err(PlaylistWriteError::new(
                service,
                operation,
                PlaylistWriteErrorKind::Timeout,
                format!("playlist write exceeded {:?}", self.timeout),
            )),
        }
    }

    async fn dispatch(
        &self,
        request: PlaylistWriteRequest,
    ) -> Result<PlaylistWriteOutcome, PlaylistWriteError> {
        match request {
            PlaylistWriteRequest::Create { name, description } => self
                .provider
                .create_playlist(name, description)
                .await
                .map(|playlist_id| PlaylistWriteOutcome::Created { playlist_id }),
            PlaylistWriteRequest::AddTracks {
                playlist_id,
                track_ids,
            } => {
                let count = track_ids.len();
                self.provider
                    .add_tracks(playlist_id, track_ids)
                    .await
                    .map(move |()| PlaylistWriteOutcome::TracksAdded { count })
            }
            PlaylistWriteRequest::Update {
                playlist_id,
                name,
                description,
            } => self
                .provider
                .update_playlist(playlist_id, name, description)
                .await
                .map(|()| PlaylistWriteOutcome::Updated),
            PlaylistWriteRequest::RemoveItems {
                playlist_id,
                item_ids,
            } => {
                let count = item_ids.len();
                self.provider
                    .remove_items(playlist_id, item_ids)
                    .await
                    .map(move |()| PlaylistWriteOutcome::ItemsRemoved { count })
            }
            PlaylistWriteRequest::ReorderItems {
                playlist_id,
                item_ids,
            } => {
                let count = item_ids.len();
                self.provider
                    .reorder_items(playlist_id, item_ids)
                    .await
                    .map(move |()| PlaylistWriteOutcome::ItemsReordered { count })
            }
        }
    }
}

/// Adaptador de los dos endpoints Qobuz ya cableados en `QobuzClient`.
pub struct QobuzPlaylistWriter<'a> {
    client: &'a QobuzClient,
}

impl<'a> QobuzPlaylistWriter<'a> {
    pub const CAPABILITIES: PlaylistWriteCapabilities = PlaylistWriteCapabilities::CREATE_ADD;

    pub fn new(client: &'a QobuzClient) -> Self {
        Self { client }
    }
}

impl PlaylistWriteProvider for QobuzPlaylistWriter<'_> {
    fn service_name(&self) -> &'static str {
        "qobuz"
    }

    fn capabilities(&self) -> PlaylistWriteCapabilities {
        Self::CAPABILITIES
    }

    async fn create_playlist(
        &self,
        name: String,
        description: String,
    ) -> Result<String, PlaylistWriteError> {
        let playlist_id = self
            .client
            .create_playlist(&name, &description)
            .await
            .map_err(|message| {
                PlaylistWriteError::from_provider_message(
                    "qobuz",
                    PlaylistWriteOperation::Create,
                    &message,
                )
            })?;
        if playlist_id <= 0 {
            return Err(PlaylistWriteError::new(
                "qobuz",
                PlaylistWriteOperation::Create,
                PlaylistWriteErrorKind::Permanent,
                "Qobuz returned an invalid playlist id",
            ));
        }
        Ok(playlist_id.to_string())
    }

    async fn add_tracks(
        &self,
        playlist_id: String,
        track_ids: Vec<String>,
    ) -> Result<(), PlaylistWriteError> {
        let playlist_id = parse_qobuz_id(
            "qobuz",
            PlaylistWriteOperation::AddTracks,
            "playlist_id",
            &playlist_id,
        )?;
        let track_ids = track_ids
            .iter()
            .map(|id| parse_qobuz_id("qobuz", PlaylistWriteOperation::AddTracks, "track_id", id))
            .collect::<Result<Vec<_>, _>>()?;
        self.client
            .add_playlist_tracks(playlist_id, &track_ids)
            .await
            .map_err(|message| {
                PlaylistWriteError::from_provider_message(
                    "qobuz",
                    PlaylistWriteOperation::AddTracks,
                    &message,
                )
            })
    }
}

fn parse_qobuz_id(
    service: &str,
    operation: PlaylistWriteOperation,
    field: &str,
    raw: &str,
) -> Result<i64, PlaylistWriteError> {
    raw.parse::<i64>().ok().filter(|id| *id > 0).ok_or_else(|| {
        PlaylistWriteError::validation(
            service,
            operation,
            format!("{field} must be a positive Qobuz integer id"),
        )
    })
}

/// Adaptador de los dos endpoints Tidal ya cableados en `TidalClient`.
pub struct TidalPlaylistWriter<'a> {
    client: &'a TidalClient,
}

impl<'a> TidalPlaylistWriter<'a> {
    pub const CAPABILITIES: PlaylistWriteCapabilities = PlaylistWriteCapabilities::CREATE_ADD;

    pub fn new(client: &'a TidalClient) -> Self {
        Self { client }
    }
}

impl PlaylistWriteProvider for TidalPlaylistWriter<'_> {
    fn service_name(&self) -> &'static str {
        "tidal"
    }

    fn capabilities(&self) -> PlaylistWriteCapabilities {
        Self::CAPABILITIES
    }

    async fn create_playlist(
        &self,
        name: String,
        description: String,
    ) -> Result<String, PlaylistWriteError> {
        let playlist_id = self
            .client
            .create_playlist(&name, &description)
            .await
            .map_err(|message| {
                PlaylistWriteError::from_provider_message(
                    "tidal",
                    PlaylistWriteOperation::Create,
                    &message,
                )
            })?;
        validate_tidal_playlist_id(playlist_id)
    }

    async fn add_tracks(
        &self,
        playlist_id: String,
        track_ids: Vec<String>,
    ) -> Result<(), PlaylistWriteError> {
        self.client
            .add_playlist_tracks(&playlist_id, &track_ids)
            .await
            .map_err(|message| {
                PlaylistWriteError::from_provider_message(
                    "tidal",
                    PlaylistWriteOperation::AddTracks,
                    &message,
                )
            })
    }
}

fn validate_tidal_playlist_id(playlist_id: String) -> Result<String, PlaylistWriteError> {
    if playlist_id.trim().is_empty() {
        return Err(PlaylistWriteError::new(
            "tidal",
            PlaylistWriteOperation::Create,
            PlaylistWriteErrorKind::Permanent,
            "Tidal returned an empty playlist id",
        ));
    }
    Ok(playlist_id)
}

#[path = "playlist_write_fake.rs"]
mod fake;
pub use fake::{FakePlaylistItem, FakePlaylistSnapshot, FakePlaylistWriteProvider};

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn existing_clients_treat_an_empty_add_as_a_noop() {
        let qobuz = QobuzClient::new("app".into(), "secret".into());
        qobuz
            .add_playlist_tracks(7, &[])
            .await
            .expect("an empty Qobuz add must not issue HTTP");

        let tidal = TidalClient::new("token".into()).with_user("user".into(), "MX".into());
        tidal
            .add_playlist_tracks("playlist", &[])
            .await
            .expect("an empty Tidal add must not issue HTTP");
    }

    #[test]
    fn classifies_provider_failures_from_structured_status_prefixes() {
        let cases = [
            (
                "Qobuz API error (401): unauthorized",
                PlaylistWriteErrorKind::Unauthorized,
                Some(401),
            ),
            (
                "RequiresAuth: Tidal API authentication failed (HTTP 403): forbidden",
                PlaylistWriteErrorKind::Forbidden,
                Some(403),
            ),
            (
                "Tidal API error 429: slow down",
                PlaylistWriteErrorKind::RateLimited,
                Some(429),
            ),
            (
                "Request failed: operation timed out",
                PlaylistWriteErrorKind::Timeout,
                None,
            ),
            (
                "Qobuz API error (400): track 401 is invalid",
                PlaylistWriteErrorKind::Validation,
                Some(400),
            ),
            (
                "Qobuz API error (422): track 401 is invalid",
                PlaylistWriteErrorKind::Validation,
                Some(422),
            ),
            (
                "Tidal API error 408: request timeout",
                PlaylistWriteErrorKind::Timeout,
                Some(408),
            ),
            ("User ID not set", PlaylistWriteErrorKind::Permanent, None),
            (
                "Tidal API error 404: missing",
                PlaylistWriteErrorKind::NotFound,
                Some(404),
            ),
            (
                "Qobuz API error (503): busy",
                PlaylistWriteErrorKind::Transient,
                Some(503),
            ),
        ];

        for (message, expected_kind, expected_status) in cases {
            let error = PlaylistWriteError::from_provider_message(
                "qobuz",
                PlaylistWriteOperation::AddTracks,
                message,
            );
            assert_eq!(error.kind, expected_kind, "{message}");
            assert_eq!(error.http_status, expected_status, "{message}");
        }
    }

    #[test]
    fn tidal_create_rejects_an_empty_remote_id() {
        let error = validate_tidal_playlist_id("   ".into())
            .expect_err("an empty Tidal id is a malformed provider response");
        assert_eq!(error.kind, PlaylistWriteErrorKind::Permanent);
        assert_eq!(error.operation, PlaylistWriteOperation::Create);
    }

    async fn assert_only_create_and_add<P: PlaylistWriteProvider>(provider: &P) {
        assert_eq!(
            provider.capabilities(),
            PlaylistWriteCapabilities::CREATE_ADD
        );
        let gateway = PlaylistWriteGateway::new(provider);
        for request in [
            PlaylistWriteRequest::Update {
                playlist_id: "playlist".into(),
                name: Some("Renamed".into()),
                description: None,
            },
            PlaylistWriteRequest::RemoveItems {
                playlist_id: "playlist".into(),
                item_ids: vec!["item".into()],
            },
            PlaylistWriteRequest::ReorderItems {
                playlist_id: "playlist".into(),
                item_ids: vec!["item".into()],
            },
        ] {
            let error = gateway
                .execute(request)
                .await
                .expect_err("must be unsupported");
            assert_eq!(error.kind, PlaylistWriteErrorKind::Unsupported);
            assert_eq!(error.service, provider.service_name());
        }
    }

    #[tokio::test]
    async fn real_adapters_declare_only_the_methods_already_wired() {
        let qobuz = QobuzClient::new("app".into(), "secret".into());
        assert_only_create_and_add(&QobuzPlaylistWriter::new(&qobuz)).await;

        let tidal = TidalClient::new("token".into()).with_user("user".into(), "MX".into());
        assert_only_create_and_add(&TidalPlaylistWriter::new(&tidal)).await;
    }

    #[tokio::test]
    async fn gateway_validates_before_checking_capabilities_or_opening_http() {
        let qobuz =
            QobuzClient::new("app".into(), "secret".into()).with_base_url("not-a-url".into());
        let writer = QobuzPlaylistWriter::new(&qobuz);
        let gateway = PlaylistWriteGateway::new(&writer);

        let error = gateway
            .execute(PlaylistWriteRequest::Update {
                playlist_id: "playlist".into(),
                name: None,
                description: None,
            })
            .await
            .expect_err("empty update is invalid");
        assert_eq!(error.kind, PlaylistWriteErrorKind::Validation);

        let error = gateway
            .execute(PlaylistWriteRequest::AddTracks {
                playlist_id: "7".into(),
                track_ids: vec!["not-an-integer".into()],
            })
            .await
            .expect_err("invalid Qobuz id must fail before HTTP");
        assert_eq!(error.kind, PlaylistWriteErrorKind::Validation);
    }

    #[tokio::test]
    async fn gateway_executes_all_five_operations_against_the_fake() {
        let provider = FakePlaylistWriteProvider::new();
        let gateway = PlaylistWriteGateway::new(&provider);

        let created = gateway
            .execute(PlaylistWriteRequest::Create {
                name: "Mix".into(),
                description: "Original".into(),
            })
            .await
            .expect("create");
        let playlist_id = match created {
            PlaylistWriteOutcome::Created { playlist_id } => playlist_id,
            other => panic!("unexpected outcome: {other:?}"),
        };

        assert_eq!(
            gateway
                .execute(PlaylistWriteRequest::AddTracks {
                    playlist_id: playlist_id.clone(),
                    track_ids: vec!["10".into(), "11".into(), "12".into()],
                })
                .await
                .expect("add"),
            PlaylistWriteOutcome::TracksAdded { count: 3 }
        );
        let initial = provider.playlist(&playlist_id).expect("fake playlist");
        let first = initial.items[0].id.clone();
        let middle = initial.items[1].id.clone();
        let last = initial.items[2].id.clone();

        assert_eq!(
            gateway
                .execute(PlaylistWriteRequest::Update {
                    playlist_id: playlist_id.clone(),
                    name: Some("Renamed".into()),
                    description: Some(String::new()),
                })
                .await
                .expect("update"),
            PlaylistWriteOutcome::Updated
        );
        assert_eq!(
            gateway
                .execute(PlaylistWriteRequest::RemoveItems {
                    playlist_id: playlist_id.clone(),
                    item_ids: vec![middle],
                })
                .await
                .expect("remove"),
            PlaylistWriteOutcome::ItemsRemoved { count: 1 }
        );
        assert_eq!(
            gateway
                .execute(PlaylistWriteRequest::ReorderItems {
                    playlist_id: playlist_id.clone(),
                    item_ids: vec![last.clone(), first.clone()],
                })
                .await
                .expect("reorder"),
            PlaylistWriteOutcome::ItemsReordered { count: 2 }
        );

        let snapshot = provider.playlist(&playlist_id).expect("fake playlist");
        assert_eq!(snapshot.name, "Renamed");
        assert_eq!(snapshot.description, "");
        assert_eq!(
            snapshot
                .items
                .iter()
                .map(|item| (item.id.as_str(), item.track_id.as_str()))
                .collect::<Vec<_>>(),
            vec![(last.as_str(), "12"), (first.as_str(), "10")]
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
                PlaylistWriteOperation::Update,
                PlaylistWriteOperation::RemoveItems,
                PlaylistWriteOperation::ReorderItems,
            ]
        );
    }

    #[tokio::test]
    async fn an_absent_capability_is_reported_without_mutating_the_fake() {
        let provider =
            FakePlaylistWriteProvider::new().with_capabilities(PlaylistWriteCapabilities {
                update: false,
                ..PlaylistWriteCapabilities::ALL
            });
        let gateway = PlaylistWriteGateway::new(&provider);

        let error = gateway
            .execute(PlaylistWriteRequest::Update {
                playlist_id: "playlist".into(),
                name: Some("Renamed".into()),
                description: None,
            })
            .await
            .expect_err("missing capability");
        assert_eq!(error.kind, PlaylistWriteErrorKind::Unsupported);
        assert_eq!(error.operation, PlaylistWriteOperation::Update);
        assert!(provider.calls().is_empty());
        assert_eq!(provider.playlist_count(), 0);
    }

    #[tokio::test]
    async fn invalid_requests_never_reach_the_fake() {
        let provider = FakePlaylistWriteProvider::new();
        let gateway = PlaylistWriteGateway::new(&provider);
        let requests = [
            PlaylistWriteRequest::Create {
                name: "   ".into(),
                description: String::new(),
            },
            PlaylistWriteRequest::AddTracks {
                playlist_id: "playlist".into(),
                track_ids: Vec::new(),
            },
            PlaylistWriteRequest::RemoveItems {
                playlist_id: "playlist".into(),
                item_ids: vec!["item".into(), "item".into()],
            },
            PlaylistWriteRequest::ReorderItems {
                playlist_id: "playlist".into(),
                item_ids: Vec::new(),
            },
        ];

        for request in requests {
            let error = gateway.execute(request).await.expect_err("invalid request");
            assert_eq!(error.kind, PlaylistWriteErrorKind::Validation);
        }
        assert!(provider.calls().is_empty());
    }

    #[tokio::test]
    async fn a_scheduled_failure_is_classified_and_consumed_once() {
        let provider = FakePlaylistWriteProvider::new().with_failure(
            PlaylistWriteOperation::AddTracks,
            PlaylistWriteErrorKind::RateLimited,
        );
        let gateway = PlaylistWriteGateway::new(&provider);
        let playlist_id = match gateway
            .execute(PlaylistWriteRequest::Create {
                name: "Mix".into(),
                description: String::new(),
            })
            .await
            .expect("create")
        {
            PlaylistWriteOutcome::Created { playlist_id } => playlist_id,
            other => panic!("unexpected outcome: {other:?}"),
        };
        let request = PlaylistWriteRequest::AddTracks {
            playlist_id: playlist_id.clone(),
            track_ids: vec!["10".into()],
        };

        let error = gateway
            .execute(request.clone())
            .await
            .expect_err("scheduled rate limit");
        assert_eq!(error.kind, PlaylistWriteErrorKind::RateLimited);
        assert_eq!(error.http_status, Some(429));
        assert_eq!(error.operation, PlaylistWriteOperation::AddTracks);
        assert!(provider
            .playlist(&playlist_id)
            .expect("fake playlist")
            .items
            .is_empty());

        gateway
            .execute(request)
            .await
            .expect("failure was one-shot");
        assert_eq!(
            provider
                .playlist(&playlist_id)
                .expect("fake playlist")
                .items
                .len(),
            1
        );
    }

    #[tokio::test]
    async fn gateway_timeout_can_mask_an_already_committed_fake_write() {
        let provider = FakePlaylistWriteProvider::new().with_delay(Duration::from_millis(50));
        let gateway = PlaylistWriteGateway::new(&provider).with_timeout(Duration::from_millis(1));

        let error = gateway
            .execute(PlaylistWriteRequest::Create {
                name: "Slow".into(),
                description: String::new(),
            })
            .await
            .expect_err("gateway timeout");
        assert_eq!(error.kind, PlaylistWriteErrorKind::Timeout);
        assert_eq!(error.operation, PlaylistWriteOperation::Create);
        assert_eq!(error.service, "fake");
        assert_eq!(provider.calls().len(), 1);
        assert_eq!(provider.playlist_count(), 1);
        let committed = provider
            .playlist("fake-playlist-1")
            .expect("the fake committed before losing the response");
        assert_eq!(committed.name, "Slow");
    }

    #[tokio::test]
    async fn fake_preserves_repeated_tracks_as_distinct_occurrences() {
        let provider = FakePlaylistWriteProvider::new();
        let gateway = PlaylistWriteGateway::new(&provider);
        let playlist_id = match gateway
            .execute(PlaylistWriteRequest::Create {
                name: "Repeated".into(),
                description: String::new(),
            })
            .await
            .expect("create")
        {
            PlaylistWriteOutcome::Created { playlist_id } => playlist_id,
            other => panic!("unexpected outcome: {other:?}"),
        };
        gateway
            .execute(PlaylistWriteRequest::AddTracks {
                playlist_id: playlist_id.clone(),
                track_ids: vec!["10".into(), "10".into()],
            })
            .await
            .expect("add repeated track");

        let snapshot = provider.playlist(&playlist_id).expect("fake playlist");
        assert_eq!(snapshot.items.len(), 2);
        assert_ne!(snapshot.items[0].id, snapshot.items[1].id);
        assert_eq!(snapshot.items[0].track_id, "10");
        assert_eq!(snapshot.items[1].track_id, "10");
    }

    #[tokio::test]
    async fn fake_rejects_an_incomplete_reorder_without_changing_order() {
        let provider = FakePlaylistWriteProvider::new();
        let gateway = PlaylistWriteGateway::new(&provider);
        let playlist_id = match gateway
            .execute(PlaylistWriteRequest::Create {
                name: "Ordered".into(),
                description: String::new(),
            })
            .await
            .expect("create")
        {
            PlaylistWriteOutcome::Created { playlist_id } => playlist_id,
            other => panic!("unexpected outcome: {other:?}"),
        };
        gateway
            .execute(PlaylistWriteRequest::AddTracks {
                playlist_id: playlist_id.clone(),
                track_ids: vec!["10".into(), "11".into()],
            })
            .await
            .expect("add");
        let before = provider.playlist(&playlist_id).expect("fake playlist");

        let error = gateway
            .execute(PlaylistWriteRequest::ReorderItems {
                playlist_id: playlist_id.clone(),
                item_ids: vec![before.items[1].id.clone()],
            })
            .await
            .expect_err("incomplete permutation");
        assert_eq!(error.kind, PlaylistWriteErrorKind::Validation);
        assert_eq!(
            provider.playlist(&playlist_id).expect("fake playlist"),
            before
        );
    }

    #[test]
    fn write_requests_round_trip_through_json_for_the_future_queue() {
        let request = PlaylistWriteRequest::AddTracks {
            playlist_id: "playlist".into(),
            track_ids: vec!["10".into(), "11".into()],
        };
        let json = serde_json::to_string(&request).expect("serialize request");
        assert_eq!(
            serde_json::from_str::<PlaylistWriteRequest>(&json).expect("deserialize request"),
            request
        );
    }
}

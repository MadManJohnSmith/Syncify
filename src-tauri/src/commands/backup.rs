#[allow(unused_imports)]
use super::*;

// Backup & Restore Commands - submodule of crate::commands
// Manages JSON/SQLite library backup export and atomic restore across machines

use sha2::{Digest, Sha256};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BackupTrackDto {
    pub isrc: Option<String>,
    pub title: String,
    pub artist: String,
    pub album: Option<String>,
    pub track_number: Option<i32>,
    pub disc_number: Option<i32>,
    pub duration_ms: Option<i64>,
    pub explicit: Option<i32>,
    pub favorite_at: Option<String>,
    pub service: Option<String>,
    pub service_track_id: Option<String>,
    pub downloaded_format: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BackupAlbumDto {
    pub title: String,
    pub artist: String,
    pub upc: Option<String>,
    pub release_date: Option<String>,
    pub favorite_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BackupArtistDto {
    pub name: String,
    pub favorite_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BackupPlaylistDto {
    pub name: String,
    pub description: Option<String>,
    pub track_isrcs: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LibraryBackupManifest {
    pub version: String,
    pub schema_version: i64,
    pub exported_at: String,
    pub app_version: String,
    pub checksum_sha256: Option<String>,
    pub tracks: Vec<BackupTrackDto>,
    pub albums: Vec<BackupAlbumDto>,
    pub artists: Vec<BackupArtistDto>,
    pub playlists: Vec<BackupPlaylistDto>,
    // Manifest v2 (ADR-0020): ajustes completos y metadatos de cuenta, sin
    // ningún secreto. Opcionales para que un manifest v1 siga deserializando.
    #[serde(default)]
    pub settings: Option<BackupSettingsDto>,
    #[serde(default)]
    pub accounts: Option<Vec<BackupAccountDto>>,
}

/// Ajustes exportables del manifest v2: todas las superficies tipadas y la KV
/// de ajustes de usuario. La KV excluye explícitamente las claves sensibles
/// (tokens, claves de API, credenciales); ninguna tabla tipada de este
/// catálogo tiene columna de credenciales.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct BackupSettingsDto {
    #[serde(default)]
    pub kv: std::collections::BTreeMap<String, String>,
    #[serde(default)]
    pub service_preferences: Vec<serde_json::Value>,
    #[serde(default)]
    pub sync_settings: Vec<serde_json::Value>,
    #[serde(default)]
    pub service_sync_settings: Vec<serde_json::Value>,
    #[serde(default)]
    pub quality_preferences: Vec<serde_json::Value>,
    #[serde(default)]
    pub folder_settings: Vec<serde_json::Value>,
    #[serde(default)]
    pub duplicate_settings: Vec<serde_json::Value>,
    #[serde(default)]
    pub audio_processing_settings: Vec<serde_json::Value>,
    #[serde(default)]
    pub lyrics_provider_settings: Vec<serde_json::Value>,
    #[serde(default)]
    pub lyrics_config: Vec<serde_json::Value>,
    #[serde(default)]
    pub metadata_preferences: Vec<serde_json::Value>,
    #[serde(default)]
    pub advanced_settings: Vec<serde_json::Value>,
}

/// Metadatos de cuenta del manifest v2: solo identidad, nunca credenciales.
/// `credentials_json` y cualquier token se quedan en la BD; al restaurar en
/// otra máquina la cuenta pide login (ADR-0020 decisión 1, cláusula 10 de
/// ADR-0007).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BackupAccountDto {
    pub service: String,
    pub external_user_id: Option<String>,
    pub display_name: Option<String>,
    pub email: Option<String>,
    pub is_active: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExportLibraryResult {
    pub file_path: String,
    pub tracks_count: usize,
    pub albums_count: usize,
    pub artists_count: usize,
    pub playlists_count: usize,
    pub file_size_bytes: u64,
    pub checksum: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImportLibraryResult {
    pub tracks_imported: i64,
    pub albums_imported: i64,
    pub artists_imported: i64,
    pub playlists_imported: i64,
    pub favorites_restored: i64,
    /// Filas de ajustes restauradas del manifest v2 (superficies tipadas +
    /// entradas de la KV). 0 en manifests v1.
    pub settings_rows_restored: i64,
    /// Cuentas aplicadas desde el manifest v2, siempre sin sesión.
    pub accounts_restored: i64,
    /// Servicios de las cuentas recién insertadas sin credenciales: piden
    /// login al terminar la restauración (ADR-0020 decisión 1).
    pub accounts_awaiting_reauth: Vec<String>,
    pub message: String,
}

/// Calculate SHA-256 hex string of bytes
pub fn compute_sha256_hex(data: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(data);
    format!("{:x}", hasher.finalize())
}

/// Allowed extension for backup library export manifest.
pub const ALLOWED_BACKUP_EXTENSIONS: &[&str] = &["json"];

/// Returns the set of allowed base directories for library backup export.
/// Strictly confined to the user's Downloads, Documents, and app data directory.
pub fn get_allowed_backup_export_directories() -> Vec<std::path::PathBuf> {
    let mut bases = Vec::new();

    if let Some(download) = dirs::download_dir() {
        if let Ok(canon) = std::fs::canonicalize(&download) {
            bases.push(canon);
        }
        bases.push(download);
    }

    if let Some(doc) = dirs::document_dir() {
        if let Ok(canon) = std::fs::canonicalize(&doc) {
            bases.push(canon);
        }
        bases.push(doc);
    }

    if let Some(data_local) = dirs::data_local_dir() {
        let app_dir = data_local.join("com.syncify.app");
        if let Ok(canon) = std::fs::canonicalize(&app_dir) {
            bases.push(canon);
        }
        bases.push(app_dir);
    }

    if let Some(data) = dirs::data_dir() {
        let app_dir = data.join("com.syncify.app");
        if let Ok(canon) = std::fs::canonicalize(&app_dir) {
            bases.push(canon);
        }
        bases.push(app_dir);
    }

    bases.sort();
    bases.dedup();
    bases
}

/// Validates that a backup export destination path conforms to sandbox confinement,
/// path traversal restrictions, and JSON extension enforcement.
pub fn validate_safe_backup_export_path_with_bases(
    target_path: &std::path::Path,
    allowed_bases: &[std::path::PathBuf],
) -> Result<std::path::PathBuf, String> {
    // 1. Must be an absolute path
    if !target_path.is_absolute() {
        return Err("Acceso denegado: la ruta debe ser absoluta (sandbox violation)".to_string());
    }

    // 2. Reject path traversal sequences (.. or ParentDir)
    for component in target_path.components() {
        if matches!(component, std::path::Component::ParentDir) {
            return Err(
                "Acceso denegado: secuencias de escape ('..') detectadas (sandbox violation)"
                    .to_string(),
            );
        }
    }

    // 3. Reject hidden files
    let file_name = target_path
        .file_name()
        .and_then(|f| f.to_str())
        .ok_or_else(|| {
            "Acceso denegado: nombre de archivo no válido (sandbox violation)".to_string()
        })?;

    if file_name.starts_with('.') {
        return Err("Acceso denegado: no se permite escribir archivos ocultos o de configuración (sandbox violation)".to_string());
    }

    // 4. Strict extension check: .json (case-insensitive)
    let ext = target_path
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| e.to_lowercase());

    let ext_str = match &ext {
        Some(e) => e.as_str(),
        None => {
            return Err("Acceso denegado: el archivo debe tener extensión obligatoria .json (sandbox violation)".to_string());
        }
    };

    if !ALLOWED_BACKUP_EXTENSIONS.contains(&ext_str) {
        return Err(format!(
            "Acceso denegado: extensión '.{}' no permitida. El backup debe ser un archivo .json (sandbox violation)",
            ext_str
        ));
    }

    // 5. Defense in depth: reject sensitive system directories
    let path_str = target_path.to_string_lossy();
    if path_str.starts_with("/etc")
        || path_str.starts_with("/proc")
        || path_str.starts_with("/sys")
        || path_str.starts_with("/dev")
        || path_str.starts_with("/var")
        || path_str.contains("/.ssh")
        || path_str.contains("/.gnupg")
        || path_str.contains("/.aws")
    {
        return Err(
            "Acceso denegado: ruta en directorio protegido del sistema (sandbox violation)"
                .to_string(),
        );
    }

    if allowed_bases.is_empty() {
        return Err(
            "Acceso denegado: no se definieron directorios base permitidos (sandbox violation)"
                .to_string(),
        );
    }

    // 6. Lexical containment check against allowed bases
    let matches_lexical = allowed_bases
        .iter()
        .any(|base| target_path.starts_with(base));
    if !matches_lexical {
        return Err(
            "Acceso denegado: la ruta está fuera de los directorios permitidos (sandbox violation)"
                .to_string(),
        );
    }

    // 7. Parent directory resolution and creation
    let parent = target_path.parent().ok_or_else(|| {
        "Acceso denegado: ruta sin directorio padre válido (sandbox violation)".to_string()
    })?;

    if !parent.exists() {
        std::fs::create_dir_all(parent)
            .map_err(|e| format!("No se pudo crear el directorio {}: {}", parent.display(), e))?;
    }

    // 8. Canonicalize parent directory and verify containment
    let canonical_parent = std::fs::canonicalize(parent).map_err(|e| {
        format!(
            "Error al canonicalizar directorio {}: {}",
            parent.display(),
            e
        )
    })?;

    let mut canonical_allowed_bases = Vec::new();
    for b in allowed_bases {
        if let Ok(c) = std::fs::canonicalize(b) {
            canonical_allowed_bases.push(c);
        }
        canonical_allowed_bases.push(b.clone());
    }

    if !canonical_allowed_bases
        .iter()
        .any(|base| canonical_parent.starts_with(base))
    {
        return Err("Acceso denegado: el directorio destino canonicalizado está fuera del sandbox permitido (sandbox violation)".to_string());
    }

    let safe_target = canonical_parent.join(file_name);

    // 9. Prevent symlink overwriting or escaping via existing symlinks
    if safe_target.is_symlink()
        || std::fs::symlink_metadata(&safe_target)
            .map(|m| m.file_type().is_symlink())
            .unwrap_or(false)
    {
        return Err(
            "Acceso denegado: no se permite sobreescribir enlaces simbólicos (sandbox violation)"
                .to_string(),
        );
    }

    if safe_target.exists() {
        let canonical_target = std::fs::canonicalize(&safe_target)
            .map_err(|e| format!("Error al canonicalizar archivo existente: {}", e))?;
        if !canonical_allowed_bases
            .iter()
            .any(|base| canonical_target.starts_with(base))
        {
            return Err("Acceso denegado: el archivo destino existente resuelve fuera del sandbox permitido (sandbox violation)".to_string());
        }
    }

    Ok(safe_target)
}

/// Maximum accepted import manifest size. This bounds renderer-triggered memory use.
pub const MAX_BACKUP_IMPORT_BYTES: u64 = 64 * 1024 * 1024;

pub fn validate_safe_backup_import_path_with_bases(
    target_path: &std::path::Path,
    allowed_bases: &[std::path::PathBuf],
) -> Result<std::path::PathBuf, String> {
    if !target_path.is_absolute()
        || target_path
            .components()
            .any(|c| matches!(c, std::path::Component::ParentDir))
    {
        return Err(
            "Access denied: backup import path must be absolute and cannot contain traversal"
                .to_string(),
        );
    }
    if target_path
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| e.eq_ignore_ascii_case("json"))
        != Some(true)
    {
        return Err("Access denied: backup import must be a .json file".to_string());
    }
    let canonical = std::fs::canonicalize(target_path)
        .map_err(|e| format!("Failed to resolve backup import path: {}", e))?;
    let metadata = std::fs::symlink_metadata(target_path)
        .map_err(|e| format!("Failed to inspect backup import file: {}", e))?;
    if metadata.file_type().is_symlink() || !metadata.is_file() {
        return Err("Access denied: backup import must be a regular, non-symlink file".to_string());
    }
    let allowed = allowed_bases
        .iter()
        .filter_map(|base| std::fs::canonicalize(base).ok())
        .any(|base| canonical.starts_with(base));
    if !allowed {
        return Err("Access denied: backup import is outside allowed user directories".to_string());
    }
    if metadata.len() > MAX_BACKUP_IMPORT_BYTES {
        return Err(format!(
            "Backup import exceeds the {} byte size limit",
            MAX_BACKUP_IMPORT_BYTES
        ));
    }
    Ok(canonical)
}

// ---------------------------------------------------------------------------
// Manifest v2: ajustes y metadatos de cuenta (ADR-0020)
// ---------------------------------------------------------------------------

/// Convierte una fila SQLite en un objeto JSON tipando cada celda en orden de
/// intento: entero, real, texto, blob. Los blobs (que no deben existir en
/// superficies de ajustes) viajan como texto base64 en vez de fallar el export.
fn sqlite_row_to_json(row: &sqlx::sqlite::SqliteRow) -> serde_json::Value {
    use base64::Engine as _;
    use sqlx::{Column, Row};

    let mut map = serde_json::Map::new();
    for (idx, col) in row.columns().iter().enumerate() {
        let value: serde_json::Value = if let Ok(v) = row.try_get::<Option<i64>, _>(idx) {
            v.map(serde_json::Value::from)
                .unwrap_or(serde_json::Value::Null)
        } else if let Ok(v) = row.try_get::<Option<f64>, _>(idx) {
            v.map(serde_json::Value::from)
                .unwrap_or(serde_json::Value::Null)
        } else if let Ok(v) = row.try_get::<Option<String>, _>(idx) {
            v.map(serde_json::Value::String)
                .unwrap_or(serde_json::Value::Null)
        } else if let Ok(v) = row.try_get::<Option<Vec<u8>>, _>(idx) {
            v.map(|b| {
                serde_json::Value::String(base64::engine::general_purpose::STANDARD.encode(b))
            })
            .unwrap_or(serde_json::Value::Null)
        } else {
            serde_json::Value::Null
        };
        map.insert(col.name().to_string(), value);
    }
    serde_json::Value::Object(map)
}

async fn export_settings_table_rows(
    db: &crate::DbPool,
    table: &str,
) -> Result<Vec<serde_json::Value>, String> {
    let sql = format!("SELECT * FROM {} ORDER BY id ASC", table);
    let rows = sqlx::query(&sql)
        .fetch_all(db)
        .await
        .map_err(|e| format!("Failed to export settings surface '{}': {}", table, e))?;
    Ok(rows.iter().map(sqlite_row_to_json).collect())
}

/// Exporta la KV de ajustes: solo las claves del catálogo de usuario
/// (`SETTINGS_KV_KEYS`, el mismo universo que borra el reset) y, dentro de
/// él, ninguna clave sensible: tokens, claves de API ni credenciales de
/// ningún tipo se quedan fuera del manifest (contrato testeable).
async fn export_settings_kv(
    db: &crate::DbPool,
) -> Result<std::collections::BTreeMap<String, String>, String> {
    let rows: Vec<(String, String)> =
        sqlx::query_as("SELECT key, value FROM settings ORDER BY key ASC")
            .fetch_all(db)
            .await
            .map_err(|e| format!("Failed to export settings KV: {}", e))?;

    Ok(rows
        .into_iter()
        .filter(|(key, _)| {
            crate::settings_defaults::SETTINGS_KV_KEYS.contains(&key.as_str())
                && !is_sensitive_setting_key(key)
        })
        .collect())
}

async fn export_settings_manifest(db: &crate::DbPool) -> Result<BackupSettingsDto, String> {
    Ok(BackupSettingsDto {
        kv: export_settings_kv(db).await?,
        service_preferences: export_settings_table_rows(db, "service_preferences").await?,
        sync_settings: export_settings_table_rows(db, "sync_settings").await?,
        service_sync_settings: export_settings_table_rows(db, "service_sync_settings").await?,
        quality_preferences: export_settings_table_rows(db, "quality_preferences").await?,
        folder_settings: export_settings_table_rows(db, "folder_settings").await?,
        duplicate_settings: export_settings_table_rows(db, "duplicate_settings").await?,
        audio_processing_settings: export_settings_table_rows(db, "audio_processing_settings")
            .await?,
        lyrics_provider_settings: export_settings_table_rows(db, "lyrics_provider_settings")
            .await?,
        lyrics_config: export_settings_table_rows(db, "lyrics_config").await?,
        metadata_preferences: export_settings_table_rows(db, "metadata_preferences").await?,
        advanced_settings: export_settings_table_rows(db, "advanced_settings").await?,
    })
}

/// Metadatos de cuenta: servicio, identidad externa y nombre. La consulta no
/// toca `credentials_json` ni ninguna columna de autenticación.
async fn export_account_metadata(db: &crate::DbPool) -> Result<Vec<BackupAccountDto>, String> {
    let rows: Vec<(String, Option<String>, Option<String>, Option<String>, i64)> = sqlx::query_as(
        r#"
            SELECT s.name, a.external_user_id, a.display_name, a.email, a.is_active
            FROM accounts a
            JOIN services s ON s.id = a.service_id
            ORDER BY s.name ASC, a.id ASC
            "#,
    )
    .fetch_all(db)
    .await
    .map_err(|e| format!("Failed to export account metadata: {}", e))?;

    Ok(rows
        .into_iter()
        .map(
            |(service, external_user_id, display_name, email, is_active)| BackupAccountDto {
                service,
                external_user_id,
                display_name,
                email,
                is_active: is_active != 0,
            },
        )
        .collect())
}

/// Export the full library into a portable, versioned backup manifest JSON file
#[tauri::command]
pub async fn export_library(
    state: State<'_, AppState>,
    output_path: Option<String>,
) -> Result<ExportLibraryResult, String> {
    let allowed_bases = get_allowed_backup_export_directories();
    export_library_with_bases(state, output_path, &allowed_bases).await
}

/// Export implementation with explicit sandbox bases for hermetic tests.
pub async fn export_library_with_bases(
    state: State<'_, AppState>,
    output_path: Option<String>,
    allowed_bases: &[std::path::PathBuf],
) -> Result<ExportLibraryResult, String> {
    // 1. Query all artists
    let artists_rows: Vec<(String, Option<String>)> =
        sqlx::query_as("SELECT name, favorite_at FROM artists ORDER BY name ASC")
            .fetch_all(&state.db)
            .await
            .map_err(|e| format!("Failed to export artists: {}", e))?;

    let backup_artists: Vec<BackupArtistDto> = artists_rows
        .into_iter()
        .map(|(name, favorite_at)| BackupArtistDto { name, favorite_at })
        .collect();

    // 2. Query all albums
    let albums_rows: Vec<(
        String,
        Option<String>,
        Option<String>,
        Option<String>,
        Option<String>,
    )> = sqlx::query_as(
        r#"
        SELECT a.title, art.name, a.upc, a.release_date, a.favorite_at
        FROM albums a
        LEFT JOIN album_artists aa ON aa.album_id = a.id
        LEFT JOIN artists art ON art.id = aa.artist_id
        ORDER BY a.title ASC
        "#,
    )
    .fetch_all(&state.db)
    .await
    .map_err(|e| format!("Failed to export albums: {}", e))?;

    let backup_albums: Vec<BackupAlbumDto> = albums_rows
        .into_iter()
        .map(
            |(title, artist, upc, release_date, favorite_at)| BackupAlbumDto {
                title,
                artist: artist.unwrap_or_else(|| "Unknown Artist".to_string()),
                upc,
                release_date,
                favorite_at,
            },
        )
        .collect();

    // 3. Query all tracks with sources & downloads
    let tracks_rows: Vec<(
        Option<String>,
        String,
        Option<String>,
        Option<String>,
        Option<i32>,
        Option<i32>,
        Option<i64>,
        Option<i32>,
        Option<String>,
        Option<String>,
        Option<String>,
        Option<String>,
    )> = sqlx::query_as(
        r#"
        SELECT
            t.isrc,
            t.title,
            COALESCE(art.name, 'Unknown Artist') as artist,
            a.title as album,
            t.track_number,
            t.disc_number,
            t.duration_ms,
            t.explicit,
            t.favorite_at,
            s.name as service,
            ts.service_track_id,
            d.file_format as downloaded_format
        FROM tracks t
        LEFT JOIN track_artists ta ON ta.track_id = t.id AND ta.role = 'primary'
        LEFT JOIN artists art ON art.id = ta.artist_id
        LEFT JOIN albums a ON a.id = t.album_id
        LEFT JOIN track_sources ts ON ts.track_id = t.id
        LEFT JOIN services s ON s.id = ts.service_id
        LEFT JOIN downloads d ON d.track_id = t.id
        ORDER BY t.title ASC
        "#,
    )
    .fetch_all(&state.db)
    .await
    .map_err(|e| format!("Failed to export tracks: {}", e))?;

    let backup_tracks: Vec<BackupTrackDto> = tracks_rows
        .into_iter()
        .map(
            |(
                isrc,
                title,
                artist,
                album,
                track_number,
                disc_number,
                duration_ms,
                explicit,
                favorite_at,
                service,
                service_track_id,
                downloaded_format,
            )| {
                BackupTrackDto {
                    isrc,
                    title,
                    artist: artist.unwrap_or_else(|| "Unknown Artist".to_string()),
                    album,
                    track_number,
                    disc_number,
                    duration_ms,
                    explicit,
                    favorite_at,
                    service,
                    service_track_id,
                    downloaded_format,
                }
            },
        )
        .collect();

    // 4. Query playlists
    let playlists_rows: Vec<(i64, String, Option<String>)> =
        sqlx::query_as("SELECT id, name, description FROM playlists ORDER BY name ASC")
            .fetch_all(&state.db)
            .await
            .map_err(|e| format!("Failed to export playlists: {}", e))?;

    let mut backup_playlists: Vec<BackupPlaylistDto> = Vec::new();
    for (pid, name, description) in playlists_rows {
        let isrcs: Vec<(Option<String>,)> = sqlx::query_as(
            r#"
            SELECT t.isrc
            FROM playlist_tracks pt
            JOIN tracks t ON t.id = pt.track_id
            WHERE pt.playlist_id = ?
            ORDER BY pt.position ASC
            "#,
        )
        .bind(pid)
        .fetch_all(&state.db)
        .await
        .unwrap_or_default();

        let track_isrcs: Vec<String> = isrcs.into_iter().filter_map(|(i,)| i).collect();
        backup_playlists.push(BackupPlaylistDto {
            name,
            description,
            track_isrcs,
        });
    }

    let tracks_count = backup_tracks.len();
    let albums_count = backup_albums.len();
    let artists_count = backup_artists.len();
    let playlists_count = backup_playlists.len();

    // Manifest v2 (ADR-0020): la biblioteca viaja como en v1, más todas las
    // superficies de ajustes y los metadatos de cuenta sin secretos. La
    // versión de esquema se deriva del estado real de esta BD.
    let schema_version = detect_schema_version(&state.db)
        .await
        .map_err(|e| format!("Failed to derive manifest schema version: {}", e))?;
    let settings = export_settings_manifest(&state.db).await?;
    let accounts = export_account_metadata(&state.db).await?;

    let mut manifest = LibraryBackupManifest {
        version: "2.0.0".to_string(),
        schema_version,
        exported_at: chrono::Utc::now().to_rfc3339(),
        app_version: env!("CARGO_PKG_VERSION").to_string(),
        checksum_sha256: None,
        tracks: backup_tracks,
        albums: backup_albums,
        artists: backup_artists,
        playlists: backup_playlists,
        settings: Some(settings),
        accounts: Some(accounts),
    };

    // Serialize to JSON without checksum to compute hash
    let raw_json = serde_json::to_string_pretty(&manifest)
        .map_err(|e| format!("Serialization error: {}", e))?;
    let checksum = compute_sha256_hex(raw_json.as_bytes());
    manifest.checksum_sha256 = Some(checksum.clone());

    // Serialize final with checksum
    let final_json = serde_json::to_string_pretty(&manifest)
        .map_err(|e| format!("Serialization error: {}", e))?;

    let raw_target = match output_path {
        Some(p) => {
            let trimmed = p.trim();
            if trimmed.is_empty() {
                return Err(
                    "Acceso denegado: la ruta de destino no puede estar vacía (sandbox violation)"
                        .to_string(),
                );
            }
            std::path::PathBuf::from(trimmed)
        }
        None => {
            let timestamp = chrono::Utc::now().format("%Y%m%d_%H%M%S");
            let filename = format!("Syncify_Backup_{}.json", timestamp);
            let default_dir = dirs::download_dir()
                .or_else(dirs::document_dir)
                .or_else(|| dirs::data_local_dir().map(|p| p.join("com.syncify.app")))
                .unwrap_or_else(|| std::path::PathBuf::from("."));
            default_dir.join(filename)
        }
    };

    let dest_path = validate_safe_backup_export_path_with_bases(&raw_target, allowed_bases)?;

    if let Some(parent) = dest_path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }

    std::fs::write(&dest_path, &final_json).map_err(|e| {
        format!(
            "Failed to write backup file to {}: {}",
            dest_path.display(),
            e
        )
    })?;

    let file_size_bytes = std::fs::metadata(&dest_path)
        .map(|m| m.len())
        .unwrap_or(final_json.len() as u64);

    Ok(ExportLibraryResult {
        file_path: dest_path.to_string_lossy().to_string(),
        tracks_count,
        albums_count,
        artists_count,
        playlists_count,
        file_size_bytes,
        checksum,
    })
}

// ---------------------------------------------------------------------------
// Import v2: ajustes por superficie y metadatos de cuenta sin sesiones
// (ADR-0020 decisión 1). El import de manifests v1 no entra en esta sección.
// ---------------------------------------------------------------------------

/// Major máximo de manifest aceptado por el import: 1.x (formato histórico)
/// y 2.x (ADR-0020). Un major mayor es una versión futura y se rechaza en
/// frío, antes de escribir nada.
const MAX_SUPPORTED_MANIFEST_MAJOR: u64 = 2;

/// Celda del manifest tipada para el bind dinámico. Los ajustes son
/// escalares: cualquier valor compuesto es un manifest inválido, no algo que
/// silenciar.
enum ManifestCell {
    Null,
    Int(i64),
    Real(f64),
    Text(String),
}

fn manifest_cell(value: &serde_json::Value) -> Result<ManifestCell, String> {
    match value {
        serde_json::Value::Null => Ok(ManifestCell::Null),
        serde_json::Value::Bool(b) => Ok(ManifestCell::Int(i64::from(*b))),
        serde_json::Value::Number(n) => {
            if let Some(i) = n.as_i64() {
                Ok(ManifestCell::Int(i))
            } else if let Some(f) = n.as_f64() {
                Ok(ManifestCell::Real(f))
            } else {
                Err("numeric value out of range".to_string())
            }
        }
        serde_json::Value::String(s) => Ok(ManifestCell::Text(s.clone())),
        other => {
            let kind = if other.is_object() { "object" } else { "array" };
            Err(format!(
                "composite value ({kind}) is not a valid settings cell"
            ))
        }
    }
}

fn manifest_major_version(version: &str) -> Result<u64, String> {
    let major = version.split('.').next().unwrap_or("").trim();
    major.parse::<u64>().map_err(|_| {
        format!(
            "Manifest de backup con versión no reconocida ('{version}'): se espera semver (mayor.menor.parche)"
        )
    })
}

/// Validación por superficie del payload v2 (ADR-0020), antes de abrir la
/// transacción: cada fila de cada superficie debe ser un objeto escalar.
/// Cualquier desviación aborta con diagnóstico claro sin haber escrito nada.
fn validate_settings_payload(settings: &BackupSettingsDto) -> Result<(), String> {
    let surfaces: [(&str, &Vec<serde_json::Value>); 11] = [
        ("service_preferences", &settings.service_preferences),
        ("sync_settings", &settings.sync_settings),
        ("service_sync_settings", &settings.service_sync_settings),
        ("quality_preferences", &settings.quality_preferences),
        ("folder_settings", &settings.folder_settings),
        ("duplicate_settings", &settings.duplicate_settings),
        (
            "audio_processing_settings",
            &settings.audio_processing_settings,
        ),
        (
            "lyrics_provider_settings",
            &settings.lyrics_provider_settings,
        ),
        ("lyrics_config", &settings.lyrics_config),
        ("metadata_preferences", &settings.metadata_preferences),
        ("advanced_settings", &settings.advanced_settings),
    ];

    for (surface, rows) in surfaces {
        for row in rows {
            let obj = row.as_object().ok_or_else(|| {
                format!(
                    "Invalid v2 manifest: settings surface '{surface}' must carry one JSON object per row"
                )
            })?;
            for (column, value) in obj {
                if value.is_object() || value.is_array() {
                    return Err(format!(
                        "Invalid v2 manifest: settings surface '{surface}' column '{column}' carries a composite value; only scalar cells are restorable"
                    ));
                }
            }
        }
    }
    Ok(())
}

/// Restaura una superficie de ajustes del manifest v2 con upsert por su clave
/// natural (el `id` autogenerado solo en singletons): los catálogos nunca
/// llevan el `id` del origen, la BD destino asigna el suyo. Solo se escriben
/// las columnas que la tabla destino realmente conoce — una columna que el
/// manifest traiga de más (futura o inyectada, p. ej. `credentials_json`) se
/// descarta, no se fabrica.
async fn restore_settings_surface(
    tx: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
    table: &'static str,
    conflict_keys: &[&str],
    carries_row_id: bool,
    rows: &[serde_json::Value],
) -> Result<i64, String> {
    if rows.is_empty() {
        return Ok(0);
    }

    // Columnas reales de la tabla destino. El nombre de tabla viene de las
    // llamadas de este módulo (whitelist), no del manifest.
    let target_columns: std::collections::HashSet<String> =
        sqlx::query_scalar::<_, String>("SELECT name FROM pragma_table_info(?)")
            .bind(table)
            .fetch_all(&mut **tx)
            .await
            .map_err(|e| format!("Failed to inspect settings surface '{table}': {e}"))?
            .into_iter()
            .collect();

    for row in rows {
        let obj = row.as_object().ok_or_else(|| {
            format!("Invalid v2 manifest: settings surface '{table}' must carry one JSON object per row")
        })?;

        let mut columns: Vec<&str> = Vec::new();
        let mut cells: Vec<ManifestCell> = Vec::new();
        for (key, value) in obj {
            if key == "id" && !carries_row_id {
                continue;
            }
            if !target_columns.contains(key) {
                continue;
            }
            cells.push(
                manifest_cell(value)
                    .map_err(|e| format!("Invalid v2 manifest: settings surface '{table}' column '{key}': {e}"))?,
            );
            columns.push(key.as_str());
        }

        if columns.is_empty() {
            return Err(format!(
                "Invalid v2 manifest: settings surface '{table}' row carries no column known to this schema"
            ));
        }
        for key in conflict_keys {
            if !columns.contains(key) {
                return Err(format!(
                    "Invalid v2 manifest: settings surface '{table}' row is missing its identity column '{key}'"
                ));
            }
        }
        let update_columns: Vec<&str> = columns
            .iter()
            .copied()
            .filter(|c| c != &"id" && !conflict_keys.contains(c))
            .collect();
        if update_columns.is_empty() {
            return Err(format!(
                "Invalid v2 manifest: settings surface '{table}' row carries no restorable column beyond its identity"
            ));
        }

        let sql = format!(
            "INSERT INTO {table} ({}) VALUES ({}) ON CONFLICT ({}) DO UPDATE SET {}",
            columns.join(", "),
            vec!["?"; columns.len()].join(", "),
            conflict_keys.join(", "),
            update_columns
                .iter()
                .map(|c| format!("{c} = excluded.{c}"))
                .collect::<Vec<_>>()
                .join(", "),
        );

        let mut query = sqlx::query(&sql);
        for cell in &cells {
            query = match cell {
                ManifestCell::Null => query.bind(None::<i64>),
                ManifestCell::Int(v) => query.bind(*v),
                ManifestCell::Real(v) => query.bind(*v),
                ManifestCell::Text(v) => query.bind(v.clone()),
            };
        }
        query
            .execute(&mut **tx)
            .await
            .map_err(|e| format!("Failed to restore settings surface '{table}': {e}"))?;
    }

    Ok(rows.len() as i64)
}

/// Restaura la KV de ajustes con la misma selección del export: solo claves
/// del catálogo de usuario y ninguna sensible. Un manifest que traiga una
/// clave sensible o desconocida no la escribe: la restauración no fabrica
/// credenciales (contrato testeable de ADR-0020).
async fn restore_settings_kv(
    tx: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
    kv: &std::collections::BTreeMap<String, String>,
) -> Result<i64, String> {
    let mut restored = 0i64;
    for (key, value) in kv {
        if !crate::settings_defaults::SETTINGS_KV_KEYS.contains(&key.as_str())
            || is_sensitive_setting_key(key)
        {
            continue;
        }
        sqlx::query(
            "INSERT INTO settings (key, value, updated_at) VALUES (?, ?, datetime('now'))
             ON CONFLICT(key) DO UPDATE SET value = excluded.value, updated_at = excluded.updated_at",
        )
        .bind(key)
        .bind(value)
        .execute(&mut **tx)
        .await
        .map_err(|e| format!("Failed to restore settings KV '{key}': {e}"))?;
        restored += 1;
    }
    Ok(restored)
}

async fn restore_settings_manifest(
    tx: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
    settings: &BackupSettingsDto,
) -> Result<i64, String> {
    let mut restored = 0i64;
    restored += restore_settings_surface(
        tx,
        "service_preferences",
        &["service_name"],
        false,
        &settings.service_preferences,
    )
    .await?;
    restored += restore_settings_surface(tx, "sync_settings", &["id"], true, &settings.sync_settings)
        .await?;
    restored += restore_settings_surface(
        tx,
        "service_sync_settings",
        &["service_name"],
        false,
        &settings.service_sync_settings,
    )
    .await?;
    restored += restore_settings_surface(
        tx,
        "quality_preferences",
        &["service_name"],
        false,
        &settings.quality_preferences,
    )
    .await?;
    restored += restore_settings_surface(
        tx,
        "folder_settings",
        &["id"],
        true,
        &settings.folder_settings,
    )
    .await?;
    restored += restore_settings_surface(
        tx,
        "duplicate_settings",
        &["id"],
        true,
        &settings.duplicate_settings,
    )
    .await?;
    restored += restore_settings_surface(
        tx,
        "audio_processing_settings",
        &["id"],
        true,
        &settings.audio_processing_settings,
    )
    .await?;
    restored += restore_settings_surface(
        tx,
        "lyrics_provider_settings",
        &["provider_id"],
        false,
        &settings.lyrics_provider_settings,
    )
    .await?;
    restored += restore_settings_surface(tx, "lyrics_config", &["id"], true, &settings.lyrics_config)
        .await?;
    restored += restore_settings_surface(
        tx,
        "metadata_preferences",
        &["id"],
        true,
        &settings.metadata_preferences,
    )
    .await?;
    restored += restore_settings_surface(
        tx,
        "advanced_settings",
        &["id"],
        true,
        &settings.advanced_settings,
    )
    .await?;
    restored += restore_settings_kv(tx, &settings.kv).await?;
    Ok(restored)
}

/// Aplica los metadatos de cuenta del manifest v2 SIN crear sesiones: al
/// insertar, `credentials_json` queda NULL y la cuenta queda a la espera de
/// reautenticar; al actualizar una cuenta existente, `credentials_json` no se
/// toca jamás. Devuelve los servicios de las cuentas recién insertadas, que
/// piden login al terminar la restauración.
async fn restore_account_metadata(
    tx: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
    accounts: &[BackupAccountDto],
) -> Result<Vec<String>, String> {
    let mut awaiting_reauth = Vec::new();
    for account in accounts {
        let service_id: i64 =
            match sqlx::query_scalar::<_, i64>("SELECT id FROM services WHERE name = ?")
                .bind(&account.service)
                .fetch_optional(&mut **tx)
                .await
                .map_err(|e| format!("Failed to resolve service '{}': {}", account.service, e))?
            {
                Some(id) => id,
                None => sqlx::query_scalar("INSERT INTO services (name) VALUES (?) RETURNING id")
                    .bind(&account.service)
                    .fetch_one(&mut **tx)
                    .await
                    .map_err(|e| format!("Failed to create service '{}': {}", account.service, e))?,
            };

        // Deduplicación NULL-safe para que reimportar sea idempotente:
        // identidad externa, luego email, y como último recurso la cuenta del
        // servicio sin identidad ninguna.
        let existing_id: Option<i64> = match (&account.external_user_id, &account.email) {
            (Some(external), _) => {
                sqlx::query_scalar::<_, i64>(
                    "SELECT id FROM accounts WHERE service_id = ? AND external_user_id IS ? LIMIT 1",
                )
                .bind(service_id)
                .bind(external)
                .fetch_optional(&mut **tx)
                .await
                .map_err(|e| {
                    format!(
                        "Failed to look up account for service '{}': {}",
                        account.service, e
                    )
                })?
            }
            (None, Some(email)) => {
                sqlx::query_scalar::<_, i64>(
                    "SELECT id FROM accounts WHERE service_id = ? AND email IS ? LIMIT 1",
                )
                .bind(service_id)
                .bind(email)
                .fetch_optional(&mut **tx)
                .await
                .map_err(|e| {
                    format!(
                        "Failed to look up account for service '{}': {}",
                        account.service, e
                    )
                })?
            }
            (None, None) => {
                sqlx::query_scalar::<_, i64>(
                    "SELECT id FROM accounts WHERE service_id = ? AND external_user_id IS NULL AND email IS NULL ORDER BY id ASC LIMIT 1",
                )
                .bind(service_id)
                .fetch_optional(&mut **tx)
                .await
                .map_err(|e| {
                    format!(
                        "Failed to look up account for service '{}': {}",
                        account.service, e
                    )
                })?
            }
        };

        match existing_id {
            Some(id) => {
                // Sin sesión: credentials_json no aparece en esta sentencia.
                sqlx::query(
                    "UPDATE accounts SET display_name = ?, email = ?, \
                     external_user_id = COALESCE(external_user_id, ?), is_active = ? WHERE id = ?",
                )
                .bind(&account.display_name)
                .bind(&account.email)
                .bind(&account.external_user_id)
                .bind(i64::from(account.is_active))
                .bind(id)
                .execute(&mut **tx)
                .await
                .map_err(|e| {
                    format!(
                        "Failed to update account for service '{}': {}",
                        account.service, e
                    )
                })?;
            }
            None => {
                sqlx::query(
                    "INSERT INTO accounts (service_id, display_name, email, external_user_id, is_active)
                     VALUES (?, ?, ?, ?, ?)",
                )
                .bind(service_id)
                .bind(&account.display_name)
                .bind(&account.email)
                .bind(&account.external_user_id)
                .bind(i64::from(account.is_active))
                .execute(&mut **tx)
                .await
                .map_err(|e| {
                    format!(
                        "Failed to insert account for service '{}': {}",
                        account.service, e
                    )
                })?;
                awaiting_reauth.push(account.service.clone());
            }
        }
    }
    Ok(awaiting_reauth)
}

/// ¿Trae este nombre de artista del manifest un artista restaurable?
///
/// El export fabrica `'Unknown Artist'` (COALESCE del JOIN) cuando un álbum
/// o pista no tiene artista enlazado, y manifests v1 antiguos pueden llevar
/// filas de artista con nombres basura. La migración 0079 los rechaza en la
/// BD destino (`trg_artists_reject_garbage_ins`: vacío, 'unknown',
/// 'unknown artist', '\\p', '\\\\p'), así que el import los trata como «sin
/// artista»: no crea la fila ni enlaza album_artists/track_artists.
fn artist_name_is_restorable(name: &str) -> bool {
    let trimmed = name.trim();
    !trimmed.is_empty()
        && !matches!(
            trimmed.to_ascii_lowercase().as_str(),
            "unknown" | "unknown artist" | "\\p" | "\\\\p"
        )
}

/// Import a backup manifest into the database with validation and atomic rollback
#[tauri::command]
pub async fn import_library(
    state: State<'_, AppState>,
    file_path: String,
    ignore_checksum_error: Option<bool>,
) -> Result<ImportLibraryResult, String> {
    let allowed_bases = get_allowed_backup_export_directories();
    import_library_with_bases(state, file_path, ignore_checksum_error, &allowed_bases).await
}

/// Import implementation with explicit sandbox bases for hermetic tests.
pub async fn import_library_with_bases(
    state: State<'_, AppState>,
    file_path: String,
    ignore_checksum_error: Option<bool>,
    allowed_bases: &[std::path::PathBuf],
) -> Result<ImportLibraryResult, String> {
    let p = validate_safe_backup_import_path_with_bases(
        std::path::Path::new(&file_path),
        allowed_bases,
    )?;
    let content = tokio::fs::read_to_string(&p)
        .await
        .map_err(|e| format!("Failed to read backup file: {}", e))?;

    let manifest: LibraryBackupManifest = serde_json::from_str(&content)
        .map_err(|e| format!("Invalid backup manifest format: {}", e))?;

    // Puerta de versión futura (ADR-0020): un manifest de un major mayor que
    // el soportado se rechaza antes de tocar nada, aunque sus campos se
    // deserialicen parcialmente. Diagnóstico claro y cero escrituras.
    let manifest_major = manifest_major_version(&manifest.version)?;
    if manifest_major > MAX_SUPPORTED_MANIFEST_MAJOR {
        return Err(format!(
            "El manifest de backup es de una versión futura ({}): este import soporta 1.x y 2.x. \
             Actualiza la app e inténtalo de nuevo; nada se ha restaurado.",
            manifest.version
        ));
    }

    // Validación por superficie del payload v2 (si viaja), antes de abrir la
    // transacción: falla seguro sin escribir a medias.
    if let Some(settings) = &manifest.settings {
        validate_settings_payload(settings)?;
    }

    // Checksum verification (if provided)
    if let Some(expected_checksum) = &manifest.checksum_sha256 {
        // Recompute on manifest without checksum field
        let mut cloned = manifest.clone();
        cloned.checksum_sha256 = None;
        if let Ok(raw) = serde_json::to_string_pretty(&cloned) {
            let computed = compute_sha256_hex(raw.as_bytes());
            if computed != *expected_checksum && !ignore_checksum_error.unwrap_or(false) {
                return Err(format!(
                    "Backup file checksum mismatch (expected {}, got {}). File may be corrupted or modified.",
                    expected_checksum, computed
                ));
            }
        }
    }

    // Puerta de esquema (ADR-0020): un manifest derivado de un esquema más
    // nuevo que esta BD puede llevar columnas que aquí no existen; se rechaza
    // en frío con diagnóstico claro, sin abrir siquiera la transacción.
    let target_schema_version = detect_schema_version(&state.db)
        .await
        .map_err(|e| format!("Failed to detect target schema version: {}", e))?;
    if manifest.schema_version > target_schema_version {
        return Err(format!(
            "El manifest procede de un esquema más nuevo ({}) que esta instalación ({}): \
             actualiza la app antes de restaurar; nada se ha restaurado.",
            manifest.schema_version, target_schema_version
        ));
    }

    // Atomic SQLite Transaction
    let mut tx = state
        .db
        .begin_with("BEGIN IMMEDIATE")
        .await
        .map_err(|e| format!("Failed to start database transaction: {}", e))?;

    let mut artists_imported = 0i64;
    let mut albums_imported = 0i64;
    let mut tracks_imported = 0i64;
    let mut playlists_imported = 0i64;
    let mut favorites_restored = 0i64;

    // 1. Import Artists
    for artist in manifest
        .artists
        .iter()
        .filter(|a| artist_name_is_restorable(&a.name))
    {
        let res = sqlx::query(
            r#"
            INSERT INTO artists (name, favorite_at)
            VALUES (?, ?)
            ON CONFLICT(name) DO UPDATE SET
                favorite_at = COALESCE(excluded.favorite_at, artists.favorite_at)
            "#,
        )
        .bind(&artist.name)
        .bind(&artist.favorite_at)
        .execute(&mut *tx)
        .await;

        if res.is_ok() {
            artists_imported += 1;
            if artist.favorite_at.is_some() {
                favorites_restored += 1;
            }
        }
    }

    // 2. Import Albums
    for album in &manifest.albums {
        // Ensure artist exists. El export fabrica 'Unknown Artist' cuando el
        // álbum no tiene artista enlazado; la migración 0079 rechaza ese
        // nombre como basura (trg_artists_reject_garbage_ins), así que el
        // import lo trata como «sin artista»: no crea la fila ni la enlaza.
        let artist_id: Option<i64> = if artist_name_is_restorable(&album.artist) {
            Some(
                match sqlx::query_scalar::<_, i64>("SELECT id FROM artists WHERE name = ?")
                    .bind(&album.artist)
                    .fetch_optional(&mut *tx)
                    .await
                    .ok()
                    .flatten()
                {
                    Some(id) => id,
                    None => {
                        sqlx::query_scalar("INSERT INTO artists (name) VALUES (?) RETURNING id")
                            .bind(&album.artist)
                            .fetch_one(&mut *tx)
                            .await
                            .map_err(|e| {
                                format!("Failed to create artist {}: {}", album.artist, e)
                            })?
                    }
                },
            )
        } else {
            None
        };

        let album_id: i64 = match sqlx::query_scalar::<_, i64>("SELECT id FROM albums WHERE title = ?")
            .bind(&album.title)
            .fetch_optional(&mut *tx)
            .await
            .ok()
            .flatten()
        {
            Some(id) => {
                if let Some(fav) = &album.favorite_at {
                    let _ = crate::db::write_or_log(
                        "restore_backup",
                        sqlx::query("UPDATE albums SET favorite_at = ? WHERE id = ?")
                            .bind(fav)
                            .bind(id)
                            .execute(&mut *tx),
                    )
                    .await;
                }
                id
            }
            None => {
                sqlx::query_scalar(
                    "INSERT INTO albums (title, upc, release_date, favorite_at) VALUES (?, ?, ?, ?) RETURNING id"
                )
                .bind(&album.title)
                .bind(&album.upc)
                .bind(&album.release_date)
                .bind(&album.favorite_at)
                .fetch_one(&mut *tx)
                .await
                .map_err(|e| format!("Failed to insert album {}: {}", album.title, e))?
            }
        };

        // Link album_artists (solo si el manifest trajo artista restaurable)
        if let Some(artist_id) = artist_id {
            let _ = crate::db::write_or_log(
                "restore_backup",
                sqlx::query("INSERT OR IGNORE INTO album_artists (album_id, artist_id) VALUES (?, ?)")
                    .bind(album_id)
                    .bind(artist_id)
                    .execute(&mut *tx),
            )
            .await;
        }

        albums_imported += 1;
        if album.favorite_at.is_some() {
            favorites_restored += 1;
        }
    }

    // 3. Import Tracks
    for track in &manifest.tracks {
        // Resolve artist. Mismo trato que los álbumes: el sentinel
        // 'Unknown Artist' del export no es un artista restaurable (0079).
        let artist_id: Option<i64> = if artist_name_is_restorable(&track.artist) {
            Some(
                match sqlx::query_scalar::<_, i64>("SELECT id FROM artists WHERE name = ?")
                    .bind(&track.artist)
                    .fetch_optional(&mut *tx)
                    .await
                    .ok()
                    .flatten()
                {
                    Some(id) => id,
                    None => {
                        sqlx::query_scalar("INSERT INTO artists (name) VALUES (?) RETURNING id")
                            .bind(&track.artist)
                            .fetch_one(&mut *tx)
                            .await
                            .map_err(|e| {
                                format!("Failed to create artist {}: {}", track.artist, e)
                            })?
                    }
                },
            )
        } else {
            None
        };

        // Resolve album if any
        let album_id: Option<i64> = if let Some(alb_name) = &track.album {
            sqlx::query_scalar::<_, i64>("SELECT id FROM albums WHERE title = ?")
                .bind(alb_name)
                .fetch_optional(&mut *tx)
                .await
                .ok()
                .flatten()
        } else {
            None
        };

        // Upsert track (deduplicating by ISRC if present, otherwise by title & album_id)
        let track_id: i64 = if let Some(isrc_val) = &track.isrc {
            match sqlx::query_scalar::<_, i64>("SELECT id FROM tracks WHERE isrc = ?")
                .bind(isrc_val)
                .fetch_optional(&mut *tx)
                .await
                .ok()
                .flatten()
            {
                Some(existing_id) => {
                    if let Some(fav) = &track.favorite_at {
                        let _ = crate::db::write_or_log(
                            "restore_backup",
                            sqlx::query("UPDATE tracks SET favorite_at = ? WHERE id = ?")
                                .bind(fav)
                                .bind(existing_id)
                                .execute(&mut *tx),
                        )
                        .await;
                    }
                    existing_id
                }
                None => {
                    sqlx::query_scalar(
                        r#"
                        INSERT INTO tracks (title, album_id, duration_ms, track_number, disc_number, isrc, explicit, favorite_at)
                        VALUES (?, ?, ?, ?, ?, ?, ?, ?)
                        RETURNING id
                        "#
                    )
                    .bind(&track.title)
                    .bind(album_id)
                    .bind(track.duration_ms)
                    .bind(track.track_number)
                    .bind(track.disc_number.unwrap_or(1))
                    .bind(&track.isrc)
                    .bind(track.explicit.unwrap_or(0))
                    .bind(&track.favorite_at)
                    .fetch_one(&mut *tx)
                    .await
                    .map_err(|e| format!("Failed to insert track {}: {}", track.title, e))?
                }
            }
        } else {
            match sqlx::query_scalar::<_, i64>("SELECT id FROM tracks WHERE title = ? AND (album_id = ? OR (album_id IS NULL AND ? IS NULL))")
                .bind(&track.title)
                .bind(album_id)
                .bind(album_id)
                .fetch_optional(&mut *tx)
                .await
                .ok()
                .flatten()
            {
                Some(existing_id) => {
                    if let Some(fav) = &track.favorite_at {
                        let _ = crate::db::write_or_log(
                            "restore_backup",
                            sqlx::query("UPDATE tracks SET favorite_at = ? WHERE id = ?")
                                .bind(fav)
                                .bind(existing_id)
                                .execute(&mut *tx),
                        )
                        .await;
                    }
                    existing_id
                }
                None => {
                    sqlx::query_scalar(
                        r#"
                        INSERT INTO tracks (title, album_id, duration_ms, track_number, disc_number, isrc, explicit, favorite_at)
                        VALUES (?, ?, ?, ?, ?, ?, ?, ?)
                        RETURNING id
                        "#
                    )
                    .bind(&track.title)
                    .bind(album_id)
                    .bind(track.duration_ms)
                    .bind(track.track_number)
                    .bind(track.disc_number.unwrap_or(1))
                    .bind(&track.isrc)
                    .bind(track.explicit.unwrap_or(0))
                    .bind(&track.favorite_at)
                    .fetch_one(&mut *tx)
                    .await
                    .map_err(|e| format!("Failed to insert track {}: {}", track.title, e))?
                }
            }
        };

        // Link track_artists (solo si el manifest trajo artista restaurable)
        if let Some(artist_id) = artist_id {
            let _ = crate::db::write_or_log(
                "restore_backup",
                sqlx::query(
                    "INSERT OR IGNORE INTO track_artists (track_id, artist_id, role) VALUES (?, ?, 'primary')"
                )
                .bind(track_id)
                .bind(artist_id)
                .execute(&mut *tx),
            )
            .await;
        }

        tracks_imported += 1;
        if track.favorite_at.is_some() {
            favorites_restored += 1;
        }
    }

    // 4. Import Playlists
    for pl in &manifest.playlists {
        // Find or create playlist
        let pl_id: i64 = sqlx::query_scalar(
            "INSERT INTO playlists (account_id, name, description) VALUES (1, ?, ?) RETURNING id",
        )
        .bind(&pl.name)
        .bind(&pl.description)
        .fetch_one(&mut *tx)
        .await
        .map_err(|e| format!("Failed to create playlist {}: {}", pl.name, e))?;

        for (pos, isrc) in pl.track_isrcs.iter().enumerate() {
            if let Some(tid) = sqlx::query_scalar::<_, i64>("SELECT id FROM tracks WHERE isrc = ?")
                .bind(isrc)
                .fetch_optional(&mut *tx)
                .await
                .ok()
                .flatten()
            {
                let _ = crate::db::write_or_log(
                    "restore_backup",
                    sqlx::query(
                        "INSERT OR IGNORE INTO playlist_tracks (playlist_id, track_id, position) VALUES (?, ?, ?)"
                    )
                    .bind(pl_id)
                    .bind(tid)
                    .bind(pos as i64)
                    .execute(&mut *tx),
                )
                .await;
            }
        }

        playlists_imported += 1;
    }

    // 5. Manifest v2 (ADR-0020): ajustes por superficie y metadatos de cuenta.
    // En manifests v1 ambos campos son None y este bloque no se ejecuta.
    let mut settings_rows_restored = 0i64;
    if let Some(settings) = &manifest.settings {
        settings_rows_restored += restore_settings_manifest(&mut tx, settings).await?;
    }
    let accounts_awaiting_reauth = if let Some(accounts) = &manifest.accounts {
        restore_account_metadata(&mut tx, accounts).await?
    } else {
        Vec::new()
    };
    let accounts_restored = manifest
        .accounts
        .as_ref()
        .map_or(0, |accounts| accounts.len() as i64);

    tx.commit()
        .await
        .map_err(|e| format!("Failed to commit import transaction: {}", e))?;

    let mut message = format!(
        "Successfully imported {} tracks, {} albums, {} artists, {} playlists with {} favorites restored",
        tracks_imported, albums_imported, artists_imported, playlists_imported, favorites_restored
    );
    if settings_rows_restored > 0 {
        message.push_str(&format!(", {} settings restored", settings_rows_restored));
    }
    if !accounts_awaiting_reauth.is_empty() {
        message.push_str(&format!(
            "; {} account(s) await re-authentication: {}",
            accounts_awaiting_reauth.len(),
            accounts_awaiting_reauth.join(", ")
        ));
    }

    Ok(ImportLibraryResult {
        tracks_imported,
        albums_imported,
        artists_imported,
        playlists_imported,
        favorites_restored,
        settings_rows_restored,
        accounts_restored,
        accounts_awaiting_reauth,
        message,
    })
}

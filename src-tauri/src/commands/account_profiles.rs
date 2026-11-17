//! Perfiles de webview por cuenta, cifrados en reposo (issue #86).
//!
//! Contrato: ADR-0007, decisión 2 — UN perfil de webview por cuenta; el
//! directorio de datos se conserva CIFRADO EN REPOSO con la misma clave que
//! las credenciales (AES-256-GCM, [`crate::crypto`]); se descifra a un
//! directorio temporal SOLO mientras la ventana de login está abierta y se
//! vuelve a cifrar al cerrarla; ningún texto plano persistente queda en
//! disco. Los temporales huérfanos (cierre abrupto) se limpian al arrancar y
//! un perfil que no descifra pasa la cuenta a «sin sesión» sin bloquear la
//! app: la ficha ofrece reautenticar (criterios del issue #86).
//!
//! Cerrar sesión y eliminar borran el archivo; desactivar y archivar (ciclo
//! de vida de la migración 0108) lo conservan — el borrado del FICHERO es
//! explícito aquí: la FK de `account_profiles` solo impide filas huérfanas.
//!
//! El contenedor `SYNCPROF1`: manifiesto JSON del árbol (walkdir ordenado,
//! determinista), prefijo de longitud del manifiesto, blobs concatenados, gzip
//! y cifrado binario. Cada prueba de unidad del módulo ejercita el ciclo
//! completo con directorios temporales y webview falso (contenido inventado),
//! sin red.

use std::io::{Read as _, Write as _};
use std::path::{Path, PathBuf};

use sqlx::SqlitePool;

/// Directorio base de los ARCHIVOS cifrados de perfil, relativo al
/// app_local_data_dir. Separado del base de temporales de login
/// (`account_login::LOGIN_PROFILE_BASE`): la higiene de arranque borra
/// cualquier subdirectorio de los temporales y SOLO de los temporales.
pub const PROFILE_ARCHIVE_BASE: &str = "profile-archives";

/// Extensión de los archivos de perfil cifrados.
pub const PROFILE_ARCHIVE_EXT: &str = "syncprof";

/// `account_profiles.status`: el perfil está cifrado en reposo y legible.
pub const PROFILE_STATUS_ARCHIVED: &str = "archived";

/// `account_profiles.status`: el archivo no descifra con la clave activa
/// (clave perdida, base restaurada en otra máquina, fichero dañado).
pub const PROFILE_STATUS_UNRECOVERABLE: &str = "unrecoverable";

/// `accounts.invalid_reason` de una cuenta cuyo perfil no descifra: la ficha
/// ofrece reautenticar (criterio del issue #86), igual que una credencial
/// caducada.
pub const INVALID_REASON_PROFILE_UNREADABLE: &str = "profile_unreadable";

/// Magia del contenedor: una página o fichero ajeno nunca empieza así.
const PROFILE_ARCHIVE_MAGIC: &[u8; 9] = b"SYNCPROF1";

// ==============================================
// CONTENEDOR CIFRADO DEL PERFIL
// ==============================================

#[derive(serde::Serialize, serde::Deserialize)]
struct ArchivedFile {
    path: String,
    size: u64,
}

#[derive(serde::Serialize, serde::Deserialize)]
struct ArchiveManifest {
    files: Vec<ArchivedFile>,
    total_size: u64,
}

/// Serializa el árbol de `dir` al contenedor CIFRADO. Determinista: las
/// entradas van ordenadas por ruta relativa, así que el mismo árbol produce
/// el mismo manifiesto (los nonces del GCM son aleatorios).
///
/// Los directorios vacíos no se representan: el perfil de un webview no
/// depende de ellos para funcionar.
pub fn archive_directory(dir: &Path, key: &[u8; 32]) -> Result<Vec<u8>, String> {
    if !dir.is_dir() {
        return Err(format!(
            "profile directory '{}' does not exist or is not a directory",
            dir.display()
        ));
    }

    let mut entries: Vec<PathBuf> = walkdir::WalkDir::new(dir)
        .follow_links(false)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().is_file())
        .map(|e| e.into_path())
        .collect();
    entries.sort();
    entries.dedup();

    let mut files = Vec::with_capacity(entries.len());
    let mut blobs: Vec<u8> = Vec::new();
    for entry in &entries {
        let relative = entry
            .strip_prefix(dir)
            .map_err(|e| format!("Failed to relativize {}: {}", entry.display(), e))?
            .to_string_lossy()
            .replace('\\', "/");
        let contents = std::fs::read(entry)
            .map_err(|e| format!("Failed to read profile file {}: {}", entry.display(), e))?;
        files.push(ArchivedFile {
            path: relative,
            size: contents.len() as u64,
        });
        blobs.extend_from_slice(&contents);
    }

    let manifest = ArchiveManifest {
        files,
        total_size: blobs.len() as u64,
    };
    let manifest_json = serde_json::to_vec(&manifest)
        .map_err(|e| format!("Failed to serialize profile manifest: {}", e))?;

    // Cuerpo comprimido: u32 LE de longitud del manifiesto || manifiesto || blobs.
    let mut body: Vec<u8> = Vec::with_capacity(4 + manifest_json.len() + blobs.len());
    body.extend_from_slice(&(manifest_json.len() as u32).to_le_bytes());
    body.extend_from_slice(&manifest_json);
    body.extend_from_slice(&blobs);

    let mut compressed = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::default());
    compressed
        .write_all(&body)
        .map_err(|e| format!("Failed to compress profile archive: {}", e))?;
    let gz = compressed
        .finish()
        .map_err(|e| format!("Failed to finish profile archive: {}", e))?;

    let mut out = PROFILE_ARCHIVE_MAGIC.to_vec();
    out.extend(crate::crypto::encrypt_bytes_with_key(&gz, key)?);
    Ok(out)
}

/// Reconstruye el árbol desde un contenedor cifrado en `dest`. Devuelve el
/// número de ficheros restaurados. Falla si la magia no coincide, la etiqueta
/// GCM no valida (clave equivocada o manipulación) o el cuerpo no cuadra con
/// el manifiesto.
///
/// Sin llamante en producción todavía: las issues #87 y #91 (bloqueadas por
/// #86) materializan perfiles cifrados. `expect` no cabe aquí (en el objetivo
/// lib el lint no dispara y la expectativa quedaría sin cumplir); el `allow`
/// es inerte en lib y solo calla el bin.
#[allow(dead_code)]
pub fn restore_directory(archive: &[u8], dest: &Path, key: &[u8; 32]) -> Result<usize, String> {
    if archive.len() < PROFILE_ARCHIVE_MAGIC.len() + 12 {
        return Err("profile archive too short to be valid".into());
    }
    let (magic, encrypted) = archive.split_at(PROFILE_ARCHIVE_MAGIC.len());
    if magic != PROFILE_ARCHIVE_MAGIC {
        return Err("not a syncify profile archive (bad magic)".into());
    }

    let gz = crate::crypto::decrypt_bytes_with_key(encrypted, key)?;
    let mut decoder = flate2::read::GzDecoder::new(gz.as_slice());
    let mut body = Vec::new();
    decoder
        .read_to_end(&mut body)
        .map_err(|e| format!("Failed to decompress profile archive: {}", e))?;

    if body.len() < 4 {
        return Err("profile archive body truncated".into());
    }
    let manifest_len = u32::from_le_bytes([body[0], body[1], body[2], body[3]]) as usize;
    if body.len() < 4 + manifest_len {
        return Err("profile archive manifest truncated".into());
    }
    let manifest: ArchiveManifest = serde_json::from_slice(&body[4..4 + manifest_len])
        .map_err(|e| format!("Failed to parse profile manifest: {}", e))?;
    let blobs = &body[4 + manifest_len..];
    if blobs.len() as u64 != manifest.total_size {
        return Err(format!(
            "profile archive blob length mismatch: manifest says {} bytes, body carries {}",
            manifest.total_size,
            blobs.len()
        ));
    }

    std::fs::create_dir_all(dest).map_err(|e| {
        format!(
            "Failed to create restore directory {}: {}",
            dest.display(),
            e
        )
    })?;
    harden_dir_permissions(dest);

    let mut offset: usize = 0;
    for file in &manifest.files {
        let end = offset + file.size as usize;
        if end > blobs.len() {
            return Err(format!(
                "profile archive blob for '{}' exceeds the body",
                file.path
            ));
        }
        let target = safe_join(dest, &file.path)?;
        if let Some(parent) = target.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|e| format!("Failed to create directory {}: {}", parent.display(), e))?;
            harden_dir_permissions(parent);
        }
        std::fs::write(&target, &blobs[offset..end])
            .map_err(|e| format!("Failed to write {}: {}", target.display(), e))?;
        harden_file_permissions(&target);
        offset = end;
    }

    Ok(manifest.files.len())
}

/// Ruta bajo `dest` que rechaza rutas absolutas y componentes `..`: el
/// manifiesto se restauró de bytes descifrados y no se le da poder de
/// escribir fuera del directorio temporal. Ratchet: ver [`restore_directory`].
#[allow(dead_code)]
fn safe_join(dest: &Path, relative: &str) -> Result<PathBuf, String> {
    let relative_path = Path::new(relative);
    if relative_path.is_absolute() {
        return Err(format!(
            "profile manifest carries an absolute path '{}'",
            relative
        ));
    }
    for component in relative_path.components() {
        if matches!(component, std::path::Component::ParentDir) {
            return Err(format!(
                "profile manifest carries a parent component in '{}'",
                relative
            ));
        }
    }
    Ok(dest.join(relative_path))
}

fn harden_dir_permissions(path: &Path) {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o700));
    }
    #[cfg(not(unix))]
    let _ = path;
}

fn harden_file_permissions(path: &Path) {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600));
    }
    #[cfg(not(unix))]
    let _ = path;
}

/// Cifra el árbol de `dir` y lo escribe en `out_path` (escritura atómica
/// tmp+rename, 0600 en Unix). El texto plano del perfil solo existe en
/// `dir`, que el llamante borra SIEMPRE a continuación.
pub fn archive_profile_dir_to_file(
    dir: &Path,
    out_path: &Path,
    key: &[u8; 32],
) -> Result<(), String> {
    let bytes = archive_directory(dir, key)?;
    if let Some(parent) = out_path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| {
            format!(
                "Failed to create archive directory {}: {}",
                parent.display(),
                e
            )
        })?;
        harden_dir_permissions(parent);
    }
    let tmp = out_path.with_extension(format!("{}.tmp", PROFILE_ARCHIVE_EXT));
    std::fs::write(&tmp, &bytes)
        .map_err(|e| format!("Failed to write profile archive {}: {}", tmp.display(), e))?;
    harden_file_permissions(&tmp);
    std::fs::rename(&tmp, out_path).map_err(|e| {
        format!(
            "Failed to finalize profile archive {}: {}",
            out_path.display(),
            e
        )
    })?;
    Ok(())
}

/// Restaura el contenedor de `archive_path` en `dest_dir`.
/// Ratchet: ver [`restore_directory`].
#[allow(dead_code)]
pub fn restore_profile_dir_from_file(
    archive_path: &Path,
    dest_dir: &Path,
    key: &[u8; 32],
) -> Result<usize, String> {
    let bytes = std::fs::read(archive_path).map_err(|e| {
        format!(
            "Failed to read profile archive {}: {}",
            archive_path.display(),
            e
        )
    })?;
    restore_directory(&bytes, dest_dir, key)
}

/// Comprobación de descifrado SIN escribir nada en disco: la higiene de
/// arranque la usa para detectar perfiles ilegibles (criterio del issue #86)
/// sin materializar texto plano.
fn decrypt_check(archive_path: &Path, key: &[u8; 32]) -> Result<(), String> {
    let bytes = std::fs::read(archive_path).map_err(|e| {
        format!(
            "Failed to read profile archive {}: {}",
            archive_path.display(),
            e
        )
    })?;
    restore_directory_probe(&bytes, key)
}

/// Descifra y valida el manifiesto en memoria, sin restaurar nada.
fn restore_directory_probe(archive: &[u8], key: &[u8; 32]) -> Result<(), String> {
    if archive.len() < PROFILE_ARCHIVE_MAGIC.len() + 12 {
        return Err("profile archive too short to be valid".into());
    }
    let (magic, encrypted) = archive.split_at(PROFILE_ARCHIVE_MAGIC.len());
    if magic != PROFILE_ARCHIVE_MAGIC {
        return Err("not a syncify profile archive (bad magic)".into());
    }
    let gz = crate::crypto::decrypt_bytes_with_key(encrypted, key)?;
    let mut decoder = flate2::read::GzDecoder::new(gz.as_slice());
    let mut body = Vec::new();
    decoder
        .read_to_end(&mut body)
        .map_err(|e| format!("Failed to decompress profile archive: {}", e))?;
    if body.len() < 4 {
        return Err("profile archive body truncated".into());
    }
    let manifest_len = u32::from_le_bytes([body[0], body[1], body[2], body[3]]) as usize;
    if body.len() < 4 + manifest_len {
        return Err("profile archive manifest truncated".into());
    }
    let manifest: ArchiveManifest = serde_json::from_slice(&body[4..4 + manifest_len])
        .map_err(|e| format!("Failed to parse profile manifest: {}", e))?;
    if (body.len() - 4 - manifest_len) as u64 != manifest.total_size {
        return Err("profile archive blob length mismatch".into());
    }
    Ok(())
}

// ==============================================
// RUTAS
// ==============================================

/// Bases de perfiles para esta instalación: (temporales de login, archivos
/// cifrados). Los temporales viven en `login-profiles/` (los crea y borra el
/// intento de login de #84); los archivos en `profile-archives/`.
pub fn profile_bases(app: &tauri::AppHandle) -> Result<(PathBuf, PathBuf), String> {
    use tauri::Manager;
    let data_dir = app
        .path()
        .app_local_data_dir()
        .map_err(|e| format!("Failed to resolve app local data dir: {}", e))?;
    Ok((
        data_dir.join(crate::commands::account_login::LOGIN_PROFILE_BASE),
        data_dir.join(PROFILE_ARCHIVE_BASE),
    ))
}

/// Ruta del archivo cifrado de un `profile_key`.
pub fn profile_archive_path(archives_base: &Path, profile_key: &str) -> PathBuf {
    archives_base.join(format!("{}.{}", profile_key, PROFILE_ARCHIVE_EXT))
}

// ==============================================
// TABLAS account_profiles / login_attempts (migración 0111)
// ==============================================

/// `service_id` de un servicio por nombre (los seis vienen sembrados por las
/// migraciones).
pub async fn resolve_service_id(db: &SqlitePool, service: &str) -> Result<i64, String> {
    sqlx::query_scalar("SELECT id FROM services WHERE LOWER(name) = LOWER(?)")
        .bind(service)
        .fetch_optional(db)
        .await
        .map_err(|e| format!("Failed to resolve service '{}': {}", service, e))?
        .ok_or_else(|| format!("Unknown service '{}'", service))
}

/// Registra el intento de login ('pending') al abrir la ventana. El
/// `profile_key` provisional ya tiene su directorio temporal creado.
pub async fn record_login_attempt(
    db: &SqlitePool,
    service_id: i64,
    profile_key: &str,
) -> Result<(), String> {
    sqlx::query(
        "INSERT INTO login_attempts (service_id, profile_key, status) VALUES (?, ?, 'pending')",
    )
    .bind(service_id)
    .bind(profile_key)
    .execute(db)
    .await
    .map_err(|e| format!("Failed to record login attempt: {}", e))?;
    Ok(())
}

/// Cierra el intento con estado terminal ('completed' | 'failed'). Cancelar
/// NO pasa por aquí: borra la fila ([`cancel_login_attempt`]).
pub async fn finish_login_attempt(
    db: &SqlitePool,
    profile_key: &str,
    status: &str,
) -> Result<(), String> {
    if status != "completed" && status != "failed" {
        return Err(format!("Invalid login attempt status '{}'", status));
    }
    sqlx::query(
        "UPDATE login_attempts SET status = ?, finished_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now') WHERE profile_key = ?",
    )
    .bind(status)
    .bind(profile_key)
    .execute(db)
    .await
    .map_err(|e| format!("Failed to finish login attempt: {}", e))?;
    Ok(())
}

/// Cancelación: el intento y su perfil provisional desaparecen (contrato de
/// la spec #83). La fila se borra, no se marca.
pub async fn cancel_login_attempt(db: &SqlitePool, profile_key: &str) -> Result<(), String> {
    sqlx::query("DELETE FROM login_attempts WHERE profile_key = ?")
        .bind(profile_key)
        .execute(db)
        .await
        .map_err(|e| format!("Failed to delete cancelled login attempt: {}", e))?;
    Ok(())
}

/// Vincula el perfil de un intento COMPLETADO a la cuenta creada o
/// actualizada: cifra el directorio temporal del webview al archivo
/// (`archive_path`), inserta la fila y borra nada aquí — el llamante
/// ([`crate::commands::account_login::finalize_attempt`]) borra el temporal
/// SIEMPRE a continuación. Un perfil anterior de la misma cuenta se sustituye
/// (re-login reutiliza la cuenta, ADR-0007 decisión 6): su archivo y su fila
/// desaparecen.
///
/// Devuelve el id de la fila de `account_profiles`.
pub async fn archive_attempt_profile(
    db: &SqlitePool,
    account_id: i64,
    service_id: i64,
    profile_key: &str,
    profile_dir: &Path,
    archive_path: &Path,
) -> Result<i64, String> {
    archive_profile_dir_to_file(profile_dir, archive_path, crate::crypto::key()?)?;
    let key_id = crate::crypto::key_fingerprint()?;

    let mut tx = db
        .begin_with("BEGIN IMMEDIATE")
        .await
        .map_err(|e| format!("Failed to begin profile link transaction: {}", e))?;

    // Sustitución: el perfil previo de la cuenta (login anterior) pierde su
    // archivo y su fila; el artefacto nuevo es el único vivo.
    let previous: Vec<(String,)> =
        sqlx::query_as("SELECT archive_path FROM account_profiles WHERE account_id = ?")
            .bind(account_id)
            .fetch_all(&mut *tx)
            .await
            .map_err(|e| {
                format!(
                    "Failed to read previous profile of account {}: {}",
                    account_id, e
                )
            })?;
    sqlx::query("DELETE FROM account_profiles WHERE account_id = ?")
        .bind(account_id)
        .execute(&mut *tx)
        .await
        .map_err(|e| {
            format!(
                "Failed to clear previous profiles of account {}: {}",
                account_id, e
            )
        })?;

    let profile_id: i64 = sqlx::query_scalar(
        "INSERT INTO account_profiles (account_id, service_id, profile_key, archive_path, key_id, status)
         VALUES (?, ?, ?, ?, ?, ?) RETURNING id",
    )
    .bind(account_id)
    .bind(service_id)
    .bind(profile_key)
    .bind(archive_path.to_string_lossy().as_ref())
    .bind(&key_id)
    .bind(PROFILE_STATUS_ARCHIVED)
    .fetch_one(&mut *tx)
    .await
    .map_err(|e| format!("Failed to link profile to account {}: {}", account_id, e))?;

    tx.commit()
        .await
        .map_err(|e| format!("Failed to commit profile link: {}", e))?;

    for (old_path,) in previous {
        remove_archive_file(&old_path);
    }
    Ok(profile_id)
}

/// Borra el perfil de una cuenta: fila y ARCHIVO (cerrar sesión y eliminar lo
/// borran; desactivar y archivar NO llaman aquí — criterio del issue #86).
/// Devuelve cuántas filas se borraron.
pub async fn delete_profiles_for_account(db: &SqlitePool, account_id: i64) -> Result<u32, String> {
    let rows: Vec<(String,)> =
        sqlx::query_as("SELECT archive_path FROM account_profiles WHERE account_id = ?")
            .bind(account_id)
            .fetch_all(db)
            .await
            .map_err(|e| format!("Failed to read profiles of account {}: {}", account_id, e))?;

    sqlx::query("DELETE FROM account_profiles WHERE account_id = ?")
        .bind(account_id)
        .execute(db)
        .await
        .map_err(|e| format!("Failed to delete profiles of account {}: {}", account_id, e))?;

    let deleted = rows.len() as u32;
    for (archive_path,) in rows {
        remove_archive_file(&archive_path);
    }
    Ok(deleted)
}

/// Borra el archivo cifrado si existe. Un fallo de borrado NO es silencioso
/// (warn) pero no aborta al llamante: la higiene de arranque elimina los
/// archivos no referenciados en la siguiente ejecución.
fn remove_archive_file(archive_path: &str) {
    match std::fs::remove_file(archive_path) {
        Ok(()) => {}
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
        Err(e) => tracing::warn!(
            "Failed to remove profile archive {}: {} (the startup hygiene pass will retry)",
            archive_path,
            e
        ),
    }
}

/// Marca un perfil ilegible y pasa su cuenta a «sin sesión»: la ficha ofrece
/// reautenticar (criterio del issue #86). La cuenta y su biblioteca se
/// conservan; nada se borra y la app no se bloquea.
pub async fn mark_profile_unrecoverable(db: &SqlitePool, profile_id: i64) -> Result<(), String> {
    let account_id: Option<i64> = sqlx::query_scalar(
        "UPDATE account_profiles SET status = ?, updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now') WHERE id = ? RETURNING account_id",
    )
    .bind(PROFILE_STATUS_UNRECOVERABLE)
    .bind(profile_id)
    .fetch_one(db)
    .await
    .map_err(|e| format!("Failed to mark profile {} unrecoverable: {}", profile_id, e))?;

    if let Some(account_id) = account_id {
        sqlx::query(
            "UPDATE accounts SET credentials_invalid = 1, invalid_reason = ?, is_active = 0 WHERE id = ?",
        )
        .bind(INVALID_REASON_PROFILE_UNREADABLE)
        .bind(account_id)
        .execute(db)
        .await
        .map_err(|e| format!("Failed to mark account {} without session: {}", account_id, e))?;
    }
    Ok(())
}

// ==============================================
// HIGIENE DE ARRANQUE
// ==============================================

/// Resultado legible de la higiene de arranque.
#[derive(Debug, Default, Clone, serde::Serialize)]
pub struct ProfileHygieneReport {
    /// Directorios temporales de login huérfanos borrados (cierre abrupto).
    pub removed_temp_dirs: u32,
    /// Archivos cifrados verificados (descifran con la clave activa).
    pub verified_archives: u32,
    /// Perfiles marcados 'unrecoverable' (no descifran; cuenta «sin sesión»).
    pub unrecoverable_profiles: u32,
    /// Archivos cifrados sin fila que los respalde, borrados.
    pub removed_orphan_archives: u32,
}

/// Higiene de arranque (criterio 2 y 4 del issue #86):
///
/// 1. Borra TODOS los subdirectorios de `temp_base`: al arrancar no hay
///    ventana de login abierta, así que todo lo que quiera bajo
///    `login-profiles/` es huérfano de un cierre abrupto. Los ARCHIVOS
///    cifrados viven en otra base y no pasan por aquí.
/// 2. Borra archivos cifrados de `archives_base` sin fila en
///    `account_profiles` (residuos de un borrado cuyo unlink falló).
/// 3. Verifica que cada perfil 'archived' descifra con la clave activa; el
///    que no, pasa a 'unrecoverable' y su cuenta a «sin sesión».
///
/// Ningún fallo individual aborta la pasada: se registra y se cuenta.
pub async fn startup_profile_hygiene(
    db: &SqlitePool,
    temp_base: &Path,
    archives_base: &Path,
) -> Result<ProfileHygieneReport, String> {
    let mut report = ProfileHygieneReport::default();

    // 1. Huérfanos de texto plano.
    std::fs::create_dir_all(temp_base).map_err(|e| {
        format!(
            "Failed to create login profiles base {}: {}",
            temp_base.display(),
            e
        )
    })?;
    for entry in std::fs::read_dir(temp_base).map_err(|e| {
        format!(
            "Failed to read login profiles base {}: {}",
            temp_base.display(),
            e
        )
    })? {
        let Ok(entry) = entry else { continue };
        let path = entry.path();
        if path.is_dir() {
            match std::fs::remove_dir_all(&path) {
                Ok(()) => report.removed_temp_dirs += 1,
                Err(e) => tracing::warn!(
                    "Failed to remove orphan login profile directory {}: {}",
                    path.display(),
                    e
                ),
            }
        }
    }

    // 2. Archivos cifrados huérfanos y 3. verificación de descifrado.
    std::fs::create_dir_all(archives_base).map_err(|e| {
        format!(
            "Failed to create profile archives base {}: {}",
            archives_base.display(),
            e
        )
    })?;
    let referenced: Vec<(String,)> = sqlx::query_as("SELECT archive_path FROM account_profiles")
        .fetch_all(db)
        .await
        .map_err(|e| format!("Failed to read archived profiles: {}", e))?;
    let referenced_set: std::collections::HashSet<String> =
        referenced.into_iter().map(|(p,)| p).collect();

    for entry in std::fs::read_dir(archives_base).map_err(|e| {
        format!(
            "Failed to read profile archives base {}: {}",
            archives_base.display(),
            e
        )
    })? {
        let Ok(entry) = entry else { continue };
        let path = entry.path();
        if !path.is_file() {
            continue;
        }
        let path_str = path.to_string_lossy().to_string();
        if !referenced_set.contains(&path_str) {
            match std::fs::remove_file(&path) {
                Ok(()) => report.removed_orphan_archives += 1,
                Err(e) => tracing::warn!(
                    "Failed to remove orphan profile archive {}: {}",
                    path.display(),
                    e
                ),
            }
            continue;
        }

        if let Err(e) = decrypt_check(&path, crate::crypto::key()?) {
            tracing::warn!(
                "Profile archive {} does not decrypt with the active key: {}. \
                 Its account moves to 'sin sesión' and can re-authenticate.",
                path.display(),
                e
            );
            let profile_id: i64 =
                sqlx::query_scalar("SELECT id FROM account_profiles WHERE archive_path = ?")
                    .bind(&path_str)
                    .fetch_one(db)
                    .await
                    .map_err(|e| format!("Failed to resolve profile for {}: {}", path_str, e))?;
            mark_profile_unrecoverable(db, profile_id).await?;
            report.unrecoverable_profiles += 1;
        } else {
            report.verified_archives += 1;
        }
    }

    if report.removed_temp_dirs > 0
        || report.unrecoverable_profiles > 0
        || report.removed_orphan_archives > 0
    {
        tracing::info!(
            removed_temp_dirs = report.removed_temp_dirs,
            verified_archives = report.verified_archives,
            unrecoverable_profiles = report.unrecoverable_profiles,
            removed_orphan_archives = report.removed_orphan_archives,
            "Account profile hygiene finished"
        );
    }
    Ok(report)
}

// ==============================================
// TESTS (webview falso, directorios temporales, sin red)
// ==============================================

#[cfg(test)]
mod tests {
    use super::*;

    /// Clave falsa del binario de pruebas unitarias: `init_crypto` solo
    /// instancia una vez por proceso, igual que en arranque.
    fn init_test_key() -> [u8; 32] {
        let key = [42u8; 32];
        let _ = crate::crypto::init_crypto(key);
        key
    }

    /// Webview falso: un directorio de perfil con contenido inventado
    /// (Cookies, Local Storage, preferencias) y un marcador identificable.
    fn fake_webview_profile(base: &Path, name: &str, marker: &str) -> PathBuf {
        let dir = base.join(name);
        std::fs::create_dir_all(dir.join("Local Storage/leveldb")).unwrap();
        std::fs::write(dir.join("Cookies"), format!("sqlite jar {}", marker)).unwrap();
        std::fs::write(
            dir.join("Local Storage/leveldb/000003.log"),
            format!("leveldb {}", marker),
        )
        .unwrap();
        std::fs::write(
            dir.join("Preferences"),
            format!("{{\"user\":\"{}\"}}", marker),
        )
        .unwrap();
        dir
    }

    fn temp_base() -> PathBuf {
        std::env::temp_dir().join(format!("syncify-a86-{}", uuid::Uuid::new_v4()))
    }

    // ── Cifrado en reposo ────────────────────────────────────────────────

    #[test]
    fn el_archivo_cifra_y_restaura_el_arbol_completo() {
        let key = init_test_key();
        let base = temp_base();
        let profile = fake_webview_profile(&base, "orig", "cuenta-A");

        let archive = base.join("archivos").join("a.syncprof");
        archive_profile_dir_to_file(&profile, &archive, &key).unwrap();

        let restored = base.join("restaurado");
        let files = restore_profile_dir_from_file(&archive, &restored, &key).unwrap();
        assert_eq!(files, 3, "Cookies, leveldb y Preferences");

        assert_eq!(
            std::fs::read_to_string(restored.join("Cookies")).unwrap(),
            format!("sqlite jar {}", "cuenta-A")
        );
        assert_eq!(
            std::fs::read_to_string(restored.join("Local Storage/leveldb/000003.log")).unwrap(),
            format!("leveldb {}", "cuenta-A")
        );
    }

    #[test]
    fn el_archivo_en_reposo_no_lleva_texto_plano_del_perfil() {
        let key = init_test_key();
        let base = temp_base();
        let marker = "TOKEN_SECRETO_A86";
        let profile = fake_webview_profile(&base, "orig", marker);

        let archive = base.join("a.syncprof");
        archive_profile_dir_to_file(&profile, &archive, &key).unwrap();

        let raw = std::fs::read(&archive).unwrap();
        let as_text = String::from_utf8_lossy(&raw).to_ascii_lowercase();
        assert!(
            !as_text.contains(marker.to_ascii_lowercase().as_str()),
            "el marcador del perfil aparece en claro en el archivo cifrado"
        );
        assert!(
            !as_text.contains("preferences") && !as_text.contains("cookies"),
            "el manifiesto está dentro del cuerpo cifrado, no en claro"
        );
        assert!(raw.starts_with(PROFILE_ARCHIVE_MAGIC));
    }

    #[test]
    fn una_etiqueta_gcm_rota_o_clave_distinta_no_descifra() {
        let key = init_test_key();
        let base = temp_base();
        let profile = fake_webview_profile(&base, "orig", "cuenta-B");
        let archive = base.join("a.syncprof");
        archive_profile_dir_to_file(&profile, &archive, &key).unwrap();

        // Manipulación de UN byte: la etiqueta GCM no valida.
        let mut tampered = std::fs::read(&archive).unwrap();
        let last = tampered.len() - 1;
        tampered[last] ^= 0x01;
        assert!(restore_directory(&tampered, &base.join("x"), &key).is_err());

        // Clave distinta (otra máquina, base restaurada): tampoco.
        let other_key = [7u8; 32];
        assert!(restore_profile_dir_from_file(&archive, &base.join("y"), &other_key).is_err());
    }

    #[test]
    fn el_manifiesto_no_puede_escribir_fuera_del_directorio_temporal() {
        let key = init_test_key();
        let base = temp_base();
        let manifest = ArchiveManifest {
            files: vec![ArchivedFile {
                path: "../../escaped".to_string(),
                size: 5,
            }],
            total_size: 5,
        };
        let manifest_json = serde_json::to_vec(&manifest).unwrap();
        let mut body: Vec<u8> = Vec::new();
        body.extend_from_slice(&(manifest_json.len() as u32).to_le_bytes());
        body.extend_from_slice(&manifest_json);
        body.extend_from_slice(b"hello");
        let mut compressed =
            flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::default());
        compressed.write_all(&body).unwrap();
        let gz = compressed.finish().unwrap();
        let mut archive = PROFILE_ARCHIVE_MAGIC.to_vec();
        archive.extend(crate::crypto::encrypt_bytes_with_key(&gz, &key).unwrap());

        assert!(restore_directory(&archive, &base.join("dest"), &key)
            .unwrap_err()
            .contains("parent component"));
    }

    // ── Criterio 1: dos cuentas del mismo servicio, perfiles separados ───

    #[test]
    fn dos_cuentas_del_mismo_servicio_tienen_archivos_y_contenido_separado() {
        let key = init_test_key();
        let base = temp_base();
        let archives = base.join("profile-archives");

        let profile_a = fake_webview_profile(&base, "sess-a", "cuenta-A");
        let profile_b = fake_webview_profile(&base, "sess-b", "cuenta-B");

        let path_a = profile_archive_path(&archives, "key-a");
        let path_b = profile_archive_path(&archives, "key-b");
        assert_ne!(path_a, path_b, "cada perfil tiene SU archivo");

        archive_profile_dir_to_file(&profile_a, &path_a, &key).unwrap();
        archive_profile_dir_to_file(&profile_b, &path_b, &key).unwrap();

        let restored_a = base.join("restore-a");
        let restored_b = base.join("restore-b");
        restore_profile_dir_from_file(&path_a, &restored_a, &key).unwrap();
        restore_profile_dir_from_file(&path_b, &restored_b, &key).unwrap();

        assert_eq!(
            std::fs::read_to_string(restored_a.join("Cookies")).unwrap(),
            "sqlite jar cuenta-A",
            "el archivo A restaura el contenido de A"
        );
        assert_eq!(
            std::fs::read_to_string(restored_b.join("Cookies")).unwrap(),
            "sqlite jar cuenta-B",
            "el archivo B restaura el contenido de B, no el de A"
        );
    }

    #[test]
    fn restaurar_en_un_directorio_temporal_deja_permisos_0700_0600() {
        let key = init_test_key();
        let base = temp_base();
        let profile = fake_webview_profile(&base, "orig", "permisos");
        let archive = base.join("a.syncprof");
        archive_profile_dir_to_file(&profile, &archive, &key).unwrap();

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let restored = base.join("restore");
            restore_profile_dir_from_file(&archive, &restored, &key).unwrap();
            let mode = std::fs::metadata(&restored).unwrap().permissions().mode() & 0o777;
            assert_eq!(mode, 0o700, "el directorio temporal queda 0700");
            let file_mode = std::fs::metadata(restored.join("Cookies"))
                .unwrap()
                .permissions()
                .mode()
                & 0o777;
            assert_eq!(file_mode, 0o600, "los ficheros temporales quedan 0600");
        }
    }
}

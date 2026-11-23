//! D16 (#128): escritor ID3v2 compartido para entregas MP3.
//!
//! Es la única implementación de tagging MP3 del repo. Qobuz/Tidal escriben
//! FLAC con `syncify-flac-writer`; M4A/AAC con `mp4ameta`. MP3 no tenía
//! escritor, así que Deezer/SoundCloud (D11/D12) habrían acabado con dos
//! copias. Este módulo cierra el hueco: un DTO neutro, una escritura y una
//! relectura que debe coincidir con lo escrito.
//!
//! ## Crate: `id3` 1.17
//!
//! Se midió antes de elegir (issue #128, checkpoint 1):
//!
//! - `id3` solo añade a `Cargo.lock` un paquete sin dependencias transitivas
//!   nuevas: `bitflags 2.10`, `byteorder 1.5` y `flate2` ya estaban en el
//!   workspace. Es MIT y cubre ID3v2.2/2.3/2.4, texto, `TSRC` y `APIC`.
//! - `lofty` cubre FLAC/M4A/MP3, pero duplicaría los lectores que el repo ya
//!   tiene (`metaflac` y `mp4ameta`) y arrastra su propio stack de parsing para
//!   formatos que no necesitamos aquí. El alcance es MP3/ID3, no un lector
//!   universal.
//! - Implementación propia: descartada. La estructura de frames, los
//!   encodings y el tamaño syncsafe son justo donde un writer casero rompe la
//!   compatibilidad con reproductores externos.
//!
//! Se escribe **ID3v2.3**: es la versión que leen sin ambigüedad los
//! reproductores externos de escritorio (Windows, TagLib, VLC, foobar2000).
//! v2.4 es válida, pero sustituye `TYER` por `TDRC` y hay lectores antiguos que
//! no la interpretan. `TSRC` (ISRC) y `APIC` existen en ambas.
//!
//! ## Contrato de fallo
//!
//! Antes de tocar el fichero se copia a un hermano temporal. La escritura se
//! relee y se compara campo a campo. Si la escritura o la relectura fallan, el
//! original se restaura y la función devuelve error: nunca se deja un MP3 a
//! medias ni una entrega "completada" con tags que no se pudieron verificar.

use std::io::Read;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use id3::frame::{Picture, PictureType};
use id3::{Tag, TagLike, Version};

use crate::download::progress::DownloadRequest;

/// Versión escrita. Ver la nota de compatibilidad de la cabecera.
pub const ID3_VERSION: Version = Version::Id3v23;

/// Número máximo de bytes que se leen para reconocer un MP3 por cabecera.
const HEADER_PROBE_BYTES: usize = 3;

static BACKUP_SEQUENCE: AtomicU64 = AtomicU64::new(0);

// Solo en tests: fuerza un fallo de relectura DESPUÉS de escribir para probar
// que el original se restaura. Es thread-local para que los tests en paralelo
// no se roben el interruptor; no existe en producción (`cfg(test)`).
#[cfg(test)]
thread_local! {
    static FORCE_VERIFICATION_FAILURE: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

/// Portada a incrustar en el frame `APIC`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Mp3Cover {
    pub mime_type: String,
    pub data: Vec<u8>,
}

impl Mp3Cover {
    /// Portada con tipo MIME deducido de la magia del payload. Un payload que
    /// no es JPEG/PNG se rechaza: la misma regla que el gate de D05, para no
    /// escribir una portada que la verificación rechazaría después.
    /// Solo la usan los tests: la portada no viaja en `DownloadRequest`.
    #[cfg(test)]
    pub fn from_bytes(data: Vec<u8>) -> Result<Self, Mp3TagError> {
        let mime_type = image_mime_type(&data)
            .ok_or_else(|| Mp3TagError::InvalidCover("payload is not JPEG or PNG".to_string()))?;
        Ok(Self {
            mime_type: mime_type.to_string(),
            data,
        })
    }
}

/// Metadatos que el escritor gestiona. Semántica de parche: `Some` escribe,
/// `None` deja el frame existente intacto (importante para conservar la
/// portada que el adaptador ya incrustó). `title` y `artist` son obligatorios
/// y se validan antes de tocar el fichero.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Mp3Tags {
    pub title: Option<String>,
    pub artist: Option<String>,
    pub album: Option<String>,
    pub album_artist: Option<String>,
    pub year: Option<i32>,
    pub track_number: Option<u32>,
    pub track_total: Option<u32>,
    pub disc_number: Option<u32>,
    pub disc_total: Option<u32>,
    pub isrc: Option<String>,
    pub cover: Option<Mp3Cover>,
}

impl Mp3Tags {
    /// Proyecta la petición de descarga al DTO de ID3. La portada no viaja en
    /// `DownloadRequest` (solo `cover_url`): el adaptador que descargue bytes
    /// de portada la añade con [`Mp3Tags::with_cover`] antes de llamar al
    /// escritor.
    pub fn from_download_request(request: &DownloadRequest) -> Self {
        let track_number = positive_u32(request.track_number);
        let track_total = positive_u32(request.total_tracks)
            // `total_tracks` cae a 1 cuando el catálogo no lo conoce; un total
            // menor o igual que el número de pista no es información y
            // escribir "5/1" empeoraría el tag.
            .filter(|total| track_number.is_some_and(|number| *total > number));
        Self {
            title: non_empty(&request.track_name),
            artist: non_empty(&request.artist_name),
            album: non_empty(&request.album_name),
            album_artist: request.album_artist.as_deref().and_then(non_empty),
            year: request.release_date.as_deref().and_then(parse_year),
            track_number,
            track_total,
            disc_number: positive_u32(request.disc_number),
            disc_total: None,
            isrc: request.isrc.as_deref().and_then(non_empty),
            cover: None,
        }
    }
}

/// Error del escritor. Las variantes distinguen el fallo original del fallo al
/// restaurar: si la restauración también falla, el llamador necesita saber que
/// el fichero pudo quedar tocado.
#[derive(Debug)]
pub enum Mp3TagError {
    InvalidInput(String),
    InvalidCover(String),
    Io { context: String, message: String },
    Write(String),
    Verification(String),
    RestoreFailed { original: String, restore: String },
}

impl std::fmt::Display for Mp3TagError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidInput(message) => write!(f, "invalid ID3 input: {message}"),
            Self::InvalidCover(message) => write!(f, "invalid cover: {message}"),
            Self::Io { context, message } => write!(f, "{context}: {message}"),
            Self::Write(message) => write!(f, "ID3 write failed: {message}"),
            Self::Verification(message) => write!(f, "ID3 re-read mismatch: {message}"),
            Self::RestoreFailed { original, restore } => write!(
                f,
                "ID3 tagging failed ({original}) and restoring the original file also failed ({restore})"
            ),
        }
    }
}

impl std::error::Error for Mp3TagError {}

/// Escribe el tag, lo relee y exige que coincida. Si algo falla después de
/// empezar, restaura el original y devuelve error.
///
/// Devuelve el tag releído (no el de entrada) como evidencia de lo que el
/// fichero declara realmente.
pub fn apply_and_verify_mp3_tags(path: &Path, tags: &Mp3Tags) -> Result<Mp3Tags, Mp3TagError> {
    validate(path, tags)?;

    let backup = backup_path(path);
    copy_file(path, &backup, "cannot back up the original MP3")?;

    match write_and_verify(path, tags) {
        Ok(readback) => {
            remove_backup(&backup);
            Ok(readback)
        }
        Err(error) => match restore_backup(&backup, path) {
            Ok(()) => {
                remove_backup(&backup);
                Err(error)
            }
            Err(restore) => Err(Mp3TagError::RestoreFailed {
                original: error.to_string(),
                restore,
            }),
        },
    }
}

/// Relee el tag ID3v2 de un MP3. Un fichero sin tag devuelve el DTO vacío (la
/// lectura funcionó; simplemente no hay frames). Solo un error de E/S o un tag
/// corrupto es `Err`.
pub fn read_mp3_tags(path: &Path) -> Result<Mp3Tags, Mp3TagError> {
    let tag = match Tag::read_from_path(path) {
        Ok(tag) => tag,
        Err(error) if matches!(error.kind, id3::ErrorKind::NoTag) => return Ok(Mp3Tags::default()),
        Err(error) => {
            return Err(Mp3TagError::Io {
                context: format!("cannot read ID3 tags from '{}'", path.display()),
                message: error.to_string(),
            })
        }
    };

    Ok(snapshot(&tag))
}

/// `true` si el fichero empieza como un MP3: tag `ID3` o sync MPEG. La
/// extensión sola no basta (un FLAC renombrado a `.mp3` no debe recibir un
/// ID3 encima); para el gate, la extensión `.mp3` sí manda a este lector.
pub fn is_mp3_file(path: &Path) -> bool {
    let mut prefix = [0u8; HEADER_PROBE_BYTES];
    let Ok(mut file) = std::fs::File::open(path) else {
        return false;
    };
    let Ok(read) = file.read(&mut prefix) else {
        return false;
    };
    looks_like_mp3_header(&prefix[..read])
}

fn looks_like_mp3_header(prefix: &[u8]) -> bool {
    prefix.starts_with(b"ID3") || is_mpeg_audio_sync(prefix)
}

fn is_mpeg_audio_sync(prefix: &[u8]) -> bool {
    prefix.len() >= 2 && prefix[0] == 0xFF && (prefix[1] & 0xE0) == 0xE0
}

fn validate(path: &Path, tags: &Mp3Tags) -> Result<(), Mp3TagError> {
    if non_empty(tags.title.as_deref().unwrap_or("")).is_none() {
        return Err(Mp3TagError::InvalidInput("title is required".to_string()));
    }
    if non_empty(tags.artist.as_deref().unwrap_or("")).is_none() {
        return Err(Mp3TagError::InvalidInput("artist is required".to_string()));
    }
    if let Some(year) = tags.year.filter(|year| !(1000..=9999).contains(year)) {
        return Err(Mp3TagError::InvalidInput(format!(
            "year {year} is outside 1000..=9999"
        )));
    }
    if tags
        .cover
        .as_ref()
        .is_some_and(|cover| image_mime_type(&cover.data).is_none())
    {
        return Err(Mp3TagError::InvalidCover(
            "payload is not JPEG or PNG".to_string(),
        ));
    }
    if tags
        .cover
        .as_ref()
        .is_some_and(|cover| cover.mime_type.trim().is_empty())
    {
        return Err(Mp3TagError::InvalidCover(
            "cover MIME type is empty".to_string(),
        ));
    }
    let metadata = std::fs::metadata(path).map_err(|error| Mp3TagError::Io {
        context: format!("cannot stat MP3 '{}'", path.display()),
        message: error.to_string(),
    })?;
    if !metadata.is_file() || metadata.len() == 0 {
        return Err(Mp3TagError::InvalidInput(format!(
            "'{}' is not a non-empty regular file",
            path.display()
        )));
    }
    if !is_mp3_file(path) {
        return Err(Mp3TagError::InvalidInput(format!(
            "'{}' does not look like an MP3",
            path.display()
        )));
    }
    Ok(())
}

fn write_and_verify(path: &Path, tags: &Mp3Tags) -> Result<Mp3Tags, Mp3TagError> {
    // Se conserva el tag existente (frames ajenos incluidos) y se parchean los
    // campos que esta entrega conoce.
    let mut tag = Tag::read_from_path(path).unwrap_or_else(|_| Tag::with_version(ID3_VERSION));
    apply_fields(&mut tag, tags);
    tag.write_to_path(path, ID3_VERSION)
        .map_err(|error| Mp3TagError::Write(error.to_string()))?;

    #[cfg(test)]
    if FORCE_VERIFICATION_FAILURE.with(|flag| flag.replace(false)) {
        return Err(Mp3TagError::Verification(
            "forced re-read failure for the restore test".to_string(),
        ));
    }

    let readback = read_mp3_tags(path)?;
    verify_roundtrip(tags, &readback)?;
    Ok(readback)
}

fn apply_fields(tag: &mut Tag, tags: &Mp3Tags) {
    if let Some(value) = tags.title.as_deref().and_then(non_empty) {
        tag.set_title(value);
    }
    if let Some(value) = tags.artist.as_deref().and_then(non_empty) {
        tag.set_artist(value);
    }
    if let Some(value) = tags.album.as_deref().and_then(non_empty) {
        tag.set_album(value);
    }
    if let Some(value) = tags.album_artist.as_deref().and_then(non_empty) {
        tag.set_album_artist(value);
    }
    if let Some(year) = tags.year {
        tag.set_year(year);
    }
    if let Some(number) = tags.track_number {
        tag.set_track(number);
        if let Some(total) = tags.track_total {
            tag.set_total_tracks(total);
        }
    }
    if let Some(number) = tags.disc_number {
        tag.set_disc(number);
        if let Some(total) = tags.disc_total {
            tag.set_total_discs(total);
        }
    }
    if let Some(value) = tags.isrc.as_deref().and_then(non_empty) {
        tag.set_text("TSRC", value);
    }
    if let Some(cover) = &tags.cover {
        // Una portada nueva sustituye la anterior; si no hay portada nueva se
        // conserva la que el adaptador hubiera incrustado.
        tag.remove("APIC");
        tag.add_frame(Picture {
            mime_type: cover.mime_type.clone(),
            picture_type: PictureType::CoverFront,
            description: String::new(),
            data: cover.data.clone(),
        });
    }
}

fn snapshot(tag: &Tag) -> Mp3Tags {
    Mp3Tags {
        title: tag.title().map(str::to_string),
        artist: tag.artist().map(str::to_string),
        album: tag.album().map(str::to_string),
        album_artist: tag.album_artist().map(str::to_string),
        year: tag.year(),
        track_number: tag.track(),
        track_total: tag.total_tracks(),
        disc_number: tag.disc(),
        disc_total: tag.total_discs(),
        isrc: tag
            .get("TSRC")
            .and_then(|frame| frame.content().text())
            .map(str::to_string),
        cover: tag.pictures().find_map(|picture| {
            (picture.picture_type == PictureType::CoverFront).then(|| Mp3Cover {
                mime_type: picture.mime_type.clone(),
                data: picture.data.clone(),
            })
        }),
    }
}

/// La relectura debe declarar exactamente lo escrito en cada campo gestionado.
/// Se comparan los textos sin espacios sobrantes; números e imagen, exactos.
fn verify_roundtrip(expected: &Mp3Tags, actual: &Mp3Tags) -> Result<(), Mp3TagError> {
    let mut mismatches = Vec::new();
    compare_text(&mut mismatches, "title", &expected.title, &actual.title);
    compare_text(&mut mismatches, "artist", &expected.artist, &actual.artist);
    compare_text(&mut mismatches, "album", &expected.album, &actual.album);
    compare_text(
        &mut mismatches,
        "album_artist",
        &expected.album_artist,
        &actual.album_artist,
    );
    compare_text(&mut mismatches, "isrc", &expected.isrc, &actual.isrc);
    compare_value(&mut mismatches, "year", expected.year, actual.year);
    compare_value(
        &mut mismatches,
        "track_number",
        expected.track_number,
        actual.track_number,
    );
    compare_value(
        &mut mismatches,
        "track_total",
        expected.track_total,
        actual.track_total,
    );
    compare_value(
        &mut mismatches,
        "disc_number",
        expected.disc_number,
        actual.disc_number,
    );
    compare_value(
        &mut mismatches,
        "disc_total",
        expected.disc_total,
        actual.disc_total,
    );
    compare_cover(
        &mut mismatches,
        expected.cover.as_ref(),
        actual.cover.as_ref(),
    );

    if mismatches.is_empty() {
        Ok(())
    } else {
        Err(Mp3TagError::Verification(mismatches.join("; ")))
    }
}

fn compare_text(
    mismatches: &mut Vec<String>,
    field: &str,
    expected: &Option<String>,
    actual: &Option<String>,
) {
    let expected = expected.as_deref().and_then(non_empty);
    let actual = actual.as_deref().and_then(non_empty);
    if expected != actual {
        mismatches.push(format!("{field} {:?} != {:?}", expected, actual));
    }
}

fn compare_value<T: PartialEq + std::fmt::Debug>(
    mismatches: &mut Vec<String>,
    field: &str,
    expected: Option<T>,
    actual: Option<T>,
) {
    if expected != actual {
        mismatches.push(format!("{field} {:?} != {:?}", expected, actual));
    }
}

fn compare_cover(
    mismatches: &mut Vec<String>,
    expected: Option<&Mp3Cover>,
    actual: Option<&Mp3Cover>,
) {
    let Some(expected) = expected else {
        return;
    };
    match actual {
        Some(actual)
            if actual.data == expected.data
                && actual.mime_type.eq_ignore_ascii_case(&expected.mime_type) => {}
        Some(actual) => mismatches.push(format!(
            "cover {} bytes/{} != {} bytes/{}",
            actual.data.len(),
            actual.mime_type,
            expected.data.len(),
            expected.mime_type
        )),
        None => mismatches.push("cover missing after write".to_string()),
    }
}

fn non_empty(value: &str) -> Option<String> {
    let trimmed = value.trim();
    (!trimmed.is_empty()).then(|| trimmed.to_string())
}

fn positive_u32(value: i32) -> Option<u32> {
    u32::try_from(value).ok().filter(|number| *number > 0)
}

/// Año del catálogo desde `YYYY`, `YYYY-MM-DD` o un timestamp ISO.
fn parse_year(value: &str) -> Option<i32> {
    let digits: String = value.chars().take_while(char::is_ascii_digit).collect();
    if digits.len() != 4 {
        return None;
    }
    digits.parse().ok()
}

fn image_mime_type(data: &[u8]) -> Option<&'static str> {
    if data.starts_with(&[0xFF, 0xD8, 0xFF]) {
        Some("image/jpeg")
    } else if data.starts_with(&[0x89, b'P', b'N', b'G']) {
        Some("image/png")
    } else {
        None
    }
}

fn backup_path(path: &Path) -> PathBuf {
    let sequence = BACKUP_SEQUENCE.fetch_add(1, Ordering::Relaxed);
    let name = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("track.mp3");
    path.with_file_name(format!(
        ".{name}.id3-backup-{}-{sequence}",
        std::process::id()
    ))
}

fn copy_file(from: &Path, to: &Path, context: &str) -> Result<(), Mp3TagError> {
    std::fs::copy(from, to)
        .map(|_| ())
        .map_err(|error| Mp3TagError::Io {
            context: format!("{context} ('{}' -> '{}')", from.display(), to.display()),
            message: error.to_string(),
        })
}

fn restore_backup(backup: &Path, path: &Path) -> Result<(), String> {
    std::fs::copy(backup, path)
        .map(|_| ())
        .map_err(|error| format!("cannot restore '{}': {error}", path.display()))
}

fn remove_backup(backup: &Path) {
    if let Err(error) = std::fs::remove_file(backup) {
        if error.kind() != std::io::ErrorKind::NotFound {
            tracing::warn!(
                backup = %backup.display(),
                error = %error,
                "D16: no se pudo borrar el backup temporal del MP3"
            );
        }
    }
}

/// Contenido de audio sin los bloques ID3v2/ID3v1. Sirve a los tests para
/// probar que etiquetar no transcodifica ni reordena un byte del audio.
#[cfg(test)]
pub(crate) fn audio_payload(path: &Path) -> Vec<u8> {
    let mut bytes = std::fs::read(path).expect("read mp3");
    if bytes.starts_with(b"ID3") && bytes.len() >= 10 {
        let size = syncsafe_size(&bytes[6..10]);
        let footer = (bytes[5] & 0x10) != 0;
        let end = 10 + size + usize::from(footer) * 10;
        bytes.drain(..end.min(bytes.len()));
    }
    if bytes.len() >= 128 && bytes[bytes.len() - 128..bytes.len() - 125] == *b"TAG" {
        bytes.truncate(bytes.len() - 128);
    }
    bytes
}

#[cfg(test)]
fn syncsafe_size(bytes: &[u8]) -> usize {
    ((bytes[0] as usize) << 21)
        | ((bytes[1] as usize) << 14)
        | ((bytes[2] as usize) << 7)
        | (bytes[3] as usize)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// MP3 mínimo: cabecera de frame MPEG-1 Layer III (128 kbps, 44.1 kHz,
    /// estéreo) y un frame vacío. No es audio reproducible, pero sí un MP3
    /// estructuralmente válido para caracterizar el escritor.
    const MINIMAL_MP3: &[u8] = &[
        0xFF, 0xFB, 0x90, 0x64, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00,
    ];

    /// 0.3 s de silencio mono a 64 kbps generados con LAME (`anullsrc`): un MP3
    /// real de fixture, sin versionar el binario. Se genera una vez y se
    /// embebe para que el test sea hermético y no dependa de ffmpeg.
    const REAL_MP3_BASE64: &str = concat!(
        "//tQxAAAAAAAAAAAAAAAAAAAAAAASW5mbwAAAA8AAAANAAALbAAkJCQkJCQkNjY2NjY2NjZJSUlJSUlJSVtbW1tbW1ttbW1t",
        "bW1tbX9/f39/f39/kpKSkpKSkqSkpKSkpKSktra2tra2trbJycnJycnJ29vb29vb29vt7e3t7e3t7f////////8AAAAATGF2",
        "YzYzLjI0AAAAAAAAAAAAAAAAJASSAAAAAAAAC2yTY/JSAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAP/7UMQAA8AA",
        "AaQAAAAgAAA0gAAABExBTUU0LjBVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVMQU1FNC4w",
        "VVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVV",
        "VVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVX/+1LEXYPAAAGkAAAAIAAA",
        "NIAAAARVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVUxBTUU0LjBVVVVVVVVV",
        "VVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVV",
        "VVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVf/7UsShg8AAAaQAAAAgAAA0gAAABFVV",
        "VVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVTEFNRTQuMFVVVVVVVVVVVVVVVVVV",
        "VVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVV",
        "VVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVV//tSxKGDwAABpAAAACAAADSAAAAEVVVVVVVVVVVV",
        "VVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVMQU1FNC4wVVVVVVVVVVVVVVVVVVVVVVVVVVVV",
        "VVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVV",
        "VVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVX/+1LEoYPAAAGkAAAAIAAANIAAAARVVVVVVVVVVVVVVVVVVVVV",
        "VVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVUxBTUU0LjBVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVV",
        "VVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVV",
        "VVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVf/7UsShg8AAAaQAAAAgAAA0gAAABFVVVVVVVVVVVVVVVVVVVVVVVVVVVVVV",
        "VVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVTEFNRTQuMFVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVV",
        "VVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVV",
        "VVVVVVVVVVVVVVVVVVVVVVVVVVVV//tSxKGDwAABpAAAACAAADSAAAAEVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVV",
        "VVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVMQU1FNC4wVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVV",
        "VVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVV",
        "VVVVVVVVVVVVVVVVVVX/+1LEoYPAAAGkAAAAIAAANIAAAARVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVV",
        "VVVVVVVVVVVVVVVVVVVVVUxBTUU0LjBVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVV",
        "VVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVV",
        "VVVVVVVVVf/7UsShg8AAAaQAAAAgAAA0gAAABFVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVV",
        "VVVVVVVVVVVVTEFNRTQuMFVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVV",
        "VVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVV",
        "//tSxKGDwAABpAAAACAAADSAAAAEVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVV",
        "VVVMQU1FNC4wVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVV",
        "VVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVX/+1LEoYPA",
        "AAGkAAAAIAAANIAAAARVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVUxBTUU0",
        "LjBVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVV",
        "VVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVf/7UsShg8AAAaQAAAAg",
        "AAA0gAAABFVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVV",
        "VVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVV",
        "VVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVV//tSxKGDwAABpAAAACAAADSAAAAE",
        "VVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVV",
        "VVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVV",
        "VVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVVU=",
    );

    const PNG_COVER: &[u8] = &[
        0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A, 0x00, 0x00, 0x00, 0x0D, b'I', b'H', b'D',
        b'R',
    ];

    fn real_mp3() -> Vec<u8> {
        let mut base64 = REAL_MP3_BASE64.as_bytes().to_vec();
        base64.retain(|byte| !byte.is_ascii_whitespace());
        decode_base64(&base64)
    }

    /// Decoder base64 mínimo para no añadir una dependencia de test.
    fn decode_base64(input: &[u8]) -> Vec<u8> {
        fn value(byte: u8) -> Option<u8> {
            match byte {
                b'A'..=b'Z' => Some(byte - b'A'),
                b'a'..=b'z' => Some(byte - b'a' + 26),
                b'0'..=b'9' => Some(byte - b'0' + 52),
                b'+' => Some(62),
                b'/' => Some(63),
                _ => None,
            }
        }
        let mut out = Vec::with_capacity(input.len() * 3 / 4);
        let mut buffer = 0u32;
        let mut bits = 0u8;
        for &byte in input {
            if byte == b'=' {
                break;
            }
            let Some(value) = value(byte) else { continue };
            buffer = (buffer << 6) | u32::from(value);
            bits += 6;
            if bits >= 8 {
                bits -= 8;
                out.push((buffer >> bits) as u8);
            }
        }
        out
    }

    fn full_tags() -> Mp3Tags {
        Mp3Tags {
            title: Some("Tema de Prueba".to_string()),
            artist: Some("Artista".to_string()),
            album: Some("Álbum".to_string()),
            album_artist: Some("Artista del Álbum".to_string()),
            year: Some(2024),
            track_number: Some(3),
            track_total: Some(12),
            disc_number: Some(1),
            disc_total: Some(2),
            isrc: Some("USRC17607839".to_string()),
            cover: Some(Mp3Cover::from_bytes(PNG_COVER.to_vec()).unwrap()),
        }
    }

    fn write_temp(dir: &tempfile::TempDir, name: &str, bytes: &[u8]) -> PathBuf {
        let path = dir.path().join(name);
        std::fs::write(&path, bytes).unwrap();
        path
    }

    #[test]
    fn writes_and_reads_back_every_field() {
        let dir = tempfile::TempDir::new().unwrap();
        let path = write_temp(&dir, "track.mp3", &real_mp3());
        let before = audio_payload(&path);

        let readback = apply_and_verify_mp3_tags(&path, &full_tags()).unwrap();

        assert_eq!(readback, full_tags());
        assert_eq!(read_mp3_tags(&path).unwrap(), full_tags());
        assert_eq!(
            audio_payload(&path),
            before,
            "etiquetar no puede tocar el audio (sin transcodificar)"
        );
        assert_eq!(
            std::fs::read_dir(dir.path()).unwrap().count(),
            1,
            "el backup temporal se borra al terminar"
        );
    }

    #[test]
    fn tag_is_id3v23_and_frames_are_externally_parseable() {
        let dir = tempfile::TempDir::new().unwrap();
        let path = write_temp(&dir, "track.mp3", &real_mp3());
        apply_and_verify_mp3_tags(&path, &full_tags()).unwrap();

        let bytes = std::fs::read(&path).unwrap();
        assert_eq!(&bytes[..3], b"ID3", "el fichero empieza por ID3v2");
        assert_eq!(bytes[3], 3, "versión mayor ID3v2.3");
        assert_eq!(bytes[4], 0, "revisión ID3v2.3.0");
        let tag_size = syncsafe_size(&bytes[6..10]);
        let tag_end = 10 + tag_size;
        assert!(tag_end <= bytes.len());

        // Parser independiente del crate: recorre frames v2.3 (tamaño plano,
        // big endian) y comprueba que están los IDs que un lector externo
        // necesita. Si el writer emitiera algo raro, esto falla.
        let mut offset = 10;
        let mut ids = Vec::new();
        while offset + 10 <= tag_end && &bytes[offset..offset + 4] != b"\0\0\0\0" {
            let id = std::str::from_utf8(&bytes[offset..offset + 4]).unwrap();
            let size = u32::from_be_bytes([
                bytes[offset + 4],
                bytes[offset + 5],
                bytes[offset + 6],
                bytes[offset + 7],
            ]) as usize;
            ids.push(id.to_string());
            offset += 10 + size;
        }
        for expected in [
            "TIT2", "TPE1", "TALB", "TPE2", "TYER", "TRCK", "TPOS", "TSRC", "APIC",
        ] {
            assert!(
                ids.iter().any(|id| id == expected),
                "falta el frame {expected}; frames: {ids:?}"
            );
        }
        assert_eq!(
            offset, tag_end,
            "los tamaños de frame cierran exactamente el tag"
        );
    }

    #[test]
    fn missing_title_or_artist_is_rejected_before_touching_the_file() {
        let dir = tempfile::TempDir::new().unwrap();
        let path = write_temp(&dir, "track.mp3", &real_mp3());
        let original = std::fs::read(&path).unwrap();

        for tags in [
            Mp3Tags {
                title: None,
                ..full_tags()
            },
            Mp3Tags {
                artist: Some("   ".to_string()),
                ..full_tags()
            },
        ] {
            let error = apply_and_verify_mp3_tags(&path, &tags).unwrap_err();
            assert!(matches!(error, Mp3TagError::InvalidInput(_)), "{error}");
            assert_eq!(std::fs::read(&path).unwrap(), original);
        }
    }

    #[test]
    fn non_mp3_payload_is_rejected() {
        let dir = tempfile::TempDir::new().unwrap();
        let path = write_temp(&dir, "fake.mp3", b"fLaC not an mp3");
        let error = apply_and_verify_mp3_tags(&path, &full_tags()).unwrap_err();
        assert!(matches!(error, Mp3TagError::InvalidInput(_)), "{error}");
        let generic = write_temp(&dir, "fake.bin", &real_mp3());
        apply_and_verify_mp3_tags(&generic, &full_tags())
            .expect("la cabecera MPEG basta aunque la extensión no sea .mp3");
    }

    #[test]
    fn a_failed_write_restores_the_exact_original() {
        let dir = tempfile::TempDir::new().unwrap();
        let path = write_temp(&dir, "track.mp3", &real_mp3());
        let original = std::fs::read(&path).unwrap();

        // Se fuerza el fallo de relectura después de una escritura real: el
        // contrato exige restaurar el original byte a byte y propagar error.
        FORCE_VERIFICATION_FAILURE.with(|flag| flag.set(true));
        let error = apply_and_verify_mp3_tags(&path, &full_tags()).unwrap_err();
        assert!(
            matches!(error, Mp3TagError::Verification(_)),
            "se esperaba el fallo de relectura inyectado, obtuve {error}"
        );
        assert_eq!(
            std::fs::read(&path).unwrap(),
            original,
            "el fichero original debe volver intacto"
        );
        assert_eq!(
            std::fs::read_dir(dir.path()).unwrap().count(),
            1,
            "no debe quedar backup temporal"
        );

        // Tras el fallo, el escritor sigue funcionando sobre el original.
        apply_and_verify_mp3_tags(&path, &full_tags()).expect("segunda escritura limpia");
    }

    #[test]
    fn minimal_mp3_is_accepted_and_tagged() {
        let dir = tempfile::TempDir::new().unwrap();
        let path = write_temp(&dir, "minimal.mp3", MINIMAL_MP3);
        let tags = Mp3Tags {
            title: Some("Minimal".to_string()),
            artist: Some("Fixture".to_string()),
            ..Mp3Tags::default()
        };
        let readback = apply_and_verify_mp3_tags(&path, &tags).unwrap();
        assert_eq!(readback.title.as_deref(), Some("Minimal"));
        assert!(read_mp3_tags(&path).unwrap().title.is_some());
    }

    #[test]
    fn duplicate_tagging_is_idempotent_and_preserves_audio() {
        let dir = tempfile::TempDir::new().unwrap();
        let path = write_temp(&dir, "track.mp3", &real_mp3());
        let audio = audio_payload(&path);
        let tags = full_tags();
        apply_and_verify_mp3_tags(&path, &tags).unwrap();
        let first = std::fs::read(&path).unwrap();
        apply_and_verify_mp3_tags(&path, &tags).unwrap();
        let second = std::fs::read(&path).unwrap();
        assert_eq!(first, second, "reescribir los mismos tags es determinista");
        assert_eq!(audio_payload(&path), audio);
    }

    #[test]
    fn from_download_request_maps_catalog_metadata() {
        let request = DownloadRequest {
            track_name: "Canción".to_string(),
            artist_name: "Artista".to_string(),
            album_name: "Álbum".to_string(),
            album_artist: Some("Varios".to_string()),
            release_date: Some("2021-07-09".to_string()),
            track_number: 5,
            total_tracks: 12,
            disc_number: 2,
            isrc: Some("ES1234567890".to_string()),
            ..DownloadRequest::default()
        };
        let tags = Mp3Tags::from_download_request(&request);
        assert_eq!(tags.title.as_deref(), Some("Canción"));
        assert_eq!(tags.artist.as_deref(), Some("Artista"));
        assert_eq!(tags.album.as_deref(), Some("Álbum"));
        assert_eq!(tags.album_artist.as_deref(), Some("Varios"));
        assert_eq!(tags.year, Some(2021));
        assert_eq!(tags.track_number, Some(5));
        assert_eq!(tags.track_total, Some(12));
        assert_eq!(tags.disc_number, Some(2));
        assert_eq!(tags.isrc.as_deref(), Some("ES1234567890"));
        assert!(tags.cover.is_none(), "la portada la aporta el adaptador");

        // `total_tracks` desconocido cae a 1 en el pipeline: no debe escribirse
        // como total de una pista 5.
        let unknown_total = Mp3Tags::from_download_request(&DownloadRequest {
            track_number: 5,
            total_tracks: 1,
            ..request
        });
        assert_eq!(unknown_total.track_total, None);
    }

    #[test]
    fn invalid_cover_is_rejected_without_writing() {
        let dir = tempfile::TempDir::new().unwrap();
        let path = write_temp(&dir, "track.mp3", &real_mp3());
        let original = std::fs::read(&path).unwrap();
        let mut tags = full_tags();
        tags.cover = Some(Mp3Cover {
            mime_type: "image/jpeg".to_string(),
            data: b"not-an-image".to_vec(),
        });
        let error = apply_and_verify_mp3_tags(&path, &tags).unwrap_err();
        assert!(matches!(error, Mp3TagError::InvalidCover(_)), "{error}");
        assert_eq!(std::fs::read(&path).unwrap(), original);
    }
}

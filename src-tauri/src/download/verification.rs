//! D05 (#117): verificación de entrega antes de `complete`.
//!
//! El «completado» que solo comprueba que el fichero existe y no está vacío
//! dejaba pasar entregas truncadas o mal etiquetadas. Este gate relee el
//! fichero físico ANTES de que el worker promueva la fila y escriba el ledger:
//!
//! - el fichero existe, es regular y no está vacío (lo comprueba el llamador;
//!   aquí se re-verifica por ser la precondition del resto);
//! - la duración física (STREAMINFO / MP4) está dentro de una tolerancia
//!   respecto a la duración del catálogo;
//! - los tags releídos (título, artista, álbum, disco, pista) corroboran el
//!   catálogo;
//! - la portada es válida (datos presentes con magia JPEG/PNG) cuando el
//!   usuario pidió incrustarla.
//!
//! Si el gate falla, el worker marca un fallo de verificación y NO escribe el
//! ledger de `downloads`: un fichero que no pasa la verificación no entra en
//! la biblioteca.
//!
//! Alcance honesto de los formatos: FLAC se verifica completo (metaflac).
//! M4A/AAC se verifica con `mp4ameta`. MP3 se lee con el escritor ID3v2
//! compartido (`download/id3_writer.rs`, D16): sus tags y portada son
//! evidencia desde que existe lector, y un MP3 sin tags falla los checks que
//! el catálogo declara. La duración MP3 sigue sin lector aquí (`audio_inspector`
//! no la calcula), así que ese check se salta por falta de evidencia.
//!
//! #338: «falta de lector» y «lectura fallida» dejan de confundirse. Para un
//! formato con lector (FLAC/M4A/MP3) un fichero no vacío que el lector no puede
//! parsear es un fallo cerrado del gate, no un check saltado.
//!
//! Límite conocido de la duración: sale de la cabecera (STREAMINFO / mvhd), así
//! que un fichero cortado por la cola conserva la duración completa y este
//! check no lo detecta. Lo que sí detecta: un fichero con cabecera propia más
//! corta que el catálogo (preview, edición o pista equivocadas). Si no hay
//! ninguna duración física el check se salta por falta de evidencia; bajo
//! política estricta el worker sí rechaza la reinspección fallida en vez de
//! creer la calidad anunciada. La corroboración de longitud decodificable queda
//! fuera porque duplicaría un decode completo por pista; el árbol ya tiene el
//! verificador (`syncify_flac_writer::verify_flac_integrity_stream`: MD5 PCM de
//! STREAMINFO o decode-check), pero su coste por pista debe medirse antes de
//! cablearlo al gate. El test `characterization_header_duration_cannot_detect_tail_truncation`
//! fija el límite y `shorter_header_detects_preview_or_wrong_file` el caso
//! cubierto.

use std::path::Path;

use syncify_core_domain::byte_validators::AudioByteValidator;

/// Tolerancia de duración fijada por el test de este ticket (issue #117):
/// ±2 s entre la duración física y la del catálogo.
pub const DURATION_TOLERANCE_SECS: f64 = 2.0;

/// Lo que el catálogo promete para la entrega.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct ExpectedDelivery {
    pub title: String,
    pub artist: String,
    pub album: Option<String>,
    pub track_number: Option<i64>,
    pub disc_number: Option<i64>,
    /// Duración del catálogo en ms; `None` o 0 = desconocida (no se compara).
    pub catalog_duration_ms: Option<i64>,
}

/// Lo que el fichero entregado declara tras releerlo.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct DeliveredTags {
    pub title: Option<String>,
    pub artist: Option<String>,
    pub album: Option<String>,
    pub track_number: Option<i64>,
    pub disc_number: Option<i64>,
    /// Portada frontal presente con datos no vacíos y magia JPEG/PNG.
    pub has_valid_cover: bool,
}

/// #338: resultado de releer los tags, con la distinción que el gate necesita.
#[derive(Debug, Clone, PartialEq)]
pub enum TagReadOutcome {
    /// El formato tiene lector y la lectura funcionó.
    Readable(DeliveredTags),
    /// El formato no tiene lector en este árbol: falta de evidencia, no fallo.
    /// Con D16 ya solo aplica a extensiones sin lector (p. ej. WAV): MP3 tiene
    /// lector ID3.
    NoReader,
    /// El formato declara lector y la lectura falló (fichero ilegible): el
    /// gate falla cerrado.
    Unreadable(String),
}

/// Verifica la entrega física contra lo que el catálogo promete.
///
/// `require_cover` refleja la preferencia del usuario (embed_artwork): la
/// portada solo se exige cuando se pidió incrustarla. `extra_tolerance_secs`
/// absorbe el recorte de silencio muerto (TASK-76): si el trimmer quitó
/// segundos de plomo, la diferencia frente al catálogo es legítima.
///
/// En error devuelve la lista de incumplimientos detectados (uno por check),
/// con mensajes listos para el `error_message` de la fila de cola.
pub fn verify_delivery(
    path: &Path,
    expected: &ExpectedDelivery,
    require_cover: bool,
    extra_tolerance_secs: f64,
) -> Result<DeliveredTags, Vec<String>> {
    let mut failures: Vec<String> = Vec::new();

    // Precondition re-verificada: existe, es regular y no está vacío.
    match std::fs::metadata(path) {
        Ok(m) if m.is_file() && m.len() > 0 => {}
        _ => {
            failures.push(format!(
                "file is missing, not a regular file, or empty at '{}'",
                path.display()
            ));
            return Err(failures);
        }
    }

    let physical = crate::download::audio_inspector::inspect_physical_audio_file(path);

    // #338: un formato con lector que no se deja leer es un fallo cerrado, no
    // falta de evidencia. Solo una extensión sin lector se salta.
    let delivered = match read_delivered_tags(path) {
        TagReadOutcome::Readable(delivered) => {
            verify_tags(&delivered, expected, &mut failures);
            if require_cover && !delivered.has_valid_cover {
                failures.push("cover art missing or invalid in the delivered file".to_string());
            }
            delivered
        }
        TagReadOutcome::NoReader => DeliveredTags::default(),
        TagReadOutcome::Unreadable(reason) => {
            failures.push(format!(
                "tags unreadable in a format that declares a reader: {reason}"
            ));
            DeliveredTags::default()
        }
    };

    // Duración física dentro de tolerancia frente al catálogo.
    if let (Some(phys), Some(catalog_ms)) = (
        physical.as_ref().and_then(|p| p.duration_secs),
        non_zero_catalog_duration(expected),
    ) {
        let catalog_secs = catalog_ms as f64 / 1000.0;
        let tolerance = DURATION_TOLERANCE_SECS + extra_tolerance_secs;
        let delta = (phys - catalog_secs).abs();
        if delta > tolerance {
            failures.push(format!(
                "duration out of tolerance: physical {:.1}s vs catalog {:.1}s (allowed ±{:.1}s)",
                phys, catalog_secs, tolerance
            ));
        }
    }

    if failures.is_empty() {
        Ok(delivered)
    } else {
        Err(failures)
    }
}

/// Solo una duración de catálogo positiva es evidencia. Un `duration_ms = 0`
/// del pipeline significa «desconocida» y no se compara.
fn non_zero_catalog_duration(expected: &ExpectedDelivery) -> Option<i64> {
    expected.catalog_duration_ms.filter(|ms| *ms > 0)
}

/// Corrobora los tags releídos contra el catálogo. Un tag ausente cuando el
/// catálogo sí lo declara es fallo; el catálogo sin dato no exige nada.
fn verify_tags(delivered: &DeliveredTags, expected: &ExpectedDelivery, failures: &mut Vec<String>) {
    match_tag(
        delivered.title.as_deref(),
        Some(expected.title.as_str()),
        "title",
        failures,
    );
    match_tag(
        delivered.artist.as_deref(),
        Some(expected.artist.as_str()),
        "artist",
        failures,
    );
    match_tag(
        delivered.album.as_deref(),
        expected.album.as_deref(),
        "album",
        failures,
    );
    if let Some(expected_track) = expected.track_number {
        if delivered.track_number != Some(expected_track) {
            failures.push(format!(
                "track number mismatch: file {:?} vs catalog {}",
                delivered.track_number, expected_track
            ));
        }
    }
    if let Some(expected_disc) = expected.disc_number {
        if delivered.disc_number != Some(expected_disc) {
            failures.push(format!(
                "disc number mismatch: file {:?} vs catalog {}",
                delivered.disc_number, expected_disc
            ));
        }
    }
}

/// Compara un tag de texto con tolerancia de mayúsculas y espacios: el
/// contenido debe coincidir, el formato no.
fn match_tag(
    delivered: Option<&str>,
    expected: Option<&str>,
    field: &str,
    failures: &mut Vec<String>,
) {
    let Some(expected) = expected else {
        return;
    };
    let expected_norm = expected.trim();
    if expected_norm.is_empty() {
        return;
    }
    let Some(delivered) = delivered.map(str::trim).filter(|t| !t.is_empty()) else {
        failures.push(format!("tag '{field}' missing in the delivered file"));
        return;
    };
    if !delivered.eq_ignore_ascii_case(expected_norm) {
        failures.push(format!(
            "tag '{field}' mismatch: file '{}' vs catalog '{}'",
            delivered, expected_norm
        ));
    }
}

/// Relee los tags del fichero entregado distinguiendo lector ausente de
/// lectura fallida (#338). `NoReader` es falta de evidencia; `Unreadable` es
/// un fichero de un formato con lector que no se deja parsear, y el gate lo
/// trata como fallo.
///
/// D16: MP3 pasa a tener lector (ID3v2). Un MP3 sin tag es `Readable` con
/// campos ausentes —no `NoReader`—, para que el gate exija los tags que el
/// catálogo declara en vez de saltárselos.
pub fn read_delivered_tags(path: &Path) -> TagReadOutcome {
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_lowercase();

    if ext == "flac" || looks_like_flac(path) {
        return match read_flac_tags(path) {
            Ok(tags) => TagReadOutcome::Readable(tags),
            Err(reason) => TagReadOutcome::Unreadable(reason),
        };
    }
    if ext == "m4a" || ext == "aac" || ext == "mp4" {
        return match read_mp4_tags(path) {
            Ok(tags) => TagReadOutcome::Readable(tags),
            Err(reason) => TagReadOutcome::Unreadable(reason),
        };
    }
    if ext == "mp3" || crate::download::id3_writer::is_mp3_file(path) {
        return match read_mp3_tags(path) {
            Ok(tags) => TagReadOutcome::Readable(tags),
            Err(reason) => TagReadOutcome::Unreadable(reason),
        };
    }
    TagReadOutcome::NoReader
}

fn looks_like_flac(path: &Path) -> bool {
    read_prefix(path, 4).is_some_and(|bytes| AudioByteValidator::is_flac_magic(&bytes))
}

fn read_prefix(path: &Path, max_bytes: usize) -> Option<Vec<u8>> {
    use std::io::Read;
    let file = std::fs::File::open(path).ok()?;
    let mut buffer = Vec::new();
    file.take(max_bytes as u64).read_to_end(&mut buffer).ok()?;
    Some(buffer)
}

fn vorbis_first(vorbis: &metaflac::block::VorbisComment, key: &str) -> Option<String> {
    vorbis
        .get(key)
        .and_then(|values| values.first())
        .map(|v| v.trim().to_string())
}

/// "3" o "3/12" → 3. Solo el número principal interesa al gate.
fn parse_number_prefix(raw: &str) -> Option<i64> {
    let digits: String = raw.chars().take_while(|c| c.is_ascii_digit()).collect();
    digits.parse::<i64>().ok()
}

fn read_flac_tags(path: &Path) -> Result<DeliveredTags, String> {
    let tag = metaflac::Tag::read_from_path(path)
        .map_err(|e| format!("metaflac cannot parse '{}': {e}", path.display()))?;
    let vorbis = tag.vorbis_comments();
    let cover = tag
        .pictures()
        .find(|pic| pic.picture_type == metaflac::block::PictureType::CoverFront)
        .map(|pic| is_valid_image_payload(&pic.data))
        .unwrap_or(false);
    Ok(DeliveredTags {
        title: vorbis.and_then(|v| vorbis_first(v, "TITLE")),
        artist: vorbis.and_then(|v| vorbis_first(v, "ARTIST")),
        album: vorbis.and_then(|v| vorbis_first(v, "ALBUM")),
        track_number: vorbis
            .and_then(|v| vorbis_first(v, "TRACKNUMBER"))
            .and_then(|raw| parse_number_prefix(&raw)),
        disc_number: vorbis
            .and_then(|v| vorbis_first(v, "DISCNUMBER"))
            .and_then(|raw| parse_number_prefix(&raw)),
        has_valid_cover: cover,
    })
}

fn read_mp4_tags(path: &Path) -> Result<DeliveredTags, String> {
    let tag = mp4ameta::Tag::read_from_path(path)
        .map_err(|e| format!("mp4ameta cannot parse '{}': {e}", path.display()))?;
    let cover = tag
        .artwork()
        .map(|img| is_valid_image_payload(img.data))
        .unwrap_or(false);
    Ok(DeliveredTags {
        title: tag.title().map(|v| v.trim().to_string()),
        artist: tag.artist().map(|v| v.trim().to_string()),
        album: tag.album().map(|v| v.trim().to_string()),
        track_number: tag.track_number().map(|v| v as i64),
        disc_number: tag.disc_number().map(|v| v as i64),
        has_valid_cover: cover,
    })
}

/// D16: lee el ID3v2 de un MP3 entregado. `id3_writer::read_mp3_tags` distingue
/// «sin tag» (`Ok` vacío) de «tag ilegible» (`Err`): aquí el segundo es
/// `Unreadable` y falla cerrado, como FLAC/M4A.
fn read_mp3_tags(path: &Path) -> Result<DeliveredTags, String> {
    let tags = crate::download::id3_writer::read_mp3_tags(path)
        .map_err(|e| format!("id3 cannot parse '{}': {e}", path.display()))?;
    Ok(DeliveredTags {
        title: tags.title,
        artist: tags.artist,
        album: tags.album,
        track_number: tags.track_number.map(i64::from),
        disc_number: tags.disc_number.map(i64::from),
        has_valid_cover: tags
            .cover
            .as_ref()
            .is_some_and(|cover| is_valid_image_payload(&cover.data)),
    })
}

/// Una portada válida son bytes presentes que empiezan por la magia JPEG o
/// PNG. Un payload vacío, un placeholder o un formato ajeno no pasan.
fn is_valid_image_payload(data: &[u8]) -> bool {
    if data.len() < 8 {
        return false;
    }
    data.starts_with(&[0xFF, 0xD8, 0xFF]) // JPEG
        || data.starts_with(&[0x89, b'P', b'N', b'G']) // PNG
}

#[cfg(test)]
mod tests {
    use super::*;

    /// FLAC sintético con STREAMINFO 16-bit/44.1kHz y total_samples que fija
    /// la duración (patrón de provider_gateway_test).
    fn write_flac_with_duration(path: &Path, total_samples: u64) {
        let mut flac_bytes = Vec::new();
        flac_bytes.extend_from_slice(b"fLaC");
        flac_bytes.extend_from_slice(&[0x80, 0x00, 0x00, 0x22]);
        let mut streaminfo = [0u8; 34];
        let block_size = (total_samples.min(65535)) as u16;
        streaminfo[0..2].copy_from_slice(&block_size.to_be_bytes());
        streaminfo[2..4].copy_from_slice(&block_size.to_be_bytes());
        // sample_rate (20 bits) = 44100, channels-1 (3 bits) = 1, bps-1 (5 bits) = 15
        streaminfo[10] = 0x0A;
        streaminfo[11] = 0xC4;
        streaminfo[12] = 0x42;
        streaminfo[13] = 0xF0;
        // total_samples: 36 bits, el byte 13 bajo el nibble 0xF0 aportado ya
        // vale 0, así que los 32 bits bajos van en los bytes 14-17.
        streaminfo[14..18]
            .copy_from_slice(&(u32::try_from(total_samples).unwrap_or(u32::MAX)).to_be_bytes());
        flac_bytes.extend_from_slice(&streaminfo);
        flac_bytes.extend_from_slice(&[0xFF, 0xF8, 0x18, 0x00, 0x00, 0x00, 0x00, 0x00]);
        std::fs::write(path, &flac_bytes).unwrap();
    }

    fn expected(duration_ms: i64) -> ExpectedDelivery {
        ExpectedDelivery {
            title: "Verified Track".to_string(),
            artist: "Verified Artist".to_string(),
            album: Some("Verified Album".to_string()),
            track_number: Some(3),
            disc_number: Some(1),
            catalog_duration_ms: Some(duration_ms),
        }
    }

    /// Expectativa vacía de identidad: solo compara duración (los tags vacíos
    /// del catálogo no exigen nada).
    fn duration_only(duration_ms: i64) -> ExpectedDelivery {
        ExpectedDelivery {
            catalog_duration_ms: Some(duration_ms),
            ..ExpectedDelivery::default()
        }
    }

    #[test]
    fn file_missing_or_empty_fails_closed() {
        let dir = tempfile::TempDir::new().unwrap();
        let missing = dir.path().join("missing.flac");
        let failures = verify_delivery(&missing, &expected(0), false, 0.0).unwrap_err();
        assert!(failures[0].contains("missing, not a regular file, or empty"));

        let empty = dir.path().join("empty.flac");
        std::fs::write(&empty, b"").unwrap();
        let failures = verify_delivery(&empty, &expected(0), false, 0.0).unwrap_err();
        assert!(failures[0].contains("missing, not a regular file, or empty"));
    }

    #[test]
    fn duration_within_tolerance_passes_and_beyond_fails() {
        let dir = tempfile::TempDir::new().unwrap();
        // Catálogo 200 s; físico 200.5 s → dentro de ±2 s.
        let path = dir.path().join("ok.flac");
        write_flac_with_duration(&path, 200 * 44_100 + 22_050);
        verify_delivery(&path, &duration_only(200_000), false, 0.0).expect("dentro de tolerancia");

        // Físico 197 s vs catálogo 200 s → fuera de la tolerancia fijada.
        let path = dir.path().join("truncated.flac");
        write_flac_with_duration(&path, 197 * 44_100);
        let failures = verify_delivery(&path, &duration_only(200_000), false, 0.0).unwrap_err();
        assert!(
            failures
                .iter()
                .any(|f| f.contains("duration out of tolerance")),
            "fallo de duración esperado, obtuve: {failures:?}"
        );

        // El trimmer de silencio amplía la tolerancia lo que recortó: el
        // mismo fichero truncado pasa con la tolerancia del trim legítimo.
        verify_delivery(&path, &duration_only(200_000), false, 3.0)
            .expect("3 s de trim legítimo no deben fallar la duración");
    }

    #[test]
    fn catalog_duration_zero_or_unknown_is_not_compared() {
        let dir = tempfile::TempDir::new().unwrap();
        let path = dir.path().join("short.flac");
        write_flac_with_duration(&path, 10 * 44_100);
        verify_delivery(&path, &duration_only(0), false, 0.0).expect("catálogo 0 = desconocido");
        let no_catalog = ExpectedDelivery {
            catalog_duration_ms: None,
            ..duration_only(0)
        };
        verify_delivery(&path, &no_catalog, false, 0.0).expect("catálogo ausente no se compara");
    }

    #[test]
    fn flac_tags_and_cover_are_corroborated() {
        let dir = tempfile::TempDir::new().unwrap();
        let path = dir.path().join("tagged.flac");
        write_flac_with_duration(&path, 200 * 44_100);

        // Sin tags: el gate falla por título, artista y álbum (el catálogo los
        // declara) y por portada cuando se pidió incrustarla.
        let failures = verify_delivery(&path, &expected(200_000), true, 0.0).unwrap_err();
        assert!(failures.iter().any(|f| f.contains("tag 'title' missing")));
        assert!(failures.iter().any(|f| f.contains("tag 'artist' missing")));
        assert!(failures.iter().any(|f| f.contains("tag 'album' missing")));
        assert!(failures.iter().any(|f| f.contains("cover art missing")));

        // Con tags correctos y sin pedir portada: pasa.
        let mut tag = metaflac::Tag::default();
        let vc = tag.vorbis_comments_mut();
        vc.set("TITLE", vec!["Verified Track"]);
        vc.set("ARTIST", vec!["Verified Artist"]);
        vc.set("ALBUM", vec!["Verified Album"]);
        vc.set("TRACKNUMBER", vec!["3"]);
        vc.set("DISCNUMBER", vec!["1"]);
        tag.write_to_path(&path).unwrap();
        verify_delivery(&path, &expected(200_000), false, 0.0)
            .expect("tags correctos y portada no exigida");
    }

    #[test]
    fn flac_cover_validity_requires_image_magic() {
        let dir = tempfile::TempDir::new().unwrap();
        let path = dir.path().join("cover.flac");
        write_flac_with_duration(&path, 200 * 44_100);
        let mut tag = metaflac::Tag::default();
        let vc = tag.vorbis_comments_mut();
        vc.set("TITLE", vec!["Verified Track"]);
        vc.set("ARTIST", vec!["Verified Artist"]);
        vc.set("ALBUM", vec!["Verified Album"]);
        tag.write_to_path(&path).unwrap();

        // Payload de portada vacío o sin magia no es una portada válida.
        for (name, payload) in [
            ("empty", Vec::new()),
            ("not-image", b"GIF89a-not-an-image".to_vec()),
        ] {
            let mut tag = metaflac::Tag::read_from_path(&path).unwrap();
            tag.remove_picture_type(metaflac::block::PictureType::CoverFront);
            tag.add_picture(
                "image/jpeg",
                metaflac::block::PictureType::CoverFront,
                payload,
            );
            tag.write_to_path(&path).unwrap();
            let TagReadOutcome::Readable(delivered) = read_delivered_tags(&path) else {
                panic!("el FLAC de prueba debe ser legible");
            };
            assert!(
                !delivered.has_valid_cover,
                "portada {name} no debe contar como válida"
            );
            let failures = verify_delivery(&path, &expected(200_000), true, 0.0).unwrap_err();
            assert!(failures.iter().any(|f| f.contains("cover art missing")));
        }

        // Una portada PNG real pasa el gate completo (con los tags correctos).
        let png = vec![
            0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A, 0, 0, 0, 13, b'I', b'H', b'D', b'R',
        ];
        let mut tag = metaflac::Tag::read_from_path(&path).unwrap();
        tag.remove_picture_type(metaflac::block::PictureType::CoverFront);
        tag.add_picture("image/png", metaflac::block::PictureType::CoverFront, png);
        tag.write_to_path(&path).unwrap();
        let mut tag = metaflac::Tag::read_from_path(&path).unwrap();
        let vc = tag.vorbis_comments_mut();
        vc.set("TITLE", vec!["Verified Track"]);
        vc.set("ARTIST", vec!["Verified Artist"]);
        vc.set("ALBUM", vec!["Verified Album"]);
        vc.set("TRACKNUMBER", vec!["3"]);
        vc.set("DISCNUMBER", vec!["1"]);
        tag.write_to_path(&path).unwrap();
        verify_delivery(&path, &expected(200_000), true, 0.0).expect("portada PNG válida");
    }

    #[test]
    fn tag_mismatch_and_format_tolerances() {
        let dir = tempfile::TempDir::new().unwrap();
        let path = dir.path().join("mismatch.flac");
        write_flac_with_duration(&path, 200 * 44_100);
        let mut tag = metaflac::Tag::default();
        let vc = tag.vorbis_comments_mut();
        vc.set("TITLE", vec!["Wrong Track"]);
        vc.set("ARTIST", vec!["verified artist "]); // casing/espacios: tolerado
        vc.set("TRACKNUMBER", vec!["7/12"]); // "3" esperado
        tag.write_to_path(&path).unwrap();

        let failures = verify_delivery(&path, &expected(200_000), false, 0.0).unwrap_err();
        assert!(failures.iter().any(|f| f.contains("tag 'title' mismatch")));
        assert!(failures
            .iter()
            .any(|f| f.contains("track number mismatch: file Some(7) vs catalog 3")));
        assert!(failures.iter().any(|f| f.contains("tag 'album' missing")));
        assert!(
            !failures.iter().any(|f| f.contains("artist")),
            "casing/espacios del artista se toleran: {failures:?}"
        );

        // Catálogo sin número de disco no exige DISCNUMBER.
        let partial = ExpectedDelivery {
            disc_number: None,
            ..expected(200_000)
        };
        let failures = verify_delivery(&path, &partial, false, 0.0).unwrap_err();
        assert!(!failures.iter().any(|f| f.contains("disc")));
    }

    /// #338, caracterización: la duración verificada sale de la cabecera
    /// (STREAMINFO), así que un fichero cortado por la cola conserva la
    /// duración completa y este check no lo detecta. El límite es deliberado
    /// en este hito: se documenta aquí antes de decidir si se corrobora con
    /// una longitud decodificable (coste de un decode completo por pista).
    #[test]
    fn characterization_header_duration_cannot_detect_tail_truncation() {
        let dir = tempfile::TempDir::new().unwrap();
        let path = dir.path().join("tail-truncated.flac");
        write_flac_with_duration(&path, 200 * 44_100);
        // Quita bytes del final del payload: la cabecera sigue intacta y
        // declara los 200 s completos.
        let len = std::fs::metadata(&path).unwrap().len();
        let file = std::fs::OpenOptions::new().write(true).open(&path).unwrap();
        file.set_len(len - 4).unwrap();
        verify_delivery(&path, &duration_only(200_000), false, 0.0)
            .expect("cabecera intacta: el truncado de cola no se detecta aquí");
    }

    /// #338: el caso que la duración de cabecera SÍ cubre — un preview o un
    /// fichero equivocado trae su propia cabecera más corta que el catálogo.
    #[test]
    fn shorter_header_detects_preview_or_wrong_file() {
        let dir = tempfile::TempDir::new().unwrap();
        let path = dir.path().join("preview.flac");
        write_flac_with_duration(&path, 30 * 44_100);
        let failures = verify_delivery(&path, &duration_only(200_000), false, 0.0).unwrap_err();
        assert!(failures
            .iter()
            .any(|f| f.contains("duration out of tolerance")));
    }

    #[test]
    fn mp3_tags_are_read_and_verified_with_the_shared_id3_writer() {
        use crate::download::id3_writer::{Mp3Cover, Mp3Tags};

        // Frame MPEG mínimo: basta para que el detector lo reconozca como MP3.
        const MINIMAL_MP3: &[u8] = &[0xFF, 0xFB, 0x90, 0x64, 0, 0, 0, 0];
        const PNG_COVER: &[u8] = &[
            0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A, 0x00, 0x00, 0x00, 0x0D, b'I', b'H',
            b'D', b'R',
        ];

        let dir = tempfile::TempDir::new().unwrap();
        let path = dir.path().join("soundcloud.mp3");
        std::fs::write(&path, MINIMAL_MP3).unwrap();

        // D16: MP3 ya tiene lector. Un MP3 sin tag no se salta: los tags que
        // el catálogo declara son exigibles.
        assert!(matches!(
            read_delivered_tags(&path),
            TagReadOutcome::Readable(_)
        ));
        let failures = verify_delivery(&path, &expected(0), false, 0.0).unwrap_err();
        assert!(failures.iter().any(|f| f.contains("tag 'title' missing")));
        assert!(failures.iter().any(|f| f.contains("tag 'artist' missing")));
        assert!(failures.iter().any(|f| f.contains("tag 'album' missing")));

        // Con el escritor compartido, el gate verifica los tags y el APIC.
        let tags = Mp3Tags {
            title: Some("Verified Track".to_string()),
            artist: Some("Verified Artist".to_string()),
            album: Some("Verified Album".to_string()),
            track_number: Some(3),
            disc_number: Some(1),
            cover: Some(Mp3Cover::from_bytes(PNG_COVER.to_vec()).unwrap()),
            ..Mp3Tags::default()
        };
        crate::download::id3_writer::apply_and_verify_mp3_tags(&path, &tags).unwrap();
        verify_delivery(&path, &expected(0), true, 0.0)
            .expect("MP3 con ID3 y portada pasa el gate");
    }

    /// #338: un FLAC no vacío que metaflac no puede parsear ya no pasa el
    /// gate por falta de evidencia: es un formato con lector y la lectura
    /// falló, así que falla cerrado.
    #[test]
    fn unreadable_flac_fails_closed() {
        let dir = tempfile::TempDir::new().unwrap();
        let path = dir.path().join("corrupt.flac");
        std::fs::write(&path, b"fLaC-not-a-parseable-tag-chain").unwrap();

        assert!(matches!(
            read_delivered_tags(&path),
            TagReadOutcome::Unreadable(_)
        ));
        let failures = verify_delivery(&path, &duration_only(200_000), false, 0.0).unwrap_err();
        assert!(
            failures
                .iter()
                .any(|f| f.contains("tags unreadable in a format that declares a reader")),
            "fallo cerrado esperado, obtuve: {failures:?}"
        );
    }

    /// #338: lo mismo para M4A: mp4ameta no puede parsear el contenedor y el
    /// gate no lo trata como «sin lector».
    #[test]
    fn unreadable_m4a_fails_closed() {
        let dir = tempfile::TempDir::new().unwrap();
        let path = dir.path().join("corrupt.m4a");
        std::fs::write(&path, b"not-an-mp4-container").unwrap();

        assert!(matches!(
            read_delivered_tags(&path),
            TagReadOutcome::Unreadable(_)
        ));
        let failures = verify_delivery(&path, &duration_only(200_000), false, 0.0).unwrap_err();
        assert!(
            failures
                .iter()
                .any(|f| f.contains("tags unreadable in a format that declares a reader")),
            "fallo cerrado esperado, obtuve: {failures:?}"
        );
    }
}

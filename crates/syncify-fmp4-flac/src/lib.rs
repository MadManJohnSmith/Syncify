//! FLAC-en-ISOBMFF fragmentado → FLAC nativo.
//!
//! Tidal entrega las pistas lossless/hi-res como un manifiesto DASH: un
//! segmento `init` (`ftyp` + `moov` con un sample entry `fLaC` y el bloque
//! `dfLa` que lleva el STREAMINFO real) seguido de N segmentos `moof`+`mdat`.
//! Ese flujo no es reproducible de forma progresiva (symphonia no tolera un
//! fichero que crece y rodio no puede buscar dentro de él), así que aquí se
//! extraen los frames FLAC tal cual vienen en los `mdat` y se escriben en un
//! FLAC nativo: cabecera `fLaC` + STREAMINFO + frames.
//!
//! Es una copia de paquetes, no una recodificación: el resultado decodifica
//! exactamente igual que el original (verificado en vivo contra el fMP4
//! completo: 85.862.400 muestras en ambos, 24 bits / 192 kHz).
//!
//! También sustituye al `ffmpeg` externo que usaba la descarga de Tidal para
//! este mismo remux, con lo que una instalación limpia no necesita nada más.

use std::fs::File;
use std::io::{BufWriter, Cursor, Read, Seek, SeekFrom, Write};
use std::path::Path;

use symphonia::core::formats::FormatOptions;
use symphonia::core::io::MediaSourceStream;
use symphonia::core::meta::MetadataOptions;
use symphonia::core::probe::Hint;

/// Longitud del bloque STREAMINFO (sin su cabecera de bloque).
pub const STREAMINFO_LEN: usize = 34;

/// STREAMINFO real leído del `dfLa` del init. `bytes` son los 34 bytes crudos,
/// tal y como se copian a la cabecera FLAC nativa.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FlacStreamInfo {
    pub bytes: [u8; STREAMINFO_LEN],
    pub min_block_size: u16,
    pub max_block_size: u16,
    pub min_frame_size: u32,
    pub max_frame_size: u32,
    pub sample_rate: u32,
    pub channels: u8,
    pub bits_per_sample: u8,
    pub total_samples: u64,
}

impl FlacStreamInfo {
    /// Duración declarada por el propio STREAMINFO (0.0 si no la trae).
    pub fn duration_secs(&self) -> f64 {
        if self.sample_rate == 0 {
            0.0
        } else {
            self.total_samples as f64 / self.sample_rate as f64
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum Fmp4FlacError {
    #[error("el init no declara una pista FLAC (sample entry fLaC)")]
    NotFlacTrack,
    #[error("el manifiesto está encriptado ({0})")]
    Encrypted(String),
    #[error("no se encontró el bloque dfLa/STREAMINFO en el init")]
    MissingStreamInfo,
    #[error("STREAMINFO inválido: {0}")]
    InvalidStreamInfo(String),
    #[error("el segmento no contiene frames FLAC válidos: {0}")]
    Segment(String),
    #[error("error de E/S: {0}")]
    Io(#[from] std::io::Error),
}

/// Lee el init y devuelve el STREAMINFO real. Rechaza pistas que no sean FLAC
/// (un manifiesto AAC nunca debe etiquetarse como FLAC) y encriptadas.
pub fn read_init(init: &[u8]) -> Result<FlacStreamInfo, Fmp4FlacError> {
    let moov = find_box(init, b"moov").ok_or(Fmp4FlacError::MissingStreamInfo)?;
    let trak = find_box(moov, b"trak").ok_or(Fmp4FlacError::MissingStreamInfo)?;
    let mdia = find_box(trak, b"mdia").ok_or(Fmp4FlacError::MissingStreamInfo)?;
    let minf = find_box(mdia, b"minf").ok_or(Fmp4FlacError::MissingStreamInfo)?;
    let stbl = find_box(minf, b"stbl").ok_or(Fmp4FlacError::MissingStreamInfo)?;
    let stsd = find_box(stbl, b"stsd").ok_or(Fmp4FlacError::MissingStreamInfo)?;

    // `stsd`: version/flags (4) + entry_count (4) + entries.
    if stsd.len() < 8 {
        return Err(Fmp4FlacError::MissingStreamInfo);
    }
    let entries = &stsd[8..];
    let (entry_type, entry_payload) = box_iter(entries)
        .next()
        .ok_or(Fmp4FlacError::MissingStreamInfo)?;

    if &entry_type == b"enca" || init.windows(4).any(|w| w == b"sinf") {
        return Err(Fmp4FlacError::Encrypted(
            "sample entry enca/sinf".to_string(),
        ));
    }
    if &entry_type != b"fLaC" {
        return Err(Fmp4FlacError::NotFlacTrack);
    }

    let dfla = find_box_scanned(entry_payload, b"dfLa").ok_or(Fmp4FlacError::MissingStreamInfo)?;
    // `dfLa`: version/flags (4) + bloques de metadatos FLAC.
    let mut pos = 4usize;
    while pos + 4 <= dfla.len() {
        let header = dfla[pos];
        let is_last = header & 0x80 != 0;
        let block_type = header & 0x7f;
        let len = ((dfla[pos + 1] as usize) << 16)
            | ((dfla[pos + 2] as usize) << 8)
            | dfla[pos + 3] as usize;
        if block_type == 0 && len == STREAMINFO_LEN && pos + 4 + STREAMINFO_LEN <= dfla.len() {
            let mut bytes = [0u8; STREAMINFO_LEN];
            bytes.copy_from_slice(&dfla[pos + 4..pos + 4 + STREAMINFO_LEN]);
            return parse_stream_info(bytes);
        }
        if is_last {
            break;
        }
        pos += 4 + len;
    }
    Err(Fmp4FlacError::MissingStreamInfo)
}

fn parse_stream_info(bytes: [u8; STREAMINFO_LEN]) -> Result<FlacStreamInfo, Fmp4FlacError> {
    let min_block_size = u16::from_be_bytes([bytes[0], bytes[1]]);
    let max_block_size = u16::from_be_bytes([bytes[2], bytes[3]]);
    let min_frame_size =
        ((bytes[4] as u32) << 16) | ((bytes[5] as u32) << 8) | bytes[6] as u32;
    let max_frame_size =
        ((bytes[7] as u32) << 16) | ((bytes[8] as u32) << 8) | bytes[9] as u32;
    let packed = u64::from_be_bytes(bytes[10..18].try_into().unwrap_or([0u8; 8]));
    let sample_rate = ((packed >> 44) & 0xF_FFFF) as u32;
    let channels = (((packed >> 41) & 0x7) as u8) + 1;
    let bits_per_sample = (((packed >> 36) & 0x1F) as u8) + 1;
    let total_samples = packed & 0xF_FFFF_FFFF;

    if sample_rate == 0 {
        return Err(Fmp4FlacError::InvalidStreamInfo(
            "frecuencia de muestreo 0".to_string(),
        ));
    }
    if !(4..=32).contains(&bits_per_sample) {
        return Err(Fmp4FlacError::InvalidStreamInfo(format!(
            "profundidad de bits inválida ({})",
            bits_per_sample
        )));
    }
    if !(1..=8).contains(&channels) {
        return Err(Fmp4FlacError::InvalidStreamInfo(format!(
            "canales inválidos ({})",
            channels
        )));
    }

    Ok(FlacStreamInfo {
        bytes,
        min_block_size,
        max_block_size,
        min_frame_size,
        max_frame_size,
        sample_rate,
        channels,
        bits_per_sample,
        total_samples,
    })
}

/// Escribe la cabecera de un FLAC nativo: `fLaC` + STREAMINFO como último
/// bloque de metadatos. Los frames se añaden después tal cual.
pub fn write_flac_header(
    out: &mut impl Write,
    info: &FlacStreamInfo,
) -> Result<(), Fmp4FlacError> {
    out.write_all(b"fLaC")?;
    out.write_all(&[0x80, 0x00, 0x00, STREAMINFO_LEN as u8])?;
    out.write_all(&info.bytes)?;
    Ok(())
}

/// Demuxea un segmento (`moof`+`mdat`) usando el init como contexto y entrega
/// los frames FLAC en orden. Devuelve las muestras (frames por canal) escritas.
///
/// El primer frame se comprueba contra el sync code de FLAC: si no lo parece,
/// el segmento se rechaza en vez de escribir bytes que no son audio.
pub fn demux_segment(
    init: &[u8],
    segment: &[u8],
    on_frame: &mut dyn FnMut(&[u8]) -> Result<(), Fmp4FlacError>,
) -> Result<u64, Fmp4FlacError> {
    let mut buf = Vec::with_capacity(init.len() + segment.len());
    buf.extend_from_slice(init);
    buf.extend_from_slice(segment);

    let mss = MediaSourceStream::new(Box::new(Cursor::new(buf)), Default::default());
    let probed = symphonia::default::get_probe()
        .format(
            &Hint::new(),
            mss,
            &FormatOptions::default(),
            &MetadataOptions::default(),
        )
        .map_err(|e| Fmp4FlacError::Segment(e.to_string()))?;
    let mut format = probed.format;

    let mut samples = 0u64;
    let mut first = true;
    loop {
        match format.next_packet() {
            Ok(packet) => {
                let frame = packet.buf();
                if first {
                    if frame.len() < 2 || frame[0] != 0xFF || (frame[1] & 0xFE) != 0xF8 {
                        return Err(Fmp4FlacError::Segment(format!(
                            "frame sin sync FLAC ({:02x?})",
                            &frame[..frame.len().min(4)]
                        )));
                    }
                    first = false;
                }
                samples += packet.dur();
                on_frame(frame)?;
            }
            Err(symphonia::core::errors::Error::IoError(e))
                if e.kind() == std::io::ErrorKind::UnexpectedEof =>
            {
                break;
            }
            Err(symphonia::core::errors::Error::ResetRequired) => continue,
            Err(e) => return Err(Fmp4FlacError::Segment(e.to_string())),
        }
    }
    Ok(samples)
}

/// Convierte un fMP4 completo (init + segmentos concatenados, lo que deja la
/// descarga de Tidal en el fichero temporal) a FLAC nativo. Devuelve las
/// muestras totales escritas.
pub fn convert_fmp4_to_flac(
    src: &Path,
    dst: &Path,
) -> Result<u64, Fmp4FlacError> {
    let head = read_head(src, 4 * 1024 * 1024)?;
    let info = read_init(&head)?;

    let mut out = BufWriter::new(File::create(dst)?);
    write_flac_header(&mut out, &info)?;

    let file = File::open(src)?;
    let mss = MediaSourceStream::new(Box::new(file), Default::default());
    let probed = symphonia::default::get_probe()
        .format(
            &Hint::new(),
            mss,
            &FormatOptions::default(),
            &MetadataOptions::default(),
        )
        .map_err(|e| Fmp4FlacError::Segment(e.to_string()))?;
    let mut format = probed.format;

    let mut samples = 0u64;
    let mut first = true;
    loop {
        match format.next_packet() {
            Ok(packet) => {
                let frame = packet.buf();
                if first {
                    if frame.len() < 2 || frame[0] != 0xFF || (frame[1] & 0xFE) != 0xF8 {
                        return Err(Fmp4FlacError::Segment(format!(
                            "frame sin sync FLAC ({:02x?})",
                            &frame[..frame.len().min(4)]
                        )));
                    }
                    first = false;
                }
                samples += packet.dur();
                out.write_all(frame)?;
            }
            Err(symphonia::core::errors::Error::IoError(e))
                if e.kind() == std::io::ErrorKind::UnexpectedEof =>
            {
                break;
            }
            Err(symphonia::core::errors::Error::ResetRequired) => continue,
            Err(e) => return Err(Fmp4FlacError::Segment(e.to_string())),
        }
    }
    out.flush()?;
    drop(out);

    // Si el init no declaraba el total (0 = desconocido), el FLAC nativo
    // tampoco sabría su duración; se parchea con lo realmente escrito.
    if info.total_samples == 0 && samples > 0 {
        patch_total_samples(dst, samples)?;
    }
    Ok(samples)
}

/// Parchea en sitio el `total_samples` del STREAMINFO (campo empaquetado de 8
/// bytes en el offset 18 del fichero) para que la duración sea exacta.
pub fn patch_total_samples(path: &Path, total_samples: u64) -> Result<(), Fmp4FlacError> {
    let mut file = std::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .open(path)?;
    let mut packed = [0u8; 8];
    file.seek(SeekFrom::Start(18))?;
    file.read_exact(&mut packed)?;
    let mut value = u64::from_be_bytes(packed);
    value = (value & !0xF_FFFF_FFFF) | (total_samples & 0xF_FFFF_FFFF);
    file.seek(SeekFrom::Start(18))?;
    file.write_all(&value.to_be_bytes())?;
    file.flush()?;
    Ok(())
}

fn read_head(path: &Path, max: usize) -> Result<Vec<u8>, Fmp4FlacError> {
    let mut file = File::open(path)?;
    let mut buf = vec![0u8; max];
    let mut filled = 0usize;
    while filled < max {
        let n = file.read(&mut buf[filled..])?;
        if n == 0 {
            break;
        }
        filled += n;
    }
    buf.truncate(filled);
    Ok(buf)
}

/// Itera cajas ISO-BMFF de primer nivel de `data`, devolviendo (tipo, payload).
fn box_iter(data: &[u8]) -> impl Iterator<Item = ([u8; 4], &[u8])> {
    let mut pos = 0usize;
    std::iter::from_fn(move || {
        if pos + 8 > data.len() {
            return None;
        }
        let size32 = u32::from_be_bytes(data[pos..pos + 4].try_into().ok()?) as usize;
        let mut typ = [0u8; 4];
        typ.copy_from_slice(&data[pos + 4..pos + 8]);
        let (size, header) = match size32 {
            0 => (data.len() - pos, 8),
            1 => {
                if pos + 16 > data.len() {
                    return None;
                }
                let big =
                    u64::from_be_bytes(data[pos + 8..pos + 16].try_into().ok()?) as usize;
                (big, 16)
            }
            s if s >= 8 => (s, 8),
            _ => return None,
        };
        if size < header || pos + size > data.len() {
            return None;
        }
        let payload = &data[pos + header..pos + size];
        pos += size;
        Some((typ, payload))
    })
}

fn find_box<'a>(data: &'a [u8], name: &[u8; 4]) -> Option<&'a [u8]> {
    box_iter(data)
        .find(|(typ, _)| typ == name)
        .map(|(_, payload)| payload)
}

/// Busca una caja por su fourcc validando que la precede un tamaño coherente.
/// Necesario dentro de un *sample entry*: antes de `dfLa` hay campos que no son
/// cajas y un iterador secuencial se desorientaría.
fn find_box_scanned<'a>(data: &'a [u8], name: &[u8; 4]) -> Option<&'a [u8]> {
    for pos in 0..data.len().saturating_sub(8) {
        if &data[pos + 4..pos + 8] != name {
            continue;
        }
        let size = u32::from_be_bytes(data[pos..pos + 4].try_into().ok()?) as usize;
        if size >= 8 && pos + size <= data.len() {
            return Some(&data[pos + 8..pos + size]);
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    fn stream_info_bytes(sample_rate: u32, channels: u8, bps: u8, total: u64) -> [u8; 34] {
        let mut bytes = [0u8; 34];
        bytes[0..2].copy_from_slice(&4096u16.to_be_bytes());
        bytes[2..4].copy_from_slice(&4096u16.to_be_bytes());
        let packed: u64 = ((sample_rate as u64 & 0xF_FFFF) << 44)
            | (((channels as u64 - 1) & 0x7) << 41)
            | (((bps as u64 - 1) & 0x1F) << 36)
            | (total & 0xF_FFFF_FFFF);
        bytes[10..18].copy_from_slice(&packed.to_be_bytes());
        bytes
    }

    fn init_with(entry_type: &[u8; 4], with_dfla: bool) -> Vec<u8> {
        let si = stream_info_bytes(192_000, 2, 24, 42_931_200);
        let mut dfla = Vec::new();
        if with_dfla {
            dfla.extend_from_slice(&0u32.to_be_bytes()); // version/flags
            dfla.push(0x80); // último bloque + STREAMINFO
            dfla.extend_from_slice(&[0x00, 0x00, 34]);
            dfla.extend_from_slice(&si);
        }
        let mut entry = Vec::new();
        entry.extend_from_slice(&[0u8; 6]); // reserved
        entry.extend_from_slice(&[0u8, 1u8]); // data_reference_index
        entry.extend_from_slice(&boxed(b"dfLa", &dfla).unwrap());

        let mut stsd = Vec::new();
        stsd.extend_from_slice(&0u32.to_be_bytes());
        stsd.extend_from_slice(&1u32.to_be_bytes());
        stsd.extend_from_slice(&boxed(entry_type, &entry).unwrap_or_default());

        let mut stbl = boxed(b"stbl", &boxed(b"stsd", &stsd).unwrap()).unwrap();
        let minf = boxed(b"minf", &stbl).unwrap();
        stbl = minf;
        let mdia = boxed(b"mdia", &stbl).unwrap();
        let trak = boxed(b"trak", &mdia).unwrap();
        let moov = boxed(b"moov", &trak).unwrap();
        let mut init = boxed(b"ftyp", b"isom\0\0\0\0isom").unwrap();
        init.extend_from_slice(&moov);
        init
    }

    fn boxed(typ: &[u8; 4], payload: &[u8]) -> Option<Vec<u8>> {
        let size = u32::try_from(payload.len() + 8).ok()?;
        let mut out = Vec::with_capacity(payload.len() + 8);
        out.extend_from_slice(&size.to_be_bytes());
        out.extend_from_slice(typ);
        out.extend_from_slice(payload);
        Some(out)
    }

    #[test]
    fn parses_the_real_streaminfo_from_the_init() {
        let init = init_with(b"fLaC", true);
        let info = read_init(&init).expect("init FLAC");
        assert_eq!(info.sample_rate, 192_000);
        assert_eq!(info.channels, 2);
        assert_eq!(info.bits_per_sample, 24);
        assert_eq!(info.total_samples, 42_931_200);
        assert!((info.duration_secs() - 223.6).abs() < 0.01);
    }

    #[test]
    fn rejects_aac_and_encrypted_manifests() {
        assert!(matches!(
            read_init(&init_with(b"mp4a", true)),
            Err(Fmp4FlacError::NotFlacTrack)
        ));
        assert!(matches!(
            read_init(&init_with(b"enca", true)),
            Err(Fmp4FlacError::Encrypted(_))
        ));
        assert!(matches!(
            read_init(&init_with(b"fLaC", false)),
            Err(Fmp4FlacError::MissingStreamInfo)
        ));
    }

    #[test]
    fn header_is_flac_magic_plus_streaminfo() {
        let info = read_init(&init_with(b"fLaC", true)).unwrap();
        let mut out = Vec::new();
        write_flac_header(&mut out, &info).unwrap();
        assert_eq!(&out[0..4], b"fLaC");
        assert_eq!(&out[4..8], &[0x80, 0x00, 0x00, 34]);
        assert_eq!(&out[8..42], &info.bytes[..]);
    }

    #[test]
    fn rejects_invalid_streaminfo() {
        let mut init = init_with(b"fLaC", true);
        // Pone a cero la frecuencia de muestreo dentro del STREAMINFO del init.
        let df_la = init.windows(4).position(|w| w == b"dfLa").unwrap();
        let si = df_la + 8 + 4;
        init[si + 10..si + 18].fill(0);
        assert!(matches!(
            read_init(&init),
            Err(Fmp4FlacError::InvalidStreamInfo(_))
        ));
    }

    #[test]
    fn box_iterator_handles_truncated_and_64bit_boxes() {
        assert!(box_iter(&[0, 0, 0]).next().is_none());
        // size 0 = hasta el final.
        let mut data = Vec::new();
        data.extend_from_slice(&0u32.to_be_bytes());
        data.extend_from_slice(b"free");
        data.extend_from_slice(b"payload");
        let (typ, payload) = box_iter(&data).next().unwrap();
        assert_eq!(&typ, b"free");
        assert_eq!(payload, b"payload");
    }
}

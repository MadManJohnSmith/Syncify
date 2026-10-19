//! Verificación offline del conversor con un fMP4-FLAC real y pequeño
//! (generado con ffmpeg: 2 s de seno, 44.1 kHz, 16 bits, 4 fragmentos).

use std::fs::File;
use std::path::{Path, PathBuf};

use symphonia::core::codecs::DecoderOptions;
use symphonia::core::formats::FormatOptions;
use symphonia::core::io::MediaSourceStream;
use symphonia::core::meta::MetadataOptions;
use symphonia::core::probe::Hint;
use syncify_fmp4_flac::{convert_fmp4_to_flac, demux_segment, read_init, write_flac_header};

fn fixture() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/tiny-fmp4-flac.m4a")
}

fn out_path(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join("syncify-fmp4-flac-tests");
    std::fs::create_dir_all(&dir).expect("create test dir");
    dir.join(name)
}

/// Divide el fichero en (init, segmentos moof+mdat).
fn split_segments(data: &[u8]) -> (Vec<u8>, Vec<Vec<u8>>) {
    let mut init: Vec<u8> = Vec::new();
    let mut segments: Vec<Vec<u8>> = Vec::new();
    let mut current: Option<Vec<u8>> = None;
    let mut pos = 0usize;
    while pos + 8 <= data.len() {
        let size = u32::from_be_bytes(data[pos..pos + 4].try_into().unwrap()) as usize;
        if size < 8 || pos + size > data.len() {
            break;
        }
        let typ = &data[pos + 4..pos + 8];
        let end = pos + size;
        match typ {
            b"moof" => current = Some(data[pos..end].to_vec()),
            b"mdat" => {
                if let Some(mut seg) = current.take() {
                    seg.extend_from_slice(&data[pos..end]);
                    segments.push(seg);
                }
            }
            _ => {
                if segments.is_empty() && current.is_none() {
                    init.extend_from_slice(&data[pos..end]);
                }
            }
        }
        pos = end;
    }
    (init, segments)
}

fn decode_flac_samples(path: &Path) -> u64 {
    let file = File::open(path).expect("open flac");
    let mss = MediaSourceStream::new(Box::new(file), Default::default());
    let probed = symphonia::default::get_probe()
        .format(
            &Hint::new(),
            mss,
            &FormatOptions::default(),
            &MetadataOptions::default(),
        )
        .expect("probe flac");
    let mut format = probed.format;
    let params = format
        .default_track()
        .expect("default track")
        .codec_params
        .clone();
    let mut decoder = symphonia::default::get_codecs()
        .make(&params, &DecoderOptions::default())
        .expect("flac decoder");
    let mut total = 0u64;
    while let Ok(packet) = format.next_packet() {
        match decoder.decode(&packet) {
            Ok(audio) => total += audio.frames() as u64,
            Err(symphonia::core::errors::Error::DecodeError(_)) => continue,
            Err(_) => break,
        }
    }
    total
}

#[test]
fn segments_convert_to_a_native_flac_that_decodes_identically() {
    let data = std::fs::read(fixture()).expect("read fixture");
    let (init, segments) = split_segments(&data);
    assert_eq!(segments.len(), 4, "el fixture debe traer 4 fragmentos");

    let info = read_init(&init).expect("init FLAC");
    assert_eq!(info.sample_rate, 44_100);
    assert_eq!(info.channels, 2);
    assert_eq!(info.bits_per_sample, 16);
    assert_eq!(info.total_samples, 88_200);

    let by_segments = out_path("by-segments.flac");
    let mut out: Vec<u8> = Vec::new();
    write_flac_header(&mut out, &info).expect("header");
    let mut samples = 0u64;
    for segment in &segments {
        samples += demux_segment(&init, segment, &mut |frame| {
            out.extend_from_slice(frame);
            Ok(())
        })
        .expect("demux segment");
    }
    std::fs::write(&by_segments, &out).expect("write flac");
    assert_eq!(samples, 88_200);
    assert_eq!(decode_flac_samples(&by_segments), 88_200);

    let converted = out_path("converted.flac");
    let converted_samples =
        convert_fmp4_to_flac(&fixture(), &converted).expect("convert whole file");
    assert_eq!(converted_samples, 88_200);
    assert_eq!(decode_flac_samples(&converted), 88_200);

    // Ambas rutas (por segmento y fichero completo) deben producir el mismo
    // FLAC: misma cabecera y mismos frames.
    assert_eq!(
        std::fs::read(&by_segments).unwrap(),
        std::fs::read(&converted).unwrap()
    );
}

#[test]
fn rejects_non_flac_and_encrypted_inits() {
    let data = std::fs::read(fixture()).expect("read fixture");
    let (mut init, _) = split_segments(&data);

    let flac_entry = init
        .windows(4)
        .position(|w| w == b"fLaC")
        .expect("fLaC entry");
    let mut aac = init.clone();
    aac[flac_entry..flac_entry + 4].copy_from_slice(b"mp4a");
    assert!(matches!(
        read_init(&aac),
        Err(syncify_fmp4_flac::Fmp4FlacError::NotFlacTrack)
    ));

    init[flac_entry..flac_entry + 4].copy_from_slice(b"enca");
    assert!(matches!(
        read_init(&init),
        Err(syncify_fmp4_flac::Fmp4FlacError::Encrypted(_))
    ));
}
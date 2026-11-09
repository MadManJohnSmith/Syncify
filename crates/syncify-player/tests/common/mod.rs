//! Helpers compartidos por los tests del motor: generación de WAV PCM.
//!
//! Cada binario de test declara `mod common;` y usa el subconjunto que
//! necesita; lo que sobra se compila y no se usa (por eso el `allow`).
#![allow(dead_code)]

use std::fs::File;
use std::io::Write;
use std::path::Path;

/// Escribe un WAV PCM mono de 16 bits con una senoidal y devuelve la duración
/// en segundos que declarará el contenedor.
pub fn write_sine_wav(path: &Path, sample_rate: u32, seconds: f64) -> f64 {
    let frames = (sample_rate as f64 * seconds).round() as u32;
    let mut samples = Vec::with_capacity(frames as usize);
    for i in 0..frames {
        let value = (2.0 * std::f64::consts::PI * 440.0 * i as f64 / sample_rate as f64).sin();
        samples.push((value * 0.3 * i16::MAX as f64) as i16);
    }
    write_wav(path, sample_rate, &samples);
    frames as f64 / sample_rate as f64
}

/// Escribe un WAV con cabecera válida y **cero muestras**: el contenedor
/// declara una duración nula, que symphonia reporta como `None` (rama de
/// duración desconocida del motor).
pub fn write_empty_wav(path: &Path, sample_rate: u32) {
    write_wav(path, sample_rate, &[]);
}

fn write_wav(path: &Path, sample_rate: u32, samples: &[i16]) {
    let data_len = (samples.len() * 2) as u32;
    let mut file = File::create(path).expect("create wav");
    file.write_all(b"RIFF").unwrap();
    file.write_all(&(36 + data_len).to_le_bytes()).unwrap();
    file.write_all(b"WAVE").unwrap();
    // fmt: PCM, mono, 16 bits.
    file.write_all(b"fmt ").unwrap();
    file.write_all(&16u32.to_le_bytes()).unwrap();
    file.write_all(&1u16.to_le_bytes()).unwrap();
    file.write_all(&1u16.to_le_bytes()).unwrap();
    file.write_all(&sample_rate.to_le_bytes()).unwrap();
    file.write_all(&(sample_rate * 2).to_le_bytes()).unwrap();
    file.write_all(&2u16.to_le_bytes()).unwrap();
    file.write_all(&16u16.to_le_bytes()).unwrap();
    // data.
    file.write_all(b"data").unwrap();
    file.write_all(&data_len.to_le_bytes()).unwrap();
    for sample in samples {
        file.write_all(&sample.to_le_bytes()).unwrap();
    }
    file.flush().unwrap();
}

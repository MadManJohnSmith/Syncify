//! Caracterización del motor de EQ paramétrico con audio sintético (P09a,
//! #427): respuesta en frecuencia, plano bit-idéntico y estabilidad con
//! ganancias extremas, sin dispositivo de audio, sin dormir y sin red.

use std::num::NonZero;
use std::time::Duration;

use rodio::buffer::SamplesBuffer;
use rodio::Source;
use syncify_player::eq::{BandKind, EqBand, EqSettings, EqSource, MAX_EQ_BANDS, MAX_EQ_GAIN_DB};

const SAMPLE_RATE: u32 = 48_000;

fn channels(count: u16) -> NonZero<u16> {
    NonZero::new(count).expect("channel count")
}

fn rate(hz: u32) -> NonZero<u32> {
    NonZero::new(hz).expect("sample rate")
}

fn buffer(samples: Vec<f32>) -> SamplesBuffer {
    SamplesBuffer::new(channels(1), rate(SAMPLE_RATE), samples)
}

/// Senoidal sintética de `frequency_hz` con la amplitud pedida.
fn sine(frequency_hz: f32, amplitude: f32, frames: usize) -> Vec<f32> {
    (0..frames)
        .map(|index| {
            let phase =
                2.0 * std::f32::consts::PI * frequency_hz * index as f32 / SAMPLE_RATE as f32;
            amplitude * phase.sin()
        })
        .collect()
}

fn rms(samples: &[f32]) -> f32 {
    let mean_square =
        samples.iter().map(|sample| sample * sample).sum::<f32>() / samples.len() as f32;
    mean_square.sqrt()
}

fn peak(samples: &[f32]) -> f32 {
    samples
        .iter()
        .fold(0.0_f32, |max, sample| max.max(sample.abs()))
}

fn response(settings: &EqSettings, frequency_hz: f32) -> f32 {
    settings.response_db(frequency_hz, SAMPLE_RATE)
}

#[test]
fn flat_is_bit_identical() {
    let mut input = vec![0.0, -0.0, 0.5, -0.5, 1.0, -1.0, 1.5, -1.5];
    for index in 0..4_000 {
        input.push(((index as f32) * 0.017).sin() * 1.25);
    }

    let settings = EqSettings::flat();
    assert!(settings.is_empty());
    assert!(settings.is_flat());

    let source = EqSource::new(buffer(input.clone()), settings);
    assert!(source.is_bypassing());
    let output: Vec<f32> = source.collect();
    assert_eq!(output.len(), input.len());
    for (index, (after, before)) in output.iter().zip(&input).enumerate() {
        assert_eq!(after.to_bits(), before.to_bits(), "muestra {index}");
    }
}

#[test]
fn zero_db_peak_bands_are_flat_and_bit_identical() {
    let settings = EqSettings::from_bands(
        (0..MAX_EQ_BANDS).map(|index| EqBand::peak(31.25 * 2_f32.powi(index as i32), 2.0, 0.0)),
    );
    assert_eq!(settings.len(), MAX_EQ_BANDS);
    assert!(settings.is_flat());

    let input: Vec<f32> = (0..2_000)
        .map(|index| ((index as f32) * 0.031).sin() * 0.9)
        .collect();
    let source = EqSource::new(buffer(input.clone()), settings);
    assert!(source.is_bypassing());
    let output: Vec<f32> = source.collect();
    for (after, before) in output.iter().zip(&input) {
        assert_eq!(after.to_bits(), before.to_bits());
    }
}

#[test]
fn peak_band_boosts_its_center_in_the_audio() {
    let settings = EqSettings::from_bands([EqBand::peak(1_000.0, 1.0, 6.0)]);
    assert!(!settings.is_flat());
    assert!((response(&settings, 1_000.0) - 6.0).abs() < 1e-3);
    assert!(response(&settings, 8_000.0).abs() < 0.5);

    let input = sine(1_000.0, 0.25, 48_000);
    let source = EqSource::new(buffer(input.clone()), settings);
    assert!(!source.is_bypassing());
    let output: Vec<f32> = source.collect();

    // El primer segundo descarta el transitorio; el segundo es régimen
    // permanente y mide la ganancia de la banda.
    let ratio = rms(&output[24_000..]) / rms(&input[24_000..]);
    assert!((ratio - 2.0).abs() < 0.01, "ratio {ratio}");
    assert!((peak(&output[24_000..]) - 0.5).abs() < 0.01);
}

#[test]
fn editing_band_fields_moves_the_response() {
    let band = EqBand::peak(1_000.0, 1.0, 6.0);
    let mut settings = EqSettings::flat().with_band(band);
    assert_eq!(settings.band(0), Some(band));
    assert!((response(&settings, 1_000.0) - 6.0).abs() < 1e-3);

    assert!(settings.set_band(0, EqBand::peak(4_000.0, 1.0, 6.0)));
    assert!(response(&settings, 1_000.0).abs() < 0.5);
    assert!((response(&settings, 4_000.0) - 6.0).abs() < 1e-3);

    assert!(settings.set_band(0, EqBand::peak(4_000.0, 1.0, -9.0)));
    assert!((response(&settings, 4_000.0) + 9.0).abs() < 1e-3);

    // Una Q estrecha reduce la influencia fuera del centro.
    let wide = EqSettings::from_bands([EqBand::peak(1_000.0, 0.5, 12.0)]);
    let narrow = EqSettings::from_bands([EqBand::peak(1_000.0, 8.0, 12.0)]);
    assert!(response(&narrow, 1_500.0) < response(&wide, 1_500.0));

    assert!(!settings.set_band(settings.len(), EqBand::peak(100.0, 1.0, 1.0)));
    assert_eq!(settings.band(settings.len()), None);
}

#[test]
fn adding_and_removing_bands_changes_the_cascade() {
    let first = EqBand::peak(1_000.0, 1.0, 6.0);
    let second = EqBand::peak(1_000.0, 1.0, 6.0);
    let mut settings = EqSettings::from_bands([first]);
    let one = response(&settings, 1_000.0);
    assert!(settings.push_band(second));
    let two = response(&settings, 1_000.0);
    assert!((two - one - 6.0).abs() < 1e-3, "una {one}, dos {two}");
    assert!((two - 12.0).abs() < 1e-3);

    assert_eq!(settings.remove_band(1), Some(second));
    assert!((response(&settings, 1_000.0) - 6.0).abs() < 1e-3);

    assert_eq!(settings.remove_band(0), Some(first));
    assert!(settings.is_flat());
    assert!(settings.remove_band(0).is_none());

    // Un ajuste vaciado vuelve a ser pass-through exacto.
    let input = sine(1_000.0, 0.5, 1_024);
    let output: Vec<f32> = EqSource::new(buffer(input.clone()), settings).collect();
    assert_eq!(output, input);

    let mut cleared = EqSettings::from_bands([EqBand::peak(120.0, 2.0, 9.0)]);
    cleared.clear();
    assert!(cleared.is_empty());
    assert!(cleared.is_flat());
}

#[test]
fn the_cascade_is_capped_at_ten_bands() {
    let mut settings = EqSettings::from_bands(
        (0..MAX_EQ_BANDS + 3).map(|index| EqBand::peak(40.0 * (index as f32 + 1.0), 1.0, 3.0)),
    );
    assert_eq!(settings.len(), MAX_EQ_BANDS);

    assert!(!settings.push_band(EqBand::peak(100.0, 1.0, 1.0)));
    assert_eq!(settings.len(), MAX_EQ_BANDS);
    assert!(settings.set_band(MAX_EQ_BANDS - 1, EqBand::peak(12_000.0, 1.0, -3.0)));
    assert!(!settings.set_band(MAX_EQ_BANDS, EqBand::peak(12_000.0, 1.0, -3.0)));
}

#[test]
fn extreme_gains_and_q_stay_stable() {
    // Diez bandas alternando +24 y -24 dB con Q=10, de 20 Hz a 10 kHz.
    let settings = EqSettings::from_bands((0..MAX_EQ_BANDS).map(|index| {
        let gain_db = if index % 2 == 0 {
            MAX_EQ_GAIN_DB
        } else {
            -MAX_EQ_GAIN_DB
        };
        EqBand::peak(20.0 * 2_f32.powi(index as i32), 10.0, gain_db)
    }));

    let input = sine(1_000.0, 1.0, 48_000);
    let output: Vec<f32> = EqSource::new(buffer(input), settings).collect();
    assert!(output.iter().all(|sample| sample.is_finite()));
    assert!(peak(&output) < 1.0e6, "pico {}", peak(&output));

    // La cola de un impulso decae: ningún polo quedó sobre el círculo unidad.
    let mut impulse = vec![0.0; 10 * SAMPLE_RATE as usize];
    impulse[0] = 1.0;
    let tail: Vec<f32> = EqSource::new(buffer(impulse), settings).collect();
    assert!(tail.iter().all(|sample| sample.is_finite()));
    let tail_peak = peak(&tail[tail.len() - 1_000..]);
    assert!(tail_peak < 1e-4, "cola sin decaer: {tail_peak}");

    // Los valores fuera de rango se saturan: la ganancia efectiva no pasa de
    // MAX_EQ_GAIN_DB y una frecuencia absurda cae al mínimo estable.
    let clamped = EqSettings::from_bands([EqBand::peak(1.0, 1_000.0, 10_000.0)]);
    assert!((response(&clamped, 10.0) - MAX_EQ_GAIN_DB).abs() < 1e-3);

    // Ningún tipo pierde el diseño ni explota en los bordes del rango.
    for kind in [
        BandKind::Peak,
        BandKind::LowShelf,
        BandKind::HighShelf,
        BandKind::LowPass,
        BandKind::HighPass,
    ] {
        for frequency_hz in [10.0, 1_000.0, 20_000.0] {
            let settings = EqSettings::from_bands([EqBand {
                kind,
                frequency_hz,
                q: 10.0,
                gain_db: MAX_EQ_GAIN_DB,
            }]);
            let source = EqSource::new(buffer(sine(frequency_hz, 1.0, 4_800)), settings);
            assert!(
                !source.is_bypassing(),
                "{kind:?} a {frequency_hz} Hz no se pudo diseñar"
            );
            let output: Vec<f32> = source.collect();
            assert!(output.iter().all(|sample| sample.is_finite()));
            assert!(
                peak(&output) < 1.0e4,
                "{kind:?} a {frequency_hz} Hz: pico {}",
                peak(&output)
            );
        }
    }
}

#[test]
fn corrupt_bands_are_ignored_and_never_reach_the_audio() {
    let settings = EqSettings::from_bands([
        EqBand::peak(f32::NAN, 1.0, 6.0),
        EqBand::peak(1_000.0, 0.0, 6.0),
        EqBand::peak(1_000.0, 1.0, f32::INFINITY),
        EqBand::low_pass(f32::NAN, 1.0),
    ]);
    assert_eq!(settings.len(), 4);
    assert!(settings.is_flat());

    let input: Vec<f32> = (0..1_024)
        .map(|index| ((index as f32) * 0.05).sin())
        .collect();
    let source = EqSource::new(buffer(input.clone()), settings);
    assert!(source.is_bypassing());
    let output: Vec<f32> = source.collect();
    for (after, before) in output.iter().zip(&input) {
        assert_eq!(after.to_bits(), before.to_bits());
    }
}

#[test]
fn shelf_and_pass_bands_shape_the_response_by_type() {
    let low_shelf = EqSettings::from_bands([EqBand::low_shelf(200.0, 1.0, 6.0)]);
    assert!(response(&low_shelf, 20.0) > 5.5);
    assert!(response(&low_shelf, 10_000.0).abs() < 0.2);

    let high_shelf = EqSettings::from_bands([EqBand::high_shelf(4_000.0, 1.0, 6.0)]);
    assert!(response(&high_shelf, 20_000.0) > 5.5);
    assert!(response(&high_shelf, 100.0).abs() < 0.2);

    let low_pass = EqSettings::from_bands([EqBand::low_pass(1_000.0, 0.707)]);
    assert!(response(&low_pass, 100.0) > -0.5);
    assert!(response(&low_pass, 10_000.0) < -30.0);

    let high_pass = EqSettings::from_bands([EqBand::high_pass(1_000.0, 0.707)]);
    assert!(response(&high_pass, 10_000.0) > -0.5);
    assert!(response(&high_pass, 100.0) < -30.0);

    // El tipo forma parte del modelo: misma frecuencia, Q y ganancia, distinta
    // respuesta según el tipo.
    let peak = EqSettings::from_bands([EqBand::peak(1_000.0, 1.0, 6.0)]);
    let shelf = EqSettings::from_bands([EqBand {
        kind: BandKind::HighShelf,
        ..EqBand::peak(1_000.0, 1.0, 6.0)
    }]);
    assert!(response(&shelf, 20_000.0) > response(&peak, 20_000.0));

    // En paso bajo y paso alto la ganancia de la banda no participa.
    let low_pass_with_gain = EqSettings::from_bands([EqBand {
        kind: BandKind::LowPass,
        frequency_hz: 1_000.0,
        q: 0.707,
        gain_db: 12.0,
    }]);
    assert_eq!(
        low_pass.response_db(100.0, SAMPLE_RATE),
        low_pass_with_gain.response_db(100.0, SAMPLE_RATE)
    );
}

#[test]
fn channels_keep_independent_filter_state() {
    let settings = EqSettings::from_bands([EqBand::peak(1_000.0, 1.0, 12.0)]);
    let left = sine(1_000.0, 0.5, 4_800);
    let mut data = Vec::with_capacity(left.len() * 2);
    for sample in left {
        data.push(sample);
        data.push(0.0);
    }

    let source = EqSource::new(
        SamplesBuffer::new(channels(2), rate(SAMPLE_RATE), data),
        settings,
    );
    let output: Vec<f32> = source.collect();
    let right_peak = output
        .iter()
        .skip(1)
        .step_by(2)
        .fold(0.0_f32, |max, sample| max.max(sample.abs()));
    assert!(
        right_peak < 1e-6,
        "el canal derecho no debe heredar la señal del izquierdo: {right_peak}"
    );
}

#[test]
fn response_db_handles_the_edge_cases() {
    let settings = EqSettings::from_bands([EqBand::peak(1_000.0, 1.0, 6.0)]);
    assert_eq!(settings.response_db(1_000.0, 0), 0.0);
    assert_eq!(settings.response_db(f32::NAN, SAMPLE_RATE), 0.0);
    assert_eq!(
        settings.response_db(30_000.0, SAMPLE_RATE),
        settings.response_db(24_000.0, SAMPLE_RATE)
    );
    assert_eq!(EqSettings::flat().response_db(1_000.0, SAMPLE_RATE), 0.0);
}

#[test]
fn source_properties_and_seek_are_forwarded_with_clean_state() {
    let frames = 4_800usize;
    let mut data = Vec::with_capacity(frames * 2);
    for index in 0..frames {
        data.push((index as f32 * 0.02).sin() * 0.5);
        data.push((index as f32 * 0.02).cos() * 0.5);
    }

    let inner = SamplesBuffer::new(channels(2), rate(SAMPLE_RATE), data.clone());
    let expected_duration = inner.total_duration();
    let expected_span = inner.current_span_len();
    let expected_hint = inner.size_hint();

    let settings = EqSettings::from_bands([EqBand::peak(1_000.0, 2.0, 6.0)]);
    let mut source = EqSource::new(inner, settings);
    assert_eq!(source.channels().get(), 2);
    assert_eq!(source.sample_rate().get(), SAMPLE_RATE);
    assert_eq!(source.total_duration(), expected_duration);
    assert_eq!(source.current_span_len(), expected_span);
    assert_eq!(source.size_hint(), expected_hint);
    assert_eq!(source.settings(), &settings);

    // Calentar el estado y buscar: el tramo posterior debe coincidir con un
    // adaptador nuevo posicionado en el mismo punto.
    let _ = source.by_ref().take(1_000).count();
    source
        .try_seek(Duration::from_millis(50))
        .expect("seek a través del EQ");
    let after_seek: Vec<f32> = source.by_ref().take(512).collect();

    let mut reference = EqSource::new(
        SamplesBuffer::new(channels(2), rate(SAMPLE_RATE), data),
        settings,
    );
    reference
        .try_seek(Duration::from_millis(50))
        .expect("seek de referencia");
    let fresh: Vec<f32> = reference.collect();
    assert_eq!(after_seek, fresh[..512]);
}

//! Caracterización del motor de crossfade con fuentes sintéticas (P06a, #407):
//! curva, limiter y comportamiento gapless, sin dispositivo de audio, sin
//! dormir y sin red.

use std::f32::consts::{FRAC_1_SQRT_2, FRAC_PI_2};
use std::num::NonZero;
use std::time::Duration;

use rodio::buffer::SamplesBuffer;
use rodio::source::SeekError;
use rodio::{ChannelCount, Sample, SampleRate, Source};
use syncify_player::crossfade::{
    mix_gains, CrossfadeConfig, CrossfadeError, CrossfadeSource, PeakLimiter, Transition,
    LIMITER_CEILING,
};

const RATE: u32 = 1_000;

fn nz_u16(value: u16) -> ChannelCount {
    NonZero::new(value).unwrap()
}

fn nz_u32(value: u32) -> SampleRate {
    NonZero::new(value).unwrap()
}

/// Fuente sintética mono a [`RATE`] Hz.
fn mono(samples: Vec<f32>) -> SamplesBuffer {
    SamplesBuffer::new(nz_u16(1), nz_u32(RATE), samples)
}

fn collect(source: impl Source) -> Vec<Sample> {
    source.collect()
}

/// Coseno de 10 Hz: pico a unidad en la primera muestra, útil para forzar el
/// limiter desde el primer frame de la mezcla.
fn cosine(frames: usize) -> Vec<f32> {
    (0..frames)
        .map(|i| (2.0 * FRAC_PI_2 * 10.0 * i as f32 / RATE as f32).cos())
        .collect()
}

#[test]
fn curve_is_equal_power_with_unity_endpoints() {
    assert_eq!(mix_gains(0.0), (1.0, 0.0));
    let (out, inc) = mix_gains(1.0);
    assert!(out.abs() < 1e-7);
    assert!((inc - 1.0).abs() < 1e-7);

    let (out, inc) = mix_gains(0.5);
    assert!((out - FRAC_1_SQRT_2).abs() < 1e-6);
    assert!((inc - FRAC_1_SQRT_2).abs() < 1e-6);

    // Potencia constante en todo el recorrido: cos² + sin² = 1, con ambas
    // ganancias en [0, 1] y la entrante creciente.
    let mut previous = 0.0;
    for step in 0..=1_000 {
        let progress = step as f32 / 1_000.0;
        let (out, inc) = mix_gains(progress);
        assert!((out * out + inc * inc - 1.0).abs() < 1e-5, "p={progress}");
        assert!((0.0..=1.0).contains(&out));
        assert!((0.0..=1.0).contains(&inc));
        assert!(inc >= previous, "la entrante debe crecer en p={progress}");
        previous = inc;
    }
}

#[test]
fn curve_clamps_progress_outside_the_unit_interval() {
    assert_eq!(mix_gains(-1.0), mix_gains(0.0));
    assert_eq!(mix_gains(2.0), mix_gains(1.0));
}

#[test]
fn zero_seconds_is_an_exact_gapless_concatenation() {
    // Amplitudes por encima del techo del limiter: con 0 s no debe tocar nada.
    let outgoing = mono(vec![1.5, -1.5, 0.25]);
    let incoming = mono(vec![2.0, -2.0]);
    let joined =
        CrossfadeSource::new(outgoing, incoming, CrossfadeConfig::OFF).expect("formatos iguales");

    assert_eq!(joined.channels(), nz_u16(1));
    assert_eq!(joined.sample_rate(), nz_u32(RATE));
    assert_eq!(joined.fade_frames(), 0);
    // 3 muestras a 1000 Hz + 2 muestras a 1000 Hz.
    assert_eq!(joined.total_duration(), Some(Duration::from_millis(5)));
    assert_eq!(
        collect(joined),
        vec![1.5, -1.5, 0.25, 2.0, -2.0],
        "0 s debe concatenar sin atenuar ni limitar"
    );
}

#[test]
fn blend_follows_the_curve_and_then_hands_over_to_the_incoming_track() {
    let fade_frames = 20; // 20 ms a 1000 Hz.
    let config = CrossfadeConfig::from_secs(0.020);
    assert_eq!(config.duration(), Duration::from_millis(20));
    let outgoing = mono(vec![0.8; fade_frames]);
    let incoming = mono(vec![0.2; 100]);
    let joined = CrossfadeSource::new(outgoing, incoming, config).expect("formatos iguales");

    assert_eq!(joined.fade_frames(), fade_frames as u64);
    assert_eq!(joined.total_duration(), Some(Duration::from_millis(100)));
    let output = collect(joined);
    assert_eq!(output.len(), 100);

    for (frame, sample) in output.iter().enumerate().take(fade_frames) {
        let progress = (frame as f32 + 0.5) / fade_frames as f32;
        let (out_gain, in_gain) = mix_gains(progress);
        let expected = 0.8 * out_gain + 0.2 * in_gain;
        assert!(
            (sample - expected).abs() < 1e-6,
            "frame {frame}: {sample} != {expected}"
        );
    }
    // Tras la mezcla, la entrante pasa intacta.
    assert!(output[fade_frames..].iter().all(|sample| *sample == 0.2));
    // La mezcla arranca pegada a la saliente y termina pegada a la entrante.
    assert!((output[0] - 0.8).abs() < 0.01);
    assert!((output[fade_frames - 1] - 0.2).abs() < 0.05);
}

#[test]
fn blend_peak_never_exceeds_the_limiter_ceiling() {
    // Dos pistas a escala completa en fase: la suma cruda llega a 1.414 y el
    // limiter debe dejarla en el techo sin aplastarla.
    let config = CrossfadeConfig::from_secs(0.1);
    let fade_frames = (RATE as f64 * config.duration().as_secs_f64()).round() as usize;
    let outgoing = mono(cosine(100));
    let incoming = mono(cosine(1_000));
    let joined = CrossfadeSource::new(outgoing, incoming, config).expect("formatos iguales");

    let output = collect(joined);
    let peak = output[..fade_frames]
        .iter()
        .fold(0.0_f32, |peak, s| peak.max(s.abs()));
    assert!(
        peak <= LIMITER_CEILING + 1e-6,
        "el pico {peak} supera el techo {LIMITER_CEILING}"
    );
    assert!(
        peak > 0.9,
        "el limiter no debe aplastar la señal: pico {peak}"
    );
    // Fuera de la mezcla, la entrante pasa intacta (el limiter es del cruce,
    // no de la pista).
    assert!(output[fade_frames..]
        .iter()
        .any(|sample| (sample - 1.0).abs() < 1e-6));
}

#[test]
fn limiter_couples_channels_and_releases_toward_unity() {
    let mut limiter = PeakLimiter::new(RATE);
    let mut frame = [2.0, 0.5];
    limiter.process_frame(&mut frame);

    // La misma ganancia reduce ambos canales: la proporción (imagen estéreo)
    // se conserva y el pico queda exactamente en el techo.
    assert!((frame[0] - LIMITER_CEILING).abs() < 1e-6);
    assert!((frame[1] - 0.5 * LIMITER_CEILING / 2.0).abs() < 1e-6);
    assert!((limiter.gain() - LIMITER_CEILING / 2.0).abs() < 1e-6);

    // Un segundo de frames suaves a 1000 Hz lleva el release a ~1.0.
    for _ in 0..RATE {
        let mut quiet = [0.1, 0.1];
        limiter.process_frame(&mut quiet);
    }
    assert!(limiter.gain() > 0.999, "ganancia {}", limiter.gain());
    assert!((limiter.gain() - 1.0).abs() < 1e-3);
}

#[test]
fn limiter_holds_the_ceiling_on_sustained_peaks() {
    let mut limiter = PeakLimiter::new(RATE);
    // Un tono sostenido por encima del techo no puede colarse por el release:
    // cada frame se limita con la ganancia que exige su propio pico.
    for _ in 0..RATE {
        let mut frame = [1.5, 0.75];
        limiter.process_frame(&mut frame);
        assert!(frame[0].abs() <= LIMITER_CEILING + 1e-6);
        assert!(frame[1].abs() <= LIMITER_CEILING + 1e-6);
    }
    assert!((limiter.gain() - LIMITER_CEILING / 1.5).abs() < 1e-6);
}

#[test]
fn transport_cuts_never_start_a_crossfade() {
    let enabled = CrossfadeConfig::from_secs(3.0);
    assert!(enabled.applies_to(Transition::NaturalEnd));
    assert!(!enabled.applies_to(Transition::Seek));
    assert!(!enabled.applies_to(Transition::Pause));
    assert!(!enabled.applies_to(Transition::Stop));

    assert!(!CrossfadeConfig::OFF.applies_to(Transition::NaturalEnd));
}

#[test]
fn config_saturates_to_the_twelve_second_setting() {
    assert_eq!(CrossfadeConfig::from_secs(-1.0), CrossfadeConfig::OFF);
    assert_eq!(CrossfadeConfig::from_secs(f32::NAN), CrossfadeConfig::OFF);
    assert_eq!(
        CrossfadeConfig::from_secs(f32::INFINITY),
        CrossfadeConfig::OFF
    );
    assert_eq!(
        CrossfadeConfig::from_secs(99.0),
        CrossfadeConfig::MAX,
        "el ajuste no puede pasar de 12 s"
    );
    assert_eq!(
        CrossfadeConfig::new(Duration::from_secs(13)),
        CrossfadeConfig::MAX
    );
    assert!(CrossfadeConfig::MAX.is_enabled());
}

#[test]
fn mismatched_formats_are_rejected_instead_of_mixed() {
    let outgoing = mono(vec![0.0; 4]);
    let stereo = SamplesBuffer::new(nz_u16(2), nz_u32(RATE), vec![0.0; 4]);
    let error = CrossfadeSource::new(outgoing, stereo, CrossfadeConfig::from_secs(1.0))
        .expect_err("mono y estéreo no se pueden mezclar");
    assert_eq!(
        error,
        CrossfadeError::FormatMismatch {
            outgoing_channels: 1,
            outgoing_rate: RATE,
            incoming_channels: 2,
            incoming_rate: RATE,
        }
    );

    let outgoing = mono(vec![0.0; 4]);
    let other_rate = SamplesBuffer::new(nz_u16(1), nz_u32(48_000), vec![0.0; 4]);
    let error = CrossfadeSource::new(outgoing, other_rate, CrossfadeConfig::from_secs(1.0))
        .expect_err("frecuencias distintas no se pueden mezclar");
    assert!(matches!(
        error,
        CrossfadeError::FormatMismatch {
            incoming_rate: 48_000,
            ..
        }
    ));
}

#[test]
fn seek_is_not_supported_while_joining_tracks() {
    let mut joined = CrossfadeSource::new(
        mono(vec![0.0; 4]),
        mono(vec![0.0; 4]),
        CrossfadeConfig::from_secs(1.0),
    )
    .expect("formatos iguales");
    assert!(matches!(
        joined.try_seek(Duration::from_millis(1)),
        Err(SeekError::NotSupported { .. })
    ));
}

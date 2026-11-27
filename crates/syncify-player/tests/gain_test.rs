//! Motor de ganancia y limiter con audio sintético (ADR-0011): sin dispositivo
//! de audio, sin red y sin dormir. Cada test consume un `SamplesBuffer`
//! construido en memoria y mide nivel y pico de la salida.

use std::num::NonZero;
use std::time::Duration;

use rodio::buffer::SamplesBuffer;
use rodio::Source;
use syncify_player::gain::{
    GainOrigin, GainSettings, GainSource, ReplayGain, REPLAYGAIN_REFERENCE_LUFS,
};

const SAMPLE_RATE: u32 = 44_100;

fn channels(count: u16) -> NonZero<u16> {
    NonZero::new(count).expect("channel count")
}

fn rate(hz: u32) -> NonZero<u32> {
    NonZero::new(hz).expect("sample rate")
}

fn buffer(samples: Vec<f32>) -> SamplesBuffer {
    SamplesBuffer::new(channels(1), rate(SAMPLE_RATE), samples)
}

/// Senoidal sintética de 440 Hz con la amplitud pedida (0.0..=1.0).
fn sine(amplitude: f32, frames: usize) -> Vec<f32> {
    (0..frames)
        .map(|i| {
            let phase = 2.0 * std::f32::consts::PI * 440.0 * i as f32 / SAMPLE_RATE as f32;
            amplitude * phase.sin()
        })
        .collect()
}

fn peak(samples: &[f32]) -> f32 {
    samples
        .iter()
        .fold(0.0_f32, |max, sample| max.max(sample.abs()))
}

fn rms(samples: &[f32]) -> f32 {
    let mean_square =
        samples.iter().map(|sample| sample * sample).sum::<f32>() / samples.len() as f32;
    mean_square.sqrt()
}

#[test]
fn off_mode_ignores_tags_and_keeps_the_level() {
    let input = sine(0.6, 4_800);
    let tags = ReplayGain {
        track_gain_db: Some(-12.0),
        track_peak: Some(0.9),
        album_gain_db: Some(-3.0),
        album_peak: Some(0.99),
    };
    let settings = GainSettings::off();
    let plan = settings.plan(&tags);
    assert_eq!(plan.origin, GainOrigin::Off);
    assert_eq!(plan.gain, 1.0);
    assert_eq!(plan.gain_db, 0.0);
    assert!(!plan.limited);

    let source = GainSource::with_settings(buffer(input.clone()), settings, &tags);
    let meter = source.meter();
    let output: Vec<f32> = source.collect();
    assert_eq!(output, input);
    assert_eq!(meter.peak(), peak(&input));
    assert!(!meter.limited());
}

#[test]
fn track_mode_applies_the_track_gain() {
    let input = sine(0.8, 4_800);
    let tags = ReplayGain {
        track_gain_db: Some(-6.020_6),
        track_peak: Some(0.8),
        ..ReplayGain::default()
    };
    let plan = GainSettings::track().plan(&tags);
    assert_eq!(plan.origin, GainOrigin::Track);
    assert!((plan.gain - 0.5).abs() < 1e-3, "gain {}", plan.gain);
    assert!(
        !plan.limited,
        "atenuar una pista nunca debería disparar el limiter"
    );

    let output: Vec<f32> =
        GainSource::with_settings(buffer(input.clone()), GainSettings::track(), &tags).collect();
    let ratio = rms(&output) / rms(&input);
    assert!((ratio - 0.5).abs() < 1e-3, "ratio {ratio}");
    assert!((peak(&output) - 0.4).abs() < 1e-3);
}

#[test]
fn album_mode_applies_the_album_gain() {
    let input = sine(0.8, 4_800);
    let tags = ReplayGain {
        track_gain_db: Some(0.0),
        track_peak: Some(0.8),
        album_gain_db: Some(-12.041_2),
        album_peak: Some(0.8),
    };
    let plan = GainSettings::album().plan(&tags);
    assert_eq!(plan.origin, GainOrigin::Album);
    assert!((plan.gain - 0.25).abs() < 1e-3, "gain {}", plan.gain);

    let output: Vec<f32> =
        GainSource::with_settings(buffer(input), GainSettings::album(), &tags).collect();
    assert!((peak(&output) - 0.2).abs() < 1e-3);
}

#[test]
fn album_mode_falls_back_to_the_track_gain() {
    let tags = ReplayGain {
        track_gain_db: Some(-6.020_6),
        track_peak: Some(0.8),
        ..ReplayGain::default()
    };
    let plan = GainSettings::album().plan(&tags);
    assert_eq!(plan.origin, GainOrigin::TrackFallback);
    assert!((plan.gain - 0.5).abs() < 1e-3, "gain {}", plan.gain);
}

#[test]
fn missing_replaygain_never_changes_the_level() {
    let input = sine(0.6, 4_800);
    for settings in [GainSettings::track(), GainSettings::album()] {
        let plan = settings.plan(&ReplayGain::default());
        assert_eq!(plan.origin, GainOrigin::Missing);
        assert_eq!(plan.gain, 1.0);
        assert_eq!(plan.requested_gain_db, 0.0);
        assert!(!plan.limited);

        let output: Vec<f32> =
            GainSource::with_settings(buffer(input.clone()), settings, &ReplayGain::default())
                .collect();
        assert_eq!(output, input);
    }
}

#[test]
fn playback_target_shifts_the_tag_from_the_reference() {
    let tags = ReplayGain {
        track_gain_db: Some(-8.0),
        ..ReplayGain::default()
    };

    // Con el target igual a la referencia, la etiqueta se aplica tal cual.
    let plan = GainSettings::track()
        .with_target_lufs(REPLAYGAIN_REFERENCE_LUFS)
        .plan(&tags);
    assert!((plan.requested_gain_db - (-8.0)).abs() < 1e-6);

    // Un target más alto pide +4 dB sobre la etiqueta de -18 LUFS.
    let plan = GainSettings::track().with_target_lufs(-14.0).plan(&tags);
    assert!((plan.requested_gain_db - (-4.0)).abs() < 1e-6);
    assert!((plan.gain_db - (-4.0)).abs() < 1e-6);
    assert!((plan.gain - 10_f32.powf(-4.0 / 20.0)).abs() < 1e-5);
}

#[test]
fn zero_db_tag_keeps_the_level_without_engaging_the_limiter() {
    let input = sine(1.0, 4_800);
    let tags = ReplayGain {
        track_gain_db: Some(0.0),
        track_peak: Some(1.0),
        ..ReplayGain::default()
    };
    let plan = GainSettings::track().plan(&tags);
    assert_eq!(plan.origin, GainOrigin::Track);
    assert_eq!(plan.gain, 1.0);
    assert!(
        !plan.limited,
        "1.0 * 1.0 no supera un techo de 0 dBFS: no hay nada que limitar"
    );

    let source = GainSource::new(buffer(input.clone()), plan);
    let meter = source.meter();
    let output: Vec<f32> = source.collect();
    assert_eq!(output, input);
    assert!(!meter.limited());
}

#[test]
fn limiter_attenuates_a_boosted_signal_below_the_ceiling() {
    let tags = ReplayGain {
        track_gain_db: Some(12.0),
        track_peak: Some(0.9),
        ..ReplayGain::default()
    };
    let plan = GainSettings::track().plan(&tags);
    assert!(plan.limited);
    assert!(plan.gain_db < plan.requested_gain_db);
    assert!((plan.gain * 0.9 - plan.ceiling).abs() < 1e-5);

    let source = GainSource::new(buffer(sine(0.9, 4_800)), plan);
    let meter = source.meter();
    let output: Vec<f32> = source.collect();
    assert!(
        peak(&output) <= plan.ceiling,
        "peak {} > techo {}",
        peak(&output),
        plan.ceiling
    );
    assert!(peak(&output) > 0.99, "el boost debe acercarse al techo");
    assert!(meter.limited());
    assert!(meter.peak() <= plan.ceiling);
}

#[test]
fn limiter_clamps_samples_when_the_peak_tag_understates_the_signal() {
    let input = vec![0.9, -0.9, 0.25, -0.25];
    let tags = ReplayGain {
        track_gain_db: Some(6.020_6),
        track_peak: Some(0.5),
        ..ReplayGain::default()
    };
    let plan = GainSettings::track().plan(&tags);
    // 0.5 * ~2.0 = 1.0: la etiqueta no pide atenuación estática...
    assert!(!plan.limited);

    let source = GainSource::new(buffer(input), plan);
    let meter = source.meter();
    let output: Vec<f32> = source.collect();
    // ...pero las muestras reales llegan a 1.8 y la guarda las recorta.
    let expected = [1.0, -1.0, 0.5, -0.5];
    for (out, want) in output.iter().zip(expected) {
        assert!((out - want).abs() < 1e-4, "{out} != {want}");
    }
    assert!(meter.limited());
    assert!(
        (meter.pre_limit_peak() - 1.8).abs() < 1e-4,
        "pre-limit {}",
        meter.pre_limit_peak()
    );
    assert_eq!(meter.peak(), 1.0);
}

#[test]
fn configurable_ceiling_leaves_headroom() {
    let input = sine(1.0, 4_800);
    let tags = ReplayGain {
        track_gain_db: Some(0.0),
        track_peak: Some(1.0),
        ..ReplayGain::default()
    };
    let settings = GainSettings::track().with_limiter_ceiling_dbfs(-6.020_6);
    let plan = settings.plan(&tags);
    assert!(
        plan.limited,
        "un techo de -6 dBFS recorta hasta un seno a 0 dB"
    );
    assert!((plan.ceiling - 0.5).abs() < 1e-4);

    let output: Vec<f32> = GainSource::with_settings(buffer(input), settings, &tags).collect();
    assert!(peak(&output) <= plan.ceiling + 1e-6);
    assert!((peak(&output) - plan.ceiling).abs() < 1e-4);
}

#[test]
fn attenuation_alone_does_not_engage_the_limiter() {
    let tags = ReplayGain {
        track_gain_db: Some(-6.020_6),
        track_peak: Some(1.0),
        ..ReplayGain::default()
    };
    let plan = GainSettings::track().plan(&tags);
    assert!(!plan.limited);

    let source = GainSource::new(buffer(sine(1.0, 4_800)), plan);
    let meter = source.meter();
    let output: Vec<f32> = source.collect();
    assert!(!meter.limited());
    assert!((peak(&output) - 0.5).abs() < 1e-3);
}

#[test]
fn album_mode_limits_with_the_track_peak_when_album_peak_is_missing() {
    let tags = ReplayGain {
        album_gain_db: Some(12.0),
        track_peak: Some(0.9),
        ..ReplayGain::default()
    };
    let plan = GainSettings::album().plan(&tags);
    assert_eq!(plan.origin, GainOrigin::Album);
    assert!(plan.limited);
    assert!((plan.gain * 0.9 - plan.ceiling).abs() < 1e-5);
}

#[test]
fn silence_stays_silent_under_boost() {
    let tags = ReplayGain {
        track_gain_db: Some(12.0),
        track_peak: Some(0.0),
        ..ReplayGain::default()
    };
    let plan = GainSettings::track().plan(&tags);
    assert!(!plan.limited, "0 * ganancia nunca supera el techo");

    let source = GainSource::new(buffer(vec![0.0; 64]), plan);
    let meter = source.meter();
    let output: Vec<f32> = source.collect();
    assert!(output.iter().all(|sample| *sample == 0.0));
    assert_eq!(meter.peak(), 0.0);
    assert!(!meter.limited());
}

#[test]
fn tag_strings_follow_the_replaygain_convention() {
    let tags = ReplayGain::from_tag_strings(
        Some("-6.50 dB"),
        Some("0.988553"),
        Some("+2.25 db"),
        Some("1.000000"),
    );
    assert_eq!(tags.track_gain_db, Some(-6.5));
    assert_eq!(tags.track_peak, Some(0.988_553));
    assert_eq!(tags.album_gain_db, Some(2.25));
    assert_eq!(tags.album_peak, Some(1.0));

    // Valores vacíos, no numéricos, no finitos o con peak negativo: sin
    // medida, no se inventa nada.
    let no_measure =
        ReplayGain::from_tag_strings(Some("not a number"), Some("-0.5"), Some("inf"), Some("NaN"));
    assert_eq!(no_measure, ReplayGain::default());
    assert_eq!(
        ReplayGain::from_tag_strings(Some(""), Some(" "), None, None),
        ReplayGain::default()
    );
}

#[test]
fn non_finite_in_memory_values_are_treated_as_missing() {
    let tags = ReplayGain {
        track_gain_db: Some(f32::NAN),
        track_peak: Some(f32::INFINITY),
        ..ReplayGain::default()
    };
    let plan = GainSettings::track().plan(&tags);
    assert_eq!(plan.origin, GainOrigin::Missing);
    assert_eq!(plan.gain, 1.0);
    assert!(!plan.limited);

    let plan = GainSettings::track()
        .with_target_lufs(f32::NAN)
        .plan(&ReplayGain {
            track_gain_db: Some(-6.020_6),
            ..ReplayGain::default()
        });
    assert!((plan.requested_gain_db - (-6.020_6)).abs() < 1e-4);

    let plan = GainSettings::track()
        .with_limiter_ceiling_dbfs(f32::INFINITY)
        .plan(&ReplayGain::default());
    assert_eq!(plan.ceiling, 1.0);
}

#[test]
fn source_properties_seek_and_size_hint_are_forwarded() {
    let frames = 48_000u32;
    let mut data = Vec::with_capacity(frames as usize * 2);
    for _ in 0..frames {
        data.push(0.5);
        data.push(-0.5);
    }
    let inner = SamplesBuffer::new(channels(2), rate(48_000), data.clone());
    let expected_duration = inner.total_duration();
    let expected_span = inner.current_span_len();

    let tags = ReplayGain {
        track_gain_db: Some(-6.020_6),
        ..ReplayGain::default()
    };
    let mut source = GainSource::with_settings(inner, GainSettings::track(), &tags);
    assert_eq!(source.channels().get(), 2);
    assert_eq!(source.sample_rate().get(), 48_000);
    assert_eq!(source.total_duration(), expected_duration);
    assert_eq!(source.current_span_len(), expected_span);
    assert_eq!(source.size_hint(), (data.len(), Some(data.len())));

    let first: Vec<f32> = source.by_ref().take(4).collect();
    assert!((first[0] - 0.25).abs() < 1e-3);
    assert!((first[1] + 0.25).abs() < 1e-3);

    source
        .try_seek(Duration::from_millis(500))
        .expect("seek through the gain wrapper");
    let after_seek = source.next().expect("sample after seek");
    // 0.5 s a 48 kHz = 24 000 frames; estéreo intercalado = muestra 48 000.
    assert!((after_seek - data[48_000] * 0.5).abs() < 1e-3);
}

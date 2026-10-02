//! TASK-7.1 / CR-8: the ReplayGain/R128 tag strings are produced by the shared
//! loudness domain (`AudioLoudnessMetrics` / `LoudnessStandard`).
//!
//! The inspector feeds BOTH the FLAC VorbisComments and the 0083 `tracks`
//! loudness columns, so this is the single place where the standards
//! (-18 LUFS ReplayGain 2.0, -23 LUFS EBU R128) must be applied — once, in the
//! domain — and not re-derived as format strings at the call site.

use syncify_core_domain::quality::{AudioLoudnessMetrics, LoudnessStandard};
use syncify_tauri_lib::download::audio_inspector::parse_ebur128_output;

const EBUR128_OUTPUT: &str = r#"
[Parsed_ebur128_0 @ 0x559979e29340] Summary:

  Integrated loudness:
    I:         -14.2 LUFS
    Threshold: -24.3 LUFS

  Loudness range:
    LRA:         6.5 LU
    Threshold: -34.4 LUFS
    LRA low:   -17.8 LUFS
    LRA high:  -11.3 LUFS

  True peak:
    Peak:       -0.5 dBFS
[out#0/null @ 0x559979e29340] video:0KiB audio:86KiB
"#;

fn metrics_from(
    analysis: &syncify_tauri_lib::download::audio_inspector::LoudnessAnalysis,
) -> AudioLoudnessMetrics {
    AudioLoudnessMetrics {
        integrated_lufs: analysis.integrated_lufs,
        true_peak_dbfs: analysis.true_peak_dbtp,
        loudness_range_lu: analysis.loudness_range_lu.unwrap_or(0.0),
    }
}

#[test]
fn replaygain_and_r128_tags_come_from_the_loudness_domain() {
    let analysis = parse_ebur128_output(EBUR128_OUTPUT, -18.0).expect("parse ebur128");
    let metrics = metrics_from(&analysis);

    assert_eq!(
        analysis.replaygain_track_gain,
        metrics.format_replaygain_gain_for_target(LoudnessStandard::ReplayGain2.target_lufs())
    );
    assert_eq!(analysis.r128_track_gain, metrics.format_r128_track_gain());
    assert_eq!(
        analysis.replaygain_track_peak,
        metrics.format_replaygain_track_peak()
    );
    assert_eq!(analysis.track_peak, metrics.replaygain_peak_ratio());
}

#[test]
fn a_caller_chosen_reference_loudness_only_moves_the_replaygain_gain() {
    let streaming = parse_ebur128_output(EBUR128_OUTPUT, -14.0).expect("parse ebur128");
    let rg_default = parse_ebur128_output(EBUR128_OUTPUT, -18.0).expect("parse ebur128");

    assert_eq!(streaming.replaygain_track_gain, "+0.20 dB");
    assert_eq!(rg_default.replaygain_track_gain, "-3.80 dB");
    // The EBU R128 target is fixed by the standard, so it does not move.
    assert_eq!(streaming.r128_track_gain, rg_default.r128_track_gain);
    assert_eq!(
        streaming.replaygain_track_peak,
        rg_default.replaygain_track_peak
    );
}

#[test]
fn digital_silence_never_produces_an_invalid_peak_tag() {
    // ffmpeg reports `-inf` true peak for digital silence.
    let silence = r#"
Summary:

  Integrated loudness:
    I:         -70.0 LUFS
    Threshold: -90.0 LUFS

  True peak:
    Peak:       -inf dBFS
"#;
    let analysis = parse_ebur128_output(silence, -18.0).expect("parse ebur128");
    assert_eq!(analysis.track_peak, 0.0);
    assert_eq!(analysis.replaygain_track_peak, "0.000000");

    // No Peak line at all falls back to the -0.1 dBFS floor, not to "NaN".
    let no_peak = r#"
Summary:

  Integrated loudness:
    I:         -14.2 LUFS
    Threshold: -24.3 LUFS
"#;
    let fallback = parse_ebur128_output(no_peak, -18.0).expect("parse ebur128");
    assert_eq!(fallback.true_peak_dbtp, -0.1);
    assert_eq!(
        fallback.replaygain_track_peak,
        metrics_from(&fallback).format_replaygain_track_peak()
    );
    assert!(!fallback.replaygain_track_peak.contains("NaN"));
}

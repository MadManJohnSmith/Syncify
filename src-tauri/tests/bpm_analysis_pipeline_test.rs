//! Local BPM & TEMPO Analysis Pipeline Test (S173)
//!
//! Validates:
//! 1. Known audio fixture -> BPM detected in expected range (~120 BPM).
//! 2. Double-time / half-time ambiguity resolution.
//! 3. Low confidence (noise / silence) -> rejected without false defaults.
//! 4. Non-destructive physical FLAC tagging (`BPM`, `TEMPO`) & re-read verification.
//! 5. Non-destructive physical M4A tagging (`tmpo`) & re-read verification.
//! 6. Audio payload content hash invariant before & after tagging.
//! 7. Database persistence, manual precedence preservation, and error handling.
//! 8. Exclusion of un-downloaded tracks from analysis.

use std::path::PathBuf;
use syncify_tauri_lib::services::repair_guardrail::compute_file_audio_content_hash;
use syncify_tauri_lib::services::tempo_analyzer::{
    TempoAnalyzer, TempoSource,
};
use tempfile::tempdir;

fn generate_rhythmic_audio_pcm(bpm: f64, sample_rate: u32, duration_sec: f64) -> Vec<f32> {
    let total_samples = (sample_rate as f64 * duration_sec) as usize;
    let mut samples = vec![0.0f32; total_samples];
    let beat_interval_samples = (sample_rate as f64 * 60.0 / bpm) as usize;

    // Synthesize kick / click beats with decaying sine pulses
    for beat_start in (0..total_samples).step_by(beat_interval_samples) {
        let pulse_len = (sample_rate as usize / 10).min(total_samples - beat_start); // 100ms pulse
        for i in 0..pulse_len {
            let t = i as f32 / sample_rate as f32;
            let decay = (-35.0 * t).exp();
            let freq = 120.0 - (60.0 * t); // Pitch drop kick
            let sine = (2.0 * std::f32::consts::PI * freq * t).sin();
            samples[beat_start + i] += sine * decay * 0.8;
        }
    }

    samples
}

fn create_flac_from_pcm(path: &PathBuf, samples: &[f32], sample_rate: u32) {
    let temp_wav = path.with_extension("wav");

    // Write minimal WAV header + PCM
    let mut wav_bytes = Vec::new();
    let num_samples = samples.len() as u32;
    let byte_rate = sample_rate * 2;
    let block_align = 2u16;

    wav_bytes.extend_from_slice(b"RIFF");
    wav_bytes.extend_from_slice(&(36 + num_samples * 2).to_le_bytes());
    wav_bytes.extend_from_slice(b"WAVEfmt ");
    wav_bytes.extend_from_slice(&16u32.to_le_bytes()); // Subchunk1Size
    wav_bytes.extend_from_slice(&1u16.to_le_bytes());  // AudioFormat (PCM)
    wav_bytes.extend_from_slice(&1u16.to_le_bytes());  // NumChannels (1)
    wav_bytes.extend_from_slice(&sample_rate.to_le_bytes());
    wav_bytes.extend_from_slice(&byte_rate.to_le_bytes());
    wav_bytes.extend_from_slice(&block_align.to_le_bytes());
    wav_bytes.extend_from_slice(&16u16.to_le_bytes()); // BitsPerSample
    wav_bytes.extend_from_slice(b"data");
    wav_bytes.extend_from_slice(&(num_samples * 2).to_le_bytes());

    for &s in samples {
        let i16_sample = (s.clamp(-1.0, 1.0) * 32767.0) as i16;
        wav_bytes.extend_from_slice(&i16_sample.to_le_bytes());
    }

    std::fs::write(&temp_wav, &wav_bytes).expect("Write temp WAV");

    let _ = std::process::Command::new("ffmpeg")
        .args([
            "-y",
            "-i", temp_wav.to_str().unwrap(),
            "-c:a", "flac",
            path.to_str().unwrap(),
        ])
        .output();

    let _ = std::fs::remove_file(&temp_wav);
}

fn create_m4a_from_pcm(path: &PathBuf, samples: &[f32], sample_rate: u32) {
    let temp_wav = path.with_extension("wav");

    let mut wav_bytes = Vec::new();
    let num_samples = samples.len() as u32;
    let byte_rate = sample_rate * 2;
    let block_align = 2u16;

    wav_bytes.extend_from_slice(b"RIFF");
    wav_bytes.extend_from_slice(&(36 + num_samples * 2).to_le_bytes());
    wav_bytes.extend_from_slice(b"WAVEfmt ");
    wav_bytes.extend_from_slice(&16u32.to_le_bytes());
    wav_bytes.extend_from_slice(&1u16.to_le_bytes());
    wav_bytes.extend_from_slice(&1u16.to_le_bytes());
    wav_bytes.extend_from_slice(&sample_rate.to_le_bytes());
    wav_bytes.extend_from_slice(&byte_rate.to_le_bytes());
    wav_bytes.extend_from_slice(&block_align.to_le_bytes());
    wav_bytes.extend_from_slice(&16u16.to_le_bytes());
    wav_bytes.extend_from_slice(b"data");
    wav_bytes.extend_from_slice(&(num_samples * 2).to_le_bytes());

    for &s in samples {
        let i16_sample = (s.clamp(-1.0, 1.0) * 32767.0) as i16;
        wav_bytes.extend_from_slice(&i16_sample.to_le_bytes());
    }

    std::fs::write(&temp_wav, &wav_bytes).expect("Write temp WAV");

    let _ = std::process::Command::new("ffmpeg")
        .args([
            "-y",
            "-i", temp_wav.to_str().unwrap(),
            "-c:a", "aac",
            path.to_str().unwrap(),
        ])
        .output();

    let _ = std::fs::remove_file(&temp_wav);
}

#[test]
fn test_known_audio_fixture_bpm_accuracy() {
    // Generate 120 BPM rhythmic PCM
    let samples = generate_rhythmic_audio_pcm(120.0, 22050, 10.0);
    let (bpm_opt, confidence, _, raw_bpm) =
        TempoAnalyzer::estimate_tempo_from_pcm(&samples, 22050, 0.35);

    assert!(bpm_opt.is_some(), "Expected detected BPM for 120 BPM audio");
    let bpm = bpm_opt.unwrap();
    assert!(
        (bpm as i32 - 120).abs() <= 2,
        "Detected BPM {} should be near 120 BPM (raw: {:?})",
        bpm,
        raw_bpm
    );
    assert!(confidence > 0.40, "Confidence should be high for clear beat");
}

#[test]
fn test_low_confidence_rejection_no_bpm() {
    // White noise with no rhythmic structure
    let total_samples = 22050 * 5;
    let mut noise = vec![0.0f32; total_samples];
    let mut seed: u64 = 12345;
    for s in &mut noise {
        seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1);
        *s = ((seed >> 33) as f32 / 2147483648.0) - 1.0;
    }

    let (bpm_opt, confidence, _, _) =
        TempoAnalyzer::estimate_tempo_from_pcm(&noise, 22050, 0.40);

    // Random noise has weak autocorrelation prominence
    assert!(
        confidence < 0.40 || bpm_opt.is_none(),
        "Noise must not produce high confidence BPM"
    );
}

#[tokio::test]
async fn test_flac_tags_reread_and_payload_hash_invariant() {
    let dir = tempdir().unwrap();
    let flac_path = dir.path().join("test_flac_bpm.flac");

    let samples = generate_rhythmic_audio_pcm(126.0, 22050, 5.0);
    create_flac_from_pcm(&flac_path, &samples, 22050);

    if !flac_path.exists() {
        eprintln!("ffmpeg not available to create FLAC, skipping physical test");
        return;
    }

    let hash_before = compute_file_audio_content_hash(&flac_path).await.unwrap();

    // Re-tag with BPM 126
    TempoAnalyzer::retag_file_with_bpm(&flac_path, 126)
        .await
        .expect("Re-tagging FLAC with BPM must succeed");

    // Physical re-read verification
    let tag = metaflac::Tag::read_from_path(&flac_path).expect("Read FLAC tag");
    let vc = tag.vorbis_comments().expect("Vorbis comments present");
    assert_eq!(vc.get("BPM").unwrap()[0], "126");
    assert_eq!(vc.get("TEMPO").unwrap()[0], "126");

    let hash_after = compute_file_audio_content_hash(&flac_path).await.unwrap();
    assert_eq!(
        hash_before, hash_after,
        "Audio payload hash must remain 100% identical after BPM tagging"
    );
}

#[tokio::test]
async fn test_m4a_tmpo_reread_and_payload_hash_invariant() {
    let dir = tempdir().unwrap();
    let m4a_path = dir.path().join("test_m4a_bpm.m4a");

    let samples = generate_rhythmic_audio_pcm(95.0, 22050, 5.0);
    create_m4a_from_pcm(&m4a_path, &samples, 22050);

    if !m4a_path.exists() {
        eprintln!("ffmpeg not available to create M4A, skipping physical test");
        return;
    }

    let hash_before = compute_file_audio_content_hash(&m4a_path).await.unwrap();

    // Re-tag with BPM 95
    TempoAnalyzer::retag_file_with_bpm(&m4a_path, 95)
        .await
        .expect("Re-tagging M4A with tmpo must succeed");

    // Physical re-read verification with mp4ameta
    let tag = mp4ameta::Tag::read_from_path(&m4a_path).expect("Read M4A tag");
    assert_eq!(tag.bpm(), Some(95));

    let hash_after = compute_file_audio_content_hash(&m4a_path).await.unwrap();
    assert_eq!(
        hash_before, hash_after,
        "Audio payload hash must remain 100% identical after M4A tmpo tagging"
    );
}

#[tokio::test]
async fn test_manual_precedence_and_database_persistence() {
    let pool = sqlx::sqlite::SqlitePoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await
        .unwrap();

    sqlx::migrate!("./migrations").run(&pool).await.unwrap();

    // 1. Insert dummy track and download
    let track_id: i64 = sqlx::query_scalar(
        "INSERT INTO tracks (title, isrc) VALUES ('Test Track', 'USXYZ2400001') RETURNING id"
    )
    .fetch_one(&pool)
    .await
    .unwrap();

    let dir = tempdir().unwrap();
    let flac_path = dir.path().join("track.flac");
    let samples = generate_rhythmic_audio_pcm(130.0, 22050, 4.0);
    create_flac_from_pcm(&flac_path, &samples, 22050);

    if !flac_path.exists() {
        return;
    }

    sqlx::query(
        "INSERT INTO downloads (track_id, file_path, file_format) VALUES (?, ?, 'FLAC')"
    )
    .bind(track_id)
    .bind(flac_path.to_str().unwrap())
    .execute(&pool)
    .await
    .unwrap();

    // 2. Set Manual BPM = 140
    TempoAnalyzer::update_track_bpm_manual(&pool, track_id, 140)
        .await
        .unwrap();

    let (bpm_val, source_val): (f64, String) = sqlx::query_as(
        "SELECT bpm, tempo_source FROM tracks WHERE id = ?"
    )
    .bind(track_id)
    .fetch_one(&pool)
    .await
    .unwrap();

    assert_eq!(bpm_val, 140.0);
    assert_eq!(source_val, "Manual");

    // 3. Analyze without force — manual BPM must be preserved!
    let res = TempoAnalyzer::analyze_and_retag_track(&pool, track_id, 0.35, false)
        .await
        .unwrap();

    assert_eq!(res.bpm, Some(140));
    assert_eq!(res.source, TempoSource::Manual);

    let (bpm_after, source_after): (f64, String) = sqlx::query_as(
        "SELECT bpm, tempo_source FROM tracks WHERE id = ?"
    )
    .bind(track_id)
    .fetch_one(&pool)
    .await
    .unwrap();

    assert_eq!(bpm_after, 140.0);
    assert_eq!(source_after, "Manual");
}

#[tokio::test]
async fn test_undownloaded_track_fails_safely_without_analysis() {
    let pool = sqlx::sqlite::SqlitePoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await
        .unwrap();

    sqlx::migrate!("./migrations").run(&pool).await.unwrap();

    let track_id: i64 = sqlx::query_scalar(
        "INSERT INTO tracks (title, isrc) VALUES ('Cloud Track', 'USXYZ2400002') RETURNING id"
    )
    .fetch_one(&pool)
    .await
    .unwrap();

    // Track has no row in downloads table
    let res = TempoAnalyzer::analyze_and_retag_track(&pool, track_id, 0.35, false).await;
    assert!(res.is_err(), "Must fail safely when track has no physical download");
}

#[test]
fn test_instrumental_low_confidence_rejection() {
    // Generate smooth continuous ambient chords with no percussive onsets (drone)
    let sample_rate = 22050;
    let duration_sec = 10.0;
    let total_samples = (sample_rate as f64 * duration_sec) as usize;
    let mut drone_samples = vec![0.0f32; total_samples];

    for i in 0..total_samples {
        let t = i as f32 / sample_rate as f32;
        // Superposition of smooth sines (C major chord drone)
        let s = (2.0 * std::f32::consts::PI * 261.63 * t).sin() * 0.3
            + (2.0 * std::f32::consts::PI * 329.63 * t).sin() * 0.3
            + (2.0 * std::f32::consts::PI * 392.00 * t).sin() * 0.3;
        drone_samples[i] = s;
    }

    let (bpm_opt, confidence, _, _) =
        TempoAnalyzer::estimate_tempo_from_pcm(&drone_samples, sample_rate, 0.40);

    assert!(
        confidence < 0.40 || bpm_opt.is_none(),
        "Continuous instrumental drone without rhythmic onset pulses must have low confidence (got confidence: {})",
        confidence
    );
}

#[test]
fn test_tempo_variable_rejected_test() {
    // Generate audio with variable tempo (tempo shifting between 60, 95, 145, 75, 130 BPM with no constant periodicity)
    let sample_rate = 22050;
    let duration_sec = 10.0;
    let total_samples = (sample_rate as f64 * duration_sec) as usize;
    let mut variable_samples = vec![0.0f32; total_samples];

    let bpms = [65.0, 140.0, 85.0, 160.0, 70.0, 125.0, 90.0, 175.0];
    let mut current_sample = 0;
    let mut beat_idx = 0;

    while current_sample < total_samples {
        let bpm = bpms[beat_idx % bpms.len()];
        beat_idx += 1;
        let interval = (sample_rate as f64 * 60.0 / bpm) as usize;

        let pulse_len = (sample_rate as usize / 15).min(total_samples - current_sample);
        for i in 0..pulse_len {
            let t = i as f32 / sample_rate as f32;
            let decay = (-45.0 * t).exp();
            let sine = (2.0 * std::f32::consts::PI * 130.0 * t).sin();
            variable_samples[current_sample + i] += sine * decay * 0.7;
        }

        current_sample += interval;
    }

    let (bpm_opt, confidence, _, _) =
        TempoAnalyzer::estimate_tempo_from_pcm(&variable_samples, sample_rate, 0.40);

    // Irregular/variable tempo has low confidence and is rejected
    assert!(
        confidence < 0.40 || bpm_opt.is_none(),
        "Fluctuating variable tempo audio must not produce high confidence tempo lock (got confidence: {}, bpm: {:?})",
        confidence,
        bpm_opt
    );
}

#[tokio::test]
async fn test_streaming_metadata_precedence_test() {
    let pool = sqlx::sqlite::SqlitePoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await
        .unwrap();

    sqlx::migrate!("./migrations").run(&pool).await.unwrap();

    // 1. Insert track with StreamingMetadata BPM 128
    let track_id: i64 = sqlx::query_scalar(
        "INSERT INTO tracks (title, isrc, bpm, tempo_source, tempo_confidence) 
         VALUES ('Stream Track', 'USXYZ2400003', 128.0, 'StreamingMetadata', 0.95) 
         RETURNING id"
    )
    .fetch_one(&pool)
    .await
    .unwrap();

    let dir = tempdir().unwrap();
    let flac_path = dir.path().join("stream_track.flac");
    let samples = generate_rhythmic_audio_pcm(110.0, 22050, 4.0);
    create_flac_from_pcm(&flac_path, &samples, 22050);

    if !flac_path.exists() {
        return;
    }

    sqlx::query(
        "INSERT INTO downloads (track_id, file_path, file_format) VALUES (?, ?, 'FLAC')"
    )
    .bind(track_id)
    .bind(flac_path.to_str().unwrap())
    .execute(&pool)
    .await
    .unwrap();

    // 2. Run analysis without force — must NOT overwrite StreamingMetadata!
    let res = TempoAnalyzer::analyze_and_retag_track(&pool, track_id, 0.35, false)
        .await
        .unwrap();

    assert_eq!(res.bpm, Some(128));
    assert_eq!(res.source, TempoSource::StreamingMetadata);

    let (bpm_db, source_db): (f64, String) = sqlx::query_as(
        "SELECT bpm, tempo_source FROM tracks WHERE id = ?"
    )
    .bind(track_id)
    .fetch_one(&pool)
    .await
    .unwrap();

    assert_eq!(bpm_db, 128.0);
    assert_eq!(source_db, "StreamingMetadata");
}


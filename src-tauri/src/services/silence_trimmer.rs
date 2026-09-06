//! Dead-Silence Edge Detector & Lossless Trimmer (TASK-76)
//!
//! Post-download acoustic hygiene stage, orchestrated alongside the existing
//! analyzers (`services::audio_inspector` EBU R128 / ReplayGain, `services::tempo_analyzer`
//! BPM & key): detects dead-silence lead-in / lead-out tails on the downloaded audio and
//! trims them for gapless transitions and clean fades.
//!
//! Contract (TASK-76):
//! 1. Detection runs `ffmpeg -af silencedetect` (the repo-standard audio tool, already
//!    used by `tempo_analyzer` and `audio_inspector`) with a configurable threshold
//!    (default -50 dB) and minimum duration (default > 2 s).
//! 2. Only FLAC containers are trimmed. The trim is a lossless stream-copy remux
//!    (`ffmpeg -c copy`, frames copied verbatim — no decode/re-encode), followed by
//!    metadata restoration (`syncify_flac_writer::restore_flac_metadata_blocks`, so tags
//!    and CoverFront pictures survive the remux) and STREAMINFO finalization
//!    (`syncify_flac_writer::finalize_flac_streaminfo_after_remux`, because the remux
//!    carries over the source's stale total_samples / MD5).
//!    Lossy containers (M4A/AAC/MP3) are measured and reported but never trimmed: the
//!    existing MP4 pipeline (`services::mp4_writer`) is tag-only and has no audio
//!    re-processing path, so an AAC re-encode would be a new lossy generation.
//! 3. Idempotence: a file whose edge silences are all at or below the threshold is left
//!    strictly untouched (bit-exact passthrough — the file is never rewritten).
//! 4. Gapless exemption: only an explicit gapless flag may suppress trimming. No such
//!    flag exists anywhere in the schema (migrations 0001..0084), so the exemption
//!    parameter exists in the API for a future column but is currently always `false`;
//!    see the module-level integration notes in `worker.rs`.

use serde::{Deserialize, Serialize};
use std::path::Path;
use tracing::{debug, info, warn};

use crate::download::audio_inspector::inspect_physical_audio_file;

/// Tolerance (seconds) for anchoring an edge window to the start/end of the file.
const EDGE_ANCHOR_TOLERANCE_SEC: f64 = 0.1;

/// Minimum remaining trim gap (seconds) below which a side is not handed to ffmpeg at all.
const MIN_TRIM_DELTA_SEC: f64 = 0.01;

/// A silence window reported by `ffmpeg silencedetect`.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct SilenceWindow {
    pub start_sec: f64,
    /// `f64::INFINITY` when silencedetect never emitted the matching `silence_end`
    /// (window extends to EOF).
    pub end_sec: f64,
}

impl SilenceWindow {
    /// Effective window length in seconds given the total duration of the file.
    pub fn length_sec(&self, duration_sec: f64) -> f64 {
        let end = if self.end_sec.is_infinite() {
            duration_sec
        } else {
            self.end_sec
        };
        (end - self.start_sec).max(0.0)
    }
}

/// Tunables of the dead-silence hygiene stage (TASK-76 acceptance values by default).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SilenceTrimConfig {
    /// Amplitude threshold in dBFS below which audio counts as "dead" silence.
    pub silence_threshold_db: f64,
    /// Edge silences strictly longer than this (seconds) are trimmed.
    pub min_silence_duration_sec: f64,
    /// Residual silence kept on each trimmed edge (seconds) so the track does not start
    /// or end abruptly and the frame-granular stream-copy never clips real audio.
    pub edge_guard_sec: f64,
    /// Minimum audio span (non-silent middle) required to attempt a trim at all.
    pub min_audio_duration_sec: f64,
    /// Safety ratio: skip the trim when the combined edge silence exceeds this fraction
    /// of the file (protects pathological / fully-silent fixtures).
    pub max_edge_silence_ratio: f64,
}

impl Default for SilenceTrimConfig {
    fn default() -> Self {
        Self {
            silence_threshold_db: -50.0,
            min_silence_duration_sec: 2.0,
            edge_guard_sec: 0.15,
            min_audio_duration_sec: 1.0,
            max_edge_silence_ratio: 0.75,
        }
    }
}

/// Edge-silence measurements for one file.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EdgeSilenceAnalysis {
    pub duration_sec: f64,
    /// Window anchored at the start of the file (lead-in), if any.
    pub lead_in: Option<SilenceWindow>,
    /// Window reaching the end of the file (lead-out), if any.
    pub lead_out: Option<SilenceWindow>,
}

impl EdgeSilenceAnalysis {
    pub fn lead_in_ms(&self) -> Option<u64> {
        self.lead_in
            .map(|w| (w.length_sec(self.duration_sec) * 1000.0).round() as u64)
    }

    pub fn lead_out_ms(&self) -> Option<u64> {
        self.lead_out
            .map(|w| (w.length_sec(self.duration_sec) * 1000.0).round() as u64)
    }
}

/// Outcome of the dead-silence hygiene stage for one downloaded file (TASK-76).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct SilenceTrimReport {
    /// Whether the file was physically trimmed.
    pub trimmed: bool,
    /// Why no trim happened (never set when `trimmed == true`).
    pub skipped_reason: Option<String>,
    /// Detected lead-in dead silence before the trim (ms).
    pub lead_in_ms_detected: Option<u64>,
    /// Detected lead-out dead silence before the trim (ms).
    pub lead_out_ms_detected: Option<u64>,
    /// Re-measured lead-in dead silence after the trim (ms).
    pub lead_in_ms_after: Option<u64>,
    /// Re-measured lead-out dead silence after the trim (ms).
    pub lead_out_ms_after: Option<u64>,
    /// Container duration before the trim (ms).
    pub duration_ms_before: Option<u64>,
    /// Container duration after the trim (ms).
    pub duration_ms_after: Option<u64>,
    /// Stream-copy cut window applied to the file (seconds).
    pub trim_start_sec: Option<f64>,
    pub trim_end_sec: Option<f64>,
}

pub struct SilenceTrimmer;

impl SilenceTrimmer {
    /// Parses `ffmpeg silencedetect` stderr into silence windows.
    ///
    /// Handles both terminated windows (`silence_start` + `silence_end | silence_duration`)
    /// and an unterminated trailing window (window extends to EOF).
    pub fn parse_silencedetect_stderr(stderr: &str) -> Vec<SilenceWindow> {
        const START_MARK: &str = "silence_start:";
        const END_MARK: &str = "silence_end:";

        let mut windows = Vec::new();
        let mut pending_start: Option<f64> = None;

        for raw_line in stderr.lines() {
            let line = raw_line.trim();
            if let Some(idx) = line.find(START_MARK) {
                let token = line[idx + START_MARK.len()..]
                    .split('|')
                    .next()
                    .unwrap_or("")
                    .trim();
                if let Ok(v) = token.parse::<f64>() {
                    pending_start = Some(v);
                }
            } else if let Some(idx) = line.find(END_MARK) {
                let token = line[idx + END_MARK.len()..]
                    .split('|')
                    .next()
                    .unwrap_or("")
                    .trim();
                if let Ok(v) = token.parse::<f64>() {
                    let start = pending_start.take().unwrap_or(0.0);
                    windows.push(SilenceWindow {
                        start_sec: start,
                        end_sec: v,
                    });
                }
            }
        }

        // Unterminated trailing silence: extends to EOF.
        if let Some(start) = pending_start {
            windows.push(SilenceWindow {
                start_sec: start,
                end_sec: f64::INFINITY,
            });
        }

        windows
    }

    /// Runs `ffmpeg -af silencedetect` on the file and reduces the windows to the
    /// lead-in / lead-out edge windows.
    pub async fn detect_edge_silence(
        file_path: &Path,
        config: &SilenceTrimConfig,
    ) -> Result<EdgeSilenceAnalysis, String> {
        if !file_path.is_file() {
            return Err(format!("Audio file does not exist: {:?}", file_path));
        }

        // Report every window >= 50 ms; the TASK-76 "> min_silence_duration_sec" trim
        // decision is taken in code below. This keeps `lead_in_ms_detected` /
        // `lead_out_ms_detected` honest even when the edge silence is below the trim
        // threshold, and makes the decision independent of ffmpeg's reporting granularity.
        let filter = format!(
            "silencedetect=noise={}dB:d=0.05",
            config.silence_threshold_db
        );
        let output = crate::cmd_utils::create_tokio_command("ffmpeg")
            .args(["-hide_banner", "-nostats", "-i"])
            .arg(file_path)
            // -vn: never touch attached-pic (CoverFront) video streams; only the audio
            // stream feeds silencedetect. Without it a corrupt/unusual embedded picture
            // aborts the whole analysis instead of being ignored.
            .args(["-vn", "-af", &filter, "-f", "null", "-"])
            .output()
            .await
            .map_err(|e| {
                if e.kind() == std::io::ErrorKind::NotFound {
                    "SilenceAnalysisUnavailable: FFmpeg binary not found in system PATH".to_string()
                } else {
                    format!("Failed to spawn ffmpeg silencedetect: {}", e)
                }
            })?;

        if !output.status.success() {
            return Err(format!(
                "ffmpeg silencedetect failed ({}): {}",
                output.status,
                String::from_utf8_lossy(&output.stderr).trim()
            ));
        }

        let windows = Self::parse_silencedetect_stderr(&String::from_utf8_lossy(&output.stderr));
        let duration_sec = Self::probe_duration_seconds(file_path).await?;

        // Lead-in: first window anchored at (or before) the file start.
        let lead_in = windows
            .iter()
            .find(|w| w.start_sec <= EDGE_ANCHOR_TOLERANCE_SEC)
            .copied();
        // Lead-out: last window reaching (or passing) the file end.
        let lead_out = windows
            .iter()
            .rev()
            .find(|w| w.end_sec.is_infinite() || w.end_sec >= duration_sec - EDGE_ANCHOR_TOLERANCE_SEC)
            .copied();

        Ok(EdgeSilenceAnalysis {
            duration_sec,
            lead_in,
            lead_out,
        })
    }

    /// Resolves the container duration: physical inspection first (STREAMINFO / MP4
    /// metadata, no subprocess), ffprobe fallback.
    pub async fn probe_duration_seconds(file_path: &Path) -> Result<f64, String> {
        if let Some(phys) = inspect_physical_audio_file(file_path) {
            if let Some(dur) = phys.duration_secs {
                if dur > 0.0 {
                    return Ok(dur);
                }
            }
        }

        let output = crate::cmd_utils::create_tokio_command("ffprobe")
            .args([
                "-v", "error",
                "-show_entries", "format=duration",
                "-of", "default=nw=1:nk=1",
            ])
            .arg(file_path)
            .output()
            .await
            .map_err(|e| format!("Failed to spawn ffprobe for duration: {}", e))?;

        if !output.status.success() {
            return Err(format!(
                "ffprobe duration probe failed: {}",
                String::from_utf8_lossy(&output.stderr).trim()
            ));
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        stdout
            .trim()
            .parse::<f64>()
            .map_err(|e| format!("Failed to parse ffprobe duration '{}': {}", stdout.trim(), e))
    }

    /// Convenience entry point used by the download pipeline: default TASK-76 config,
    /// no gapless exemption (see module docs for the flag rationale).
    pub async fn process_file(file_path: &Path) -> Result<SilenceTrimReport, String> {
        Self::process_file_with_config(file_path, &SilenceTrimConfig::default(), false).await
    }

    /// Detects and (for FLAC) trims dead-silence lead-in / lead-out on a downloaded file.
    ///
    /// Fails soft by contract at the call site: any error leaves the original file
    /// strictly intact (the remux happens in a sibling temp file promoted by rename
    /// only after metadata restoration and STREAMINFO finalization succeeded).
    pub async fn process_file_with_config(
        file_path: &Path,
        config: &SilenceTrimConfig,
        gapless_exempt: bool,
    ) -> Result<SilenceTrimReport, String> {
        let mut report = SilenceTrimReport::default();

        let analysis = Self::detect_edge_silence(file_path, config).await?;
        report.lead_in_ms_detected = analysis.lead_in_ms();
        report.lead_out_ms_detected = analysis.lead_out_ms();
        report.duration_ms_before = Some((analysis.duration_sec * 1000.0).round() as u64);

        if gapless_exempt {
            report.skipped_reason = Some("gapless_album_exempt".to_string());
            debug!(
                path = %file_path.display(),
                "[TASK-76] Gapless album exemption: trim skipped"
            );
            return Ok(report);
        }

        let ext = file_path
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("")
            .to_lowercase();

        // Lossless trim is FLAC-only (see module docs for the M4A/AAC scope decision).
        if ext != "flac" {
            report.skipped_reason = Some(format!("trim_not_supported_for_container_{}", ext));
            debug!(
                path = %file_path.display(),
                lead_in_ms = ?report.lead_in_ms_detected,
                lead_out_ms = ?report.lead_out_ms_detected,
                "[TASK-76] Edge silence measured; trim skipped for container .{}",
                ext
            );
            return Ok(report);
        }

        // Trim decision: only edges strictly above the duration threshold.
        let mut trim_start_sec = 0.0f64;
        let mut trim_end_sec: Option<f64> = None;
        let mut do_trim = false;

        if let Some(win) = analysis.lead_in {
            if win.length_sec(analysis.duration_sec) > config.min_silence_duration_sec {
                trim_start_sec = (win.end_sec - config.edge_guard_sec).max(0.0);
                do_trim = true;
            }
        }
        if let Some(win) = analysis.lead_out {
            if win.length_sec(analysis.duration_sec) > config.min_silence_duration_sec {
                let guarded_end = (win.start_sec + config.edge_guard_sec).min(analysis.duration_sec);
                trim_end_sec = Some(guarded_end);
                do_trim = true;
            }
        }

        if !do_trim {
            report.skipped_reason = Some("no_edge_silence_above_threshold".to_string());
            debug!(
                path = %file_path.display(),
                "[TASK-76] No edge silence above threshold ({:.1} dB / > {}s); file left untouched",
                config.silence_threshold_db, config.min_silence_duration_sec
            );
            return Ok(report);
        }

        let effective_end = trim_end_sec.unwrap_or(analysis.duration_sec);
        let kept_audio_sec = effective_end - trim_start_sec;
        // Residual edge silence kept after the trim: the guard bands at each trimmed edge.
        let edge_silence_sec =
            trim_start_sec + (analysis.duration_sec - effective_end);

        if kept_audio_sec < config.min_audio_duration_sec {
            report.skipped_reason = Some("remaining_audio_below_floor".to_string());
            warn!(
                path = %file_path.display(),
                "[TASK-76] Trim aborted: remaining audio {:.2}s below floor {:.2}s",
                kept_audio_sec, config.min_audio_duration_sec
            );
            return Ok(report);
        }
        if analysis.duration_sec > 0.0
            && edge_silence_sec / analysis.duration_sec > config.max_edge_silence_ratio
        {
            report.skipped_reason = Some("excessive_edge_silence_ratio".to_string());
            warn!(
                path = %file_path.display(),
                "[TASK-76] Trim aborted: edge silence {:.2}s exceeds {:.0}% of the file",
                edge_silence_sec,
                config.max_edge_silence_ratio * 100.0
            );
            return Ok(report);
        }

        report.trim_start_sec = (trim_start_sec > MIN_TRIM_DELTA_SEC).then_some(trim_start_sec);
        report.trim_end_sec = trim_end_sec;

        // 1. Lossless stream-copy remux into a sibling temp file.
        let start_arg = (trim_start_sec > MIN_TRIM_DELTA_SEC).then_some(trim_start_sec);
        let end_arg = if analysis.duration_sec - effective_end > MIN_TRIM_DELTA_SEC {
            Some(effective_end)
        } else {
            None
        };

        // Owned path for the blocking thread (spawn_blocking requires 'static).
        let blocking_path = file_path.to_path_buf();
        let tmp_path = tokio::task::spawn_blocking(move || {
            syncify_flac_writer::trim_flac_stream_copy(&blocking_path, start_arg, end_arg)
        })
        .await
        .map_err(|e| format!("FLAC trim task join error: {}", e))??;

        // 2-3. Restore pipeline tags/pictures, then fix the remuxed STREAMINFO.
        let restore_path = file_path.to_path_buf();
        let finalize_result = (|| -> Result<syncify_flac_writer::FlacStreaminfoFinalization, String> {
            let restored =
                syncify_flac_writer::restore_flac_metadata_blocks(&tmp_path, &restore_path)?;
            debug!(
                tmp = %tmp_path.display(),
                restored_blocks = restored,
                "[TASK-76] Restored metadata blocks onto trimmed FLAC"
            );
            syncify_flac_writer::finalize_flac_streaminfo_after_remux(&tmp_path)
        })();

        if let Err(e) = finalize_result {
            let _ = tokio::fs::remove_file(&tmp_path).await;
            return Err(format!("FLAC trim finalization failed (original left intact): {}", e));
        }

        // 4. Atomic promotion over the original file.
        if let Err(e) = tokio::fs::rename(&tmp_path, file_path).await {
            let _ = tokio::fs::remove_file(&tmp_path).await;
            return Err(format!("Failed to promote trimmed FLAC: {}", e));
        }

        // 5. Re-measure the promoted file for the report (best-effort).
        match Self::detect_edge_silence(file_path, config).await {
            Ok(post) => {
                report.lead_in_ms_after = post.lead_in_ms();
                report.lead_out_ms_after = post.lead_out_ms();
                report.duration_ms_after = Some((post.duration_sec * 1000.0).round() as u64);
            }
            Err(e) => {
                warn!(
                    path = %file_path.display(),
                    error = %e,
                    "[TASK-76] Post-trim re-measurement failed (trim itself succeeded)"
                );
            }
        }

        report.trimmed = true;
        info!(
            path = %file_path.display(),
            lead_in_ms_before = ?report.lead_in_ms_detected,
            lead_out_ms_before = ?report.lead_out_ms_detected,
            lead_in_ms_after = ?report.lead_in_ms_after,
            lead_out_ms_after = ?report.lead_out_ms_after,
            duration_ms_before = ?report.duration_ms_before,
            duration_ms_after = ?report.duration_ms_after,
            "[TASK-76] Dead-silence edge trim applied (lossless FLAC stream copy, tags restored)"
        );
        Ok(report)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;
    use std::process::Command;
    use std::sync::atomic::{AtomicU64, Ordering};

    static TEST_SEQ: AtomicU64 = AtomicU64::new(0);

    fn ffmpeg_available() -> bool {
        Command::new("ffmpeg")
            .arg("-version")
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false)
    }

    fn unique_name(prefix: &str, ext: &str) -> PathBuf {
        let n = TEST_SEQ.fetch_add(1, Ordering::SeqCst);
        std::env::temp_dir().join(format!(
            "siltrim_{}_{}_{}.{}",
            prefix,
            std::process::id(),
            n,
            ext
        ))
    }

    struct TestAudio {
        path: PathBuf,
    }

    impl Drop for TestAudio {
        fn drop(&mut self) {
            let _ = std::fs::remove_file(&self.path);
        }
    }

    /// Generates `[lead_in silence][tone][lead_out silence]` via ffmpeg lavfi
    /// (`sine` + `adelay` + `apad`), exactly the fixture style demanded by TASK-76.
    fn generate_audio_with_silence_tails(
        prefix: &str,
        ext: &str,
        lead_in_sec: f64,
        tone_sec: f64,
        lead_out_sec: f64,
    ) -> TestAudio {
        let out = unique_name(prefix, ext);
        let _ = std::fs::remove_file(&out);
        let lead_in_ms = (lead_in_sec * 1000.0).round() as i64;
        let status = Command::new("ffmpeg")
            .args(["-v", "error", "-y"])
            .args([
                "-f", "lavfi",
                "-i", &format!("sine=frequency=440:duration={}:sample_rate=44100", tone_sec),
            ])
            .args([
                "-af", &format!("adelay={}:all=1,apad=pad_dur={}", lead_in_ms, lead_out_sec),
            ])
            .args(["-ac", "2"])
            .arg(&out)
            .status()
            .expect("spawn ffmpeg fixture generator");
        assert!(status.success(), "ffmpeg fixture generation failed");
        TestAudio { path: out }
    }

    fn sha256_hex(path: &Path) -> String {
        let bytes = std::fs::read(path).expect("read fixture");
        crate::services::repair_guardrail::compute_bytes_sha256(&bytes)
    }

    fn set_test_tags(path: &Path) {
        let mut tag = metaflac::Tag::read_from_path(path).expect("read tag");
        let comments = tag.vorbis_comments_mut();
        comments.set("TITLE", vec!["Silence Fixture"]);
        comments.set("ARTIST", vec!["TASK-76 Test Artist"]);
        comments.set("ALBUM", vec!["Silence Fixture Album"]);
        // Real 8x8 PNG front cover, generated with ffmpeg (a byte-valid picture keeps
        // the fixture close to a real download; ffmpeg must be able to decode it).
        let png_path = unique_name("cover", "png");
        let _ = std::fs::remove_file(&png_path);
        let png_status = Command::new("ffmpeg")
            .args(["-v", "error", "-y", "-f", "lavfi", "-i", "color=c=red:s=8x8:d=1"])
            .args(["-frames:v", "1"])
            .arg(&png_path)
            .status()
            .expect("spawn ffmpeg png generator");
        assert!(png_status.success(), "ffmpeg png generation failed");
        let png = std::fs::read(&png_path).expect("read generated png");
        let _ = std::fs::remove_file(&png_path);
        tag.add_picture("image/png", metaflac::block::PictureType::CoverFront, png);
        tag.write_to_path(path).expect("write tags");
    }

    fn streaminfo_md5_hex(path: &Path) -> Option<String> {
        let tag = metaflac::Tag::read_from_path(path).ok()?;
        let si = tag.get_streaminfo()?;
        Some(si.md5.iter().map(|b| format!("{:02x}", b)).collect())
    }

    #[test]
    fn test_silencedetect_parser_windows_and_unterminated_tail() {
        let stderr = "[Parsed_silencedetect_0 @ 0x1] silence_start: 0\n\
                      [Parsed_silencedetect_0 @ 0x1] silence_end: 3.500023 | silence_duration: 3.500023\n\
                      [Parsed_silencedetect_0 @ 0x1] silence_start: 8.5\n\
                      [Parsed_silencedetect_0 @ 0x1] silence_end: 12 | silence_duration: 3.5\n";
        let windows = SilenceTrimmer::parse_silencedetect_stderr(stderr);
        assert_eq!(windows.len(), 2);
        assert!((windows[0].start_sec - 0.0).abs() < 1e-9);
        assert!((windows[0].end_sec - 3.500023).abs() < 1e-6);
        assert!((windows[1].start_sec - 8.5).abs() < 1e-9);
        assert!((windows[1].end_sec - 12.0).abs() < 1e-9);

        let trailing = "silence_start: 7.25\n";
        let windows = SilenceTrimmer::parse_silencedetect_stderr(trailing);
        assert_eq!(windows.len(), 1);
        assert!(windows[0].end_sec.is_infinite());
        assert!((windows[0].length_sec(12.0) - 4.75).abs() < 1e-9);
    }

    #[tokio::test]
    async fn test_silence_exceeding_2s_is_trimmed_to_threshold_with_tags_preserved() {
        if !ffmpeg_available() {
            eprintln!("ffmpeg not available; skipping physical trim test");
            return;
        }
        let fixture = generate_audio_with_silence_tails("trim", "flac", 3.5, 5.0, 3.5);
        let path = fixture.path.clone();
        set_test_tags(&path);

        let bytes_before = std::fs::read(&path).unwrap();
        let before_report = syncify_flac_writer::inspect_and_verify_flac_stream(&path).unwrap();
        assert!(before_report.streaminfo_md5_valid);

        let report = SilenceTrimmer::process_file(&path).await.expect("process_file");

        // (a) The excessive edge silence was actually trimmed.
        assert!(report.trimmed, "expected a trim, got: {:?}", report);
        assert!(report.skipped_reason.is_none());
        assert_eq!(report.lead_in_ms_detected, Some(3500));
        assert_eq!(report.lead_out_ms_detected, Some(3500));

        // Duration collapses to the tone plus the two edge guards (± frame granularity).
        let after_ms = report.duration_ms_after.expect("post duration");
        let expected_ms = 5000.0 + 2.0 * 150.0;
        assert!(
            (after_ms as f64 - expected_ms).abs() <= 400.0,
            "duration after trim {} ms not within tolerance of {} ms",
            after_ms,
            expected_ms
        );

        // (a') Re-measured edge silence is now far below the 2 s criterion.
        assert!(
            report.lead_in_ms_after.unwrap_or(0) < 2000,
            "lead-in after trim still excessive: {:?}",
            report
        );
        assert!(
            report.lead_out_ms_after.unwrap_or(0) < 2000,
            "lead-out after trim still excessive: {:?}",
            report
        );

        // Tags + CoverFront picture survived the remux.
        let tag = metaflac::Tag::read_from_path(&path).expect("re-read tags");
        let comments = tag.vorbis_comments().expect("vorbis comments");
        assert_eq!(
            comments.get("TITLE").and_then(|v| v.first()).map(|s| s.to_string()),
            Some("Silence Fixture".to_string())
        );
        assert_eq!(
            comments.get("ARTIST").and_then(|v| v.first()).map(|s| s.to_string()),
            Some("TASK-76 Test Artist".to_string())
        );
        assert!(tag.pictures().next().is_some(), "picture block lost in trim");

        // STREAMINFO is coherent: valid MD5 that matches a fresh bit-exact PCM hash.
        let integrity = syncify_flac_writer::inspect_and_verify_flac_stream(&path)
            .expect("integrity report after trim");
        assert!(integrity.streaminfo_md5_valid, "MD5 missing after trim");
        assert!(
            integrity.verified,
            "STREAMINFO MD5 does not match decoded PCM after trim: {:?}",
            integrity
        );
        assert_eq!(
            streaminfo_md5_hex(&path).as_deref(),
            Some(integrity.computed_md5.as_str())
        );

        // Losslessness of the retained region: the trimmed decoded PCM must be a
        // contiguous slice of the original decoded PCM (stream copy, no re-encode).
        let decode = |p: &Path| -> Vec<u8> {
            Command::new("ffmpeg")
                .args(["-v", "error", "-i"])
                .arg(p)
                .args(["-f", "s16le", "-"])
                .output()
                .expect("decode")
                .stdout
        };
        let trimmed_pcm = decode(&path);
        std::fs::write(&path, &bytes_before).unwrap(); // restore fixture for the decode
        let original_pcm = decode(&path);
        assert!(
            original_pcm.len() > trimmed_pcm.len(),
            "trimmed PCM must be shorter than original"
        );
        let offset = original_pcm
            .windows(trimmed_pcm.len().min(original_pcm.len()))
            .position(|w| w == trimmed_pcm.as_slice())
            .expect("trimmed PCM is not a contiguous slice of the original PCM");
        let offset_sec = offset as f64 / 4.0 / 44100.0; // stereo s16
        assert!(
            (offset_sec - 3.5 + 0.15).abs() <= 0.25,
            "trimmed PCM starts at {} s, expected ~{} s (lead-in minus guard, ± frame)",
            offset_sec,
            3.5 - 0.15
        );

        // No temp artifacts left behind.
        let parent = path.parent().unwrap();
        let leftovers: Vec<_> = std::fs::read_dir(parent)
            .unwrap()
            .filter_map(|e| e.ok())
            .filter(|e| e.file_name().to_string_lossy().contains("silencetrim"))
            .collect();
        assert!(leftovers.is_empty(), "temp trim files leaked: {:?}", leftovers);
    }

    #[tokio::test]
    async fn test_track_without_excessive_silence_passes_through_bit_exact() {
        if !ffmpeg_available() {
            eprintln!("ffmpeg not available; skipping passthrough test");
            return;
        }
        // 0.3 s edge silences: below the 2 s criterion -> must not be touched at all.
        let fixture = generate_audio_with_silence_tails("passthrough", "flac", 0.3, 5.0, 0.3);
        let path = fixture.path.clone();
        set_test_tags(&path);

        let sha_before = sha256_hex(&path);
        let md5_before = streaminfo_md5_hex(&path).expect("streaminfo md5 before");

        let report = SilenceTrimmer::process_file(&path).await.expect("process_file");

        assert!(!report.trimmed, "file with sub-threshold silence must not be trimmed");
        assert_eq!(report.skipped_reason.as_deref(), Some("no_edge_silence_above_threshold"));
        assert_eq!(report.lead_in_ms_detected, Some(300));
        assert_eq!(report.lead_out_ms_detected, Some(300));
        assert!(report.trim_start_sec.is_none() && report.trim_end_sec.is_none());

        // (c) Bit-exact passthrough: identical file bytes => STREAMINFO/frames MD5 intact.
        let sha_after = sha256_hex(&path);
        assert_eq!(sha_before, sha_after, "file bytes changed on a no-trim pass");
        assert_eq!(md5_before, streaminfo_md5_hex(&path).expect("md5 after"));
        let integrity = syncify_flac_writer::inspect_and_verify_flac_stream(&path).unwrap();
        assert!(integrity.verified, "integrity failed on untouched file: {:?}", integrity);
    }

    #[tokio::test]
    async fn test_m4a_is_measured_but_never_trimmed() {
        if !ffmpeg_available() {
            eprintln!("ffmpeg not available; skipping M4A detection-only test");
            return;
        }
        let fixture = generate_audio_with_silence_tails("m4a", "m4a", 3.5, 5.0, 3.5);
        let path = fixture.path.clone();

        let sha_before = sha256_hex(&path);
        let report = SilenceTrimmer::process_file(&path).await.expect("process_file");

        assert!(!report.trimmed, "M4A must never be trimmed in this scope");
        assert!(
            report
                .skipped_reason
                .as_deref()
                .unwrap_or_default()
                .starts_with("trim_not_supported_for_container"),
            "unexpected skip reason: {:?}",
            report
        );
        // Detection still reports the metrics honestly.
        assert!(
            report.lead_in_ms_detected.unwrap_or(0) > 2000,
            "lead-in metric missing: {:?}",
            report
        );
        assert!(
            report.lead_out_ms_detected.unwrap_or(0) > 2000,
            "lead-out metric missing: {:?}",
            report
        );
        assert_eq!(sha_before, sha256_hex(&path), "M4A bytes must stay untouched");
    }

    #[tokio::test]
    async fn test_gapless_album_exemption_suppresses_trim() {
        if !ffmpeg_available() {
            eprintln!("ffmpeg not available; skipping gapless exemption test");
            return;
        }
        let fixture = generate_audio_with_silence_tails("gapless", "flac", 3.5, 5.0, 3.5);
        let path = fixture.path.clone();
        set_test_tags(&path);

        let sha_before = sha256_hex(&path);
        let report = SilenceTrimmer::process_file_with_config(
            &path,
            &SilenceTrimConfig::default(),
            true, // explicit gapless flag
        )
        .await
        .expect("process_file_with_config");

        assert!(!report.trimmed);
        assert_eq!(report.skipped_reason.as_deref(), Some("gapless_album_exempt"));
        assert_eq!(report.lead_in_ms_detected, Some(3500));
        assert_eq!(sha_before, sha256_hex(&path), "gapless-exempt file must stay untouched");
    }

    #[tokio::test]
    async fn test_processing_is_idempotent_on_already_trimmed_file() {
        if !ffmpeg_available() {
            eprintln!("ffmpeg not available; skipping idempotency test");
            return;
        }
        let fixture = generate_audio_with_silence_tails("idem", "flac", 3.5, 5.0, 3.5);
        let path = fixture.path.clone();

        let first = SilenceTrimmer::process_file(&path).await.expect("first pass");
        assert!(first.trimmed);
        let sha_after_first = sha256_hex(&path);

        let second = SilenceTrimmer::process_file(&path).await.expect("second pass");
        assert!(!second.trimmed, "second pass must be a no-op");
        assert_eq!(sha_after_first, sha256_hex(&path), "second pass modified the file");
    }
}

use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;
use tauri::{command, AppHandle};
use tracing::{info, warn};

use crate::db::DbPool;
use crate::services::tempo_analyzer::TempoAnalyzer;

static BPM_CANCELLATION_TOKEN: AtomicBool = AtomicBool::new(false);
static BPM_ANALYZER_RUNNING: AtomicBool = AtomicBool::new(false);

struct AnalyzerRunningGuard;
impl Drop for AnalyzerRunningGuard {
    fn drop(&mut self) {
        BPM_ANALYZER_RUNNING.store(false, Ordering::SeqCst);
    }
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct BpmAnalysisOptions {
    #[serde(default = "default_true")]
    pub only_missing: bool,
    #[serde(default = "default_threshold")]
    pub confidence_threshold: f64,
    #[serde(default)]
    pub force: bool,
    pub track_ids: Option<Vec<i64>>,
}

fn default_true() -> bool {
    true
}

fn default_threshold() -> f64 {
    0.40
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct BpmProgressEvent {
    pub current_index: usize,
    pub total: usize,
    pub track_id: i64,
    pub track_title: String,
    pub bpm: Option<u32>,
    pub confidence: f64,
    pub status: String,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, Default)]
pub struct BpmAnalysisBatchSummary {
    pub total: usize,
    pub analyzed: usize,
    pub skipped: usize,
    pub low_confidence: usize,
    pub failed: usize,
}

#[command]
pub async fn analyze_library_bpm(
    app: AppHandle,
    db: State<'_, DbPool>,
    options: Option<BpmAnalysisOptions>,
) -> Result<BpmAnalysisBatchSummary, String> {
    // 1. Dependency Preflight: Detect FFmpeg availability
    TempoAnalyzer::check_ffmpeg_available()?;

    // 2. Concurrency Safety: Ensure at most 1 simultaneous batch analysis
    if BPM_ANALYZER_RUNNING
        .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
        .is_err()
    {
        return Err("BPM analysis is already running. Only one simultaneous analysis is allowed.".to_string());
    }
    let _guard = AnalyzerRunningGuard;

    let opts = options.unwrap_or_else(|| BpmAnalysisOptions {
        only_missing: true,
        confidence_threshold: 0.40,
        force: false,
        track_ids: None,
    });

    BPM_CANCELLATION_TOKEN.store(false, Ordering::SeqCst);

    // 3. Query candidate downloaded tracks from SQLite
    let query_str = if let Some(ref ids) = opts.track_ids {
        let placeholders: Vec<String> = ids.iter().map(|_| "?".to_string()).collect();
        format!(
            "SELECT t.id, t.title, t.bpm, t.tempo_source, d.file_path 
             FROM tracks t
             JOIN downloads d ON d.track_id = t.id
             WHERE t.id IN ({})",
            placeholders.join(",")
        )
    } else if opts.only_missing && !opts.force {
        "SELECT t.id, t.title, t.bpm, t.tempo_source, d.file_path 
         FROM tracks t
         JOIN downloads d ON d.track_id = t.id
         WHERE (t.bpm IS NULL OR t.bpm = 0)
         ORDER BY t.id ASC".to_string()
    } else {
        "SELECT t.id, t.title, t.bpm, t.tempo_source, d.file_path 
         FROM tracks t
         JOIN downloads d ON d.track_id = t.id
         ORDER BY t.id ASC".to_string()
    };

    let mut query = sqlx::query_as::<_, (i64, String, Option<f64>, Option<String>, String)>(&query_str);

    if let Some(ref ids) = opts.track_ids {
        for id in ids {
            query = query.bind(id);
        }
    }

    let tracks = query
        .fetch_all(db.inner())
        .await
        .map_err(|e| format!("Failed to fetch tracks for BPM analysis: {}", e))?;

    let total = tracks.len();
    let mut summary = BpmAnalysisBatchSummary {
        total,
        ..Default::default()
    };

    info!(
        total = total,
        only_missing = opts.only_missing,
        confidence_threshold = opts.confidence_threshold,
        "[BPM Analysis] Starting batch library tempo analysis"
    );

    for (idx, (track_id, title, current_bpm, tempo_source, _file_path)) in tracks.into_iter().enumerate() {
        if BPM_CANCELLATION_TOKEN.load(Ordering::SeqCst) {
            info!("[BPM Analysis] Cancelled by user");
            break;
        }

        // 4. Resource Safety: Pause if there are active downloads
        if TempoAnalyzer::has_active_downloads(db.inner()).await.unwrap_or(false) {
            info!("[BPM Analysis] Active downloads detected, yielding briefly");
            tokio::time::sleep(Duration::from_millis(150)).await;
        }

        // Cooperative yield between tracks to avoid blocking tokio worker or UI
        tokio::task::yield_now().await;
        tokio::time::sleep(Duration::from_millis(5)).await;

        // Check if skipping is needed based on precedence and missing settings
        if opts.only_missing && current_bpm.is_some() && current_bpm.unwrap() > 0.0 && !opts.force {
            summary.skipped += 1;
            let _ = app.emit(
                "syncify:bpm_analysis_progress",
                BpmProgressEvent {
                    current_index: idx + 1,
                    total,
                    track_id,
                    track_title: title,
                    bpm: current_bpm.map(|b| b.round() as u32),
                    confidence: 1.0,
                    status: "skipped".to_string(),
                },
            );
            continue;
        }

        if tempo_source.as_deref() == Some("Manual") && !opts.force {
            summary.skipped += 1;
            let _ = app.emit(
                "syncify:bpm_analysis_progress",
                BpmProgressEvent {
                    current_index: idx + 1,
                    total,
                    track_id,
                    track_title: title,
                    bpm: current_bpm.map(|b| b.round() as u32),
                    confidence: 1.0,
                    status: "skipped".to_string(),
                },
            );
            continue;
        }

        match TempoAnalyzer::analyze_and_retag_track(
            db.inner(),
            track_id,
            opts.confidence_threshold,
            opts.force,
        )
        .await
        {
            Ok(res) => {
                if let Some(bpm) = res.bpm {
                    summary.analyzed += 1;
                    let _ = app.emit(
                        "syncify:bpm_analysis_progress",
                        BpmProgressEvent {
                            current_index: idx + 1,
                            total,
                            track_id,
                            track_title: title,
                            bpm: Some(bpm),
                            confidence: res.confidence,
                            status: "analyzed".to_string(),
                        },
                    );
                } else {
                    summary.low_confidence += 1;
                    let _ = app.emit(
                        "syncify:bpm_analysis_progress",
                        BpmProgressEvent {
                            current_index: idx + 1,
                            total,
                            track_id,
                            track_title: title,
                            bpm: None,
                            confidence: res.confidence,
                            status: "low_confidence".to_string(),
                        },
                    );
                }
            }
            Err(e) => {
                summary.failed += 1;
                warn!(track_id = track_id, error = %e, "[BPM Analysis] Track analysis error");
                let _ = app.emit(
                    "syncify:bpm_analysis_progress",
                    BpmProgressEvent {
                        current_index: idx + 1,
                        total,
                        track_id,
                        track_title: title,
                        bpm: None,
                        confidence: 0.0,
                        status: "error".to_string(),
                    },
                );
            }
        }
    }

    info!(
        analyzed = summary.analyzed,
        skipped = summary.skipped,
        low_confidence = summary.low_confidence,
        failed = summary.failed,
        "[BPM Analysis] Completed library tempo analysis batch"
    );

    Ok(summary)
}

#[command]
pub async fn cancel_bpm_analysis() -> Result<(), String> {
    BPM_CANCELLATION_TOKEN.store(true, Ordering::SeqCst);
    info!("[BPM Analysis] Cancel requested");
    Ok(())
}

#[command]
pub async fn update_track_bpm_manual(
    db: State<'_, DbPool>,
    track_id: i64,
    bpm: u32,
) -> Result<(), String> {
    TempoAnalyzer::update_track_bpm_manual(db.inner(), track_id, bpm).await
}

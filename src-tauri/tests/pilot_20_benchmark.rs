//! Limited 20-Track Physical Audio BPM Pilot & Precision Benchmark (S173)

use std::path::{Path, PathBuf};
use std::time::Instant;
use syncify_tauri_lib::services::repair_guardrail::compute_file_audio_content_hash;
use syncify_tauri_lib::services::tempo_analyzer::TempoAnalyzer;

#[derive(Debug)]
pub struct PilotBenchmarkEntry {
    pub index: usize,
    pub genre_category: &'static str,
    pub path: PathBuf,
    pub container: String,
    pub duration_sec: f64,
    pub bpm_prev: Option<u32>,
    pub bpm_new: Option<u32>,
    pub raw_bpm: Option<f64>,
    pub confidence: f64,
    pub is_ambiguous: bool,
    pub source: String,
    pub analysis_duration_ms: u128,
    pub payload_hash_before: String,
    pub payload_hash_after: String,
    pub tag_readback: Option<u32>,
    pub reference_bpm: Option<f64>,
    pub absolute_error: Option<f64>,
}

fn read_existing_bpm(path: &Path) -> Option<u32> {
    let ext = path.extension()?.to_str()?.to_lowercase();
    if ext == "flac" {
        let tag = metaflac::Tag::read_from_path(path).ok()?;
        let vc = tag.vorbis_comments()?;
        if let Some(bpm_str) = vc.get("BPM").and_then(|v| v.first()) {
            return bpm_str.parse::<u32>().ok();
        }
        if let Some(tempo_str) = vc.get("TEMPO").and_then(|v| v.first()) {
            return tempo_str.parse::<u32>().ok();
        }
    } else if ext == "m4a" || ext == "aac" || ext == "mp4" {
        let tag = mp4ameta::Tag::read_from_path(path).ok()?;
        return tag.bpm().map(|b| b as u32);
    }
    None
}

fn get_audio_duration(path: &Path) -> f64 {
    let out = std::process::Command::new("ffprobe")
        .args([
            "-v", "error",
            "-show_entries", "format=duration",
            "-of", "default=noprint_wrappers=1:nokey=1",
            path.to_str().unwrap(),
        ])
        .output()
        .ok();

    if let Some(output) = out {
        let s = String::from_utf8_lossy(&output.stdout).trim().to_string();
        s.parse::<f64>().unwrap_or(0.0)
    } else {
        0.0
    }
}

fn resolve_pilot_track(rel: &str) -> Option<PathBuf> {
    let mut roots = Vec::new();
    if let Ok(env_dir) = std::env::var("SYNCIFY_BENCHMARK_DIR") {
        roots.push(PathBuf::from(env_dir));
    }
    if let Some(audio_dir) = dirs::audio_dir() {
        roots.push(audio_dir.join("Syncify"));
        roots.push(audio_dir);
    }
    if let Some(home) = dirs::home_dir() {
        roots.push(home.join("Music").join("Syncify"));
        roots.push(home.join("Music"));
    }
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    if let Some(parent) = manifest_dir.parent() {
        roots.push(parent.join("downloads_real_test"));
        roots.push(parent.join("downloads_syncify"));
    }

    for root in roots {
        let candidate = root.join(rel);
        if candidate.exists() {
            return Some(candidate);
        }
    }
    None
}

#[tokio::test]
async fn run_20_track_physical_pilot_and_report() {
    let pilot_tracks: Vec<(&'static str, &'static str, Option<f64>)> = vec![
        // 5 Electronic / Dance
        (
            "Electronic/Dance",
            "Various Artists/Synth Pop/31 - Sandra - (I'll Never Be) Maria Magdalena.flac",
            Some(104.0),
        ),
        (
            "Electronic/Dance",
            "Com Truise/Iteration/01 - Of Your Fake Dimension.flac",
            Some(98.0),
        ),
        (
            "Electronic/Dance",
            "YACHT/(Downtown) Dancing/01 - (Downtown) Dancing.flac",
            Some(120.0),
        ),
        (
            "Electronic/Dance",
            "SHINee/1 of 1 - The 5th Album/02 - 1 of 1.flac",
            Some(112.0),
        ),
        (
            "Electronic/Dance",
            "Jackson Wang/100 Ways/01 - 100 Ways.flac",
            Some(115.0),
        ),
        // 5 Rock / Pop
        (
            "Rock-Pop",
            "Garbage/Garbage/Disc 1/18 - #1 Crush.flac",
            Some(94.0),
        ),
        (
            "Rock-Pop",
            "Britney Spears/Baby One More Time (Deluxe Version)/01 - Baby One More Time.flac",
            Some(93.0),
        ),
        (
            "Rock-Pop",
            "Blue Öyster Cult/Agents Of Fortune/03 - (Don't Fear) The Reaper.flac",
            Some(141.0),
        ),
        (
            "Rock-Pop",
            "David Bowie/_Heroes_/03 - _Heroes_.flac",
            Some(112.0),
        ),
        (
            "Rock-Pop",
            "Weezer/Weezer (White Album)/04 - (Girl We Got A) Good Thing.flac",
            Some(148.0),
        ),
        // 3 Hip-Hop
        (
            "Hip-Hop",
            "Beastie Boys/Licensed To Ill/07 - (You Gotta) Fight For Your Right (To Party!).flac",
            Some(134.0),
        ),
        (
            "Hip-Hop",
            "Cypress Hill/Skull & Bones/Disc 2/06 - (Rock) Superstar.flac",
            Some(94.0),
        ),
        (
            "Hip-Hop",
            "The Neighbourhood/#000000 & #FFFFFF (No DJ Version)/11 - #icanteven.flac",
            Some(81.0),
        ),
        // 3 Classical / Instrumental
        (
            "Classical/Instrumental",
            "Various Artists/100 Classical Favourites/Disc 5/03 - Rundfunkchor Leipzig - _Treulich geführt ziehet dahin_.flac",
            None, // Non-percussive choral classical
        ),
        (
            "Classical/Instrumental",
            "Christina Pluhar/Handel goes Wild/02 - _Venti, turbini_ (From Rinaldo, HWV 7b) [Arr. Pluhar].flac",
            Some(108.0),
        ),
        (
            "Classical/Instrumental",
            "Ileana Cotrubas/Verdi_ La Traviata/Disc 1/16 - _Ah! Dite alla giovine_.flac",
            None, // Operatic rubato aria
        ),
        // 2 Live / Tempo Variable
        (
            "Live/Tempo-Variable",
            "Rodrigo y Gabriela/Area 52/04 - 11_11.flac",
            None, // Variable tempo acoustic
        ),
        (
            "Live/Tempo-Variable",
            "Bunbury/Flamingos/15 - Y al final.flac",
            None, // Expressive vocal ballad
        ),
        // 2 M4A AAC Fallback
        (
            "M4A AAC Fallback",
            "Garbage/2024 - Absolute Garbage (Special Edition)/06 - #1 Crush.m4a",
            Some(94.0),
        ),
        (
            "M4A AAC Fallback",
            "Morat/2018 - Balas Perdidas/12 - 11 Besos.m4a",
            Some(108.0),
        ),
    ];

    println!("\n==========================================================================================================");
    println!("                                S173 20-TRACK PHYSICAL PILOT BENCHMARK REPORT                             ");
    println!("==========================================================================================================");

    let mut results = Vec::new();

    for (idx, (genre, rel_path, ref_bpm)) in pilot_tracks.into_iter().enumerate() {
        let path_buf = match resolve_pilot_track(rel_path) {
            Some(p) => p,
            None => {
                eprintln!("Pilot track not found: {}", rel_path);
                continue;
            }
        };
        let path = path_buf.as_path();

        let container = path
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("")
            .to_uppercase();

        let duration_sec = get_audio_duration(path);
        let bpm_prev = read_existing_bpm(path);

        // Pre-analysis payload hash
        let hash_before = compute_file_audio_content_hash(path)
            .await
            .unwrap_or_else(|e| format!("hash_err: {}", e));

        // DSP analysis
        let start_time = Instant::now();
        let analysis_res = TempoAnalyzer::analyze_file(path, 0.35).await;
        let analysis_duration = start_time.elapsed().as_millis();

        let (bpm_new, confidence, is_ambiguous, raw_bpm) = match analysis_res {
            Ok(ref res) => (res.bpm, res.confidence, res.is_ambiguous, res.raw_bpm),
            Err(ref e) => {
                eprintln!("Analysis error for {}: {}", path.display(), e);
                (None, 0.0, false, None)
            }
        };

        // If high confidence BPM detected, test isolated re-tag & payload invariant
        let (hash_after, tag_readback) = if let Some(bpm) = bpm_new {
            let _ = TempoAnalyzer::retag_file_with_bpm(path, bpm).await;
            let h_after = compute_file_audio_content_hash(path)
                .await
                .unwrap_or_else(|e| format!("hash_err: {}", e));
            let readback = read_existing_bpm(path);
            (h_after, readback)
        } else {
            (hash_before.clone(), bpm_prev)
        };

        // Compute error against reference if available
        let absolute_error = if let (Some(detected), Some(reference)) = (bpm_new, ref_bpm) {
            let d = detected as f64;
            // Also check half-time / double-time match
            let diff_direct = (d - reference).abs();
            let diff_double = ((d * 2.0) - reference).abs();
            let diff_half = ((d / 2.0) - reference).abs();
            let min_diff = diff_direct.min(diff_double).min(diff_half);
            Some(min_diff)
        } else {
            None
        };

        results.push(PilotBenchmarkEntry {
            index: idx + 1,
            genre_category: genre,
            path: path.to_path_buf(),
            container,
            duration_sec,
            bpm_prev,
            bpm_new,
            raw_bpm,
            confidence,
            is_ambiguous,
            source: "LocalAudioAnalysis".to_string(),
            analysis_duration_ms: analysis_duration,
            payload_hash_before: hash_before,
            payload_hash_after: hash_after,
            tag_readback,
            reference_bpm: ref_bpm,
            absolute_error,
        });
    }

    // Print markdown table
    println!("\n| # | Category | File | Container | Dur (s) | BPM Prev | BPM New | Conf | Latency | Payload Hash Invariant | Tag Readback | Ref BPM | Abs Err |");
    println!("|---|---|---|---|---|---|---|---|---|---|---|---|---|");

    for r in &results {
        let filename = r.path.file_name().and_then(|n| n.to_str()).unwrap_or("");
        let short_name = if filename.len() > 30 {
            format!("{}...", &filename[..27])
        } else {
            filename.to_string()
        };

        let hash_match = if r.payload_hash_before == r.payload_hash_after {
            "✓ MATCH"
        } else {
            "✗ CHANGED"
        };

        let prev_str = r.bpm_prev.map(|b| b.to_string()).unwrap_or_else(|| "—".to_string());
        let new_str = r.bpm_new.map(|b| b.to_string()).unwrap_or_else(|| "— (low conf)".to_string());
        let readback_str = r.tag_readback.map(|b| b.to_string()).unwrap_or_else(|| "—".to_string());
        let ref_str = r.reference_bpm.map(|b| format!("{:.0}", b)).unwrap_or_else(|| "Variable/None".to_string());
        let err_str = r.absolute_error.map(|e| format!("{:.1} BPM", e)).unwrap_or_else(|| "N/A".to_string());

        println!(
            "| {:2} | {} | {} | {} | {:.1} | {} | {} | {:.2} | {}ms | {} | {} | {} | {} |",
            r.index,
            r.genre_category,
            short_name,
            r.container,
            r.duration_sec,
            prev_str,
            new_str,
            r.confidence,
            r.analysis_duration_ms,
            hash_match,
            readback_str,
            ref_str,
            err_str,
        );
    }

    println!("\n==========================================================================================================");
}

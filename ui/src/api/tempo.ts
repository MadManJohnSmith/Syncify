import { invoke } from '@tauri-apps/api/core';
import { listen, type UnlistenFn } from '@tauri-apps/api/event';
import { asNumber, asRecord } from './normalize';

export interface BpmAnalysisOptions {
  only_missing?: boolean;
  confidence_threshold?: number;
  force?: boolean;
  track_ids?: number[];
}

export interface BpmProgressEvent {
  current_index: number;
  total: number;
  track_id: number;
  track_title: string;
  bpm: number | null;
  confidence: number;
  status: 'analyzed' | 'skipped' | 'low_confidence' | 'error';
}

export interface BpmAnalysisBatchSummary {
  total: number;
  analyzed: number;
  skipped: number;
  low_confidence: number;
  failed: number;
}

/**
 * Normalize the batch summary so every rendered counter has a safe default.
 */
function normalizeBpmSummary(raw: unknown): BpmAnalysisBatchSummary {
  const rec = asRecord(raw);
  return {
    total: asNumber(rec?.total),
    analyzed: asNumber(rec?.analyzed),
    skipped: asNumber(rec?.skipped),
    low_confidence: asNumber(rec?.low_confidence),
    failed: asNumber(rec?.failed),
  };
}

export async function analyzeLibraryBpm(options?: BpmAnalysisOptions): Promise<BpmAnalysisBatchSummary> {
  const raw = await invoke<unknown>('analyze_library_bpm', { options });
  return normalizeBpmSummary(raw);
}

export async function cancelBpmAnalysis(): Promise<void> {
  return await invoke<void>('cancel_bpm_analysis');
}

export async function updateTrackBpmManual(trackId: number, bpm: number): Promise<void> {
  return await invoke<void>('update_track_bpm_manual', { trackId, bpm });
}

export async function listenBpmProgress(callback: (event: BpmProgressEvent) => void): Promise<UnlistenFn> {
  return await listen<BpmProgressEvent>('syncify:bpm_analysis_progress', (e) => {
    callback(e.payload);
  });
}

/**
 * Download Phase Utilities & Contract Formatter (S149)
 * 
 * Maps 14-phase download telemetry and progress events into clear, compact,
 * and accessible UI status strings.
 */

import { classifyFailureReason } from '@/api/queue';

export interface PhaseFormatOptions {
  status?: string | null;
  percent?: number | null;
  bytesDownloaded?: number | null;
  totalBytes?: number | null;
  instantKbps?: number | null;
  averageKbps?: number | null;
  message?: string | null;
  errorMessage?: string | null;
}

/**
 * Format a download phase and telemetry into a concise, user-friendly label.
 */
export function formatDownloadPhase(
  phase?: string | null,
  options?: PhaseFormatOptions
): string {
  const status = (options?.status || '').toLowerCase();
  const rawPhase = (phase || '').toLowerCase().replace(/[_\s-]/g, '');
  const message = options?.message || '';
  const errorMessage = options?.errorMessage || '';

  // Terminal Cancelled
  if (status === 'cancelled' || rawPhase === 'cancelled') {
    return 'Cancelled';
  }

  // Terminal Failed
  if (
    status === 'failed' ||
    status === 'error' ||
    status === 'stale_source' ||
    status === 'rejected_quality' ||
    status === 'requires_auth' ||
    rawPhase === 'failed'
  ) {
    const failureInfo = classifyFailureReason(errorMessage || message || status);
    return `Failed: ${failureInfo.label}`;
  }

  // Terminal Completed
  if (
    status === 'complete' ||
    status === 'completed' ||
    rawPhase === 'complete' ||
    rawPhase === 'completed'
  ) {
    return 'Completed';
  }

  // Best effort messages if provided in event message for auxiliary phases
  if (message && (rawPhase === 'resolvelyrics' || rawPhase === 'resolvecover' || !rawPhase)) {
    const msgLower = message.toLowerCase();
    if (
      msgLower.includes('lyrics unavailable') ||
      msgLower.includes('no lyrics found') ||
      msgLower.includes('lyrics not found')
    ) {
      return 'Lyrics unavailable — continuing';
    }
    if (
      msgLower.includes('cover unavailable') ||
      msgLower.includes('animated cover unavailable') ||
      msgLower.includes('no cover found')
    ) {
      return 'Animated cover unavailable — continuing';
    }
  }

  switch (rawPhase) {
    case 'queued':
      return 'Queued';

    case 'queuewait':
      return 'Waiting for slot';

    case 'auth':
      return 'Authenticating service';

    case 'resolvestream':
    case 'searching':
      return 'Resolving stream';

    case 'transfer':
    case 'downloading': {
      const pct = options?.percent;
      const kbps = options?.instantKbps ?? options?.averageKbps;
      const speedStr = formatSpeed(kbps);

      if (pct !== null && pct !== undefined && pct >= 0 && speedStr) {
        return `Downloading ${Math.round(pct)}% — ${speedStr}`;
      } else if (pct !== null && pct !== undefined && pct >= 0) {
        return `Downloading ${Math.round(pct)}%`;
      } else if (speedStr) {
        return `Downloading — ${speedStr}`;
      }
      return 'Downloading';
    }

    case 'validateaudio':
      return 'Validating audio';

    case 'enrichmetadata':
      return 'Enriching metadata';

    case 'resolvelyrics':
      return 'Searching synchronized lyrics';

    case 'resolvecover':
      return 'Downloading animated cover';

    case 'tagging':
    case 'finalizing':
      return 'Writing tags';

    case 'promotion':
      return 'Moving to library';

    case 'persisting':
      return 'Persisting record';

    default:
      if (status === 'queued') return 'Queued';
      if (status === 'downloading') return 'Downloading';
      return 'In progress';
  }
}

/**
 * Format throughput speed into human-readable unit (KiB/s or MiB/s)
 */
export function formatSpeed(kbps?: number | null): string {
  if (!kbps || kbps <= 0) return '';
  if (kbps >= 1024) {
    return `${(kbps / 1024).toFixed(1)} MiB/s`;
  }
  return `${Math.round(kbps)} KiB/s`;
}

/**
 * Format milliseconds into human readable seconds or milliseconds
 */
export function formatDurationMs(ms?: number | null): string {
  if (ms === null || ms === undefined) return '--';
  if (ms < 1000) return `${ms} ms`;
  return `${(ms / 1000).toFixed(2)} s`;
}

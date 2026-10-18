/**
 * Utilities for parsing and syncing LRC and TTML lyrics.
 * Extracted and unified for the expanded/fullscreen player.
 */

export interface ParsedLyricLine {
  timeSec: number;
  label: string;
  text: string;
}

const TIMESTAMP_RE = /\[(\d{1,3}):(\d{2})(?:[.:](\d{1,3}))?\]/g;

/**
 * Formats seconds as an mm:ss.xx clock
 */
export function formatClock(sec: number): string {
  const s = Math.max(0, sec);
  const mm = Math.floor(s / 60);
  const rest = s - mm * 60;
  const whole = Math.floor(rest);
  const cents = Math.round((rest - whole) * 100);
  return `${String(mm).padStart(2, '0')}:${String(whole).padStart(2, '0')}.${String(Math.min(cents, 99)).padStart(2, '0')}`;
}

/**
 * Parses LRC text content, extracting [mm:ss.xx] timestamps.
 * If a line contains multiple timestamps, they are expanded.
 */
export function parseLrc(content: string): ParsedLyricLine[] {
  if (!content) return [];
  const out: ParsedLyricLine[] = [];

  for (const raw of content.split(/\r?\n/)) {
    TIMESTAMP_RE.lastIndex = 0;
    const stamps: number[] = [];
    let m: RegExpExecArray | null;
    while ((m = TIMESTAMP_RE.exec(raw)) !== null) {
      const mm = parseInt(m[1], 10);
      const ss = parseInt(m[2], 10);
      const fracRaw = m[3] ?? '';
      const fracMs = parseInt((fracRaw || '0').padEnd(3, '0').slice(0, 3), 10);
      stamps.push(mm * 60 + ss + fracMs / 1000);
    }
    if (stamps.length === 0) continue;
    const text = raw.replace(TIMESTAMP_RE, '').trim();
    for (const t of stamps) {
      out.push({ timeSec: t, label: formatClock(t), text });
    }
  }

  out.sort((a, b) => a.timeSec - b.timeSec);
  return out;
}

/**
 * Finds the index of the active lyric line for a given playback second.
 * Returns -1 if no line has started yet.
 */
export function findActiveLyricIndex(lines: ParsedLyricLine[], positionSec: number): number {
  if (!lines || lines.length === 0) return -1;
  let active = -1;
  for (let i = 0; i < lines.length; i++) {
    if (lines[i].timeSec <= positionSec) {
      active = i;
    } else {
      break;
    }
  }
  return active;
}

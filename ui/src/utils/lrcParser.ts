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
 * Word/inline timings shipped by enhanced LRC (LRCLib "word" sync, Apple Music
 * exports): `[line time]<word time>word<word time>word…`.
 *
 * The fullscreen player synchronizes **line by line** with the line tags; the
 * inline tags only mark where each word starts. Leaving them in the rendered
 * text is the defect they used to cause (`<00:00.94>looked <00:01.09>at` visibly
 * mixed into the lyrics), so they are consumed here and never shown.
 */
const WORD_TIMESTAMP_RE = /<(\d{1,3}):(\d{2})(?:[.:](\d{1,3}))?>/g;

function stampToSeconds(minutes: string, seconds: string, fraction?: string): number {
  const mm = parseInt(minutes, 10);
  const ss = parseInt(seconds, 10);
  const fracMs = parseInt((fraction || '0').padEnd(3, '0').slice(0, 3), 10);
  return mm * 60 + ss + fracMs / 1000;
}

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
 * Parses LRC text content, extracting `[mm:ss.xx]` line timestamps.
 * If a line contains multiple line timestamps, they are expanded.
 *
 * Enhanced LRC word timings (`<mm:ss.xx>`) are consumed: the line keeps its line
 * timestamp and its text comes out clean. A line that only carries word timings
 * (no line tag) starts at its first word timing instead of being dropped.
 */
export function parseLrc(content: string): ParsedLyricLine[] {
  if (!content) return [];
  const out: ParsedLyricLine[] = [];

  for (const raw of content.split(/\r?\n/)) {
    TIMESTAMP_RE.lastIndex = 0;
    const stamps: number[] = [];
    let firstWordStart: number | null = null;
    let text = '';
    let cursor = 0;
    let m: RegExpExecArray | null;

    // Single pass over both tag kinds, in document order, so the cleaned text
    // keeps the original word spacing.
    const COMBINED_RE = new RegExp(
      `${TIMESTAMP_RE.source}|${WORD_TIMESTAMP_RE.source}`,
      'gm',
    );
    while ((m = COMBINED_RE.exec(raw)) !== null) {
      text += raw.slice(cursor, m.index);
      cursor = m.index + m[0].length;
      if (m[4] !== undefined) {
        // `<mm:ss.xx>` word timing.
        if (firstWordStart === null) {
          firstWordStart = stampToSeconds(m[4], m[5], m[6]);
        }
      } else {
        stamps.push(stampToSeconds(m[1], m[2], m[3]));
      }
    }
    text += raw.slice(cursor);

    const startStamps = stamps.length > 0 ? stamps : firstWordStart !== null ? [firstWordStart] : [];
    if (startStamps.length === 0) continue;

    const cleanText = text.replace(/\s+/g, ' ').trim();
    for (const t of startStamps) {
      out.push({ timeSec: t, label: formatClock(t), text: cleanText });
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

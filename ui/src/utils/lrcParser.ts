/**
 * Utilities for parsing and syncing LRC and enhanced LRC lyrics.
 * Extracted and unified for the expanded/fullscreen player.
 *
 * Enhanced LRC (the format stored by the lyrics engine for word-synced sources,
 * including Kugou KRC, which the backend converts to `<mm:ss.xx>` segments) keeps
 * the per-word (or per-syllable) start times. This module preserves them so the
 * player can fill the active line word by word; when a line carries no inline
 * timings the parsed line has `words: []` and the caller falls back to the
 * line-level highlight.
 */

export interface LyricWord {
  /** Start of this word/syllable, in seconds from the beginning of the track. */
  startSec: number;
  /** Exact text of the segment, including its trailing space when the source has one. */
  text: string;
}

export interface ParsedLyricLine {
  timeSec: number;
  label: string;
  text: string;
  /**
   * Word/syllable segments with their own start time, in document order.
   * Empty for plain LRC (line timings only): explicit fallback to line highlight.
   */
  words: LyricWord[];
}

const TIMESTAMP_RE = /\[(\d{1,3}):(\d{2})(?:[.:](\d{1,3}))?\]/g;
/**
 * Word/inline timings shipped by enhanced LRC (LRCLib "word" sync, Apple Music
 * exports, Kugou KRC converted server-side): `[line time]<word time>word<word time>word…`.
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

interface RawSegment {
  startSec: number | null;
  text: string;
}

/** Collapses runs of whitespace but keeps a single leading/trailing space. */
function normalizeSegmentText(text: string): string {
  const collapsed = text.replace(/\s+/g, ' ');
  return collapsed;
}

/**
 * Turns the raw segments of one line into renderable words.
 *
 * - Whitespace-only segments are glued to their neighbour so the concatenation
 *   of all segments still reproduces the line text.
 * - The first/last segment lose their outer padding (the line is already
 *   `trim()`ed).
 * - A line whose word timings are out of order is rejected (`[]`): the caller
 *   then uses the line-level highlight instead of guessing.
 */
function toLyricWords(segments: RawSegment[], lineStartSec: number): LyricWord[] {
  const words: LyricWord[] = [];

  for (const segment of segments) {
    const text = normalizeSegmentText(segment.text);
    if (text === '') continue;

    if (text.trim() === '' && words.length > 0) {
      words[words.length - 1].text += text;
      continue;
    }

    words.push({ startSec: segment.startSec ?? lineStartSec, text });
  }

  if (words.length === 0) return [];

  words[0].text = words[0].text.replace(/^\s+/, '');
  words[words.length - 1].text = words[words.length - 1].text.replace(/\s+$/, '');

  const ordered = words.every(
    (word, index) => index === 0 || word.startSec >= words[index - 1].startSec,
  );
  return ordered ? words : [];
}

/**
 * Parses LRC text content, extracting `[mm:ss.xx]` line timestamps.
 * If a line contains multiple line timestamps, they are expanded.
 *
 * Enhanced LRC word timings (`<mm:ss.xx>`) are consumed into `words`: the line
 * keeps its line timestamp, its text comes out clean, and each word/syllable
 * keeps the instant it starts. A line that only carries word timings (no line
 * tag) starts at its first word timing instead of being dropped.
 */
export function parseLrc(content: string): ParsedLyricLine[] {
  if (!content) return [];
  const out: ParsedLyricLine[] = [];

  const COMBINED_RE = new RegExp(
    `${TIMESTAMP_RE.source}|${WORD_TIMESTAMP_RE.source}`,
    'gm',
  );

  for (const raw of content.split(/\r?\n/)) {
    const stamps: number[] = [];
    const segments: RawSegment[] = [];
    let leadingText = '';
    let pendingWord: RawSegment | null = null;
    let hasWordTags = false;
    let text = '';
    let cursor = 0;
    let m: RegExpExecArray | null;

    // Single pass over both tag kinds, in document order, so the cleaned text
    // keeps the original word spacing and each word keeps its own start.
    while ((m = COMBINED_RE.exec(raw)) !== null) {
      const between = raw.slice(cursor, m.index);
      text += between;
      cursor = m.index + m[0].length;

      if (m[4] !== undefined) {
        // `<mm:ss.xx>` word timing.
        hasWordTags = true;
        if (pendingWord) {
          pendingWord.text += between;
          segments.push(pendingWord);
        } else {
          leadingText += between;
        }
        pendingWord = { startSec: stampToSeconds(m[4], m[5], m[6]), text: '' };
      } else {
        stamps.push(stampToSeconds(m[1], m[2], m[3]));
      }
    }

    const tail = raw.slice(cursor);
    text += tail;
    if (pendingWord) {
      pendingWord.text += tail;
      segments.push(pendingWord);
    } else {
      leadingText += tail;
    }

    const firstWordStart = segments.length > 0 ? segments[0].startSec : null;
    const startStamps =
      stamps.length > 0 ? stamps : firstWordStart !== null ? [firstWordStart] : [];
    if (startStamps.length === 0) continue;

    const cleanText = text.replace(/\s+/g, ' ').trim();
    const lineStart = startStamps[0];
    if (leadingText.trim() !== '') {
      segments.unshift({ startSec: lineStart, text: leadingText });
    }

    // Multiple line stamps on one physical line repeat the text; the inline
    // timings belong to one of them, so those copies stay line-level.
    const words =
      hasWordTags && startStamps.length === 1
        ? toLyricWords(segments, lineStart)
        : [];

    for (const t of startStamps) {
      out.push({ timeSec: t, label: formatClock(t), text: cleanText, words });
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

/**
 * Index of the word/syllable currently being sung inside a line.
 * Returns -1 before the first word starts (or when the line has no word timings,
 * which is the explicit line-level fallback).
 */
export function findActiveWordIndex(words: LyricWord[], positionSec: number): number {
  if (!words || words.length === 0) return -1;
  let active = -1;
  for (let i = 0; i < words.length; i++) {
    if (words[i].startSec <= positionSec) {
      active = i;
    } else {
      break;
    }
  }
  return active;
}

/**
 * End of a word for the fill animation: the next word's start, or the line end
 * (the next line's timestamp) for the last word. `null` means "unknown": the
 * last word of the last line, which is then painted fully instead of inventing
 * a duration.
 */
export function resolveWordEndSec(
  words: LyricWord[],
  index: number,
  lineEndSec?: number | null,
): number | null {
  if (index < 0 || index >= words.length) return null;
  const next = words[index + 1];
  if (next) return next.startSec;
  if (
    typeof lineEndSec === 'number' &&
    Number.isFinite(lineEndSec) &&
    lineEndSec > words[index].startSec
  ) {
    return lineEndSec;
  }
  return null;
}

/**
 * How much of a word is already sung, 0–100. Unknown end (`null`) paints the
 * word fully as soon as it starts: no fabricated duration.
 */
export function wordFillPercent(
  startSec: number,
  endSec: number | null,
  positionSec: number,
): number {
  if (positionSec <= startSec) return 0;
  if (endSec === null) return 100;
  if (endSec <= startSec) return 100;
  if (positionSec >= endSec) return 100;
  return Math.round(((positionSec - startSec) / (endSec - startSec)) * 100);
}

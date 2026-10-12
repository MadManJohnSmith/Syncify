/**
 * Utilidades para análisis y sincronización de letras LRC y TTML.
 * Extraído y unificado para el reproductor expandido/pantalla completa.
 */

export interface ParsedLyricLine {
  timeSec: number;
  label: string;
  text: string;
}

const TIMESTAMP_RE = /\[(\d{1,3}):(\d{2})(?:[.:](\d{1,3}))?\]/g;

/**
 * Formatea segundos a reloj mm:ss.xx
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
 * Parsea el contenido de texto LRC extrayendo marcas de tiempo [mm:ss.xx].
 * Si una línea contiene múltiples marcas, se expanden.
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
 * Encuentra el índice de la línea de letra activa para un segundo de reproducción dado.
 * Devuelve -1 si ninguna línea ha comenzado aún.
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

import { describe, it, expect } from 'vitest'
import {
  parseLrc,
  findActiveLyricIndex,
  findActiveWordIndex,
  resolveWordEndSec,
  wordFillPercent,
  formatClock,
  type LyricWord,
} from '../lrcParser'

describe('lrcParser', () => {
  it('formats seconds into mm:ss.xx format', () => {
    expect(formatClock(0)).toBe('00:00.00')
    expect(formatClock(65.5)).toBe('01:05.50')
    expect(formatClock(125.123)).toBe('02:05.12')
  })

  it('parses standard LRC lyrics with timestamps', () => {
    const lrc = `
[00:01.00] Line one
[00:04.50] Line two
[00:10.20] Line three
`
    const parsed = parseLrc(lrc)
    expect(parsed).toHaveLength(3)
    expect(parsed[0]).toEqual({ timeSec: 1, label: '00:01.00', text: 'Line one', words: [] })
    expect(parsed[1]).toEqual({ timeSec: 4.5, label: '00:04.50', text: 'Line two', words: [] })
    expect(parsed[2]).toEqual({ timeSec: 10.2, label: '00:10.20', text: 'Line three', words: [] })
  })

  it('handles multiple timestamps per line and empty input', () => {
    expect(parseLrc('')).toEqual([])
    const multi = '[00:01.00][00:05.00] Chorus repeated'
    const parsed = parseLrc(multi)
    expect(parsed).toHaveLength(2)
    expect(parsed[0].timeSec).toBe(1)
    expect(parsed[1].timeSec).toBe(5)
    expect(parsed[0].text).toBe('Chorus repeated')
    expect(parsed[1].text).toBe('Chorus repeated')
  })

  it('finds active lyric line based on current playback position', () => {
    const lines = [
      { timeSec: 5, label: '00:05.00', text: 'Verse 1', words: [] },
      { timeSec: 12, label: '00:12.00', text: 'Verse 2', words: [] },
      { timeSec: 25, label: '00:25.00', text: 'Chorus', words: [] },
    ]

    expect(findActiveLyricIndex(lines, 2)).toBe(-1)
    expect(findActiveLyricIndex(lines, 5)).toBe(0)
    expect(findActiveLyricIndex(lines, 8)).toBe(0)
    expect(findActiveLyricIndex(lines, 12)).toBe(1)
    expect(findActiveLyricIndex(lines, 20)).toBe(1)
    expect(findActiveLyricIndex(lines, 25)).toBe(2)
    expect(findActiveLyricIndex(lines, 99)).toBe(2)
  })

  /**
   * UI-CORR-2: las letras sincronizadas por palabra (LRCLib "word", KRC de
   * Apple Music) llegan como LRC enriquecido `[línea]<palabra>palabra…`. El
   * texto se entrega limpio: sin `<00:00.94>` incrustado.
   *
   * LY10 (#181): además del texto limpio, cada palabra conserva su instante
   * para el relleno palabra a palabra del reproductor.
   */
  it('limpia los time tags de palabra conservando el tiempo de línea y de cada palabra', () => {
    const enhanced =
      '[00:00.94]<00:00.94>I <00:01.09>looked <00:01.24>at <00:01.60>you, <00:02.15>same <00:02.30>as <00:02.45>before\n' +
      '[00:06.07]<00:06.07>Lyrics\n' +
      '[01:46.88]<01:46.88>Three <01:47.08>words <01:47.38>that <01:47.74>I <01:48.09>can\u2019t <01:48.45>say\n'
    const parsed = parseLrc(enhanced)

    expect(parsed).toHaveLength(3)
    expect(parsed[0].timeSec).toBeCloseTo(0.94, 2)
    expect(parsed[0].text).toBe('I looked at you, same as before')
    expect(parsed[1].text).toBe('Lyrics')
    expect(parsed[2].timeSec).toBeCloseTo(106.88, 2)
    expect(parsed[2].text).toBe('Three words that I can\u2019t say')
    for (const line of parsed) {
      expect(line.text).not.toContain('<')
      expect(line.text).not.toContain('>')
    }

    expect(parsed[0].words).toEqual<LyricWord[]>([
      { startSec: 0.94, text: 'I ' },
      { startSec: 1.09, text: 'looked ' },
      { startSec: 1.24, text: 'at ' },
      { startSec: 1.6, text: 'you, ' },
      { startSec: 2.15, text: 'same ' },
      { startSec: 2.3, text: 'as ' },
      { startSec: 2.45, text: 'before' },
    ])
    // La concatenación de los segmentos reproduce el texto exacto de la línea.
    expect(parsed[0].words.map((word) => word.text).join('')).toBe(parsed[0].text)
  })

  it('una línea sólo con time tags de palabra arranca en su primer tag (no se pierde)', () => {
    const parsed = parseLrc('<00:03.20>Hello <00:03.60>world')
    expect(parsed).toHaveLength(1)
    expect(parsed[0].timeSec).toBeCloseTo(3.2, 2)
    expect(parsed[0].text).toBe('Hello world')
    expect(parsed[0].words.map((word) => word.startSec)).toEqual([3.2, 3.6])
  })

  it('no inventa líneas a partir de etiquetas de metadatos LRC', () => {
    expect(parseLrc('[ar:The Weeknd]\n[ti:Starboy]')).toEqual([])
  })
})

/**
 * LY10 (#181): palabra/sílaba activa y relleno. Todo offline: los tiempos se
 * inyectan a mano, sin reloj real ni red. El contenido es el que el motor de
 * letras (#179) ya almacena.
 */
describe('lrcParser · karaoke palabra a palabra (LY10)', () => {
  const ENHANCED = [
    '[00:01.00]<00:01.00>I <00:01.30>looked <00:01.60>at <00:02.00>you',
    '[00:04.50]<00:04.50>Same <00:04.90>as <00:05.10>before',
  ].join('\n')

  it('encuentra la palabra activa con tiempos inyectados (bordes incluidos)', () => {
    const words = parseLrc(ENHANCED)[0].words

    expect(findActiveWordIndex(words, 0.999)).toBe(-1)
    expect(findActiveWordIndex(words, 1)).toBe(0)
    expect(findActiveWordIndex(words, 1.299)).toBe(0)
    expect(findActiveWordIndex(words, 1.3)).toBe(1)
    expect(findActiveWordIndex(words, 1.99)).toBe(2)
    expect(findActiveWordIndex(words, 2)).toBe(3)
    expect(findActiveWordIndex(words, 60)).toBe(3)
    expect(findActiveWordIndex([], 1)).toBe(-1)
  })

  it('trata cada sílaba de KRC (convertido a Enhanced LRC) como un segmento', () => {
    const krcAsElrc = [
      '[00:12.00]<00:12.00>我<00:12.30>爱<00:12.60>你',
      '[00:15.00]<00:15.00>这<00:15.25>是<00:15.50>爱',
    ].join('\n')
    const lines = parseLrc(krcAsElrc)

    expect(lines).toHaveLength(2)
    expect(lines[0].text).toBe('我爱你')
    expect(lines[0].words).toEqual<LyricWord[]>([
      { startSec: 12, text: '我' },
      { startSec: 12.3, text: '爱' },
      { startSec: 12.6, text: '你' },
    ])
    expect(findActiveWordIndex(lines[0].words, 12.45)).toBe(1)
    expect(findActiveWordIndex(lines[0].words, 12.6)).toBe(2)
  })

  it('cae al resaltado por línea cuando la fuente no trae timings de palabra', () => {
    const plain = parseLrc('[00:01.00] Hello there\n[00:05.00] Second line')

    expect(plain).toHaveLength(2)
    expect(plain[0].words).toEqual([])
    expect(plain[1].words).toEqual([])
    expect(plain[0].text).toBe('Hello there')
    // Sin segmentos, no hay palabra activa: manda la línea.
    expect(findActiveWordIndex(plain[0].words, 3)).toBe(-1)
    expect(findActiveLyricIndex(plain, 5)).toBe(1)
  })

  it('una línea con varias marcas de línea se expande sin inventar palabras', () => {
    const lines = parseLrc('[00:01.00][00:03.00]<00:01.00>a <00:01.50>b')

    expect(lines.map((line) => line.timeSec)).toEqual([1, 3])
    expect(lines.every((line) => line.text === 'a b')).toBe(true)
    expect(lines.every((line) => line.words.length === 0)).toBe(true)
  })

  it('rechaza timings de palabra desordenados en vez de adivinar', () => {
    const [line] = parseLrc('[00:01.00]<00:02.00>late <00:01.00>early')

    expect(line.text).toBe('late early')
    expect(line.words).toEqual([])
  })

  it('el texto previo a la primera palabra conserva su sitio y arranca con la línea', () => {
    const [line] = parseLrc('[00:01.00]Oh <00:01.50>well')

    expect(line.text).toBe('Oh well')
    expect(line.words).toEqual<LyricWord[]>([
      { startSec: 1, text: 'Oh ' },
      { startSec: 1.5, text: 'well' },
    ])
  })

  it('usa el inicio de la siguiente palabra como final del relleno', () => {
    const words = parseLrc(ENHANCED)[0].words

    expect(resolveWordEndSec(words, 0, 4.5)).toBe(1.3)
    expect(resolveWordEndSec(words, 1, 4.5)).toBe(1.6)
    expect(resolveWordEndSec([], 0, 4.5)).toBe(null)
    expect(resolveWordEndSec(words, -1, 4.5)).toBe(null)
  })

  it('la última palabra termina en la siguiente línea, no en un tiempo inventado', () => {
    const words = parseLrc(ENHANCED)[0].words
    const lastLineWords = parseLrc(ENHANCED)[1].words

    expect(resolveWordEndSec(words, 3, 4.5)).toBe(4.5)
    // Última palabra de la última línea: final desconocido.
    expect(resolveWordEndSec(lastLineWords, lastLineWords.length - 1, null)).toBe(null)
  })

  it('calcula el porcentaje cantado de la palabra activa', () => {
    expect(wordFillPercent(1.3, 1.6, 1.0)).toBe(0)
    expect(wordFillPercent(1.3, 1.6, 1.375)).toBe(25)
    expect(wordFillPercent(1.3, 1.6, 1.45)).toBe(50)
    expect(wordFillPercent(1.3, 1.6, 1.6)).toBe(100)
    expect(wordFillPercent(1.3, 1.6, 9)).toBe(100)
  })

  it('sin final conocido pinta la palabra entera al empezar, sin fabricar duración', () => {
    expect(wordFillPercent(2, null, 1.99)).toBe(0)
    expect(wordFillPercent(2, null, 2)).toBe(0)
    expect(wordFillPercent(2, null, 2.01)).toBe(100)
    expect(wordFillPercent(2, null, 30)).toBe(100)
  })
})

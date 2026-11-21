/**
 * LY10 (#181) — parser de Enhanced LRC y cálculo de la palabra/sílaba activa.
 *
 * Todo offline: los tiempos se inyectan a mano, sin reloj real, sin sleeps y sin
 * red. El contenido es el que el motor de letras (#179) ya almacena: Enhanced
 * LRC de LRCLib/Apple y KRC de Kugou convertido a `<mm:ss.xx>` en el backend.
 */

import { describe, it, expect } from 'vitest'
import {
  parseLrc,
  findActiveLyricIndex,
  findActiveWordIndex,
  resolveWordEndSec,
  wordFillPercent,
  type LyricWord,
} from '../../utils/lrcParser'

const ENHANCED = [
  '[00:01.00]<00:01.00>I <00:01.30>looked <00:01.60>at <00:02.00>you',
  '[00:04.50]<00:04.50>Same <00:04.90>as <00:05.10>before',
].join('\n')

/** KRC de Kugou: el backend lo convierte a segmentos por sílaba sin espacios. */
const KRC_AS_ELRC = [
  '[00:12.00]<00:12.00>我<00:12.30>爱<00:12.60>你',
  '[00:15.00]<00:15.00>这<00:15.25>是<00:15.50>爱',
].join('\n')

describe('parseLrc · Enhanced LRC con timings de palabra', () => {
  it('conserva el inicio de cada palabra y limpia las etiquetas del texto', () => {
    const lines = parseLrc(ENHANCED)

    expect(lines).toHaveLength(2)
    expect(lines[0].timeSec).toBe(1)
    expect(lines[0].text).toBe('I looked at you')
    expect(lines[0].words).toEqual<LyricWord[]>([
      { startSec: 1, text: 'I ' },
      { startSec: 1.3, text: 'looked ' },
      { startSec: 1.6, text: 'at ' },
      { startSec: 2, text: 'you' },
    ])

    // El texto renderizado nunca contiene etiquetas.
    expect(lines[0].text).not.toContain('<')
    expect(lines[0].text).not.toContain('[')
  })

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

  it('mantiene la línea activa alineada con las palabras', () => {
    const lines = parseLrc(ENHANCED)

    expect(findActiveLyricIndex(lines, 0.9)).toBe(-1)
    expect(findActiveLyricIndex(lines, 1.7)).toBe(0)
    expect(findActiveLyricIndex(lines, 4.5)).toBe(1)
  })

  it('la concatenación de segmentos reproduce el texto de la línea', () => {
    for (const line of parseLrc(ENHANCED)) {
      expect(line.words.map((w) => w.text).join('')).toBe(line.text)
    }
  })
})

describe('parseLrc · KRC (sílabas) convertido a Enhanced LRC', () => {
  it('trata cada sílaba como un segmento con su propio instante', () => {
    const lines = parseLrc(KRC_AS_ELRC)

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
})

describe('parseLrc · fallback explícito al resaltado por línea', () => {
  it('letras plain o de línea no producen segmentos de palabra', () => {
    const plain = parseLrc('[00:01.00] Hello there\n[00:05.00] Second line')

    expect(plain).toHaveLength(2)
    expect(plain[0].words).toEqual([])
    expect(plain[1].words).toEqual([])
    expect(plain[0].text).toBe('Hello there')
    // Sin segmentos, el cálculo de palabra activa cae a -1 (línea a línea).
    expect(findActiveWordIndex(plain[0].words, 3)).toBe(-1)
    // El resaltado por línea sigue funcionando igual que antes.
    expect(findActiveLyricIndex(plain, 5)).toBe(1)
  })

  it('una línea con varias marcas de línea se expande sin inventar palabras', () => {
    const lines = parseLrc('[00:01.00][00:03.00]<00:01.00>a <00:01.50>b')

    expect(lines.map((l) => l.timeSec)).toEqual([1, 3])
    expect(lines.every((l) => l.text === 'a b')).toBe(true)
    expect(lines.every((l) => l.words.length === 0)).toBe(true)
  })

  it('rechaza timings de palabra desordenados en vez de adivinar', () => {
    const [line] = parseLrc('[00:01.00]<00:02.00>late <00:01.00>early')

    expect(line.text).toBe('late early')
    expect(line.words).toEqual([])
  })

  it('sin marca de línea, arranca en la primera palabra', () => {
    const [line] = parseLrc('<00:02.00>Well <00:02.50>done')

    expect(line.timeSec).toBe(2)
    expect(line.text).toBe('Well done')
    expect(line.words.map((w) => w.startSec)).toEqual([2, 2.5])
  })

  it('el texto previo a la primera palabra conserva su sitio y arranca con la línea', () => {
    const [line] = parseLrc('[00:01.00]Oh <00:01.50>well')

    expect(line.text).toBe('Oh well')
    expect(line.words).toEqual<LyricWord[]>([
      { startSec: 1, text: 'Oh ' },
      { startSec: 1.5, text: 'well' },
    ])
  })
})

describe('resolución del final de palabra y relleno (wipe)', () => {
  const words = parseLrc(ENHANCED)[0].words

  it('usa el inicio de la siguiente palabra como final', () => {
    expect(resolveWordEndSec(words, 0, 4.5)).toBe(1.3)
    expect(resolveWordEndSec(words, 1, 4.5)).toBe(1.6)
  })

  it('la última palabra termina en la siguiente línea, no en un tiempo inventado', () => {
    expect(resolveWordEndSec(words, 3, 4.5)).toBe(4.5)
    // Última palabra de la última línea: final desconocido.
    const lastLineWords = parseLrc(ENHANCED)[1].words
    expect(resolveWordEndSec(lastLineWords, lastLineWords.length - 1, null)).toBe(null)
    expect(resolveWordEndSec([], 0, 4.5)).toBe(null)
    expect(resolveWordEndSec(words, -1, 4.5)).toBe(null)
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

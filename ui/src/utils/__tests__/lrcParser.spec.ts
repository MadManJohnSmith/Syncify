import { describe, it, expect } from 'vitest'
import { parseLrc, findActiveLyricIndex, formatClock } from '../lrcParser'

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
    expect(parsed[0]).toEqual({ timeSec: 1, label: '00:01.00', text: 'Line one' })
    expect(parsed[1]).toEqual({ timeSec: 4.5, label: '00:04.50', text: 'Line two' })
    expect(parsed[2]).toEqual({ timeSec: 10.2, label: '00:10.20', text: 'Line three' })
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
      { timeSec: 5, label: '00:05.00', text: 'Verse 1' },
      { timeSec: 12, label: '00:12.00', text: 'Verse 2' },
      { timeSec: 25, label: '00:25.00', text: 'Chorus' },
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
   * reproductor sincroniza por LÍNEA con los time tags, así que el texto se
   * entrega limpio: sin `<00:00.94>` incrustado.
   */
  it('limpia los time tags de palabra conservando el tiempo de línea', () => {
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
  })

  it('una línea sólo con time tags de palabra arranca en su primer tag (no se pierde)', () => {
    const parsed = parseLrc('<00:03.20>Hello <00:03.60>world')
    expect(parsed).toHaveLength(1)
    expect(parsed[0].timeSec).toBeCloseTo(3.2, 2)
    expect(parsed[0].text).toBe('Hello world')
  })

  it('no inventa líneas a partir de etiquetas de metadatos LRC', () => {
    expect(parseLrc('[ar:The Weeknd]\n[ti:Starboy]')).toEqual([])
  })
})

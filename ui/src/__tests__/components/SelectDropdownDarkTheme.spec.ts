import { describe, it, expect } from 'vitest'
import fs from 'node:fs'
import path from 'node:path'
import { mount } from '@vue/test-utils'
import BaseSelect from '@/components/settings/BaseSelect.vue'

/**
 * R8 — Desplegables legibles en Linux.
 *
 * WebKitGTK (Linux) y WebView2 (Windows) pintan el desplegable de un <select>
 * con la paleta del SISTEMA, no con la de la página. Sin `color-scheme` el popup
 * salía blanco mientras el propio <select> heredaba el texto blanco del tema
 * oscuro: blanco sobre blanco. Los 170 <option> de la app no llevan clase
 * propia, así que la corrección tiene que venir de la capa base global, no de
 * cada componente.
 *
 * Rediseño de temas (runtime): la app ya no sigue al tema del SISTEMA sino al
 * de la APP vía data-theme en <html>. Ahora es cada bloque de tema el que fija
 * `color-scheme` (3 oscuros + riso claro), y la regla global de select/option
 * se tokeniza (bg-surface + text-ink) para contrastar en los 4 temas. La
 * antigua pareja claro/`prefers-color-scheme: dark` se retiró: un tema claro
 * sobre un SO oscuro (y viceversa) la rompía.
 */
const css = fs.readFileSync(path.resolve(__dirname, '../../styles/main.css'), 'utf-8')

/** Devuelve el bloque `{...}` que empieza en `startIndex` (el índice del `{`). */
function blockAt(source: string, startIndex: number): string {
  let depth = 0
  for (let i = startIndex; i < source.length; i++) {
    if (source[i] === '{') depth++
    if (source[i] === '}') {
      depth--
      if (depth === 0) return source.slice(startIndex, i + 1)
    }
  }
  throw new Error('Bloque CSS sin cerrar')
}

function ruleBodies(source: string, selectorPattern: RegExp): string[] {
  const bodies: string[] = []
  const regex = new RegExp(selectorPattern.source, 'gm')
  let match: RegExpExecArray | null
  while ((match = regex.exec(source)) !== null) {
    const braceIndex = source.indexOf('{', match.index)
    if (braceIndex === -1) break
    bodies.push(blockAt(source, braceIndex))
  }
  return bodies
}

function sourceFiles(dir: string, acc: string[] = []): string[] {
  for (const entry of fs.readdirSync(dir, { withFileTypes: true })) {
    const full = path.join(dir, entry.name)
    if (entry.isDirectory()) {
      if (entry.name !== 'node_modules' && entry.name !== 'dist') sourceFiles(full, acc)
    } else if (entry.name.endsWith('.vue')) {
      acc.push(full)
    }
  }
  return acc
}

describe('Desplegables de la app en tema oscuro (R8)', () => {
  it('cada tema declara color-scheme para que el popup nativo siga al tema de la app', () => {
    // Camaleón, tinta y consola son oscuros; riso es claro.
    const darkCount = (css.match(/color-scheme:\s*dark/g) ?? []).length
    expect(darkCount, 'los 3 temas oscuros deben declarar color-scheme: dark').toBeGreaterThanOrEqual(3)
    const risoIndex = css.indexOf('[data-theme="riso"]')
    expect(risoIndex).toBeGreaterThan(-1)
    const risoBlock = blockAt(css, css.indexOf('{', risoIndex))
    expect(risoBlock).toMatch(/color-scheme:\s*light/)
  })

  it('da fondo y color propios a select y a sus option con tokens del tema activo', () => {
    const bodies = ruleBodies(css, /select,\s*select option\s*\{/).filter((rule) =>
      /bg-surface/.test(rule)
    )
    expect(bodies.length).toBeGreaterThan(0)
    // Tokens, no hex: la misma regla contrasta en los 4 temas.
    expect(bodies[0]).toMatch(/text-ink/)
    expect(bodies[0]).not.toMatch(/bg-white|text-white|text-gray-900/)
  })

  it('ya no depende de prefers-color-scheme: el tema manda, no el SO', () => {
    expect(css.includes('@media (prefers-color-scheme'), 'los selects no deben colgarse del tema del SO').toBe(false)
  })

  it('ningún <option> de la app se salta la regla global con su propio fondo o color de texto', () => {
    const files = sourceFiles(path.resolve(__dirname, '../../'))
    expect(files.length).toBeGreaterThan(0)

    const offenders: string[] = []
    for (const file of files) {
      const content = fs.readFileSync(file, 'utf-8')
      for (const match of content.matchAll(/<option\b[^>]*>/g)) {
        const classes = match[0].match(/class="([^"]*)"/)?.[1] ?? ''
        // Solo la combinación que reproduce el popup blanco con texto claro:
        // un <option> puede llevar clases de otra cosa sin romper nada.
        if (/\bbg-|\btext-(white|black|gray-900)\b/.test(classes)) {
          offenders.push(`${file}: ${match[0]}`)
        }
      }
    }
    expect(offenders).toEqual([])
  })

  it('BaseSelect deja los option sin estilo propio para que hereden la capa base', async () => {
    const wrapper = mount(BaseSelect, {
      props: {
        modelValue: 'a',
        options: ['a', 'b', { label: 'C', value: 'c' }],
      },
    })
    const options = wrapper.findAll('option')
    expect(options).toHaveLength(3)
    for (const option of options) {
      expect(option.attributes('style')).toBeUndefined()
      expect(option.attributes('class')).toBeUndefined()
    }
  })
})
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
  it('declara color-scheme en :root para que el popup siga al tema del sistema', () => {
    const rootRules = ruleBodies(css, /:root\s*\{/)
    expect(rootRules.length).toBeGreaterThan(0)
    const declarations = rootRules.map((rule) => rule.replace(/\{|\}/g, '')).join(' ')
    expect(declarations).toMatch(/color-scheme:\s*light\s+dark/)
  })

  it('da fondo y color propios a select y a sus option en claro', () => {
    const bodies = ruleBodies(css, /select,\s*select option\s*\{/).filter((rule) =>
      /bg-white/.test(rule)
    )
    expect(bodies.length).toBeGreaterThan(0)
    const rule = bodies[0]
    expect(rule).toMatch(/text-gray-900/)
  })

  it('repite el fondo de select y option en prefers-color-scheme: dark', () => {
    const mediaIndex = css.indexOf('@media (prefers-color-scheme: dark)')
    expect(mediaIndex, 'main.css debe tener un bloque para el tema oscuro del sistema').toBeGreaterThan(-1)
    const media = blockAt(css, css.indexOf('{', mediaIndex))
    const darkRules = ruleBodies(media, /select,\s*select option\s*\{/)
    expect(darkRules.length).toBeGreaterThan(0)
    // El texto del option sigue siendo el de la app: sin este fondo, el popup
    // sale con la paleta blanca del sistema y el texto blanco desaparece.
    expect(darkRules[0]).toMatch(/bg-surface-dark/)
    expect(darkRules[0]).toMatch(/text-white/)
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
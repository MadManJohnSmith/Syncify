// FE-PERF-01 (#49): contrato del <KeepAlive> acotado.
//
// 1. El conjunto incluido es una lista FIJA y acotada (nunca global, nunca
//    dinámica sin límite) y cada nombre responde a una vista real.
// 2. Los nombres del include son EXACTAMENTE el `name` que cada vista
//    incluida declara con `defineOptions` (el include filtra por `name`;
//    si divergen, la vista se descachea en silencio).
// 3. App.vue siempre enlaza `:include` y `:max` — un KeepAlive sin include
//    sería global y retendría todas las vistas.
// 4. La decisión de transición para las vistas cacheadas está explícita en
//    el árbol (`mode="out-in"` se mantiene) y queda fijada aquí.
import { describe, it, expect } from 'vitest'
import { readFileSync } from 'node:fs'
import { resolve } from 'node:path'
import { KEEP_ALIVE_INCLUDE, KEEP_ALIVE_MAX } from '@/keepAliveViews'

// Vitest sirve los módulos con un esquema no-file: se resuelve desde el cwd
// de la suite (raíz de ui/).
const appSource = readFileSync(resolve(process.cwd(), 'src/App.vue'), 'utf8')

function viewSource(name: string): string {
  return readFileSync(resolve(process.cwd(), `src/views/${name}.vue`), 'utf8')
}

describe('KeepAlive acotado (FE-PERF-01)', () => {
  it('el conjunto incluido es una lista fija, no vacía y acotada', () => {
    expect(Array.isArray(KEEP_ALIVE_INCLUDE)).toBe(true)
    expect(KEEP_ALIVE_INCLUDE.length).toBeGreaterThan(0)
    // Techo duro: retener más de 6 vistas degrada la memoria (user story 2).
    expect(KEEP_ALIVE_INCLUDE.length).toBeLessThanOrEqual(6)
    expect(new Set(KEEP_ALIVE_INCLUDE).size).toBe(KEEP_ALIVE_INCLUDE.length)
    for (const name of KEEP_ALIVE_INCLUDE) {
      expect(typeof name).toBe('string')
      // Un include por prefijo/expresión regular no es un conjunto acotado de
      // vistas concretas: cada entrada es el nombre exacto de un componente.
      expect(name).toMatch(/^[A-Z][A-Za-z]+View$/)
    }
    // El límite de retención (`:max`) es estructural: no puede haber más
    // instancias retenidas que entradas en el include.
    expect(KEEP_ALIVE_MAX).toBe(KEEP_ALIVE_INCLUDE.length)
  })

  it('cada nombre del include existe como vista y declara ese `name` con defineOptions', () => {
    for (const name of KEEP_ALIVE_INCLUDE) {
      const source = viewSource(name)
      expect(source, `ui/src/views/${name}.vue debe existir`).toContain('<script setup')
      expect(
        source,
        `ui/src/views/${name}.vue debe declarar defineOptions({ name: '${name}' }) para que el include lo cachee`,
      ).toContain(`defineOptions({ name: '${name}' })`)
    }
  })

  it('App.vue enlaza :include y :max — nunca un KeepAlive global', () => {
    expect(appSource).toContain('<KeepAlive')
    expect(appSource).toContain(':include="KEEP_ALIVE_INCLUDE"')
    expect(appSource).toContain(':max="KEEP_ALIVE_MAX"')
    // Un `<KeepAlive>` sin include sería global: cada apertura de KeepAlive
    // en el árbol debe llevar el binding acotado.
    for (const match of appSource.matchAll(/<KeepAlive[^>]*>/g)) {
      expect(match[0], 'todo <KeepAlive> debe llevar :include acotado').toContain(':include=')
      expect(match[0], 'todo <KeepAlive> debe llevar :max').toContain(':max=')
    }
  })

  it('las vistas de detalle con params NO están cacheadas (instancia por param)', () => {
    // KeepAlive cachea por nombre de componente: navegar de un álbum a otro
    // reutilizaría la instancia cacheada sin remontar y mostraría el álbum
    // anterior. AlbumDetail/ArtistDetail quedan fuera del include por diseño.
    expect(KEEP_ALIVE_INCLUDE).not.toContain('AlbumDetailView')
    expect(KEEP_ALIVE_INCLUDE).not.toContain('ArtistDetailView')
  })

  it('la decisión de transición (mode="out-in" se mantiene) está explícita en App.vue', () => {
    // La decisión documentada (#49 / #34 user story 6) es mantener out-in
    // también en las vistas cacheadas; si algún día cambia, este test debe
    // cambiarse a la vez que la decisión documentada en App.vue.
    expect(appSource).toContain('mode="out-in"')
    expect(appSource).toContain('DECISIÓN de transición')
  })
})

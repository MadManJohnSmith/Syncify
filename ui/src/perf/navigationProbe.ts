/**
 * FE-PERF-01 (#49): instrumentación de navegación para la VENTANA REAL.
 *
 * Este módulo NO forma parte del bundle de producción: se carga con
 * `import()` dinámico desde `main.ts` solo cuando se cumplen TODAS estas
 * condiciones:
 *
 *   1. build de desarrollo (`import.meta.env.DEV`).
 *   2. la app corre dentro de Tauri (`__TAURI_INTERNALS__` presente), no en
 *      jsdom ni en un navegador suelto.
 *   3. se pidió la medición: `VITE_NAVPERF=1` en el entorno del dev server
 *      o `?__navperf=1` en la URL.
 *
 * En producción y en los tests de Vitest el flag no llega a evaluarse como
 * verdadero, así que el chunk ni siquiera se descarga.
 *
 * Qué mide:
 *   - Latencia de navegación: desde que el router confirma el cambio de ruta
 *     hasta el segundo `requestAnimationFrame` de la vista destino (primer
 *     frame con contenido), incluyendo la transición `out-in` real.
 *   - Si la vista destino se MONTÓ de nuevo o se REACTIVÓ desde KeepAlive:
 *     un mixin global cuenta `mounted`/`activated` por nombre de componente.
 *   - Llamadas IPC por comando: envuelve `__TAURI_INTERNALS__.invoke`.
 *
 * El resultado se publica por tres canales para que el harness de shell lo
 * recoja sin tocar datos del usuario: un overlay en pantalla (artefacto
 * gráfico), `console.log('[navperf]', ...)` y `document.title` con un
 * resumen compacto (`NAVPERF-DONE {...}`).
 */

import type { App } from 'vue'
import type { Router } from 'vue-router'

export interface NavSample {
  from: string
  to: string
  /** `mount` = la vista se montó de nuevo; `cached` = se reactivó desde KeepAlive. */
  kind: 'mount' | 'cached' | 'unknown'
  ms: number
  /** Las primeras vueltas calientan parseo de chunks y se excluyen del resumen. */
  warmup: boolean
}

export interface NavPairSummary {
  pair: string
  kind: 'mount' | 'cached' | 'mixed'
  n: number
  p50: number
  p95: number
  max: number
}

/** Vistas de la secuencia de medición: los cuatro hubs del sidebar. */
const SEQUENCE = ['Dashboard', 'Library', 'Downloads', 'Settings'] as const
/**
 * Mapa nombre de ruta -> nombre de componente (el `include` de KeepAlive y el
 * mixin de conteo usan el nombre del COMPONENTE, no el de la ruta).
 */
const ROUTE_TO_COMPONENT: Record<string, string> = {
  Dashboard: 'DashboardView',
  Library: 'LibraryView',
  Downloads: 'DownloadsView',
  Settings: 'SettingsView',
}
const WARMUP_ROUNDS = 2
const MEASURED_ROUNDS = 10
/**
 * Muestras de calentamiento: WARMUP_ROUNDS vueltas completas x 4 navegaciones
 * (la primera vuelta incluye el push duplicado Dashboard->Dashboard, que el
 * router no aborta y llega a afterEach; se mide con timeout como 'unknown').
 */
const WARMUP_SAMPLES = WARMUP_ROUNDS * SEQUENCE.length
const SETTLE_MS = 600
const SPLASH_TIMEOUT_MS = 45_000

interface ProbeState {
  samples: NavSample[]
  mountedCounts: Map<string, number>
  activatedCounts: Map<string, number>
  invokeCounts: Record<string, number>
  done: boolean
}

function bump(map: Map<string, number>, key: string | undefined): void {
  if (!key) return
  map.set(key, (map.get(key) ?? 0) + 1)
}

function quantile(sorted: number[], q: number): number {
  if (sorted.length === 0) return 0
  const idx = Math.min(sorted.length - 1, Math.ceil(q * sorted.length) - 1)
  return sorted[Math.max(0, idx)]
}

function summarize(samples: NavSample[]): NavPairSummary[] {
  const groups = new Map<string, NavSample[]>()
  for (const s of samples) {
    const key = `${s.from}->${s.to}`
    const arr = groups.get(key) ?? []
    arr.push(s)
    groups.set(key, arr)
  }
  const out: NavPairSummary[] = []
  for (const [pair, arr] of groups) {
    const durations = arr.map((s) => s.ms).sort((a, b) => a - b)
    const kinds = new Set(arr.map((s) => s.kind))
    const kind = kinds.size === 1 ? arr[0].kind : 'mixed'
    out.push({
      pair,
      kind: kind as NavPairSummary['kind'],
      n: arr.length,
      p50: quantile(durations, 0.5),
      p95: quantile(durations, 0.95),
      max: durations[durations.length - 1] ?? 0,
    })
  }
  return out.sort((a, b) => a.pair.localeCompare(b.pair))
}

/** POST de progreso al collector (diagnóstico del harness sin consola webview). */
function trace(stage: string, extra: Record<string, unknown> = {}): void {
  try {
    void fetch('/__navperf_collect', {
      method: 'POST',
      headers: { 'content-type': 'application/json' },
      body: JSON.stringify({ stage, ts: new Date().toISOString(), ...extra }),
    }).catch(() => undefined)
  } catch {
    /* el trace es best-effort */
  }
}

/**
 * Instala la sonda en la app. Devuelve false (y no toca nada) si el entorno
 * no es la ventana real de desarrollo con la medición pedida.
 */
export function installNavigationProbe(app: App, router: Router): boolean {
  const w = window as unknown as { __TAURI_INTERNALS__?: { invoke: (...args: unknown[]) => unknown } }
  const inTauri = typeof w.__TAURI_INTERNALS__?.invoke === 'function'
  const requested =
    import.meta.env.VITE_NAVPERF === '1' ||
    (typeof window !== 'undefined' && window.location.search.includes('__navperf=1'))
  trace('probe-module-loaded', { inTauri, requested, viteNavperf: String(import.meta.env.VITE_NAVPERF) })
  // FE-PERF-03 (#50): origen de página en epoch (ms). Con el ts ISO de
  // 'first-view-ready' permite derivar el tiempo de ARRANQUE (página carga ->
  // primera vista pintada) sin tocar el harness ni la definición de FE-PERF-01.
  trace('page-origin', { timeOrigin: performance.timeOrigin })
  if (!inTauri || !requested) return false

  // Errores globales de la página: visibles en el log del harness. Van ANTES
  // del cuerpo de instalación para que un fallo en él sea visible.
  window.addEventListener('error', (e) => trace('window-error', { message: String(e.message) }))
  window.addEventListener('unhandledrejection', (e) => trace('unhandled-rejection', { message: String(e.reason) }))

  try {
    installProbe(app, router)
  } catch (err) {
    trace('probe-install-error', { message: String(err) })
    return false
  }
  return true
}

function installProbe(app: App, router: Router): void {
  const w = window as unknown as { __TAURI_INTERNALS__?: { invoke: (...args: unknown[]) => unknown } }

  const state: ProbeState = {
    samples: [],
    mountedCounts: new Map(),
    activatedCounts: new Map(),
    invokeCounts: {},
    done: false,
  }

  // Contador de IPC: envuelve el invoke nativo. Solo en esta sesión de medición.
  // El objeto __TAURI_INTERNALS__ llega congelado en WebKit (asignación directa
  // => "readonly property"): se reintenta con defineProperty y, si tampoco
  // deja, la medición sigue sin contador de IPC (el criterio de llamadas
  // repetidas lo cubren los tests de comportamiento y el kind de cada muestra).
  const internals = w.__TAURI_INTERNALS__!
  const originalInvoke = internals.invoke.bind(internals)
  const countingInvoke = (cmd: unknown, ...rest: unknown[]) => {
    if (typeof cmd === 'string') state.invokeCounts[cmd] = (state.invokeCounts[cmd] ?? 0) + 1
    return originalInvoke(cmd, ...rest)
  }
  let ipcCounterActive = false
  try {
    Object.defineProperty(internals, 'invoke', {
      value: countingInvoke,
      writable: false,
      configurable: true,
    })
    ipcCounterActive = true
  } catch {
    trace('ipc-counter-skipped', { reason: 'invoke no redefinible en este webview' })
  }

  // Contador de montaje/reactivación por nombre de componente (lo que el
  // `include` de KeepAlive usa como clave).
  let mixinTraces = 0
  app.mixin({
    mounted() {
      const self = this as unknown as { $options: { name?: string; __name?: string } }
      const name = self.$options?.name ?? self.$options?.__name
      if (mixinTraces < 6) {
        mixinTraces++
        trace('mixin-mounted', { name })
      }
      bump(state.mountedCounts, name)
    },
    activated() {
      const self = this as unknown as { $options: { name?: string; __name?: string } }
      bump(state.activatedCounts, self.$options?.name ?? self.$options?.__name)
    },
  })

  // afterEach corre tras confirmar la navegación. Con `mode="out-in"` la
  // vista destino no llega a montar hasta que termina la salida de la vista
  // anterior (~150 ms), así que un doble rAF inmediato mediría solo la fase
  // de salida. Se SONDEA el DOM: la vista destino está montada cuando la raíz
  // de `#page-content` cambia de elemento. Entonces el doble rAF marca su
  // primer frame con contenido. Definición medida: confirmación de
  // navegación -> primer frame con la vista destino pintada.
  //
  // El `kind` NO depende de nombres de componente (verificado: en este
  // webview el mixin global no ve montajes de DashboardView): KeepAlive
  // REUTILIZA los mismos nodos DOM al reactivar una vista cacheada, así que
  // una raíz ya vista antes es 'cached' y una raíz nueva es 'mount'.
  const seenRoots = new Set<Element>()
  const contentHost = (): HTMLElement | null => document.getElementById('page-content')
  {
    const initial = contentHost()?.firstElementChild
    if (initial) seenRoots.add(initial)
  }

  let samplePoll: ReturnType<typeof setInterval> | null = null
  router.afterEach((to, from) => {
    const toName = String(to.name ?? to.path)
    const fromName = String(from.name ?? from.path)
    if (samplePoll) clearInterval(samplePoll)
    const pending = {
      from: fromName,
      to: toName,
      t0: performance.now(),
      warmup: (state.samples.length < WARMUP_SAMPLES),
    }
    const previousRoot = contentHost()?.firstElementChild ?? null
    const startedAt = performance.now()
    samplePoll = setInterval(() => {
      const currentRoot = contentHost()?.firstElementChild ?? null
      const arrived = currentRoot != null && currentRoot !== previousRoot
      const timedOut = performance.now() - startedAt > 2_500
      if (!arrived && !timedOut) return

      if (samplePoll) {
        clearInterval(samplePoll)
        samplePoll = null
      }
      const kind: NavSample['kind'] = arrived
        ? (seenRoots.has(currentRoot!) ? 'cached' : 'mount')
        : 'unknown'
      if (arrived) seenRoots.add(currentRoot!)
      requestAnimationFrame(() => {
        requestAnimationFrame(() => {
          state.samples.push({
            from: pending.from,
            to: pending.to,
            kind,
            ms: performance.now() - pending.t0,
            warmup: pending.warmup,
          })
        })
      })
    }, 30)
  })

  // Perfil desechable: sin completar el onboarding no aparece el asistente
  // durante la secuencia (las claves solo existen en el perfil de medición).
  try {
    window.localStorage.setItem('syncify_onboarding_completed', 'true')
    window.localStorage.setItem('syncify_onboarding_complete', 'true')
  } catch {
    /* sin localStorage no hay onboarding que silenciar */
  }

  void runSequence(router, state).catch((err) => {
    trace('probe-sequence-error', { message: String(err) })
  })
  exposeDebugApi(state)
}

async function runSequence(router: Router, state: ProbeState): Promise<void> {
  trace('probe-installed')
  await waitForFirstView()
  trace('first-view-ready')
  await sleep(1500) // deja que el app termine su init y se asiente

  for (let round = 0; round < WARMUP_ROUNDS + MEASURED_ROUNDS; round++) {
    for (const dest of SEQUENCE) {
      await router.push({ name: dest })
      await sleep(SETTLE_MS)
    }
  }

  await sleep(SETTLE_MS)
  state.done = true
  trace('sequence-complete', {
    samples: state.samples.length,
    mountedCounts: Array.from(state.mountedCounts.entries()),
    activatedCounts: Array.from(state.activatedCounts.entries()),
  })
  publishResults(state)
}

async function waitForFirstView(): Promise<void> {
  const started = Date.now()
  while (Date.now() - started < SPLASH_TIMEOUT_MS) {
    // La splash se va cuando initializeApp termina; el router-view pinta
    // antes, así que esperamos a que la splash desaparezca del DOM.
    if (!document.querySelector('.splash-screen')) return
    await sleep(250)
  }
}

function sleep(ms: number): Promise<void> {
  return new Promise((resolve) => setTimeout(resolve, ms))
}

function publishResults(state: ProbeState): void {
  const measured = state.samples.filter((s) => !s.warmup)
  const summary = summarize(measured)

  console.log('[navperf] samples:', JSON.stringify(state.samples, null, 1))
  console.log('[navperf] summary (sin warmup):', JSON.stringify(summary, null, 1))
  console.log('[navperf] ipc:', JSON.stringify(state.invokeCounts, null, 1))

  const payload = {
    label: new URLSearchParams(window.location.search).get('__navperf_label') ?? 'run',
    finishedAt: new Date().toISOString(),
    // FE-PERF-03 (#50): origen de página (epoch ms) para derivar arranque.
    pageTimeOrigin: Math.round(performance.timeOrigin),
    samples: state.samples,
    summary,
    ipc: state.invokeCounts,
  }

  // POST al collector: JSON en UNA línea (el harness lee el fichero línea a
  // línea; los hitos son una línea, el resultado final es otra).
  void fetch('/__navperf_collect', {
    method: 'POST',
    headers: { 'content-type': 'application/json' },
    body: JSON.stringify(payload),
  }).catch(() => undefined)

  // Resumen compacto y overlay: canales secundarios para inspección en vivo.
  const compact: Record<string, { kind: string; n: number; p50: number; p95: number; max: number }> = {}
  for (const s of summary) {
    compact[s.pair] = { kind: s.kind, n: s.n, p50: Math.round(s.p50 * 10) / 10, p95: Math.round(s.p95 * 10) / 10, max: Math.round(s.max * 10) / 10 }
  }
  try {
    document.title = `NAVPERF-DONE ${JSON.stringify(compact)}`
  } catch {
    /* el título es solo un canal de extracción; si falla quedan el overlay y el log */
  }

  renderOverlay(state, summary)
}

function renderOverlay(state: ProbeState, summary: NavPairSummary[]): void {
  if (document.getElementById('navperf-overlay')) return
  const el = document.createElement('div')
  el.id = 'navperf-overlay'
  el.style.cssText =
    'position:fixed;bottom:12px;right:12px;z-index:2147483647;background:rgba(10,12,18,0.94);color:#d6e2ff;' +
    'font:11px/1.5 monospace;padding:10px 14px;border-radius:8px;border:1px solid #2a3350;max-height:80vh;overflow:auto;white-space:pre'

  const rows = summary
    .map((s) => `${s.pair}  kind=${s.kind}  n=${s.n}  p50=${s.p50.toFixed(1)}ms  p95=${s.p95.toFixed(1)}ms  max=${s.max.toFixed(1)}ms`)
    .join('\n')
  const ipc = Object.entries(state.invokeCounts)
    .filter(([cmd, n]) => n > 0 && /^(get_dashboard_snapshot|create_library_snapshot|get_library_stats|get_library|get_queue_stats|get_top_artists|get_top_genres)/.test(cmd))
    .map(([cmd, n]) => `  ${cmd}: ${n}`)
    .join('\n')
  el.textContent = `FE-PERF-01 navegación (sin warmup, ${state.samples.length - countWarmup(state)} muestras)\n${rows}\nIPC relevantes:\n${ipc}`
  document.body.appendChild(el)
}

function countWarmup(state: ProbeState): number {
  return state.samples.filter((s) => s.warmup).length
}

/** API en consola para inspección manual: window.__navPerf.dump() */
export function exposeDebugApi(state: ProbeState): void {
  ;(window as unknown as Record<string, unknown>).__navPerf = {
    samples: () => state.samples,
    summary: () => summarize(state.samples.filter((s) => !s.warmup)),
    ipc: () => state.invokeCounts,
    dump: () => JSON.stringify({ samples: state.samples, ipc: state.invokeCounts }, null, 1),
  }
}

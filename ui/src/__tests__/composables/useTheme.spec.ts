import { describe, it, expect, beforeEach } from 'vitest'
import { readFileSync } from 'node:fs'
import { resolve } from 'node:path'
import { runInNewContext } from 'node:vm'
import { mount } from '@vue/test-utils'
import {
  useTheme,
  STANDARD_THEMES,
  SERVICE_THEMES,
  THEMES,
  STORAGE_KEY,
  DYNAMIC_COVER_KEY,
  isServiceTheme,
} from '@/composables/useTheme'
import SettingsAppearance from '@/views/settings/SettingsAppearance.vue'
import { mockInvoke, resetMocks } from '../setup'

describe('useTheme composable and appearance settings (G-02, AJ-05, AJ-06)', () => {
  beforeEach(() => {
    resetMocks()
    localStorage.clear()
    delete document.documentElement.dataset.theme
    delete document.documentElement.dataset.dynamicCover
  })

  it('first frame matches composable default and preserves each service theme', () => {
    const html = readFileSync(resolve(process.cwd(), 'index.html'), 'utf8')
    const script = html.match(/<script>([\s\S]*?)<\/script>/)?.[1]
    expect(script).toBeDefined()
    for (const stored of [null, 'not-a-theme', ...SERVICE_THEMES.map(theme => theme.id)]) {
      const root = { dataset: {} as Record<string, string> }
      runInNewContext(script!, {
        localStorage: { getItem: () => stored },
        document: { documentElement: root },
      })
      expect(root.dataset.theme).toBe(stored && SERVICE_THEMES.some(theme => theme.id === stored) ? stored : 'tinta')
    }
  })

  it('service themes define different full token palettes', () => {
    const css = readFileSync(resolve(process.cwd(), 'src/styles/main.css'), 'utf8')
    const palettes = SERVICE_THEMES.map(theme => {
      const block = css.match(new RegExp(`\\[data-theme="${theme.id}"\\] \\{([^}]+)\\}`))?.[1]
      expect(block).toBeDefined()
      for (const token of ['--bg-base', '--surface-1', '--text-1', '--accent', '--on-accent']) {
        expect(block).toContain(token + ':')
      }
      return block!.match(/--accent:\s*([^;]+)/)?.[1]
    })
    expect(new Set(palettes).size).toBe(SERVICE_THEMES.length)
  })

  it('G-02: defaults to "tinta" when no theme is stored or preset', () => {
    const { current } = useTheme()
    expect(current.value).toBe('tinta')
    expect(document.documentElement.dataset.theme).toBe('tinta')
  })

  it('AJ-05: exposes both standard themes and service themes', () => {
    const { themes, standardThemes, serviceThemes } = useTheme()
    expect(standardThemes.map(t => t.id)).toEqual(['tinta', 'camaleon', 'consola', 'riso'])
    expect(serviceThemes.map(t => t.id)).toEqual([
      'spotify',
      'apple_music',
      'tidal',
      'qobuz',
      'deezer',
      'soundcloud',
    ])
    expect(themes.length).toBe(standardThemes.length + serviceThemes.length)
    expect(isServiceTheme('spotify')).toBe(true)
    expect(isServiceTheme('apple_music')).toBe(true)
    expect(isServiceTheme('tidal')).toBe(true)
    expect(isServiceTheme('qobuz')).toBe(true)
    expect(isServiceTheme('deezer')).toBe(true)
    expect(isServiceTheme('soundcloud')).toBe(true)
    expect(isServiceTheme('tinta')).toBe(false)
  })

  it('AJ-05 & Persistence: setting a service theme updates DOM, localStorage and invokes save_setting', async () => {
    const invokeCalls: Array<{ cmd: string; args?: Record<string, unknown> }> = []
    mockInvoke((cmd, args) => {
      invokeCalls.push({ cmd, args })
      return {}
    })

    const { setTheme, current } = useTheme()
    setTheme('spotify')

    expect(current.value).toBe('spotify')
    expect(document.documentElement.dataset.theme).toBe('spotify')
    expect(localStorage.getItem(STORAGE_KEY)).toBe('spotify')

    const saveCall = invokeCalls.find(
      c => c.cmd === 'save_setting' && c.args?.key === STORAGE_KEY,
    )
    expect(saveCall).toBeDefined()
    expect(saveCall?.args?.value).toBe('spotify')
  })

  it('AJ-06: toggles dynamic cover art tinting and persists to localStorage and backend', () => {
    const invokeCalls: Array<{ cmd: string; args?: Record<string, unknown> }> = []
    mockInvoke((cmd, args) => {
      invokeCalls.push({ cmd, args })
      return {}
    })

    const { dynamicCoverEnabled, setDynamicCover } = useTheme()
    expect(dynamicCoverEnabled.value).toBe(false)

    setDynamicCover(true)
    expect(dynamicCoverEnabled.value).toBe(true)
    expect(document.documentElement.dataset.dynamicCover).toBe('true')
    expect(localStorage.getItem(DYNAMIC_COVER_KEY)).toBe('true')

    const saveCall = invokeCalls.find(
      c => c.cmd === 'save_setting' && c.args?.key === DYNAMIC_COVER_KEY,
    )
    expect(saveCall).toBeDefined()
    expect(saveCall?.args?.value).toBe('true')

    setDynamicCover(false)
    expect(dynamicCoverEnabled.value).toBe(false)
    expect(document.documentElement.dataset.dynamicCover).toBe('false')
    expect(localStorage.getItem(DYNAMIC_COVER_KEY)).toBe('false')
  })

  it('Persistence: syncFromBackend restores theme and dynamic cover from SQLite KV store', async () => {
    mockInvoke((cmd, args) => {
      if (cmd === 'get_kv_settings') {
        const keys = (args?.keys as string[]) || []
        const res: Record<string, string> = {}
        if (keys.includes(STORAGE_KEY)) res[STORAGE_KEY] = 'tidal'
        if (keys.includes(DYNAMIC_COVER_KEY)) res[DYNAMIC_COVER_KEY] = 'true'
        return res
      }
      return {}
    })

    const { syncFromBackend, current, dynamicCoverEnabled } = useTheme()
    await syncFromBackend()

    expect(current.value).toBe('tidal')
    expect(document.documentElement.dataset.theme).toBe('tidal')
    expect(localStorage.getItem(STORAGE_KEY)).toBe('tidal')

    expect(dynamicCoverEnabled.value).toBe(true)
    expect(document.documentElement.dataset.dynamicCover).toBe('true')
    expect(localStorage.getItem(DYNAMIC_COVER_KEY)).toBe('true')
  })

  it('SettingsAppearance.vue: renders separate sections for standard and service themes, plus dynamic cover switch', async () => {
    const wrapper = mount(SettingsAppearance)

    // Standard themes section
    const standardSection = wrapper.find('[data-testid="appearance-themes"]')
    expect(standardSection.exists()).toBe(true)
    expect(standardSection.findAll('article').length).toBe(STANDARD_THEMES.length)
    expect(wrapper.find('[data-testid="theme-card-tinta"]').exists()).toBe(true)

    // Service themes section
    const serviceSection = wrapper.find('[data-testid="appearance-service-themes"]')
    expect(serviceSection.exists()).toBe(true)
    expect(serviceSection.findAll('article').length).toBe(SERVICE_THEMES.length)
    expect(wrapper.find('[data-testid="theme-card-spotify"]').exists()).toBe(true)
    expect(wrapper.find('[data-testid="theme-card-apple_music"]').exists()).toBe(true)
    expect(wrapper.find('[data-testid="theme-card-tidal"]').exists()).toBe(true)
    expect(wrapper.find('[data-testid="theme-card-qobuz"]').exists()).toBe(true)
    expect(wrapper.find('[data-testid="theme-card-deezer"]').exists()).toBe(true)
    expect(wrapper.find('[data-testid="theme-card-soundcloud"]').exists()).toBe(true)

    // Dynamic cover toggle
    const toggle = wrapper.find('[data-testid="toggle-dynamic-cover"]')
    expect(toggle.exists()).toBe(true)

    const initialChecked = toggle.attributes('aria-checked') === 'true'
    await toggle.trigger('click')
    expect(toggle.attributes('aria-checked')).toBe(String(!initialChecked))

    // Selecting a service theme from the card
    const qobuzBtn = wrapper.find('[data-testid="theme-card-qobuz"] button')
    await qobuzBtn.trigger('click')
    expect(document.documentElement.dataset.theme).toBe('qobuz')
  })
})

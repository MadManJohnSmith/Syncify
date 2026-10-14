/**
 * Unit tests for SettingsQuality.vue (AQ-01, FS-02)
 * Tests global download-quality ceiling and per-service quality preferences
 */
import { describe, it, expect, beforeEach, vi } from 'vitest'
import { mount, flushPromises } from '@vue/test-utils'
import SettingsQuality from '../../views/settings/SettingsQuality.vue'
import { mockInvoke, resetMocks } from '../setup'
import { invoke } from '@tauri-apps/api/core'

const mockQualityPreferences = [
  {
    id: 1,
    service_name: 'qobuz',
    max_quality: 'hires',
    preferred_format: 'flac',
    fallback_quality: 'high',
    fallback_format: 'mp3',
  },
  {
    id: 2,
    service_name: 'tidal',
    max_quality: 'lossless',
    preferred_format: 'flac',
    fallback_quality: 'high',
    fallback_format: 'mp3',
  },
  {
    id: 3,
    service_name: 'spotify',
    max_quality: 'high',
    preferred_format: 'mp3',
    fallback_quality: 'normal',
    fallback_format: 'ogg',
  }
]

describe('SettingsQuality.vue (AQ-01, FS-02)', () => {
  beforeEach(() => {
    resetMocks()
    mockInvoke((command) => {
      if (command === 'get_global_max_quality') return 'lossless'
      if (command === 'set_global_max_quality') return 'high'
      if (command === 'get_quality_preferences') return mockQualityPreferences
      if (command === 'get_available_qualities_for_service') return ['inherit', 'high', 'lossless']
      if (command === 'update_quality_preference') return mockQualityPreferences[0]
      return null
    })
  })

  it('renders global ceiling selector with all canonical options and per-service caps', async () => {
    const wrapper = mount(SettingsQuality)
    await flushPromises()

    expect(wrapper.text()).toContain('Global maximum quality')
    expect(wrapper.text()).toContain('Effective download ceiling')
    expect(wrapper.text()).toContain('Quality Caps (Per Service)')

    const ceilingSelect = wrapper.find('select[data-testid="global-max-quality-select"]')
    expect(ceilingSelect.exists()).toBe(true)

    const options = ceilingSelect.findAll('option')
    expect(options.map(o => o.attributes('value'))).toEqual(['any', 'hires', 'lossless', 'high'])
  })

  it('loads and reflects the persisted global ceiling from backend', async () => {
    const wrapper = mount(SettingsQuality)
    await flushPromises()

    expect(invoke).toHaveBeenCalledWith('get_global_max_quality')
    const ceilingSelect = wrapper.find('select[data-testid="global-max-quality-select"]')
    expect((ceilingSelect.element as HTMLSelectElement).value).toBe('lossless')

    const ceilingText = wrapper.find('[data-testid="effective-ceiling-text"]')
    expect(ceilingText.exists()).toBe(true)
    expect(ceilingText.text()).toContain('lossless')
    // AQ-01: no longer tells the user to go to another screen
    expect(wrapper.text()).not.toContain('Configúralo en Ajustes → Descargas')
  })

  it('persists changes to the global ceiling via set_global_max_quality', async () => {
    const wrapper = mount(SettingsQuality)
    await flushPromises()

    const ceilingSelect = wrapper.find('select[data-testid="global-max-quality-select"]')
    await ceilingSelect.setValue('high')
    await flushPromises()

    expect(invoke).toHaveBeenCalledWith('set_global_max_quality', { value: 'high' })
  })

  it('uses backend capabilities for each service without inventing extra quality tiers', async () => {
    const wrapper = mount(SettingsQuality)
    await flushPromises()

    expect(invoke).toHaveBeenCalledWith('get_available_qualities_for_service', { serviceName: 'qobuz' })
    expect(invoke).toHaveBeenCalledWith('get_available_qualities_for_service', { serviceName: 'spotify' })
    const serviceSelects = wrapper.findAll('select').filter(select =>
      select.find('option[value="inherit"]').exists()
    )
    expect(serviceSelects.length).toBe(3)
    const qobuz = serviceSelects[0]!
    expect(qobuz.find('option[value="hires"]').exists()).toBe(true) // valor ya guardado
    expect(qobuz.find('option[value="normal"]').exists()).toBe(false)
  })

  it('renders and allows editing per-service quality preferences', async () => {
    const wrapper = mount(SettingsQuality)
    await flushPromises()

    expect(wrapper.text()).toContain('Qobuz')
    expect(wrapper.text()).toContain('Tidal')
    expect(wrapper.text()).toContain('Spotify')

    const selects = wrapper.findAll('select')
    // 1 global ceiling select + 2 selects per service (3 services = 7 selects)
    expect(selects.length).toBeGreaterThanOrEqual(7)
  })
})

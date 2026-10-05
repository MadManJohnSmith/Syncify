/**
 * SettingsLyrics.vue drag-to-reorder regression (R9).
 *
 * The view said "Drag to reorder" but only offered arrow buttons, and nothing
 * persisted the new order except those buttons.
 */
import { describe, it, expect, beforeEach, vi } from 'vitest'
import { mount, flushPromises } from '@vue/test-utils'

const orderedProviders = ref_stub([
  { provider_id: 'apple_music', provider_name: 'Apple Music', enabled: true, priority: 1, sync_level: 'syllable' },
  { provider_id: 'lrclib', provider_name: 'LRCLIB', enabled: true, priority: 2, sync_level: 'line' },
  { provider_id: 'genius', provider_name: 'Genius', enabled: false, priority: 3, sync_level: 'none' },
])

function ref_stub<T>(value: T) {
  return { value }
}

const reorderProviders = vi.fn(async () => {})
const moveProviderUp = vi.fn(async () => {})
const moveProviderDown = vi.fn(async () => {})
const toggleProvider = vi.fn(async () => {})

vi.mock('@/composables/useLyricsSettings', () => ({
  useLyricsSettings: () => ({
    isLoading: { value: false },
    isSaving: { value: false },
    orderedProviders,
    providers: orderedProviders,
    config: {
      auto_fetch_on_import: false,
      retry_failed: false,
      min_sync_level: 'line',
      preferred_language: 'en',
      storage_format: 'lrc',
      retry_frequency: 'daily',
    },
    syncLevelOptions: [],
    languageOptions: [],
    retryFrequencyOptions: [],
    loadSettings: vi.fn(async () => {}),
    updateConfigField: vi.fn(async () => {}),
    reorderProviders,
    moveProviderUp,
    moveProviderDown,
    toggleProvider,
  }),
}))

vi.mock('@/api/settings', () => ({
  settingsApi: {
    getLyricsConfig: vi.fn(async () => ({})),
    updateLyricsConfig: vi.fn(async () => ({})),
  },
}))

import SettingsLyrics from '@/views/settings/SettingsLyrics.vue'

function dragEventStub() {
  return { dataTransfer: { setData: () => {}, effectAllowed: '' } }
}

describe('SettingsLyrics — drag to reorder providers', () => {
  beforeEach(() => {
    reorderProviders.mockClear()
    moveProviderUp.mockClear()
    moveProviderDown.mockClear()
  })

  async function mountView() {
    const wrapper = mount(SettingsLyrics)
    await flushPromises()
    return wrapper
  }

  it('renders every provider as a draggable row', async () => {
    const wrapper = await mountView()

    const rows = wrapper.findAll('[data-testid^="draggable-item-"]')
    expect(rows).toHaveLength(3)
    rows.forEach(row => expect(row.attributes('draggable')).toBe('true'))
  })

  it('persists the new order when a row is dropped on another one', async () => {
    const wrapper = await mountView()

    const rows = wrapper.findAll('[data-testid^="draggable-item-"]')
    await rows[2].trigger('dragstart', dragEventStub())
    await rows[0].trigger('drop', dragEventStub())
    await flushPromises()

    expect(reorderProviders).toHaveBeenCalledWith(['genius', 'apple_music', 'lrclib'])
  })

  it('ignores a drop on the row the drag started from', async () => {
    const wrapper = await mountView()

    const rows = wrapper.findAll('[data-testid^="draggable-item-"]')
    await rows[1].trigger('dragstart', dragEventStub())
    await rows[1].trigger('drop', dragEventStub())
    await flushPromises()

    expect(reorderProviders).not.toHaveBeenCalled()
  })

  it('does not persist anything when a drop happens without a preceding drag', async () => {
    const wrapper = await mountView()

    await wrapper.findAll('[data-testid^="draggable-item-"]')[1].trigger('drop', dragEventStub())
    await flushPromises()

    expect(reorderProviders).not.toHaveBeenCalled()
  })

  it('still reorders with the arrow buttons', async () => {
    const wrapper = await mountView()

    await wrapper.get('button[aria-label="Move LRCLIB up"]').trigger('click')
    await flushPromises()

    expect(moveProviderUp).toHaveBeenCalledWith('lrclib')
  })

  it('shows the sync level of each provider', async () => {
    const wrapper = await mountView()

    expect(wrapper.text()).toContain('syllable-level sync')
    expect(wrapper.text()).toContain('line-level sync')
  })
})
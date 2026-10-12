/**
 * Unit tests for SettingsDuplicates.vue
 * Tests DUP-01 (live detection stats) and DUP-02/SC-04 (descriptions, help tooltips, and toggles)
 */
import { describe, it, expect, beforeEach } from 'vitest'
import { mount, flushPromises } from '@vue/test-utils'
import { createRouter, createMemoryHistory } from 'vue-router'
import SettingsDuplicates from '../../views/settings/SettingsDuplicates.vue'
import { mockInvoke, resetMocks } from '../setup'
import { invoke } from '@tauri-apps/api/core'

const mockDuplicateSettings = {
  id: 1,
  enable_detection: true,
  prefer_higher_quality: true,
  prefer_lossless: true,
  replace_same_quality_different_source: false,
  quality_threshold_kbps: 64,
  delete_duplicates_immediately: false,
  move_to_trash: true,
}

describe('SettingsDuplicates.vue', () => {
  let router: any

  beforeEach(() => {
    resetMocks()
    router = createRouter({
      history: createMemoryHistory(),
      routes: [{ path: '/library', component: { template: '<div>Library</div>' } }],
    })
    mockInvoke((command) => {
      if (command === 'get_duplicate_settings') return mockDuplicateSettings
      if (command === 'update_duplicate_settings') return mockDuplicateSettings
      if (command === 'get_duplicate_stats') return 4
      if (command === 'get_folder_settings') return null
      if (command === 'get_quality_preferences') return []
      if (command === 'get_audio_processing_settings') return null
      if (command === 'get_download_settings') return null
      if (command === 'get_kv_settings') return {}
      if (command === 'get_default_download_path') return '/Music'
      if (command === 'get_effective_download_preferences') return null
      return null
    })
  })

  it('renders duplicate detection header, live stats card, and rule sections', async () => {
    const wrapper = mount(SettingsDuplicates, {
      global: { plugins: [router] },
    })
    await flushPromises()

    expect(wrapper.text()).toContain('Duplicate Detection & Deduplication')
    expect(wrapper.text()).toContain('Library Detection Status')
    expect(wrapper.text()).toContain('4 duplicate tracks detected')
    expect(wrapper.text()).toContain('Enable duplicate detection')
    expect(wrapper.text()).toContain('Prefer higher quality when deduplicating')
    expect(wrapper.text()).toContain('Prefer lossless formats (FLAC/ALAC)')
    expect(wrapper.text()).toContain('Replace same quality from different source')
    expect(wrapper.text()).toContain('Delete duplicates immediately')
    expect(wrapper.text()).toContain('Move to trash instead of permanent deletion')
    expect(wrapper.text()).toContain('Quality Difference Threshold')
  })

  it('provides descriptive subtitles and help buttons for all options (DUP-02 / SC-04)', async () => {
    const wrapper = mount(SettingsDuplicates, {
      global: { plugins: [router] },
    })
    await flushPromises()

    // Verify explanatory texts exist
    expect(wrapper.text()).toContain('Automatically scan and identify duplicate tracks')
    expect(wrapper.text()).toContain('Retains the copy with higher bit depth')
    expect(wrapper.text()).toContain('Always keeps FLAC or ALAC files over lossy')
    expect(wrapper.text()).toContain('Sends removed duplicate files to the operating system recycle bin')

    // Verify help buttons with tooltip titles exist
    const helpButtons = wrapper.findAll('button[title*="Compares tracks"], button[title*="When deduplicating"], button[title*="lossless codec"]')
    expect(helpButtons.length).toBeGreaterThan(0)
  })

  it('allows toggling duplicate detection and saves settings', async () => {
    const wrapper = mount(SettingsDuplicates, {
      global: { plugins: [router] },
    })
    await flushPromises()

    const switches = wrapper.findAll('button[role="switch"]')
    expect(switches.length).toBeGreaterThanOrEqual(5)

    // Toggle master switch (enable_detection)
    await switches[0].trigger('click')
    await flushPromises()

    expect(invoke).toHaveBeenCalledWith('update_duplicate_settings', expect.objectContaining({
      settings: expect.any(Object)
    }))
  })

  it('allows refreshing duplicate stats dynamically (DUP-01)', async () => {
    const wrapper = mount(SettingsDuplicates, {
      global: { plugins: [router] },
    })
    await flushPromises()

    const refreshBtn = wrapper.findAll('button').find(b => b.text().includes('Refresh'))
    expect(refreshBtn).toBeDefined()

    await refreshBtn!.trigger('click')
    await flushPromises()

    expect(invoke).toHaveBeenCalledWith('get_duplicate_stats')
  })
})

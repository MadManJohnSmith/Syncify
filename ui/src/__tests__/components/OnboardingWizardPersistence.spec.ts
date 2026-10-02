/**
 * OnboardingWizardPersistence.spec.ts
 * Regression tests for FE-5: the onboarding wizard must persist its choices
 * with the real settings APIs and must not render invented data.
 */
import { describe, it, expect, beforeEach, afterEach, vi } from 'vitest'
import { mount, flushPromises } from '@vue/test-utils'
import type { Mock } from 'vitest'
import OnboardingWizard from '@/components/OnboardingWizard.vue'
import { open } from '@tauri-apps/plugin-dialog'
import { resetMocks, mockInvoke } from '../setup'

vi.mock('@/composables/useToast', () => {
  const error = vi.fn()
  return {
    useToast: () => ({ error, success: vi.fn(), info: vi.fn(), warning: vi.fn() }),
  }
})

import { useToast } from '@/composables/useToast'

const KNOWN_SERVICES = ['qobuz', 'tidal', 'spotify', 'deezer', 'apple_music', 'soundcloud']

interface InvokeCall {
  command: string
  args: Record<string, unknown> | undefined
}

function recordingHandler(calls: InvokeCall[], overrides: Record<string, unknown> = {}) {
  return (command: string, args?: Record<string, unknown>) => {
    calls.push({ command, args })
    if (command in overrides) {
      const value = overrides[command]
      // Errors must reject (invokeCommand propagates rejections), not resolve
      if (value instanceof Error) throw value
      return value
    }
    if (command === 'get_default_download_path') return '/music/library'
    if (command === 'validate_directory_path') {
      return {
        valid: true,
        exists: true,
        is_dir: true,
        is_writable: true,
        available_bytes: 450_000_000_000,
        drive_mounted: true,
        canonical_path: args?.path,
        error_message: null,
      }
    }
    if (command === 'get_folder_settings') {
      return {
        id: 1,
        base_folder: '/music/library',
        folder_template: '{AlbumArtist}/{Album}',
        file_template: '{TrackNumber:pad2} - {Title}',
        artist_separator: ', ',
        replace_spaces_with: null,
        max_path_length: 255,
        fallback_action: 'try_next',
      }
    }
    if (command === 'get_service_import_preferences') {
      return {
        service_name: args?.service,
        favorite_tracks: true,
        favorite_albums: false,
        favorite_artists: false,
        playlists: true,
        purchases: true,
        library_history: true,
        include_appearances: true,
        incremental_sync: false,
      }
    }
    if (command === 'scan_local_library_with_progress') {
      return { success: true, data: { directory: args?.directory, total_files: 42, tracks: [] } }
    }
    if (command === 'import_from_url') {
      return { service: 'spotify', content_type: 'playlist', id: 'abc', url: args?.url }
    }
    return null
  }
}

async function mountWizard(calls: InvokeCall[], overrides: Record<string, unknown> = {}) {
  mockInvoke(recordingHandler(calls, overrides))
  const wrapper = mount(OnboardingWizard)
  await flushPromises()
  return wrapper
}

describe('OnboardingWizard persistence (FE-5)', () => {
  let calls: InvokeCall[]

  beforeEach(() => {
    resetMocks()
    localStorage.clear()
    vi.restoreAllMocks()
    calls = []
    ;(useToast().error as Mock).mockClear()
  })

  afterEach(() => {
    localStorage.clear()
    vi.restoreAllMocks()
    document.body.innerHTML = ''
  })

  it('sends the selected quality preset to the per-service quality API on completeSetup', async () => {
    const wrapper = await mountWizard(calls)
    const vm = wrapper.vm as any

    vm.selectedQuality = 'balanced'
    await vm.completeSetup()

    const qualityCalls = calls.filter(c => c.command === 'update_quality_preference')
    expect(qualityCalls.length).toBe(KNOWN_SERVICES.length)
    for (const svc of KNOWN_SERVICES) {
      const call = qualityCalls.find(c => c.args?.serviceName === svc)
      expect(call).toBeTruthy()
      expect(call!.args).toMatchObject({ maxQuality: 'lossless', preferredFormat: 'flac' })
    }

    expect(wrapper.emitted('complete')).toBeTruthy()
  })

  it('saves the chosen download location with the download settings KV keys', async () => {
    const wrapper = await mountWizard(calls)
    const vm = wrapper.vm as any

    vm.downloadPath = '/music/real'
    await vm.completeSetup()

    const batch = calls.find(c => c.command === 'save_settings_batch')
    expect(batch).toBeTruthy()
    expect(batch!.args?.settings).toMatchObject({
      dl_download_path: '/music/real',
      download_dir: '/music/real',
    })
    const folder = calls.find(c => c.command === 'update_folder_settings')
    expect(folder).toBeTruthy()
    expect((folder!.args?.settings as Record<string, unknown>).base_folder).toBe('/music/real')
  })

  it('never persists or renders a made-up path when the default location cannot be resolved', async () => {
    const wrapper = await mountWizard(calls, { get_default_download_path: null })
    const vm = wrapper.vm as any

    expect(vm.downloadPath).toBe('')

    vm.currentStep = 1
    await wrapper.vm.$nextTick()
    expect(wrapper.text()).not.toContain('C:\\Users')
    expect(wrapper.text()).not.toContain('450 GB')
    expect(wrapper.text()).toContain('No folder selected yet')

    await vm.completeSetup()

    const batch = calls.find(c => c.command === 'save_settings_batch')
    expect(batch).toBeTruthy()
    const settings = batch!.args?.settings as Record<string, string>
    expect(settings.dl_download_path).toBeUndefined()
    expect(settings.download_dir).toBeUndefined()
    // Nothing real to sync into the folder settings table either
    expect(calls.some(c => c.command === 'update_folder_settings')).toBe(false)
    // Without a location there is nothing to measure
    expect(calls.some(c => c.command === 'validate_directory_path')).toBe(false)
  })

  it('persists the auto-update toggle with the same auto_updates key as Settings > General', async () => {
    const wrapper = await mountWizard(calls)
    const vm = wrapper.vm as any

    expect(vm.autoUpdate).toBe(true)
    await vm.completeSetup()
    let batch = calls.find(c => c.command === 'save_settings_batch')
    expect((batch!.args?.settings as Record<string, string>).auto_updates).toBe('true')

    vm.autoUpdate = false
    calls.length = 0
    await vm.completeSetup()
    batch = calls.find(c => c.command === 'save_settings_batch')
    expect((batch!.args?.settings as Record<string, string>).auto_updates).toBe('false')
  })

  it('offers no toggle for features the app does not have', async () => {
    const wrapper = await mountWizard(calls)
    const vm = wrapper.vm as any

    vm.currentStep = 5
    await wrapper.vm.$nextTick()

    // There is no tips feature anywhere in the app, so the wizard must not
    // offer a preference for it.
    expect(wrapper.text()).not.toContain('tips')
    const labels = wrapper.findAll('label').map(l => l.text())
    expect(labels).toEqual(['Take a quick tour', 'Check for updates automatically'])
  })

  it('persists the selected import options per connected service, preserving unmanaged fields', async () => {
    const wrapper = await mountWizard(calls)
    const vm = wrapper.vm as any

    const spotify = vm.services.find((s: any) => s.id === 'spotify')
    spotify.connected = true
    vm.importOptions.find((o: any) => o.id === 'playlists').selected = true
    vm.importOptions.find((o: any) => o.id === 'artists').selected = true
    vm.importOptions.find((o: any) => o.id === 'favorites').selected = false

    await vm.completeSetup()

    const importCalls = calls.filter(c => c.command === 'update_service_import_preferences')
    expect(importCalls.length).toBe(1)
    const prefs = importCalls[0].args?.preferences as Record<string, unknown>
    expect(prefs).toMatchObject({
      service_name: 'spotify',
      favorite_tracks: false,
      favorite_albums: false,
      favorite_artists: true,
      playlists: true,
      // fields the wizard does not manage must not be clobbered
      purchases: true,
      library_history: true,
      include_appearances: true,
      incremental_sync: false,
    })
  })

  it('persists the auto-download favorites toggle only when a service is connected', async () => {
    const wrapper = await mountWizard(calls)
    const vm = wrapper.vm as any

    // No service connected: the toggle was never shown, nothing must be written
    await vm.completeSetup()
    expect(calls.filter(c => c.command === 'save_setting')).toHaveLength(0)
    expect(calls.filter(c => c.command === 'update_service_import_preferences')).toHaveLength(0)

    const apple = vm.services.find((s: any) => s.id === 'apple')
    apple.connected = true
    vm.autoDownload = true
    await vm.completeSetup()

    const kv = calls.filter(c => c.command === 'save_setting')
    expect(kv).toHaveLength(1)
    expect(kv[0].args).toMatchObject({ key: 'dl_auto_download_favorites', value: 'true' })
    // 'apple' must be translated to the backend name 'apple_music'
    const prefs = calls.find(c => c.command === 'update_service_import_preferences')
    expect((prefs!.args?.preferences as Record<string, unknown>).service_name).toBe('apple_music')
  })

  it('still finishes onboarding (emits complete) when persistence fails, and reports the failure', async () => {
    const wrapper = await mountWizard(calls, {
      save_settings_batch: new Error('db locked'),
      update_quality_preference: new Error('db locked'),
    })
    const vm = wrapper.vm as any

    await vm.completeSetup()

    expect(wrapper.emitted('complete')).toBeTruthy()
    expect(useToast().error).toHaveBeenCalled()
  })

  it('queries and renders the real available disk space instead of a hardcoded value', async () => {
    const wrapper = await mountWizard(calls)
    const vm = wrapper.vm as any

    expect(calls.some(c => c.command === 'validate_directory_path')).toBe(true)
    // 450,000,000,000 bytes ≈ 419 GiB
    expect(vm.availableSpaceLabel).toBe('419 GB')
  })

  it('hides the space metric when the query fails instead of showing a fake value', async () => {
    mockInvoke((command, args) => {
      calls.push({ command, args })
      if (command === 'get_default_download_path') return '/music/library'
      if (command === 'validate_directory_path') throw new Error('no disk')
      return null
    })
    const wrapper = mount(OnboardingWizard)
    await flushPromises()
    const vm = wrapper.vm as any

    expect(vm.availableSpaceLabel).toBeNull()
    expect(wrapper.text()).not.toContain('Available space')
  })

  it('opens the real folder dialog from "Choose Different Folder" and re-checks space', async () => {
    const wrapper = await mountWizard(calls)
    const vm = wrapper.vm as any
    ;(open as Mock).mockResolvedValueOnce('/music/other')

    vm.currentStep = 1
    await wrapper.vm.$nextTick()

    const button = wrapper.findAll('button').find(b => b.text().includes('Choose Different Folder'))
    expect(button).toBeTruthy()
    await button!.trigger('click')
    await flushPromises()

    expect(open).toHaveBeenCalled()
    expect(vm.downloadPath).toBe('/music/other')
    const validations = calls.filter(c => c.command === 'validate_directory_path')
    expect(validations.some(c => c.args?.path === '/music/other')).toBe(true)
  })

  it('scans a local folder through the real library API when no service is connected', async () => {
    const wrapper = await mountWizard(calls)
    const vm = wrapper.vm as any
    ;(open as Mock).mockResolvedValueOnce('/music/local')

    vm.currentStep = 4
    await wrapper.vm.$nextTick()

    const browse = wrapper.findAll('button').find(b => b.text() === 'Browse')
    expect(browse).toBeTruthy()
    await browse!.trigger('click')
    await flushPromises()

    expect(vm.localFolderPath ?? (vm as any).$refs).toBeTruthy()
    const scan = calls.find(c => c.command === 'scan_local_library_with_progress')
    expect(scan).toBeTruthy()
    expect(scan!.args).toMatchObject({ directory: '/music/local', recursive: true })
    expect(vm.localFolderPath).toBe('/music/local')
    expect(vm.scanStatus).toContain('42')
  })

  it('imports from URL through the real import endpoint when no service is connected', async () => {
    const wrapper = await mountWizard(calls)
    const vm = wrapper.vm as any

    vm.currentStep = 4
    await wrapper.vm.$nextTick()

    const inputs = wrapper.findAll('input[type="text"]')
    await inputs[1].setValue('https://open.spotify.com/playlist/abc')

    const importBtn = wrapper.findAll('button').find(b => b.text() === 'Import')
    expect(importBtn).toBeTruthy()
    await importBtn!.trigger('click')
    await flushPromises()

    const urlCall = calls.find(c => c.command === 'import_from_url')
    expect(urlCall).toBeTruthy()
    expect(urlCall!.args).toMatchObject({ url: 'https://open.spotify.com/playlist/abc' })
    expect(vm.urlImportStatus).toContain('Parsed spotify playlist')
  })

  it('runs the tour against real data-tour targets and emits complete at the end', async () => {
    const wrapper = await mountWizard(calls)
    const vm = wrapper.vm as any

    // A real app element the spotlight can attach to (App.vue marks nav links)
    const target = document.createElement('a')
    target.setAttribute('data-tour', 'library')
    document.body.appendChild(target)

    vm.currentStep = 5
    await wrapper.vm.$nextTick()
    const checkboxes = wrapper.findAll('input[type="checkbox"]')
    await checkboxes[0].setValue(true) // takeTour

    const startBtn = wrapper.findAll('button').find(b => b.text().includes('Start Using Syncify'))
    await startBtn!.trigger('click')
    await flushPromises()

    expect(vm.isVisible).toBe(false)
    expect(vm.showTourOverlay).toBe(true)
    expect(document.querySelector('.app-tour')).toBeTruthy()
    // spotlight positioned from the real element rect (jsdom rect is 0 + padding)
    expect(vm.spotlightStyle.width).toBe('16px')

    vm.skipTour()
    expect(vm.showTourOverlay).toBe(false)
    expect(wrapper.emitted('complete')).toBeTruthy()
  })

  it('ends the tour (instead of showing a placeholder spotlight) when no target elements exist', async () => {
    const wrapper = await mountWizard(calls)
    const vm = wrapper.vm as any

    vm.currentStep = 5
    await wrapper.vm.$nextTick()
    const checkboxes = wrapper.findAll('input[type="checkbox"]')
    await checkboxes[0].setValue(true) // takeTour

    const startBtn = wrapper.findAll('button').find(b => b.text().includes('Start Using Syncify'))
    await startBtn!.trigger('click')
    await flushPromises()

    // No data-tour elements in the document: the tour cannot point at anything
    // real, so it must end and emit complete rather than hang on a fake overlay.
    expect(vm.showTourOverlay).toBe(false)
    expect(wrapper.emitted('complete')).toBeTruthy()
  })
})

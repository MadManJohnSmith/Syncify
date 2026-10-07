import { describe, it, expect, beforeEach, afterEach, vi } from 'vitest'
import { flushPromises, mount } from '@vue/test-utils'
import { nextTick } from 'vue'
import QuickActionsFab from '@/components/QuickActionsFab.vue'
import type { ActionCallback } from '@/components/QuickActionsFab.vue'
import { mockInvoke, resetMocks } from '../setup'
import { createMemoryHistory, createRouter } from 'vue-router'

describe('QuickActionsFab.vue (TASK-20)', () => {
  beforeEach(() => {
    vi.useFakeTimers()
  })

  afterEach(() => {
    vi.useRealTimers()
  })

  it('renders FAB button in idle state by default', () => {
    const wrapper = mount(QuickActionsFab, {
      props: { currentTab: 'library' }
    })

    const button = wrapper.find('button.quick-actions-fab')
    expect(button.exists()).toBe(true)
    expect(button.classes()).toContain('bg-accent')
    expect(wrapper.vm.feedbackState).toBe('idle')
  })

  it('toggles menu open and closed when clicking FAB', async () => {
    const wrapper = mount(QuickActionsFab, {
      props: { currentTab: 'library' }
    })

    const mainBtn = wrapper.find('button.quick-actions-fab')
    expect(wrapper.vm.feedbackState).toBe('idle')

    await mainBtn.trigger('click')
    expect(wrapper.find('.fab-backdrop').exists()).toBe(true)

    await mainBtn.trigger('click')
    expect(wrapper.find('.fab-backdrop').exists()).toBe(false)
  })

  it('emits typed event on action click and does NOT arbitrarily succeed after 500ms', async () => {
    let capturedCallback: ActionCallback | undefined

    const wrapper = mount(QuickActionsFab, {
      props: {
        currentTab: 'library',
        onSyncAll: (callback: ActionCallback) => {
          callback.defer()
          capturedCallback = callback
        }
      }
    })

    // Open menu
    await wrapper.find('button.quick-actions-fab').trigger('click')

    // Find and click "Sync All Services" action
    const syncActionBtn = wrapper.find('button[title="Sync All Services"]')
    expect(syncActionBtn.exists()).toBe(true)

    await syncActionBtn.trigger('click')

    // Verify the event was emitted
    expect(wrapper.emitted('sync-all')).toBeTruthy()
    expect(capturedCallback).toBeDefined()

    // It should now be in loading state
    expect(wrapper.vm.feedbackState).toBe('loading')

    // Advance timers by 500ms (the old fake timer interval)
    await vi.advanceTimersByTimeAsync(500)
    await nextTick()

    // CRITICAL CHECK: In the old code, this would have flipped to 'success' after 500ms.
    // In our fixed implementation, it must STAY 'loading' because the real task has not resolved.
    expect(wrapper.vm.feedbackState).toBe('loading')

    // Now resolve the real task
    capturedCallback!()
    await vi.advanceTimersByTimeAsync(0)
    await nextTick()

    // Now it should be 'success'
    expect(wrapper.vm.feedbackState).toBe('success')

    // After success duration, it resets to 'idle'
    await vi.advanceTimersByTimeAsync(1500)
    await nextTick()
    expect(wrapper.vm.feedbackState).toBe('idle')
  })

  it('reacts with error feedback state when action promise rejects', async () => {
    let pendingReject: (err: unknown) => void = () => {}
    const controlledPromise = new Promise((_, reject) => {
      pendingReject = reject
    })

    const wrapper = mount(QuickActionsFab, {
      props: {
        currentTab: 'library',
        onSyncAll: (callback: ActionCallback) => {
          callback(controlledPromise)
        }
      }
    })

    // Open menu and click Sync All
    await wrapper.find('button.quick-actions-fab').trigger('click')
    await wrapper.find('button[title="Sync All Services"]').trigger('click')

    expect(wrapper.vm.feedbackState).toBe('loading')

    // Reject the promise
    pendingReject(new Error('Network offline'))
    await vi.advanceTimersByTimeAsync(0)
    await nextTick()

    // Verify error state
    expect(wrapper.vm.feedbackState).toBe('error')
    expect(wrapper.find('button.quick-actions-fab').classes()).toContain('bg-error')

    // Resets to idle after error duration
    await vi.advanceTimersByTimeAsync(2000)
    await nextTick()
    expect(wrapper.vm.feedbackState).toBe('idle')
  })

  it('reacts with error feedback state when callback is called with an Error', async () => {
    let capturedCallback: ActionCallback | undefined

    const wrapper = mount(QuickActionsFab, {
      props: {
        currentTab: 'library',
        onScanFolder: (callback: ActionCallback) => {
          callback.defer()
          capturedCallback = callback
        }
      }
    })

    await wrapper.find('button.quick-actions-fab').trigger('click')
    await wrapper.find('button[title="Scan Local Folder"]').trigger('click')

    expect(wrapper.vm.feedbackState).toBe('loading')
    expect(capturedCallback).toBeDefined()

    // Call callback with error
    capturedCallback!(new Error('Directory not found'))
    await vi.advanceTimersByTimeAsync(0)
    await nextTick()

    expect(wrapper.vm.feedbackState).toBe('error')
  })

  it('supports custom actionHandler prop with promise resolution', async () => {
    let resolveAction: () => void = () => {}
    const customHandler = vi.fn().mockImplementation(() => {
      return new Promise<void>((resolve) => {
        resolveAction = resolve
      })
    })

    const wrapper = mount(QuickActionsFab, {
      props: {
        currentTab: 'library',
        actionHandler: customHandler
      }
    })

    await wrapper.find('button.quick-actions-fab').trigger('click')
    await wrapper.find('button[title="Download from URL"]').trigger('click')

    expect(customHandler).toHaveBeenCalledWith('download-url')
    expect(wrapper.vm.feedbackState).toBe('loading')

    // Advance 500ms, still loading
    await vi.advanceTimersByTimeAsync(500)
    await nextTick()
    expect(wrapper.vm.feedbackState).toBe('loading')

    // Resolve
    resolveAction()
    await vi.advanceTimersByTimeAsync(0)
    await nextTick()
    expect(wrapper.vm.feedbackState).toBe('success')
  })

  it('filters actions contextually based on currentTab', () => {
    const libraryWrapper = mount(QuickActionsFab, {
      props: { currentTab: 'library' }
    })
    const libraryActionIds = libraryWrapper.vm.visibleActions.map((a: any) => a.id)
    expect(libraryActionIds).toContain('download-url')
    expect(libraryActionIds).toContain('scan-folder')
    expect(libraryActionIds).not.toContain('pause-all')

    const downloadsWrapper = mount(QuickActionsFab, {
      props: { currentTab: 'downloads' }
    })
    const downloadsActionIds = downloadsWrapper.vm.visibleActions.map((a: any) => a.id)
    expect(downloadsActionIds).toContain('pause-all')
    expect(downloadsActionIds).toContain('retry-failed')
    expect(downloadsActionIds).toContain('clear-completed')
  })

  it('shows metadata/lyrics tab actions that have real handlers bound in App.vue', () => {
    const metadataWrapper = mount(QuickActionsFab, {
      props: { currentTab: 'metadata' }
    })
    const metadataIds = metadataWrapper.vm.visibleActions.map((a: any) => a.id)
    expect(metadataIds).toContain('auto-fix')
    expect(metadataIds).toContain('fetch-metadata')

    const lyricsWrapper = mount(QuickActionsFab, {
      props: { currentTab: 'lyrics' }
    })
    const lyricsIds = lyricsWrapper.vm.visibleActions.map((a: any) => a.id)
    expect(lyricsIds).toContain('fetch-lyrics')
    expect(lyricsIds).toContain('upgrade-lyrics')
  })

  it('no longer offers selection-scoped actions that had no reachable handler (IN-3)', () => {
    const libraryWrapper = mount(QuickActionsFab, {
      props: { currentTab: 'library' }
    })
    const libraryActionIds = libraryWrapper.vm.visibleActions.map((a: any) => a.id)
    expect(libraryActionIds).not.toContain('download-selected')
    expect(libraryActionIds).not.toContain('add-to-playlist')
    expect(libraryActionIds).not.toContain('batch-edit')
  })

  it('fails visibly instead of faking success when an action has no bound handler (IN-3)', async () => {
    const wrapper = mount(QuickActionsFab, {
      props: { currentTab: 'lyrics' }
    })

    await wrapper.find('button.quick-actions-fab').trigger('click')
    await wrapper.find('button[title="Fetch Missing Lyrics"]').trigger('click')

    await vi.advanceTimersByTimeAsync(0)
    await nextTick()

    // CRITICAL CHECK: with no handler bound the FAB must report an error,
    // never the old fake 'success'.
    expect(wrapper.vm.feedbackState).toBe('error')
    expect(wrapper.find('button.quick-actions-fab').classes()).toContain('bg-error')

    // Resets to idle after the error duration
    await vi.advanceTimersByTimeAsync(2000)
    await nextTick()
    expect(wrapper.vm.feedbackState).toBe('idle')
  })
})

describe('App.vue + QuickActionsFab Event Wiring (TASK-20)', () => {
  let mockPush: ReturnType<typeof vi.fn>

  beforeEach(() => {
    mockPush = vi.fn()
  })

  it('connects @download-url to open URL import modal and handles import submission', async () => {
    const { default: App } = await import('@/App.vue')

    const wrapper = mount(App, {
      global: {
        stubs: {
          RouterView: true,
          RouterLink: true,
          SplashScreen: true,
          StatusBar: true,
          NowPlayingBar: true,
          ToastNotifications: true,
          CommandPalette: true,
          KeyboardShortcuts: true,
          HelpPanel: true,
          OnboardingWizard: true
        },
        mocks: {
          $route: { path: '/library' },
          $router: { push: mockPush }
        }
      }
    })

    const fab = wrapper.findComponent(QuickActionsFab)
    expect(fab.exists()).toBe(true)

    // Initially modal is hidden
    expect(wrapper.find('input[placeholder*="open.spotify.com"]').exists()).toBe(false)

    // Trigger download-url
    fab.vm.$emit('download-url')
    await nextTick()

    // Modal should now be open
    const input = wrapper.find('input[placeholder*="open.spotify.com"]')
    expect(input.exists()).toBe(true)
  })

  it('connects @new-playlist to navigate to playlists with creation intent', async () => {
    const { default: App } = await import('@/App.vue')

    const wrapper = mount(App, {
      global: {
        stubs: {
          RouterView: true,
          RouterLink: true,
          SplashScreen: true,
          StatusBar: true,
          NowPlayingBar: true,
          ToastNotifications: true,
          CommandPalette: true,
          KeyboardShortcuts: true,
          HelpPanel: true,
          OnboardingWizard: true
        }
      }
    })

    const fab = wrapper.findComponent(QuickActionsFab)
    expect(fab.exists()).toBe(true)

    // Verify callback can be passed
    let callbackCalled = false
    const cb: ActionCallback = Object.assign(
      () => { callbackCalled = true },
      {
        resolve: () => { callbackCalled = true },
        reject: () => {},
        waitUntil: () => {},
        defer: () => {}
      }
    )

    fab.vm.$emit('new-playlist', cb)
    await nextTick()

    expect(callbackCalled).toBe(true)
  })

  it('connects @sync-all to invoke sync flow', async () => {
    const { default: App } = await import('@/App.vue')

    const wrapper = mount(App, {
      global: {
        stubs: {
          RouterView: true,
          RouterLink: true,
          SplashScreen: true,
          StatusBar: true,
          NowPlayingBar: true,
          ToastNotifications: true,
          CommandPalette: true,
          KeyboardShortcuts: true,
          HelpPanel: true,
          OnboardingWizard: true
        }
      }
    })

    const fab = wrapper.findComponent(QuickActionsFab)
    expect(fab.exists()).toBe(true)

    let registeredPromise: Promise<unknown> | null = null
    const cb: ActionCallback = Object.assign(
      (errOrPromise?: unknown) => {
        if (errOrPromise instanceof Promise) {
          registeredPromise = errOrPromise
        }
      },
      {
        resolve: () => {},
        reject: () => {},
        waitUntil: (p: Promise<unknown>) => { registeredPromise = p },
        defer: () => {}
      }
    )

    fab.vm.$emit('sync-all', cb)
    await nextTick()

    expect(registeredPromise).toBeDefined()
    expect(registeredPromise).toBeInstanceOf(Promise)
  })

  it('connects @auto-fix, @fetch-metadata, @fetch-lyrics and @upgrade-lyrics to real async handlers (IN-3)', async () => {
    const { default: App } = await import('@/App.vue')

    const wrapper = mount(App, {
      global: {
        stubs: {
          RouterView: true,
          RouterLink: true,
          SplashScreen: true,
          StatusBar: true,
          NowPlayingBar: true,
          ToastNotifications: true,
          CommandPalette: true,
          KeyboardShortcuts: true,
          HelpPanel: true,
          OnboardingWizard: true
        }
      }
    })

    const fab = wrapper.findComponent(QuickActionsFab)
    expect(fab.exists()).toBe(true)

    for (const event of ['auto-fix', 'fetch-metadata', 'fetch-lyrics', 'upgrade-lyrics']) {
      let registeredPromise: Promise<unknown> | null = null
      const cb: ActionCallback = Object.assign(
        (errOrPromise?: unknown) => {
          if (errOrPromise instanceof Promise) {
            registeredPromise = errOrPromise
          }
        },
        {
          resolve: () => {},
          reject: () => {},
          waitUntil: (p: Promise<unknown>) => { registeredPromise = p },
          defer: () => {}
        }
      )

      fab.vm.$emit(event, cb)
      await nextTick()

      // Each handler must register a real promise so the FAB feedback waits
      // for the backend operation instead of faking success.
      expect(registeredPromise).toBeInstanceOf(Promise)
      const p = registeredPromise as unknown as Promise<unknown>
      await p.catch(() => {})
    }
  })
})

// ============================================================================
// IN-3: the FAB actions are only meaningful if the handler bound in App.vue
// really reaches the backend command. Asserting "a promise was registered"
// is not enough: an empty handler would also pass. These tests drive the FAB
// through executeAction and check the IPC contract end to end.
// ============================================================================

const APP_STUBS = {
  RouterView: true,
  RouterLink: true,
  SplashScreen: true,
  StatusBar: true,
  NowPlayingBar: true,
  ToastNotifications: true,
  CommandPalette: true,
  KeyboardShortcuts: true,
  HelpPanel: true,
  OnboardingWizard: true
}

describe('QuickActionsFab real backend commands (IN-3)', () => {
  let calls: Array<{ command: string; args?: Record<string, unknown> }>

  beforeEach(() => {
    calls = []
    resetMocks()
  })

  afterEach(() => {
    resetMocks()
  })

  /** Records every invoke and answers with the given per-command behaviour. */
  function stubInvoke(
    handler: (command: string, args?: Record<string, unknown>) => unknown
  ) {
    mockInvoke((command, args) => {
      calls.push({ command, args })
      return handler(command, args)
    })
  }

  function invokedCommands(command: string): Array<Record<string, unknown> | undefined> {
    return calls.filter(c => c.command === command).map(c => c.args)
  }

  async function mountAppAtTab(tab: string) {
    const { default: App } = await import('@/App.vue')
    // A memory router is required: App derives `currentTab` from the route,
    // which is what decides which FAB actions are offered.
    const router = createRouter({
      history: createMemoryHistory(),
      routes: [{ path: '/:tab(.*)', component: { template: '<div />' } }]
    })
    router.push(`/${tab}`)
    await router.isReady()

    const wrapper = mount(App, {
      global: { plugins: [router], stubs: { ...APP_STUBS } }
    })
    const fab = wrapper.findComponent(QuickActionsFab)
    expect(fab.exists()).toBe(true)
    return fab
  }

  function actionById(fab: any, id: string): any {
    const action = fab.vm.visibleActions.find((a: any) => a.id === id)
    expect(action, `action "${id}" is not offered on the current tab`).toBeDefined()
    return action
  }

  it('every action offered on any tab is consumed by a handler bound in App.vue', async () => {
    stubInvoke(() => undefined)

    const offered = new Map<string, string>()
    for (const tab of ['dashboard', 'library', 'downloads', 'metadata', 'lyrics']) {
      const fab = await mountAppAtTab(tab)
      for (const action of fab.vm.visibleActions) {
        offered.set(action.event, action.label)
      }
    }

    // Every tab-scoped action must still be reachable somewhere.
    for (const event of [
      'pause-all',
      'retry-failed',
      'clear-completed',
      'auto-fix',
      'fetch-metadata',
      'fetch-lyrics',
      'upgrade-lyrics'
    ]) {
      expect([...offered.keys()]).toContain(event)
    }

    const fab = await mountAppAtTab('dashboard')
    const pending: Promise<unknown>[] = []
    for (const [event, label] of offered) {
      let registered = false
      const cb: ActionCallback = Object.assign(
        (errOrPromise?: unknown) => {
          registered = true
          if (errOrPromise instanceof Promise) {
            // Attach the rejection handler immediately so a failing operation
            // (e.g. a scan that fails) is not reported as an unhandled rejection.
            errOrPromise.catch(() => {})
            pending.push(errOrPromise)
          }
        },
        {
          resolve: () => { registered = true },
          reject: () => { registered = true },
          waitUntil: () => { registered = true },
          defer: () => { registered = true }
        }
      )

      fab.vm.$emit(event, cb)
      await flushPromises()

      expect(registered, `no handler consumed the "${label}" action`).toBe(true)
    }

    await Promise.all(pending.map(p => p.catch(() => {})))
  })

  it('auto-fix invokes enrich_metadata_musicbrainz and only succeeds once it resolves', async () => {
    let release: (value: unknown) => void = () => {}
    const pending = new Promise(resolve => { release = resolve })
    stubInvoke(cmd => (cmd === 'enrich_metadata_musicbrainz' ? pending : undefined))

    const fab = await mountAppAtTab('metadata')
    const action = actionById(fab, 'auto-fix')

    void fab.vm.executeAction(action)
    await flushPromises()

    expect(invokedCommands('enrich_metadata_musicbrainz')).toHaveLength(1)
    expect(fab.vm.feedbackState).toBe('loading')

    release({ total: 4, enriched: 3, failed: 1 })
    await vi.waitFor(() => expect(fab.vm.feedbackState).toBe('success'))
  })

  it('fetch-metadata invokes start_library_enrichment in incomplete_only mode', async () => {
    stubInvoke(cmd =>
      cmd === 'start_library_enrichment'
        ? { totalTracks: 5, modifiedTracks: 5, failedTracks: 0 }
        : undefined
    )

    const fab = await mountAppAtTab('metadata')
    await fab.vm.executeAction(actionById(fab, 'fetch-metadata'))

    expect(invokedCommands('start_library_enrichment')).toEqual([{ mode: 'incomplete_only' }])
    expect(fab.vm.feedbackState).toBe('success')
  })

  it('fetch-lyrics invokes fetch_missing_lyrics and reports success after it resolves', async () => {
    stubInvoke(cmd =>
      cmd === 'fetch_missing_lyrics' ? { fetched: 2, failed: 0, skipped: 1 } : undefined
    )

    const fab = await mountAppAtTab('lyrics')
    await fab.vm.executeAction(actionById(fab, 'fetch-lyrics'))

    expect(invokedCommands('fetch_missing_lyrics')).toHaveLength(1)
    expect(fab.vm.feedbackState).toBe('success')
  })

  it('upgrade-lyrics re-fetches unsynced lyrics through fetch_and_save_lyrics', async () => {
    const plain = {
      id: 1,
      track_id: 42,
      format: 'plain',
      sync_level: null,
      source: null,
      content: 'line',
      language: null,
      embedded_in_file: false,
      created_at: '2026-01-01T00:00:00Z'
    }
    stubInvoke(cmd => {
      if (cmd === 'get_all_lyrics') return [plain]
      if (cmd === 'fetch_and_save_lyrics') return { ...plain, format: 'lrc' }
      return undefined
    })

    const fab = await mountAppAtTab('lyrics')
    await fab.vm.executeAction(actionById(fab, 'upgrade-lyrics'))

    expect(invokedCommands('get_all_lyrics')).toHaveLength(1)
    expect(invokedCommands('fetch_and_save_lyrics')).toEqual([{ trackId: 42 }])
    expect(fab.vm.feedbackState).toBe('success')
  })

  it('shows error feedback, never success, when the backend command fails', async () => {
    stubInvoke(cmd => {
      if (cmd === 'fetch_missing_lyrics') throw new Error('lyrics backend unavailable')
      return undefined
    })

    const fab = await mountAppAtTab('lyrics')
    await fab.vm.executeAction(actionById(fab, 'fetch-lyrics'))

    expect(fab.vm.feedbackState).toBe('error')
    expect(fab.find('button.quick-actions-fab').classes()).toContain('bg-error')
  })
})


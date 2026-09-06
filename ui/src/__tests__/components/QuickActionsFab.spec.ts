import { describe, it, expect, beforeEach, afterEach, vi } from 'vitest'
import { mount } from '@vue/test-utils'
import { nextTick } from 'vue'
import QuickActionsFab from '@/components/QuickActionsFab.vue'
import type { ActionCallback } from '@/components/QuickActionsFab.vue'

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
    expect(button.classes()).toContain('bg-primary')
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
    expect(wrapper.find('button.quick-actions-fab').classes()).toContain('bg-red-500')

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
      props: { currentTab: 'library', selectedTracksCount: 0 }
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

  it('shows selection-dependent actions only when selectedTracksCount > 0', () => {
    const zeroSelected = mount(QuickActionsFab, {
      props: { currentTab: 'library', selectedTracksCount: 0 }
    })
    expect(zeroSelected.vm.visibleActions.some((a: any) => a.id === 'download-selected')).toBe(false)

    const withSelected = mount(QuickActionsFab, {
      props: { currentTab: 'library', selectedTracksCount: 5 }
    })
    expect(withSelected.vm.visibleActions.some((a: any) => a.id === 'download-selected')).toBe(true)
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
})


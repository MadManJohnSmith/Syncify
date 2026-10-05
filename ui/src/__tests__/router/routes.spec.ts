/**
 * Router & Navigation Structure Tests (TASK-17)
 * 
 * Verifies:
 * 1. Navigating to `/queue` redirects to `/downloads`.
 * 2. Navigating to an unknown route (404 fallback) redirects to `/dashboard`.
 * 3. La búsqueda ya no es una pestaña del sidebar: se abre como modal desde el navbar (R17).
 * 4. Keyboard shortcut tabRoutes map slot 7 to `/downloads`.
 */
import { describe, it, expect, beforeEach, afterEach } from 'vitest'
import { mount, flushPromises } from '@vue/test-utils'
import { createRouter, createMemoryHistory } from 'vue-router'
import { routes } from '@/main'
import App from '@/App.vue'
import KeyboardShortcuts from '@/components/KeyboardShortcuts.vue'
import { resetMocks, mockInvoke } from '../setup'

describe('Router & Navigation Graph (TASK-17)', () => {
  let router: ReturnType<typeof createRouter>

  beforeEach(() => {
    resetMocks()
    mockInvoke((cmd) => {
      if (cmd === 'get_worker_status') return { running: true, paused: false, active_downloads: 0, max_concurrent: 3 }
      if (cmd === 'get_queue_stats') return { total: 0, queued: 0, downloading: 0, completed: 0, failed: 0, paused: 0 }
      return []
    })

    router = createRouter({
      history: createMemoryHistory(),
      routes,
    })
  })

  afterEach(() => {
    document.body.innerHTML = ''
  })

  it('redirects /queue to /downloads', async () => {
    await router.push('/queue')
    await router.isReady()
    expect(router.currentRoute.value.path).toBe('/downloads')
  })

  it('redirects root path / to /dashboard', async () => {
    await router.push('/')
    await router.isReady()
    expect(router.currentRoute.value.path).toBe('/dashboard')
  })

  it('redirects unknown route to /dashboard via catch-all fallback', async () => {
    await router.push('/unknown-route')
    await router.isReady()
    expect(router.currentRoute.value.path).toBe('/dashboard')

    await router.push('/deeply/nested/non-existent/page')
    await router.isReady()
    expect(router.currentRoute.value.path).toBe('/dashboard')
  })

  it('no longer exposes a /search sidebar tab and opens the search modal from the navbar (R17)', async () => {
    const wrapper = mount(App, {
      global: {
        plugins: [router],
        stubs: {
          SplashScreen: true,
          StatusBar: true,
          NowPlayingBar: true,
          ToastNotifications: true,
          CommandPalette: true,
          KeyboardShortcuts: true,
          HelpPanel: true,
          QuickActionsFab: true,
          OnboardingWizard: true,
        },
      },
    })
    await flushPromises()

    expect(wrapper.find('aside nav router-link[to="/search"], aside nav a[href="/search"]').exists()).toBe(false)

    const searchButton = wrapper.find('header button[data-tour="search"]')
    expect(searchButton.exists()).toBe(true)
    expect(searchButton.text()).toContain('Search...')

    await searchButton.trigger('click')
    await flushPromises()

    expect(document.body.querySelector('[data-testid="search-modal"]')).not.toBeNull()
  })

  it('redirects a stale /search navigation to /dashboard: the route no longer exists (R17)', async () => {
    const wrapper = mount(App, {
      global: {
        plugins: [router],
        stubs: {
          SplashScreen: true,
          StatusBar: true,
          NowPlayingBar: true,
          ToastNotifications: true,
          CommandPalette: true,
          KeyboardShortcuts: true,
          HelpPanel: true,
          QuickActionsFab: true,
          OnboardingWizard: true,
        },
      },
    })
    await flushPromises()

    // main.ts ya no registra /search (la búsqueda es solo un modal): el
    // catch-all redirige al dashboard y el modal no se auto-abre.
    await router.push('/search')
    await flushPromises()
    await wrapper.vm.$nextTick()

    expect(router.currentRoute.value.path).toBe('/dashboard')
    expect(routes.find(r => r.path === '/search')).toBeUndefined()
  })

  it('updates KeyboardShortcuts Ctrl+7 tab route to /downloads', async () => {
    const wrapper = mount(KeyboardShortcuts, {
      attachTo: document.body,
      global: {
        plugins: [router],
      },
    })
    await flushPromises()

    // Dispatch Ctrl+7 keydown event
    const event = new KeyboardEvent('keydown', {
      key: '7',
      ctrlKey: true,
      bubbles: true,
    })
    document.dispatchEvent(event)
    await flushPromises()

    // Should navigate to /downloads (previously /queue)
    expect(router.currentRoute.value.path).toBe('/downloads')

    // Open modal to render Teleport content into document.body
    ;(wrapper.vm as any).show()
    await flushPromises()

    // Also verify the descriptive text in the shortcut list
    expect(document.body.textContent).toContain('Go to Downloads / Queue')
    wrapper.unmount()
  })
})

/**
 * First-run landing (issue #93, ADR-0007 decision 15).
 *
 * Zero accounts IS the empty state of the accounts screen: the shell lands on
 * /accounts (six service cards with Connect) and never opens a separate
 * welcome wizard. With accounts on file, the shell stays on the default
 * dashboard, and an unreadable account count means "unknown": no navigation.
 */
import { describe, it, expect, beforeEach } from 'vitest'
import { mount, flushPromises } from '@vue/test-utils'
import { createRouter, createMemoryHistory } from 'vue-router'
import App from '@/App.vue'
import { mockInvoke, resetMocks } from '../setup'

function stubBackend(accounts: unknown[]) {
  mockInvoke((cmd: string) => {
    if (cmd === 'run_health_check') {
      return {
        database_ok: true,
        python_ok: true,
        ffmpeg_available: true,
        chromaprint_available: true,
        services_configured: ['qobuz'],
        errors: [],
      }
    }
    if (cmd === 'get_accounts') return accounts
    if (cmd === 'get_download_settings') {
      return { library_root: '/music/library', staging_root: '/music/library/.staging' }
    }
    return []
  })
}

/** Minimal in-memory router: the shell only needs to land somewhere real. */
function testRouter() {
  return createRouter({
    history: createMemoryHistory(),
    routes: [
      { path: '/', redirect: '/dashboard' },
      { path: '/dashboard', name: 'dashboard', component: { template: '<div />' } },
      { path: '/accounts', name: 'accounts', component: { template: '<div />' } },
    ],
  })
}

function mountApp(router: ReturnType<typeof testRouter>) {
  return mount(App, {
    global: {
      plugins: [router],
      stubs: {
        RouterView: true,
        RouterLink: true,
        StatusBar: true,
        ToastNotifications: true,
        CommandPalette: true,
        KeyboardShortcuts: true,
        OnboardingWizard: true,
      },
    },
  })
}

beforeEach(() => {
  resetMocks()
  localStorage.clear()
})

describe('first-run landing on the accounts empty state (#93)', () => {
  it('lands on /accounts with zero accounts and never opens the wizard', async () => {
    stubBackend([])
    const router = testRouter()
    router.push('/')
    await router.isReady()
    const wrapper = mountApp(router)
    await flushPromises()

    expect(router.currentRoute.value.path).toBe('/accounts')
    expect((wrapper.vm as unknown as { showOnboarding: boolean }).showOnboarding).toBe(false)
    wrapper.unmount()
  })

  it('stays on the dashboard when accounts already exist', async () => {
    stubBackend([{ id: 1, service_id: 2, service_name: 'qobuz', is_active: true }])
    const router = testRouter()
    router.push('/')
    await router.isReady()
    const wrapper = mountApp(router)
    await flushPromises()

    expect(router.currentRoute.value.path).toBe('/dashboard')
    expect((wrapper.vm as unknown as { showOnboarding: boolean }).showOnboarding).toBe(false)
    wrapper.unmount()
  })

  it('does not navigate when the account count cannot be read', async () => {
    mockInvoke((cmd: string) => {
      if (cmd === 'run_health_check') {
        return {
          database_ok: true,
          python_ok: true,
          ffmpeg_available: true,
          chromaprint_available: true,
          services_configured: ['qobuz'],
          errors: [],
        }
      }
      if (cmd === 'get_accounts') throw new Error('backend unavailable')
      if (cmd === 'get_download_settings') {
        return { library_root: '/music/library', staging_root: '/music/library/.staging' }
      }
      return []
    })
    const router = testRouter()
    router.push('/')
    await router.isReady()
    const wrapper = mountApp(router)
    await flushPromises()

    expect(router.currentRoute.value.path).toBe('/dashboard')
    expect((wrapper.vm as unknown as { showOnboarding: boolean }).showOnboarding).toBe(false)
    wrapper.unmount()
  })
})

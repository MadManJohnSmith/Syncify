/**
 * OnboardingWizardResilience.spec.ts
 * Tests for TASK-62: Elimination of mocks & false completion in OnboardingWizard.vue and App.vue.
 */
import { describe, it, expect, beforeEach, afterEach, vi } from 'vitest'
import { mount, flushPromises } from '@vue/test-utils'
import OnboardingWizard from '@/components/OnboardingWizard.vue'
import App from '@/App.vue'
import { resetMocks, mockInvoke } from '../setup'

describe('OnboardingWizard Resilience & Determinism (TASK-62)', () => {
  beforeEach(() => {
    resetMocks()
    localStorage.clear()
    vi.restoreAllMocks()
  })

  afterEach(() => {
    localStorage.clear()
    vi.restoreAllMocks()
  })

  describe('testConnection determinism and elimination of Math.random', () => {
    it('does not invoke Math.random when testConnection is executed', async () => {
      const randomSpy = vi.spyOn(Math, 'random')

      mockInvoke((cmd, args) => {
        if (cmd === 'get_auth_status') {
          return { success: true, data: { tracks: 150 }, error: null }
        }
        if (cmd === 'get_accounts') return []
        if (cmd === 'get_services') return []
        return null
      })

      const wrapper = mount(OnboardingWizard)
      await flushPromises()

      // Ensure Math.random was not called during mount
      expect(randomSpy).not.toHaveBeenCalled()

      // Call testConnection directly
      const vm = wrapper.vm as any
      const result = await vm.testConnection('spotify')

      expect(result).toBe(true)
      expect(randomSpy).not.toHaveBeenCalled()

      const spotify = vm.services.find((s: any) => s.id === 'spotify')
      expect(spotify.connected).toBe(true)
      expect(spotify.tracks).toBe(150)
    })

    it('determines connection via existing accounts when get_auth_status returns not authenticated', async () => {
      const randomSpy = vi.spyOn(Math, 'random')

      mockInvoke((cmd, args) => {
        if (cmd === 'get_auth_status') {
          return { success: false, data: null, error: 'Not authenticated' }
        }
        if (cmd === 'get_services') {
          return [{ id: 1, name: 'spotify', display_name: 'Spotify' }]
        }
        if (cmd === 'get_accounts') {
          return [
            {
              id: 10,
              service_id: 1,
              display_name: 'TestUser',
              is_active: true,
              auth_status: 'connected',
            },
          ]
        }
        return null
      })

      const wrapper = mount(OnboardingWizard)
      await flushPromises()

      const vm = wrapper.vm as any
      const result = await vm.testConnection('spotify')

      expect(result).toBe(true)
      expect(randomSpy).not.toHaveBeenCalled()

      const spotify = vm.services.find((s: any) => s.id === 'spotify')
      expect(spotify.connected).toBe(true)
      expect(spotify.tracks).toBe(0)
    })

    it('fails deterministically without random data when service is not authenticated and has no accounts', async () => {
      const randomSpy = vi.spyOn(Math, 'random')

      mockInvoke((cmd) => {
        if (cmd === 'get_auth_status') {
          return { success: false, data: null, error: 'No session' }
        }
        if (cmd === 'get_accounts') return []
        if (cmd === 'get_services') return []
        return null
      })

      const wrapper = mount(OnboardingWizard)
      await flushPromises()

      const vm = wrapper.vm as any
      const result = await vm.testConnection('tidal')

      expect(result).toBe(false)
      expect(randomSpy).not.toHaveBeenCalled()

      const tidal = vm.services.find((s: any) => s.id === 'tidal')
      expect(tidal.connected).toBe(false)
      expect(tidal.error).toBe('No session')
    })

    it('connectService also operates deterministically without Math.random', async () => {
      const randomSpy = vi.spyOn(Math, 'random')

      mockInvoke((cmd, args) => {
        if (cmd === 'get_auth_status') {
          return { success: false, data: null, error: 'Not logged in' }
        }
        if (cmd === 'get_accounts') return []
        if (cmd === 'get_services') return []
        if (cmd === 'start_auth_and_save') {
          return { success: true, data: { display_name: 'Alice', track_count: 520 }, error: null }
        }
        return null
      })

      const wrapper = mount(OnboardingWizard)
      await flushPromises()

      const vm = wrapper.vm as any
      const qobuz = vm.services.find((s: any) => s.id === 'qobuz')
      await vm.connectService(qobuz)

      expect(randomSpy).not.toHaveBeenCalled()
      expect(qobuz.connected).toBe(true)
      expect(qobuz.tracks).toBe(520)
    })
  })

  describe('Events and Persistence in App.vue', () => {
    it('emits skip and complete events correctly from OnboardingWizard', async () => {
      const wrapper = mount(OnboardingWizard)
      await flushPromises()

      const vm = wrapper.vm as any

      vm.skipSetup()
      expect(wrapper.emitted('skip')).toBeTruthy()
      expect(wrapper.emitted('skip')!.length).toBe(1)

      vm.completeSetup()
      expect(wrapper.emitted('complete')).toBeTruthy()
      expect(wrapper.emitted('complete')!.length).toBe(1)
    })

    it('App.vue saves completion state in localStorage when OnboardingWizard emits @skip', async () => {
      mockInvoke((cmd) => {
        if (cmd === 'get_worker_status') return { running: true, paused: false, active_downloads: 0, max_concurrent: 3 }
        if (cmd === 'get_queue_stats') return { total: 0, queued: 0, downloading: 0, completed: 0, failed: 0, paused: 0 }
        return []
      })

      const wrapper = mount(App, {
        global: {
          stubs: {
            RouterView: true,
            RouterLink: true,
            OnboardingWizard: {
              name: 'OnboardingWizard',
              template: '<div class="stubbed-onboarding"><button class="btn-skip" @click="$emit(\'skip\')">Skip</button></div>',
              emits: ['complete', 'skip'],
            },
          },
        },
      })
      await flushPromises()

      // Force showOnboarding to true for testing handler
      const vm = wrapper.vm as any
      vm.showOnboarding = true
      await wrapper.vm.$nextTick()

      const skipBtn = wrapper.find('.btn-skip')
      expect(skipBtn.exists()).toBe(true)

      await skipBtn.trigger('click')
      await flushPromises()

      expect(localStorage.getItem('syncify_onboarding_completed')).toBe('true')
      expect(localStorage.getItem('syncify_onboarding_complete')).toBe('true')
      expect(vm.showOnboarding).toBe(false)
    })

    it('App.vue saves completion state in localStorage when OnboardingWizard emits @complete', async () => {
      mockInvoke((cmd) => {
        if (cmd === 'get_worker_status') return { running: true, paused: false, active_downloads: 0, max_concurrent: 3 }
        if (cmd === 'get_queue_stats') return { total: 0, queued: 0, downloading: 0, completed: 0, failed: 0, paused: 0 }
        return []
      })

      const wrapper = mount(App, {
        global: {
          stubs: {
            RouterView: true,
            RouterLink: true,
            OnboardingWizard: {
              name: 'OnboardingWizard',
              template: '<div class="stubbed-onboarding"><button class="btn-complete" @click="$emit(\'complete\')">Complete</button></div>',
              emits: ['complete', 'skip'],
            },
          },
        },
      })
      await flushPromises()

      const vm = wrapper.vm as any
      vm.showOnboarding = true
      await wrapper.vm.$nextTick()

      const completeBtn = wrapper.find('.btn-complete')
      expect(completeBtn.exists()).toBe(true)

      await completeBtn.trigger('click')
      await flushPromises()

      expect(localStorage.getItem('syncify_onboarding_completed')).toBe('true')
      expect(localStorage.getItem('syncify_onboarding_complete')).toBe('true')
      expect(vm.showOnboarding).toBe(false)
    })
  })
})

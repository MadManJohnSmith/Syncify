/**
 * LayoutSpacing.spec.ts
 *
 * TASK-18: Tests validating destructive visual collision avoidance between
 * NowPlayingBar, StatusBar, QuickActionsFab and App content padding.
 */
import { describe, it, expect, beforeEach, beforeAll, vi } from 'vitest'
import { mount, flushPromises } from '@vue/test-utils'
import App from '@/App.vue'
import NowPlayingBar from '@/components/NowPlayingBar.vue'
import QuickActionsFab from '@/components/QuickActionsFab.vue'
import StatusBar from '@/components/StatusBar.vue'
import { usePlayer, type PlayerTrack } from '@/composables/usePlayer'
import { mockInvoke } from '../setup'

// Stub audio element methods
beforeAll(() => {
  Object.defineProperty(window.HTMLMediaElement.prototype, 'play', {
    configurable: true,
    value: vi.fn(() => Promise.resolve()),
  })
  Object.defineProperty(window.HTMLMediaElement.prototype, 'pause', {
    configurable: true,
    value: vi.fn(),
  })
  Object.defineProperty(window.HTMLMediaElement.prototype, 'load', {
    configurable: true,
    value: vi.fn(),
  })
})

const sampleTrack: PlayerTrack = {
  id: 42,
  title: 'Nocturne in C-sharp minor',
  artist: 'Frédéric Chopin',
  album: 'Chopin: Piano Works',
  coverUrl: null,
}

describe('TASK-18: Footer Layout Collision and Dynamic Spacing', () => {
  const player = usePlayer()

  beforeEach(() => {
    player.stop()
    mockInvoke((cmd: string) => {
      if (cmd === 'get_worker_status') return { running: true, paused: false, active_downloads: 0, max_concurrent: 3 }
      if (cmd === 'get_queue_stats') return { total: 0, queued: 0, downloading: 0, completed: 0, failed: 0, paused: 0 }
      if (cmd === 'get_storage_stats') return { used_bytes: 0, total_bytes: 0, free_bytes: 0, breakdown: [] }
      if (cmd === 'get_service_statuses') return []
      return []
    })
  })

  describe('App.vue content bottom padding', () => {
    it('applies pb-8 when no track is playing and pb-24 when player is active', async () => {
      const wrapper = mount(App, {
        global: {
          stubs: {
            RouterView: true,
            RouterLink: true,
          },
        },
      })
      await flushPromises()

      const pageContent = wrapper.find('#page-content')
      expect(pageContent.exists()).toBe(true)

      // When stopped (current = null), page container has pb-8 for StatusBar (32px)
      expect(pageContent.classes()).toContain('pb-8')
      expect(pageContent.classes()).not.toContain('pb-24')

      // Activate player track
      player.current.value = sampleTrack
      await flushPromises()

      // When active, page container adapts to pb-24 (96px = StatusBar 32px + NowPlayingBar 64px)
      expect(pageContent.classes()).toContain('pb-24')
      expect(pageContent.classes()).not.toContain('pb-8')

      // Reset / stop playback
      player.stop()
      await flushPromises()

      // Reverts back to pb-8
      expect(pageContent.classes()).toContain('pb-8')
      expect(pageContent.classes()).not.toContain('pb-24')
    })
  })

  describe('QuickActionsFab.vue dynamic position', () => {
    it('positions at bottom-6 when idle and lifts to bottom-28 when player is active', async () => {
      const wrapper = mount(QuickActionsFab)
      await flushPromises()

      const fabBtn = wrapper.find('.quick-actions-fab')
      const fabMenu = wrapper.find('.fab-menu')

      // Idle state: bottom-6
      expect(fabBtn.classes()).toContain('bottom-6')
      expect(fabBtn.classes()).not.toContain('bottom-28')
      expect(fabMenu.classes()).toContain('bottom-6')
      expect(fabMenu.classes()).not.toContain('bottom-28')

      // Player active: lifts to bottom-28
      player.current.value = sampleTrack
      await flushPromises()

      expect(fabBtn.classes()).toContain('bottom-28')
      expect(fabBtn.classes()).not.toContain('bottom-6')
      expect(fabMenu.classes()).toContain('bottom-28')
      expect(fabMenu.classes()).not.toContain('bottom-6')

      // Player stopped: returns to bottom-6
      player.stop()
      await flushPromises()

      expect(fabBtn.classes()).toContain('bottom-6')
      expect(fabBtn.classes()).not.toContain('bottom-28')
      expect(fabMenu.classes()).toContain('bottom-6')
      expect(fabMenu.classes()).not.toContain('bottom-28')
    })
  })

  describe('NowPlayingBar.vue positioning', () => {
    it('rests exactly above StatusBar with fixed bottom-8 h-16 and z-[150]', async () => {
      const wrapper = mount(NowPlayingBar)
      await flushPromises()

      // When no track is active, bar is not rendered
      expect(wrapper.find('.fixed').exists()).toBe(false)

      // Activate track
      player.current.value = sampleTrack
      await flushPromises()

      const bar = wrapper.find('.fixed')
      expect(bar.exists()).toBe(true)
      expect(bar.classes()).toContain('bottom-8')
      expect(bar.classes()).toContain('h-16')
      expect(bar.classes()).toContain('z-[150]')
    })
  })

  describe('StatusBar.vue popover elevation and positioning', () => {
    it('elevates popovers to z-[160] and shifts them between bottom-10 and bottom-26 based on player state', async () => {
      const wrapper = mount(StatusBar)
      await flushPromises()

      // Open sync popover
      const syncStatusBtn = wrapper.find('.sync-status')
      await syncStatusBtn.trigger('click')
      await flushPromises()

      const popover = wrapper.find('.status-popover')
      expect(popover.exists()).toBe(true)
      expect(popover.classes()).toContain('z-[160]')
      // When player is idle, popover sits at bottom-10
      expect(popover.classes()).toContain('bottom-10')
      expect(popover.classes()).not.toContain('bottom-26')

      // When player becomes active, popover lifts to bottom-26
      player.current.value = sampleTrack
      await flushPromises()

      expect(popover.classes()).toContain('bottom-26')
      expect(popover.classes()).not.toContain('bottom-10')
      expect(popover.classes()).toContain('z-[160]')
    })
  })
})

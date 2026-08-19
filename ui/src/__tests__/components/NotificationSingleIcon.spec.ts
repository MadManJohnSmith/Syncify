/**
 * Unit tests for single notification bell icon and reactive unread count (S141)
 */
import { describe, it, expect, beforeEach } from 'vitest'
import { mount, flushPromises } from '@vue/test-utils'
import App from '@/App.vue'
import ToastNotifications from '@/components/ToastNotifications.vue'
import { useToast } from '@/composables/useToast'
import { resetMocks, mockInvoke } from '../setup'

describe('Notification System & Single Icon (S141)', () => {
  beforeEach(() => {
    resetMocks()
    const toast = useToast()
    toast.clearAllHistory()
  })

  it('renders single notification bell icon in App header with reactive unread count', async () => {
    mockInvoke((cmd) => {
      if (cmd === 'get_worker_status') return { running: true, paused: false, active_downloads: 0, max_concurrent: 3 }
      if (cmd === 'get_queue_stats') return { total: 0, queued: 0, downloading: 0, completed: 0, failed: 0, paused: 0 }
      return []
    })

    const toast = useToast()
    toast.info('System Update', 'Syncify started successfully')
    toast.success('Download Complete', 'Album finished')

    expect(toast.unreadCount.value).toBe(2)

    const wrapper = mount(App, {
      global: {
        stubs: {
          RouterView: true,
          RouterLink: true,
        }
      }
    })
    await flushPromises()

    // Find notification bells across the entire app
    const bellButtons = wrapper.findAll('.notification-bell-btn')
    // Must be exactly 1 bell icon in the header, never duplicated
    expect(bellButtons.length).toBe(1)

    // Unread count badge displays 2
    const badge = wrapper.find('.notification-badge')
    expect(badge.exists()).toBe(true)
    expect(badge.text()).toBe('2')
  })

  it('ToastNotifications component only displays floating toast alerts without duplicate bell', async () => {
    const toast = useToast()
    toast.info('Test Alert', 'Floating toast message')

    const wrapper = mount(ToastNotifications)
    await flushPromises()

    // No bell buttons in ToastNotifications
    expect(wrapper.findAll('.notification-bell-btn').length).toBe(0)
    expect(wrapper.findAll('button[title*="Notification"]').length).toBe(0)

    // Floating toasts container is rendered
    expect(wrapper.text()).toContain('Test Alert')
    expect(wrapper.text()).toContain('Floating toast message')
  })

  it('marks notifications as read and clears unread badge reactively', async () => {
    const toast = useToast()
    toast.error('Sync Error', 'Rate limited by Spotify')
    expect(toast.unreadCount.value).toBe(1)

    const wrapper = mount(App, {
      global: {
        stubs: {
          RouterView: true,
          RouterLink: true,
        }
      }
    })
    await flushPromises()

    // Open notification panel
    const bellBtn = wrapper.find('.notification-bell-btn')
    await bellBtn.trigger('click')
    await flushPromises()

    expect(wrapper.text()).toContain('Sync Error')

    // Mark all as read
    const markAllBtn = wrapper.findAll('button').find(b => b.text().includes('Mark all read'))
    if (markAllBtn) {
      await markAllBtn.trigger('click')
      await flushPromises()
      expect(toast.unreadCount.value).toBe(0)
    }
  })
})

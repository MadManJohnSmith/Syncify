/**
 * SettingsGeneral.vue truthfulness regression (audit items 3 and 33).
 *
 * The screen offered an "automatic updates" switch and an "anonymous usage
 * statistics" switch that nothing in the codebase reads, and described the
 * database reset as deleting accounts and settings when it deletes neither.
 */
import { describe, it, expect, beforeEach, vi } from 'vitest'
import { mount, flushPromises } from '@vue/test-utils'
import { mockInvoke, resetMocks } from '../setup'
import SettingsGeneral from '@/views/settings/SettingsGeneral.vue'

describe('SettingsGeneral — only offers settings the app honours', () => {
  beforeEach(() => {
    resetMocks()
    vi.clearAllMocks()

    mockInvoke((command) => {
      if (command === 'get_kv_settings') {
        return {
          start_on_boot: 'true',
          start_minimized: 'false',
          close_to_tray: 'true',
          db_location: '/home/user/.local/share/com.syncify.app/syncify.db',
          download_dir: '/music/Syncify',
          temp_dir: '/music/Syncify/.staging',
        }
      }
      if (command === 'get_download_settings') return null
      if (command === 'get_unified_download_settings') return null
      if (command === 'get_folder_settings') return null
      if (command === 'get_tray_settings') return null
      if (command === 'get_default_download_path') return '/music/Syncify'
      return null
    })
  })

  async function mountView() {
    const wrapper = mount(SettingsGeneral)
    await flushPromises()
    return wrapper
  }

  it('no longer offers an automatic updates switch', async () => {
    const wrapper = await mountView()

    expect(wrapper.text()).not.toContain('Check for updates automatically')
    expect(wrapper.text()).not.toContain('Updates & Telemetry')
  })

  it('no longer offers an anonymous statistics switch', async () => {
    const wrapper = await mountView()

    expect(wrapper.text()).not.toContain('Send anonymous usage statistics')
  })

  it('keeps the three startup switches, exposed as accessible switches', async () => {
    const wrapper = await mountView()

    const startupSection = wrapper.find('section')
    const names = startupSection.findAll('button[role="switch"]').map(b => b.attributes('aria-label'))
    expect(names).toHaveLength(3)
    expect(names.join(' | ')).toContain('Start on system boot')
    expect(names.join(' | ')).toContain('Start minimized to tray')
    expect(names.join(' | ')).toContain('Close to tray instead of exit')
  })

  it('loads and saves category notification preferences without touching startup toggles', async () => {
    const wrapper = await mountView()
    const section = wrapper.find('[data-testid="notification-preferences"]')
    expect(section.exists()).toBe(true)
    const errors = section.findAll('button[role="switch"]').find(b => b.attributes('aria-label') === 'Errors')
    expect(errors).toBeDefined()
    await errors!.trigger('click')
    await flushPromises()
    // Saving notification preferences is a real IPC call, not a visual-only toggle.
    const { invoke } = await import('@tauri-apps/api/core')
    expect(vi.mocked(invoke).mock.calls.some(([cmd, args]) =>
      cmd === 'update_tray_settings' && (args as { settings?: { notifyErrors?: boolean } })?.settings?.notifyErrors === false
    )).toBe(true)
  })

  it('describes the boot switch as login-based, not Windows-only', async () => {
    const wrapper = await mountView()

    expect(wrapper.text()).not.toContain('when Windows starts')
    expect(wrapper.text()).toContain('when you log in')
  })

  it('does not claim the database reset removes accounts or settings', async () => {
    const wrapper = await mountView()
    const text = wrapper.text()

    expect(text).not.toContain('Delete all library data, accounts, and settings')
    expect(text).toContain('service accounts and every settings value are kept')
  })
})
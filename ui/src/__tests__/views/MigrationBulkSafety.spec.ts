import { describe, it, expect, vi } from 'vitest'
import { mount, flushPromises } from '@vue/test-utils'
import MigrationView from '@/views/MigrationView.vue'
import { mockInvoke, resetMocks } from '../setup'

const statuses = ['spotify', 'qobuz', 'tidal'].map(name => ({ name, connected: true, account_email: null, library_count: 3, favorites_count: 3, playlists_count: 0, last_synced: null, credentials_invalid: false }))

async function setup() {
  const writes = vi.fn().mockResolvedValue('job-1')
  mockInvoke((cmd) => {
    if (cmd === 'get_service_statuses') return statuses
    if (cmd === 'get_migration_destinations') return ['qobuz', 'tidal']
    if (cmd === 'get_accounts') return [
      { id: 1, service_id: 1, service_name: 'spotify', display_name: 'Source', is_active: true },
      { id: 2, service_id: 2, service_name: 'qobuz', display_name: 'Qobuz', is_active: true },
      { id: 3, service_id: 3, service_name: 'tidal', display_name: 'Tidal', is_active: true },
    ]
    if (cmd === 'preview_migration') return { total_tracks: 3, matched_tracks: 2, unmatched_tracks: 1, playlists: [] }
    if (cmd === 'start_migration') return writes()
    return []
  })
  const wrapper = mount(MigrationView, { global: { stubs: { transition: true, teleport: true } } })
  await flushPromises()
  const next = async () => {
    await wrapper.findAll('button').find(b => b.text().includes('Next'))!.trigger('click')
    await flushPromises()
  }
  await wrapper.find('[data-service-id="spotify"]').trigger('click')
  await next()
  await next()
  return { wrapper, next, writes }
}

describe('migration batch safety and responsive layout', () => {
  it('uses available width and stacks service cards on narrow screens', async () => {
    resetMocks()
    const { wrapper } = await setup()
    expect(wrapper.find('.transfer-wizard').classes()).toContain('w-full')
    expect(wrapper.find('.service-grid').classes()).toContain('grid-cols-1')
    expect(wrapper.find('.transfer-wizard').classes().some(c => c.includes('max-w-'))).toBe(false)
    wrapper.unmount()
  })

  it('never writes a multi-destination batch with only one destination previewed', async () => {
    resetMocks()
    const { wrapper, next, writes } = await setup()
    await wrapper.find('[data-service-id="qobuz"]').trigger('click')
    await wrapper.find('[data-service-id="tidal"]').trigger('click')
    await next()
    expect(wrapper.find('[data-testid="preview-total"]').text()).toBe('3')
    await next()
    expect(wrapper.find('[data-testid="batch-blocked"]').exists()).toBe(true)
    await wrapper.find('[data-testid="start-transfer-btn"]').trigger('click')
    await flushPromises()
    expect(writes).not.toHaveBeenCalled()
    wrapper.unmount()
  })

  it('requires explicit confirmation even after a single-account preview', async () => {
    resetMocks()
    const { wrapper, next, writes } = await setup()
    await wrapper.find('[data-service-id="qobuz"]').trigger('click')
    await next()
    await next()
    await wrapper.find('[data-testid="start-transfer-btn"]').trigger('click')
    await flushPromises()
    expect(writes).not.toHaveBeenCalled()
    await wrapper.find('[data-testid="confirm-transfer"]').setValue(true)
    await wrapper.find('[data-testid="start-transfer-btn"]').trigger('click')
    await flushPromises()
    expect(writes).toHaveBeenCalledTimes(1)
    wrapper.unmount()
  })
})

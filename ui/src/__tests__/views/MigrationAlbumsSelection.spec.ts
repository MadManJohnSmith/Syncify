/**
 * MigrationAlbumsSelection.spec.ts
 * M01 (#166): the wizard offers favorite albums when a destination declares
 * them, and the started job carries the selected collections on the wire —
 * with the capability matrix still authoritative (no conflict can start).
 */
import { describe, it, expect, beforeEach, vi } from 'vitest'
import { mount, flushPromises, type VueWrapper } from '@vue/test-utils'
import MigrationView from '@/views/MigrationView.vue'
import { mockInvoke, resetMocks } from '../setup'

vi.mock('vue-router', () => ({
  useRouter: () => ({ push: vi.fn() }),
  useRoute: () => ({ query: {}, params: {} }),
}))

const statuses = ['spotify', 'qobuz', 'tidal'].map(name => ({
  name,
  connected: true,
  account_email: 'user@example.com',
  library_count: 3,
  favorites_count: 3,
  playlists_count: 1,
  last_synced: null,
  credentials_invalid: false,
}))

/** M01 matrix: albums declared for qobuz and tidal, tracks everywhere,
 * playlists for qobuz/tidal, artists nowhere. */
const capabilities = ['qobuz', 'tidal', 'spotify'].map(service => ({
  service,
  collections: [
    { collection: 'tracks', supported: true },
    { collection: 'albums', supported: service === 'qobuz' || service === 'tidal' },
    { collection: 'artists', supported: false },
    { collection: 'playlists', supported: service === 'qobuz' || service === 'tidal' },
  ],
}))

type InvokeCalls = Array<{ cmd: string; args?: Record<string, unknown> }>

function backend(calls?: InvokeCalls): void {
  mockInvoke((cmd, args) => {
    calls?.push({ cmd, args })
    if (cmd === 'get_service_statuses') return statuses
    if (cmd === 'get_migration_destinations') return ['qobuz', 'tidal', 'spotify']
    if (cmd === 'get_migration_capabilities') return capabilities
    if (cmd === 'get_accounts') return statuses.map((s, i) => ({
      id: i + 1,
      service_id: i + 1,
      service_name: s.name,
      display_name: `${s.name} account`,
      is_active: true,
    }))
    if (cmd === 'preview_migration') return { total_tracks: 3, matched_tracks: 2, unmatched_tracks: 1, playlists: [] }
    if (cmd === 'start_migration') return 'job-albums-1'
    return []
  })
}

const stubs = { transition: true, teleport: true }

async function mountView(): Promise<VueWrapper> {
  const wrapper = mount(MigrationView, { global: { stubs } })
  await flushPromises()
  return wrapper
}

async function clickNext(wrapper: VueWrapper): Promise<void> {
  const button = wrapper.findAll('button').find(b => b.text().includes('Next'))
  expect(button, 'button "Next" should exist').toBeTruthy()
  await button!.trigger('click')
  await flushPromises()
}

async function selectContent(wrapper: VueWrapper, label: string): Promise<void> {
  const card = wrapper.findAll('.content-card').find(c => c.text().includes(label))
  expect(card, `content card "${label}" should exist`).toBeTruthy()
  await card!.trigger('click')
  await flushPromises()
}

describe('M01 favorite albums in the transfer wizard', () => {
  beforeEach(() => {
    resetMocks()
    vi.clearAllMocks()
  })

  it('offers favorite albums as a selectable card when a destination declares them', async () => {
    backend()
    const wrapper = await mountView()

    await wrapper.find('[data-service-id="spotify"]').trigger('click')
    await clickNext(wrapper) // -> content

    const albumsCard = wrapper.findAll('.content-card').find(c => c.text().includes('Favorite Albums'))
    expect(albumsCard, 'the albums card should exist').toBeTruthy()
    expect(albumsCard!.attributes('disabled')).toBeUndefined()

    await albumsCard!.trigger('click')
    await flushPromises()
    // The card reacts to selection like the rest of the content cards.
    expect(albumsCard!.classes().join(' ')).toContain('border-primary')
    wrapper.unmount()
  })

  it('starts the job with the selected collections on the wire', async () => {
    const calls: InvokeCalls = []
    backend(calls)
    const wrapper = await mountView()

    await wrapper.find('[data-service-id="spotify"]').trigger('click')
    await clickNext(wrapper) // -> content
    await selectContent(wrapper, 'Favorite Albums')
    await clickNext(wrapper) // -> destinations
    await wrapper.find('[data-service-id="qobuz"]').trigger('click')
    await flushPromises()
    // Qobuz declares albums: no capability conflict on the destination step.
    expect(wrapper.find('[data-testid="capability-conflict"]').exists()).toBe(false)
    await clickNext(wrapper) // -> review (preview runs)
    expect(wrapper.find('[data-testid="preview-description"]').exists()).toBe(true)
    await clickNext(wrapper) // -> transfer step
    expect(wrapper.find('[data-testid="start-transfer-btn"]').exists()).toBe(true)

    await wrapper.find('[data-testid="start-transfer-btn"]').trigger('click')
    await flushPromises()
    await wrapper.find('[data-testid="confirm-transfer"]').setValue(true)
    await wrapper.find('[data-testid="start-transfer-btn"]').trigger('click')
    await flushPromises()

    const call = calls.find(c => c.cmd === 'start_migration')
    expect(call, 'start_migration should have been invoked').toBeTruthy()
    expect((call!.args as { collections?: string[] }).collections).toEqual(['tracks', 'albums'])
    wrapper.unmount()
  })

  it('never sends playlists without the track read that feeds them', async () => {
    const calls: InvokeCalls = []
    backend(calls)
    const wrapper = await mountView()

    await wrapper.find('[data-service-id="spotify"]').trigger('click')
    await clickNext(wrapper) // -> content
    await selectContent(wrapper, 'Favorites & Tracks') // deselect the default
    await selectContent(wrapper, 'Playlists')
    await clickNext(wrapper) // -> destinations
    await wrapper.find('[data-service-id="qobuz"]').trigger('click')
    await flushPromises()
    await clickNext(wrapper) // -> review (preview runs)
    await clickNext(wrapper) // -> transfer step

    await wrapper.find('[data-testid="start-transfer-btn"]').trigger('click')
    await flushPromises()
    await wrapper.find('[data-testid="confirm-transfer"]').setValue(true)
    await wrapper.find('[data-testid="start-transfer-btn"]').trigger('click')
    await flushPromises()

    const call = calls.find(c => c.cmd === 'start_migration')
    expect(call, 'start_migration should have been invoked').toBeTruthy()
    // The playlist mirror is built from the transferred track favorites: the
    // wire carries the track read even though only Playlists was selected.
    expect((call!.args as { collections?: string[] }).collections).toEqual(['tracks', 'playlists'])
    wrapper.unmount()
  })

  it('keeps albums out of the wire when only favorites are selected', async () => {
    const calls: InvokeCalls = []
    backend(calls)
    const wrapper = await mountView()

    await wrapper.find('[data-service-id="spotify"]').trigger('click')
    await clickNext(wrapper) // -> content
    await clickNext(wrapper) // -> destinations (favorites only)
    await wrapper.find('[data-service-id="qobuz"]').trigger('click')
    await flushPromises()
    await clickNext(wrapper) // -> review (preview runs)
    await clickNext(wrapper) // -> transfer step

    await wrapper.find('[data-testid="start-transfer-btn"]').trigger('click')
    await flushPromises()
    await wrapper.find('[data-testid="confirm-transfer"]').setValue(true)
    await wrapper.find('[data-testid="start-transfer-btn"]').trigger('click')
    await flushPromises()

    const call = calls.find(c => c.cmd === 'start_migration')
    expect(call).toBeTruthy()
    expect((call!.args as { collections?: string[] }).collections).toEqual(['tracks'])
    wrapper.unmount()
  })
})

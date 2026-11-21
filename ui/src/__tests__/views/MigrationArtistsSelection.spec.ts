/**
 * MigrationArtistsSelection.spec.ts
 * M02 (#167): the wizard offers followed artists when a destination declares
 * them, carries the collection on the wire, and still shows a destination
 * without follow as a capability conflict.
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

/** M02 matriz: artistas en qobuz/tidal/spotify (donde existe follow), álbumes
 * en qobuz/tidal, playlists en qobuz/tidal. `followable` permite simular un
 * destino sin follow para el gating. */
function capabilitiesWith(followable: { qobuz: boolean; tidal: boolean; spotify: boolean }) {
  return ['qobuz', 'tidal', 'spotify'].map(service => ({
    service,
    collections: [
      { collection: 'tracks', supported: true },
      { collection: 'albums', supported: service === 'qobuz' || service === 'tidal' },
      { collection: 'artists', supported: followable[service as keyof typeof followable] },
      { collection: 'playlists', supported: service === 'qobuz' || service === 'tidal' },
    ],
  }))
}

type InvokeCalls = Array<{ cmd: string; args?: Record<string, unknown> }>

function backend(capabilities: ReturnType<typeof capabilitiesWith>, calls?: InvokeCalls): void {
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
    if (cmd === 'start_migration') return 'job-artists-1'
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

const allFollowable = { qobuz: true, tidal: true, spotify: true }

describe('M02 followed artists in the transfer wizard', () => {
  beforeEach(() => {
    resetMocks()
    vi.clearAllMocks()
  })

  it('offers followed artists as a selectable card when a destination declares them', async () => {
    backend(capabilitiesWith(allFollowable))
    const wrapper = await mountView()

    await wrapper.find('[data-service-id="spotify"]').trigger('click')
    await clickNext(wrapper) // -> content

    const artistsCard = wrapper.findAll('.content-card').find(c => c.text().includes('Followed Artists'))
    expect(artistsCard, 'the artists card should exist').toBeTruthy()
    expect(artistsCard!.attributes('disabled')).toBeUndefined()

    await artistsCard!.trigger('click')
    await flushPromises()
    expect(artistsCard!.classes().join(' ')).toContain('border-primary')
    wrapper.unmount()
  })

  it('starts the job with artists on the wire next to the selected favorites', async () => {
    const calls: InvokeCalls = []
    backend(capabilitiesWith(allFollowable), calls)
    const wrapper = await mountView()

    await wrapper.find('[data-service-id="spotify"]').trigger('click')
    await clickNext(wrapper) // -> content
    await selectContent(wrapper, 'Followed Artists')
    await clickNext(wrapper) // -> destinations
    await wrapper.find('[data-service-id="qobuz"]').trigger('click')
    await flushPromises()
    expect(wrapper.find('[data-testid="capability-conflict"]').exists()).toBe(false)
    await clickNext(wrapper) // -> review (preview runs)
    await clickNext(wrapper) // -> transfer step

    await wrapper.find('[data-testid="start-transfer-btn"]').trigger('click')
    await flushPromises()
    await wrapper.find('[data-testid="confirm-transfer"]').setValue(true)
    await wrapper.find('[data-testid="start-transfer-btn"]').trigger('click')
    await flushPromises()

    const call = calls.find(c => c.cmd === 'start_migration')
    expect(call, 'start_migration should have been invoked').toBeTruthy()
    expect((call!.args as { collections?: string[] }).collections).toEqual(['tracks', 'artists'])
    wrapper.unmount()
  })

  it('flags a destination without follow as a capability conflict', async () => {
    backend(capabilitiesWith({ qobuz: true, tidal: true, spotify: false }))
    const wrapper = await mountView()

    await wrapper.find('[data-service-id="qobuz"]').trigger('click')
    await clickNext(wrapper) // -> content
    await selectContent(wrapper, 'Followed Artists')
    await clickNext(wrapper) // -> destinations
    await wrapper.find('[data-service-id="spotify"]').trigger('click')
    await flushPromises()

    const conflict = wrapper.find('[data-testid="capability-conflict"]')
    expect(conflict.exists(), 'spotify does not follow artists in this matrix').toBe(true)
    expect(conflict.text()).toContain('Followed Artists')
    wrapper.unmount()
  })
})

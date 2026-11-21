/**
 * MigrationCapabilityGating.spec.ts
 * M00 (#165): the wizard only offers what the capability matrix declares.
 * The matrix comes from get_migration_capabilities (backend, no network);
 * everything else renders as not available and cannot be started.
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

/** The matrix the backend declares: tracks everywhere, playlists only for
 * qobuz/tidal, albums/artists nowhere. Same shape as get_migration_capabilities. */
const capabilities = ['qobuz', 'tidal', 'spotify'].map(service => ({
  service,
  collections: [
    { collection: 'tracks', supported: true },
    { collection: 'albums', supported: false },
    { collection: 'artists', supported: false },
    { collection: 'playlists', supported: service === 'qobuz' || service === 'tidal' },
  ],
}))

type InvokeCalls = Array<{ cmd: string; args?: Record<string, unknown> }>

function backend(overrides: Record<string, unknown> = {}, calls?: InvokeCalls): void {
  mockInvoke((cmd, args) => {
    calls?.push({ cmd, args })
    if (cmd in overrides) return overrides[cmd]
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

/** The content step needs a source: pick qobuz (connected) first, so spotify
 * and tidal remain selectable as destinations. */
async function goToContentStep(wrapper: VueWrapper): Promise<void> {
  await wrapper.find('[data-service-id="qobuz"]').trigger('click')
  await flushPromises()
  await clickNext(wrapper) // -> content
}

async function selectContent(wrapper: VueWrapper, label: string): Promise<void> {
  const card = wrapper.findAll('.content-card').find(c => c.text().includes(label))
  expect(card, `content card "${label}" should exist`).toBeTruthy()
  await card!.trigger('click')
  await flushPromises()
}

describe('M00 capability matrix gating', () => {
  beforeEach(() => {
    resetMocks()
    vi.clearAllMocks()
  })

  it('shows collections not declared by any destination as not available and disabled', async () => {
    backend()
    const wrapper = await mountView()
    await goToContentStep(wrapper) // -> content step

    const unavailable = wrapper.findAll('[data-testid="content-unavailable"]')
    expect(unavailable.length).toBe(2)
    const disabledCards = unavailable.map(p => p.element.closest('button')!.textContent ?? '')
    expect(disabledCards.join(' ')).toContain('Favorite Albums')
    expect(disabledCards.join(' ')).toContain('Followed Artists')
    for (const card of disabledCards) {
      expect(card).toContain('Not available on any migration destination')
    }
    wrapper.unmount()
  })

  it('keeps favorites and playlists offered because some destination declares them', async () => {
    backend()
    const wrapper = await mountView()
    await goToContentStep(wrapper) // -> content step

    const enabled = wrapper.findAll('.content-card').filter(c => c.attributes('disabled') === undefined)
    const labels = enabled.map(c => c.text())
    expect(labels.join(' ')).toContain('Favorites & Tracks')
    expect(labels.join(' ')).toContain('Playlists')
    wrapper.unmount()
  })

  it('exposes every destination real capability on the destination cards', async () => {
    backend()
    const wrapper = await mountView()
    await goToContentStep(wrapper) // -> content
    await clickNext(wrapper) // -> destinations

    const chipsFor = (service: string, label: string) =>
      wrapper.find(`[data-testid="capability-chips-${service}"]`).findAll('span')
        .find(s => s.text() === label)

    // Qobuz declares playlists: its chip is offered, not struck through.
    const qobuzPlaylists = chipsFor('qobuz', 'Playlists')
    expect(qobuzPlaylists, 'qobuz playlists chip').toBeTruthy()
    expect(qobuzPlaylists!.classes().some(c => c.includes('line-through'))).toBe(false)
    // Spotify does not: the same collection renders struck through there.
    const spotifyPlaylists = chipsFor('spotify', 'Playlists')
    expect(spotifyPlaylists, 'spotify playlists chip').toBeTruthy()
    expect(spotifyPlaylists!.classes().some(c => c.includes('line-through'))).toBe(true)
    // Tracks are declared for both.
    expect(chipsFor('qobuz', 'Tracks')).toBeTruthy()
    expect(chipsFor('spotify', 'Tracks')).toBeTruthy()
    wrapper.unmount()
  })

  it('blocks the preview step when a selected collection is unsupported by a chosen destination', async () => {
    const calls: InvokeCalls = []
    backend({}, calls)
    const wrapper = await mountView()
    await goToContentStep(wrapper) // -> content

    await selectContent(wrapper, 'Playlists')
    await clickNext(wrapper) // -> destinations

    await wrapper.find('[data-service-id="spotify"]').trigger('click')
    await flushPromises()
    // Spotify does not declare playlists: the step shows the conflict and Next cannot advance.
    expect(wrapper.find('[data-testid="capability-conflict"]').text()).toContain('Spotify')
    await clickNext(wrapper) // -> must NOT advance to preview
    expect(wrapper.find('[data-testid="preview-description"]').exists()).toBe(false)
    expect(calls.some(c => c.cmd === 'preview_migration')).toBe(false)
    expect(calls.some(c => c.cmd === 'start_migration')).toBe(false)
    wrapper.unmount()
  })

  it('reaches the preview with playlists selected when the destination declares them', async () => {
    const calls: InvokeCalls = []
    backend({}, calls)
    const wrapper = await mountView()
    await goToContentStep(wrapper) // -> content

    await selectContent(wrapper, 'Playlists')
    await clickNext(wrapper) // -> destinations

    await wrapper.find('[data-service-id="tidal"]').trigger('click')
    await flushPromises()
    // Tidal declares playlists: no conflict, and the preview runs with the
    // playlist flag the user selected.
    expect(wrapper.find('[data-testid="capability-conflict"]').exists()).toBe(false)
    await clickNext(wrapper) // -> preview
    expect(wrapper.find('[data-testid="preview-description"]').exists()).toBe(true)
    const previewCall = calls.find(c => c.cmd === 'preview_migration')
    expect(previewCall).toBeTruthy()
    expect((previewCall!.args as { options?: { create_playlists?: boolean } }).options?.create_playlists).toBe(true)
    wrapper.unmount()
  })

  it('drops the stale preselection when the matrix answers nothing', async () => {
    backend({ get_migration_capabilities: [] })
    const wrapper = await mountView()
    await goToContentStep(wrapper) // -> content step

    expect(wrapper.findAll('.content-card').every(c => c.attributes('disabled') !== undefined)).toBe(true)
    wrapper.unmount()
  })
})

/**
 * FE-12 regression — CommandPalette "Show N more tracks..." pagination.
 *
 * The button was decorative (no @click) while the list was hard-sliced to 5;
 * it must reveal the next page on click and keyboard navigation must be able
 * to reach the remaining tracks too.
 */
import { afterEach, describe, expect, it, vi } from 'vitest'
import { flushPromises, mount } from '@vue/test-utils'
import CommandPalette from '@/components/CommandPalette.vue'
import * as libraryApi from '@/api/library'

vi.mock('vue-router', () => ({
  useRouter: () => ({ push: vi.fn() }),
}))

vi.mock('@/api/library', () => ({
  searchTracks: vi.fn(),
}))

function resultsPage(count: number) {
  return {
    tracks: Array.from({ length: count }, (_, i) => ({
      id: i + 1,
      title: `Track ${i + 1}`,
      artist: 'Artist',
      album: 'Album',
      duration_secs: 1,
    })),
    total: count,
    offset: 0,
    limit: count,
    has_more: false,
  } as any
}

async function searchAndAwait(results: number) {
  vi.mocked(libraryApi.searchTracks).mockResolvedValueOnce(resultsPage(results) as any)
  const input = document.body.querySelector('input') as HTMLInputElement
  input.value = 'track'
  input.dispatchEvent(new Event('input'))
  // Debounced search fires after 200ms.
  await vi.advanceTimersByTimeAsync(200)
  await flushPromises()
  return input
}

describe('CommandPalette track pagination (FE-12)', () => {
  afterEach(() => {
    vi.useRealTimers()
    vi.clearAllMocks()
    document.body.innerHTML = ''
  })

  it('renders one page and reveals the rest when "Show more" is clicked', async () => {
    vi.useFakeTimers()
    const wrapper = mount(CommandPalette, { attachTo: document.body })
    ;(wrapper.vm as any).open()
    await flushPromises()

    const input = await searchAndAwait(7)

    let items = document.body.querySelectorAll('.result-track')
    expect(items.length).toBe(5)

    const moreBtn = [...document.body.querySelectorAll('button')].find(b =>
      /Show 2 more tracks/.test(b.textContent || '')
    )
    expect(moreBtn).toBeDefined()
    moreBtn!.dispatchEvent(new MouseEvent('click', { bubbles: true }))
    await flushPromises()

    items = document.body.querySelectorAll('.result-track')
    expect(items.length).toBe(7)
    const remainingBtn = [...document.body.querySelectorAll('button')].find(b =>
      /Show \d+ more tracks/.test(b.textContent || '')
    )
    expect(remainingBtn).toBeUndefined()

    wrapper.unmount()
  })

  it('expands the next page when keyboard navigation passes the last visible track', async () => {
    vi.useFakeTimers()
    const wrapper = mount(CommandPalette, { attachTo: document.body })
    ;(wrapper.vm as any).open()
    await flushPromises()

    const input = await searchAndAwait(7)

    // Start at selectedIndex 0; the 5th ArrowDown leaves the last visible
    // track and must reveal the next page.
    for (let i = 0; i < 5; i++) {
      input.dispatchEvent(new KeyboardEvent('keydown', { key: 'ArrowDown', bubbles: true }))
    }
    await flushPromises()

    expect(document.body.querySelectorAll('.result-track').length).toBe(7)

    wrapper.unmount()
  })

  it('resets pagination when the palette is reopened', async () => {
    vi.useFakeTimers()
    const wrapper = mount(CommandPalette, { attachTo: document.body })
    ;(wrapper.vm as any).open()
    await flushPromises()

    let input = await searchAndAwait(7)
    const moreBtn = [...document.body.querySelectorAll('button')].find(b =>
      /Show \d+ more tracks/.test(b.textContent || '')
    )
    moreBtn!.dispatchEvent(new MouseEvent('click', { bubbles: true }))
    await flushPromises()
    expect(document.body.querySelectorAll('.result-track').length).toBe(7)

    ;(wrapper.vm as any).close()
    await flushPromises()
    ;(wrapper.vm as any).open()
    await flushPromises()

    input = await searchAndAwait(7)
    expect(document.body.querySelectorAll('.result-track').length).toBe(5)
    expect([...document.body.querySelectorAll('button')].some(b =>
      /Show \d+ more tracks/.test(b.textContent || '')
    )).toBe(true)

    wrapper.unmount()
  })
})

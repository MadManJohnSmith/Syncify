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

function deferred<T>() {
  let resolve!: (value: T) => void
  const promise = new Promise<T>((done) => {
    resolve = done
  })
  return { promise, resolve }
}

function page(title: string) {
  return {
    tracks: [{ id: title.length, title, artist: 'Artist', album: 'Album', duration_secs: 1 }],
    total: 1,
    offset: 0,
    limit: 10,
    has_more: false,
  } as any
}

describe('CommandPalette search concurrency', () => {
  afterEach(() => {
    vi.useRealTimers()
    vi.clearAllMocks()
    document.body.innerHTML = ''
  })

  it('keeps the newest results when an older request resolves last', async () => {
    vi.useFakeTimers()
    const older = deferred<ReturnType<typeof page>>()
    const newer = deferred<ReturnType<typeof page>>()
    vi.mocked(libraryApi.searchTracks)
      .mockReturnValueOnce(older.promise)
      .mockReturnValueOnce(newer.promise)

    const wrapper = mount(CommandPalette, { attachTo: document.body })
    wrapper.vm.open()
    await flushPromises()

    const input = document.body.querySelector('input') as HTMLInputElement
    input.value = 'older'
    input.dispatchEvent(new Event('input'))
    await vi.advanceTimersByTimeAsync(200)

    input.value = 'newer'
    input.dispatchEvent(new Event('input'))
    await vi.advanceTimersByTimeAsync(200)

    newer.resolve(page('Newest result'))
    await flushPromises()
    expect(document.body.textContent).toContain('Newest result')

    older.resolve(page('Stale result'))
    await flushPromises()
    expect(document.body.textContent).toContain('Newest result')
    expect(document.body.textContent).not.toContain('Stale result')

    wrapper.unmount()
  })
})

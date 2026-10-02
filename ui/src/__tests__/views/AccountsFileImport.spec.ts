/**
 * AccountsFileImport.spec.ts
 * FE-6: import de playlists desde archivo (.m3u/.m3u8/.csv/.txt) y envío real
 * de las opciones del diálogo de scan (watchForChanges / skipSmallFiles).
 *
 * Estos casos son de regresión: antes `processImportedFile` era un stub que
 * solo mostraba un toast "will be available in an upcoming update" y el scan
 * del diálogo solo enviaba `recursive`.
 */
import { describe, it, expect, beforeEach, vi } from 'vitest'
import { mount, flushPromises, type VueWrapper } from '@vue/test-utils'
import AccountsView from '@/views/AccountsView.vue'
import { useToast } from '@/composables/useToast'
import { mockInvoke, resetMocks } from '../setup'

vi.mock('vue-router', () => ({
  useRouter: () => ({ push: vi.fn() }),
  useRoute: () => ({ query: {}, params: {} }),
}))

const invokeMock = vi.hoisted(() => vi.fn())

interface InvokeCall {
  command: string
  args?: Record<string, unknown>
}

function callsTo(command: string): InvokeCall[] {
  return invokeMock.mock.calls
    .map(([command, args]) => ({ command, args }))
    .filter((call) => call.command === command)
}

/**
 * jsdom (entorno de vitest) no implementa `Blob.text()`, que sí existe en el
 * WebView real y es lo que usa `processImportedFile`. Se añade solo cuando el
 * runtime no lo provee; el contenido es el del propio File.
 */
function makeFile(content: string, name: string): File {
  const file = new File([content], name)
  if (typeof (file as File & { text?: unknown }).text !== 'function') {
    Object.defineProperty(file, 'text', {
      value: async () => content,
      configurable: true,
    })
  }
  return file
}

const M3U_CONTENT = [
  '#EXTM3U',
  '#EXTINF:245,Boards of Canada - Roygbiv',
  '/music/roygbiv.mp3',
  '',
].join('\n')

const IMPORT_RESULT = {
  playlistId: 42,
  playlistName: 'Road Trip',
  totalEntries: 2,
  matchedCount: 1,
  unmatched: [{ title: 'Never Matched', artist: 'Ghost Band' }],
}

function mountView(): VueWrapper {
  return mount(AccountsView, { global: { stubs: { teleport: true } } })
}

function dropZone(wrapper: VueWrapper) {
  const zone = wrapper.find('.drag-drop-area')
  expect(zone.exists()).toBe(true)
  return zone
}

/** `addToast` inserta con unshift: el toast más reciente es el índice 0. */
function newestToast() {
  const { toasts } = useToast()
  return toasts.value[0]
}

describe('FE-6 AccountsView: import de playlist desde archivo', () => {
  beforeEach(() => {
    resetMocks()
    invokeMock.mockReset()
    mockInvoke((command: string, args?: Record<string, unknown>) => {
      invokeMock(command, args)
      if (command === 'import_playlist_from_file') return IMPORT_RESULT
      return undefined
    })
  })

  it('1. Al soltar un .m3u llama a import_playlist_from_file con nombre y contenido del archivo', async () => {
    const wrapper = mountView()
    await flushPromises()

    await dropZone(wrapper).trigger('drop', {
      dataTransfer: { files: [makeFile(M3U_CONTENT, 'Road Trip.m3u')] },
    })
    await flushPromises()

    const calls = callsTo('import_playlist_from_file')
    expect(calls).toHaveLength(1)
    expect(calls[0]!.args).toMatchObject({
      fileName: 'Road Trip.m3u',
      content: M3U_CONTENT,
      name: null,
      accountId: null,
    })
  })

  it('2. El resultado se informa con los conteos reales y los unmatched, sin inventar pistas', async () => {
    const wrapper = mountView()
    await flushPromises()

    await dropZone(wrapper).trigger('drop', {
      dataTransfer: { files: [makeFile(M3U_CONTENT, 'Road Trip.m3u')] },
    })
    await flushPromises()

    const toast = newestToast()
    expect(toast?.type).toBe('info')
    expect(toast?.title).toContain('Road Trip')
    expect(toast?.title).toContain('1 of 2 tracks matched')
    expect(toast?.title).toContain('Ghost Band - Never Matched')
    expect(toast?.title).not.toContain('upcoming update')
  })

  it('3. Seleccionar el archivo por el input dispara el mismo comando', async () => {
    const wrapper = mountView()
    await flushPromises()

    const input = wrapper.find('input[type="file"]')
    expect(input.exists()).toBe(true)

    const file = makeFile(M3U_CONTENT, 'Road Trip.m3u')
    Object.defineProperty(input.element, 'files', { value: [file], configurable: true })
    await input.trigger('change')
    await flushPromises()

    const calls = callsTo('import_playlist_from_file')
    expect(calls).toHaveLength(1)
    expect(calls[0]!.args).toMatchObject({ fileName: 'Road Trip.m3u' })
  })

  it('4. Una extensión no soportada se rechaza sin invocar el backend', async () => {
    const wrapper = mountView()
    await flushPromises()

    await dropZone(wrapper).trigger('drop', {
      dataTransfer: { files: [makeFile('[]', 'tracks.json')] },
    })
    await flushPromises()

    expect(callsTo('import_playlist_from_file')).toHaveLength(0)
    expect(newestToast()?.type).toBe('error')
  })

  it('5. Un error del backend se muestra como error (nunca como éxito)', async () => {
    resetMocks()
    mockInvoke((command: string, args?: Record<string, unknown>) => {
      invokeMock(command, args)
      if (command === 'import_playlist_from_file') {
        throw 'No hay ninguna cuenta configurada: conecta un servicio antes de importar una playlist'
      }
      return undefined
    })

    const wrapper = mountView()
    await flushPromises()

    await dropZone(wrapper).trigger('drop', {
      dataTransfer: { files: [makeFile(M3U_CONTENT, 'Road Trip.m3u')] },
    })
    await flushPromises()

    expect(newestToast()?.type).toBe('error')
    expect(newestToast()?.title).toContain('Playlist import failed')
  })
})

describe('FE-6 AccountsView: opciones reales del diálogo de scan', () => {
  beforeEach(() => {
    resetMocks()
    invokeMock.mockReset()
    mockInvoke((command: string, args?: Record<string, unknown>) => {
      invokeMock(command, args)
      if (command === 'scan_local_library_with_progress') {
        return { success: true, data: { total_files: 3, errors: [] } }
      }
      return undefined
    })
  })

  it('6. startScan envía watchForChanges y skipSmallFiles, no solo recursive', async () => {
    const wrapper = mountView()
    await flushPromises()

    const addPath = wrapper.findAll('button').find((b) => b.text().includes('Add Library Path'))
    expect(addPath).toBeDefined()
    await addPath!.trigger('click')
    await flushPromises()

    const pathInput = wrapper.find('input[placeholder="C:/Music/Library"]')
    expect(pathInput.exists()).toBe(true)
    await pathInput.setValue('/home/user/Music')

    const checkboxes = wrapper.findAll('input[type="checkbox"]')
    const watchBox = checkboxes.find((c) =>
      c.element.closest('label')?.textContent?.includes('Watch for changes'),
    )
    const skipBox = checkboxes.find((c) =>
      c.element.closest('label')?.textContent?.includes('Skip files under 1 MB'),
    )
    expect(watchBox).toBeDefined()
    expect(skipBox).toBeDefined()
    await watchBox!.setValue(true)
    await skipBox!.setValue(true)

    const startScan = wrapper.findAll('button').find((b) => b.text().includes('Start Scan'))
    expect(startScan).toBeDefined()
    await startScan!.trigger('click')
    await flushPromises()

    const calls = callsTo('scan_local_library_with_progress')
    expect(calls).toHaveLength(1)
    expect(calls[0]!.args).toMatchObject({
      directory: '/home/user/Music',
      watchForChanges: true,
      skipSmallFiles: true,
    })
    expect(calls[0]!.args?.recursive).toBe(true)
  })
})

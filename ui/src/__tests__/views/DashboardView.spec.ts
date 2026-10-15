/**
 * Unit tests for DashboardView.vue (TASK-128)
 * Verifies zero fictitious mocks, honest empty states with CTAs,
 * real library growth derivations, and recent activity population.
 */
import { describe, it, expect, vi, beforeEach } from 'vitest'
import { mount, flushPromises } from '@vue/test-utils'
import DashboardView from '@/views/DashboardView.vue'
import { mockInvoke, resetMocks } from '../setup'

const mockPush = vi.fn()

vi.mock('vue-router', () => ({
  useRouter: () => ({ push: mockPush }),
  useRoute: () => ({ query: {}, params: {} }),
}))

describe('DashboardView.vue (TASK-128 UI Completion)', () => {
  beforeEach(() => {
    resetMocks()
    mockPush.mockReset()

    mockInvoke((command) => {
      if (command === 'create_library_snapshot') return null
      if (command === 'get_library_stats') {
        return {
          total_tracks: 0,
          total_albums: 0,
          total_artists: 0,
          playlists: 0,
          total_downloads: 0,
          queued_downloads: 0,
          active_downloads: 0,
        }
      }
      if (command === 'get_queue_stats') {
        return { total: 0, queued: 0, downloading: 0, completed: 0, failed: 0, paused: 0 }
      }
      if (command === 'get_service_statuses') return []
      if (command === 'get_metadata_stats') {
        return { total_tracks: 0, complete: 0, missing: 0, lyrics_count: 0 }
      }
      if (command === 'get_lyrics_stats') {
        return { total: 0, synchronized: 0, unsynchronized: 0, missing: 0 }
      }
      if (command === 'get_library_snapshots') return []
      if (command === 'get_storage_stats') {
        return { used_bytes: 0, available_bytes: 107374182400 }
      }
      if (command === 'get_top_artists') return []
      if (command === 'get_top_genres') return []
      if (command === 'get_audio_quality_distribution') return []
      if (command === 'get_duplicate_stats') return 0
      if (command === 'get_queue') return []
      if (command === 'check_ffmpeg') return { success: true }
      if (command === 'check_fingerprint') return { success: true }
      return null
    })
  })

  it('renders without fictitious mock data (no hardcoded Jan/Feb/Mar bars)', async () => {
    const wrapper = mount(DashboardView)
    await flushPromises()

    const text = wrapper.text()
    // Should NOT contain fabricated months from the former mock array
    expect(text).not.toContain('Jan')
    expect(text).not.toContain('Feb')
    expect(text).not.toContain('Mar')
    expect(text).not.toContain('Apr')
    expect(text).not.toContain('Jun')
    expect(text).not.toContain('Jul')
  })

  it('renders honest empty states with CTAs when library and queue are brand new', async () => {
    const wrapper = mount(DashboardView)
    await flushPromises()

    // Library growth honest empty state
    expect(wrapper.text()).toContain('No library growth data')
    expect(wrapper.text()).toContain('Track trends and history will appear here once tracks are imported.')

    // Recent activity honest empty state
    expect(wrapper.text()).toContain('No recent activity')
    expect(wrapper.text()).toContain('Activity from downloads and queue tasks will appear here.')

    // Validate CTAs are present and clickable
    const libraryCta = wrapper.findAll('button').find(b => b.text().includes('Go to Library'))
    expect(libraryCta).toBeDefined()
    await libraryCta!.trigger('click')
    expect(mockPush).toHaveBeenCalledWith('/library')

    const queueCta = wrapper.findAll('button').find(b => b.text().includes('Go to Queue'))
    expect(queueCta).toBeDefined()
    await queueCta!.trigger('click')
    expect(mockPush).toHaveBeenCalledWith('/queue')
  })

  it('derives growth stats from real library stats when historical snapshots are empty', async () => {
    mockInvoke((command) => {
      if (command === 'get_library_stats') {
        return {
          total_tracks: 100,
          total_albums: 10,
          total_artists: 5,
          playlists: 2,
          total_downloads: 60,
          queued_downloads: 0,
          active_downloads: 0,
        }
      }
      if (command === 'get_library_snapshots') return []
      return null
    })

    const wrapper = mount(DashboardView)
    await flushPromises()

    expect(wrapper.text()).not.toContain('No library growth data')
    expect(wrapper.text()).toContain('Current')
    expect(wrapper.text()).toContain('Total tracks')
    expect(wrapper.text()).toContain('Downloaded')
  })

  it('renders historical snapshot growth bars when snapshots exist', async () => {
    mockInvoke((command) => {
      if (command === 'get_library_snapshots') {
        return [
          { snapshot_date: '2025-03-15', total_tracks: 50, downloaded_tracks: 20 },
          { snapshot_date: '2025-03-16', total_tracks: 80, downloaded_tracks: 40 },
        ]
      }
      return null
    })

    const wrapper = mount(DashboardView)
    await flushPromises()

    expect(wrapper.text()).not.toContain('No library growth data')
    expect(wrapper.text()).toContain('03/15')
    expect(wrapper.text()).toContain('03/16')
  })

  it('populates recent activity from real queue downloads', async () => {
    mockInvoke((command) => {
      if (command === 'get_queue') {
        return [
          {
            id: 101,
            track_id: 1,
            title: 'Stairway to Heaven',
            artist: 'Led Zeppelin',
            status: 'complete',
            priority: 1,
            progress_percent: 100,
            created_at: new Date(Date.now() - 3600000).toISOString(),
            completed_at: new Date(Date.now() - 1800000).toISOString(),
          },
          {
            id: 102,
            track_id: 2,
            title: 'Bohemian Rhapsody',
            artist: 'Queen',
            status: 'downloading',
            priority: 1,
            progress_percent: 50,
            created_at: new Date(Date.now() - 60000).toISOString(),
            started_at: new Date(Date.now() - 30000).toISOString(),
          },
          {
            id: 103,
            track_id: 3,
            title: 'Hotel California',
            artist: 'Eagles',
            status: 'failed',
            priority: 1,
            progress_percent: 10,
            created_at: new Date(Date.now() - 120000).toISOString(),
            error_message: 'Stream unavailable',
          },
        ]
      }
      return null
    })

    const wrapper = mount(DashboardView)
    await flushPromises()

    expect(wrapper.text()).not.toContain('No recent activity')
    expect(wrapper.text()).toContain('Downloaded: Stairway to Heaven • Led Zeppelin')
    expect(wrapper.text()).toContain('Downloading: Bohemian Rhapsody • Queen')
    expect(wrapper.text()).toContain('Failed: Hotel California • Eagles')
  })

  it('D-01: does not render System Diagnostics widget', async () => {
    const wrapper = mount(DashboardView)
    await flushPromises()

    expect(wrapper.find('.system-diagnostics').exists()).toBe(false)
    expect(wrapper.text()).not.toContain('System Diagnostics')
    expect(wrapper.text()).not.toContain('FFmpeg')
    expect(wrapper.text()).not.toContain('Chromaprint')
  })

  it('D-03 and D-04: Storage Usage and Lyrics Coverage have reduced col-span-2', async () => {
    const wrapper = mount(DashboardView)
    await flushPromises()

    const storageCard = wrapper.find('.storage-usage')
    expect(storageCard.exists()).toBe(true)
    expect(storageCard.classes()).toContain('xl:col-span-2')

    const lyricsCard = wrapper.find('.lyrics-coverage')
    expect(lyricsCard.exists()).toBe(true)
    expect(lyricsCard.classes()).toContain('xl:col-span-2')
  })

  it('D-05: formats service names centralized without underscore artifacts', async () => {
    mockInvoke((command) => {
      if (command === 'get_service_statuses') {
        return [
          { name: 'apple_music', connected: true, library_count: 50 },
          { name: 'spotify', connected: true, library_count: 100 },
        ]
      }
      return null
    })

    const wrapper = mount(DashboardView)
    await flushPromises()

    const sources = wrapper.find('.service-distribution')
    expect(sources.text()).toContain('Apple Music')
    expect(sources.text()).not.toContain('Apple_music')
    expect(sources.text()).not.toContain('apple_music')
    expect(sources.text()).toContain('Spotify')
  })

  it('D-06: displays clean empty state when no sources are connected', async () => {
    mockInvoke((command) => {
      if (command === 'get_service_statuses') return []
      return null
    })

    const wrapper = mount(DashboardView)
    await flushPromises()

    const sources = wrapper.find('.service-distribution')
    expect(sources.text()).toContain('No sources connected')
  })

  it('D-07: library overview positions separator with mt-auto to avoid overlap', async () => {
    const wrapper = mount(DashboardView)
    await flushPromises()

    const overview = wrapper.find('.library-overview')
    const footer = overview.find('.border-t')
    expect(footer.exists()).toBe(true)
    expect(footer.classes()).toContain('mt-auto')
  })
})

/**
 * WF-DASH-01 — tarjeta "Sources" del dashboard.
 *
 * Contrato verificado contra el backend:
 *  - `get_service_statuses` (src-tauri/src/commands/service.rs:1219-1251) devuelve
 *    `library_count` = `COUNT(*) FROM library_entries le WHERE le.account_id = a.id`
 *    de LA cuenta activa de ese servicio. `library_entries` tiene
 *    UNIQUE(account_id, track_id) (migrations/0002_normalize_schema.sql:144), así
 *    que la misma pista en dos cuentas cuenta 1 en cada una.
 *  - `total_tracks` del dashboard viene de la VIEW `library_stats`
 *    (migrations/0085_alignment_audit.sql:212-218) = `COUNT(*) FROM tracks`,
 *    pistas únicas.
 *  Por tanto una pista presente en dos servicios suma cobertura en ambas barras:
 *  miden cobertura de biblioteca, NO reparto exclusivo, y no suman 100 %.
 */
describe('DashboardView.vue — Sources coverage card (WF-DASH-01)', () => {
  beforeEach(() => {
    resetMocks()
    mockPush.mockReset()

    mockInvoke((command) => {
      if (command === 'create_library_snapshot') return null
      if (command === 'get_library_stats') return zeroLibraryStats()
      if (command === 'get_queue_stats') return emptyQueueStats()
      if (command === 'get_metadata_stats') return emptyMetadataStats()
      if (command === 'get_lyrics_stats') return emptyLyricsStats()
      if (command === 'get_library_snapshots') return []
      if (command === 'get_storage_stats') return { used_bytes: 0, available_bytes: 107374182400 }
      if (command === 'get_top_artists') return []
      if (command === 'get_top_genres') return []
      if (command === 'get_audio_quality_distribution') return []
      if (command === 'get_duplicate_stats') return 0
      if (command === 'get_queue') return []
      return null
    })
  })

  function zeroLibraryStats() {
    return {
      total_tracks: 0, total_albums: 0, total_artists: 0, playlists: 0,
      total_downloads: 0, queued_downloads: 0, active_downloads: 0,
    }
  }

  function emptyQueueStats() {
    return { total: 0, queued: 0, downloading: 0, completed: 0, failed: 0, paused: 0 }
  }

  function emptyMetadataStats() {
    return { total_tracks: 0, complete: 0, missing: 0, lyrics_count: 0 }
  }

  function emptyLyricsStats() {
    return { total: 0, synchronized: 0, unsynchronized: 0, missing: 0 }
  }

  function serviceStatus(entries: { name: string, library_count: number, connected?: boolean }[]) {
    return entries.map(e => ({
      name: e.name,
      connected: e.connected ?? true,
      account_email: `${e.name}@example.test`,
      library_count: e.library_count,
      favorites_count: 0,
      playlists_count: 0,
      last_synced: null,
      credentials_invalid: false,
    }))
  }

  function renderSources(withTracks = 0) {
    mockInvoke((command) => {
      if (command === 'create_library_snapshot') return null
      if (command === 'get_library_stats') {
        return {
          ...zeroLibraryStats(),
          total_tracks: withTracks,
        }
      }
      if (command === 'get_queue_stats') return emptyQueueStats()
      if (command === 'get_service_statuses') {
        return serviceStatus([
          { name: 'qobuz', library_count: 30 },
          { name: 'spotify', library_count: 30 },
          { name: 'deezer', library_count: 40 },
        ])
      }
      if (command === 'get_metadata_stats') return emptyMetadataStats()
      if (command === 'get_lyrics_stats') return emptyLyricsStats()
      if (command === 'get_library_snapshots') return []
      if (command === 'get_storage_stats') return { used_bytes: 0, available_bytes: 107374182400 }
      if (command === 'get_top_artists') return []
      if (command === 'get_top_genres') return []
      if (command === 'get_audio_quality_distribution') return []
      if (command === 'get_duplicate_stats') return 0
      if (command === 'get_queue') return []
      return null
    })
  }

  function sourceRows(wrapper: ReturnType<typeof mount>) {
    return wrapper.find('.service-distribution').findAll('.space-y-2 > div')
  }

  it('shows library coverage, not an exclusive share that must sum to 100%', async () => {
    renderSources(100)
    const wrapper = mount(DashboardView)
    await flushPromises()

    const rows = sourceRows(wrapper)
    expect(rows.length).toBe(3)
    // 30/100, 30/100 y 40/100: la suma es 100 % solo porque aquí no solapan.
    expect(rows[0].text()).toContain('30')
    expect(rows[0].text()).toContain('30%')
    expect(rows[1].text()).toContain('30')
    expect(rows[1].text()).toContain('30%')
    expect(rows[2].text()).toContain('40')
    expect(rows[2].text()).toContain('40%')

    // La tarjeta debe declarar que las barras solapan y no suman 100 %.
    expect(wrapper.find('.service-distribution').text())
      .toContain('do not sum to 100%')
  })

  it('counts a track present in several services as coverage in each service (no normalization to a share)', async () => {
    // 60 pistas únicas reales, pero 30 + 30 + 40 = 100 entradas contadas.
    renderSources(60)
    const wrapper = mount(DashboardView)
    await flushPromises()

    const rows = sourceRows(wrapper)
    // Ninguna barra se recorta para que el total quepa en 100 %.
    expect(rows[0].text()).toContain('50%')  // 30/60
    expect(rows[1].text()).toContain('50%')  // 30/60, solapada con Qobuz
    expect(rows[2].text()).toContain('67%')  // 40/60
  })

  it('does not divide by zero and shows 0% bars when the library has no tracks', async () => {
    renderSources(0)
    const wrapper = mount(DashboardView)
    await flushPromises()

    const rows = sourceRows(wrapper)
    expect(rows.length).toBe(3)
    for (const row of rows) {
      expect(row.text()).toContain('0%')
    }
  })

  it('keeps one bar at 0% when a connected service has an empty library', async () => {
    mockInvoke((command) => {
      if (command === 'create_library_snapshot') return null
      if (command === 'get_library_stats') {
        return { ...zeroLibraryStats(), total_tracks: 100 }
      }
      if (command === 'get_queue_stats') return emptyQueueStats()
      if (command === 'get_service_statuses') {
        return serviceStatus([
          { name: 'spotify', library_count: 40 },
          { name: 'deezer', library_count: 0 },
        ])
      }
      if (command === 'get_metadata_stats') return emptyMetadataStats()
      if (command === 'get_lyrics_stats') return emptyLyricsStats()
      if (command === 'get_library_snapshots') return []
      if (command === 'get_storage_stats') return { used_bytes: 0, available_bytes: 107374182400 }
      if (command === 'get_top_artists') return []
      if (command === 'get_top_genres') return []
      if (command === 'get_audio_quality_distribution') return []
      if (command === 'get_duplicate_stats') return 0
      if (command === 'get_queue') return []
      return null
    })

    const wrapper = mount(DashboardView)
    await flushPromises()

    const rows = sourceRows(wrapper)
    expect(rows[0].text()).toContain('40%')
    expect(rows[1].text()).toContain('0')
    expect(rows[1].text()).toContain('0%')
  })

  it('assigns the color by normalized service name, not by array position', async () => {
    renderSources(100)
    const wrapper = mount(DashboardView)
    await flushPromises()

    const rows = sourceRows(wrapper)
    const barOf = (row: typeof rows[number]) => row.find('.h-full').classes()

    // Orden del backend: Qobuz, Spotify, Deezer.
    expect(rows[0].text()).toContain('Qobuz')
    expect(barOf(rows[0])).toContain('bg-blue-600')
    expect(rows[1].text()).toContain('Spotify')
    expect(barOf(rows[1])).toContain('bg-green-500')
    expect(rows[2].text()).toContain('Deezer')
    expect(barOf(rows[2])).toContain('bg-purple-500')

    // Mismo fixture con el orden invertido: cada servicio conserva su color.
    mockInvoke((command) => {
      if (command === 'create_library_snapshot') return null
      if (command === 'get_library_stats') {
        return { ...zeroLibraryStats(), total_tracks: 100 }
      }
      if (command === 'get_queue_stats') return emptyQueueStats()
      if (command === 'get_service_statuses') {
        return serviceStatus([
          { name: ' DEEZER ', library_count: 40 },
          { name: 'spotify', library_count: 30 },
          { name: 'QOBUZ', library_count: 30 },
        ])
      }
      if (command === 'get_metadata_stats') return emptyMetadataStats()
      if (command === 'get_lyrics_stats') return emptyLyricsStats()
      if (command === 'get_library_snapshots') return []
      if (command === 'get_storage_stats') return { used_bytes: 0, available_bytes: 107374182400 }
      if (command === 'get_top_artists') return []
      if (command === 'get_top_genres') return []
      if (command === 'get_audio_quality_distribution') return []
      if (command === 'get_duplicate_stats') return 0
      if (command === 'get_queue') return []
      return null
    })

    const reordered = mount(DashboardView)
    await flushPromises()

    const reorderedRows = sourceRows(reordered)
    expect(reorderedRows[0].text()).toContain('Deezer')
    expect(barOf(reorderedRows[0])).toContain('bg-purple-500')
    expect(reorderedRows[1].text()).toContain('Spotify')
    expect(barOf(reorderedRows[1])).toContain('bg-green-500')
    expect(reorderedRows[2].text()).toContain('Qobuz')
    expect(barOf(reorderedRows[2])).toContain('bg-blue-600')
  })

  it('reports one row per service when a service has several accounts (active account only)', async () => {
    // El backend ya acota a LA cuenta activa (service.rs:1232-1250, LEFT JOIN con
    // is_active = 1 ORDER BY id DESC LIMIT 1), así que dos cuentas del mismo
    // servicio llegan como UNA fila y no deben multiplicarse en la tarjeta.
    mockInvoke((command) => {
      if (command === 'create_library_snapshot') return null
      if (command === 'get_library_stats') {
        return { ...zeroLibraryStats(), total_tracks: 200 }
      }
      if (command === 'get_queue_stats') return emptyQueueStats()
      if (command === 'get_service_statuses') {
        // Qobuz: cuenta activa con 200 entradas (todas las pistas), cuenta
        // antigua con 0 — solo debe contarse la activa y sin duplicar filas.
        return serviceStatus([
          { name: 'qobuz', library_count: 200 },
          { name: 'spotify', library_count: 50 },
        ])
      }
      if (command === 'get_metadata_stats') return emptyMetadataStats()
      if (command === 'get_lyrics_stats') return emptyLyricsStats()
      if (command === 'get_library_snapshots') return []
      if (command === 'get_storage_stats') return { used_bytes: 0, available_bytes: 107374182400 }
      if (command === 'get_top_artists') return []
      if (command === 'get_top_genres') return []
      if (command === 'get_audio_quality_distribution') return []
      if (command === 'get_duplicate_stats') return 0
      if (command === 'get_queue') return []
      return null
    })

    const wrapper = mount(DashboardView)
    await flushPromises()

    const rows = sourceRows(wrapper)
    // Una fila por servicio, no una por cuenta.
    expect(rows.length).toBe(2)
    expect(rows[0].text()).toContain('Qobuz')
    expect(rows[0].text()).toContain('100%') // 200/200: cobertura total
    expect(rows[1].text()).toContain('Spotify')
    expect(rows[1].text()).toContain('25%')  // 50/200
  })

  it('falls back to a neutral color for an unmapped service name', async () => {
    mockInvoke((command) => {
      if (command === 'create_library_snapshot') return null
      if (command === 'get_library_stats') {
        return { ...zeroLibraryStats(), total_tracks: 10 }
      }
      if (command === 'get_queue_stats') return emptyQueueStats()
      if (command === 'get_service_statuses') {
        return serviceStatus([{ name: 'napster', library_count: 4 }])
      }
      if (command === 'get_metadata_stats') return emptyMetadataStats()
      if (command === 'get_lyrics_stats') return emptyLyricsStats()
      if (command === 'get_library_snapshots') return []
      if (command === 'get_storage_stats') return { used_bytes: 0, available_bytes: 107374182400 }
      if (command === 'get_top_artists') return []
      if (command === 'get_top_genres') return []
      if (command === 'get_audio_quality_distribution') return []
      if (command === 'get_duplicate_stats') return 0
      if (command === 'get_queue') return []
      return null
    })

    const wrapper = mount(DashboardView)
    await flushPromises()

    const rows = sourceRows(wrapper)
    expect(rows[0].text()).toContain('Napster')
    expect(rows[0].find('.h-full').classes()).toContain('bg-gray-500')
  })
})

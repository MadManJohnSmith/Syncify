/**
 * DownloadsViewQueueCovers.spec.ts
 * Regresión de la portada en las filas de la cola: `get_queue` pasará a traer
 * `cover_art_url` (otro carril lo añade al backend). Las filas deben pintarla
 * cuando llegue y caer al placeholder determinista cuando no venga, sin
 * romperse nunca por su ausencia (consumo defensivo).
 */
import { describe, it, expect, beforeEach } from 'vitest'
import { mount, flushPromises } from '@vue/test-utils'
import DownloadsView from '@/views/DownloadsView.vue'
import { mockInvoke, resetMocks } from '../setup'

function queueItem(overrides: Record<string, unknown> = {}) {
    return {
        id: 1,
        track_id: 101,
        target_title: 'Covered Track',
        target_artist: 'Artist',
        target_album: 'Album',
        service_name: 'qobuz',
        quality_preference: 'lossless',
        status: 'queued',
        priority: 50,
        progress_percent: 0,
        error_message: null,
        created_at: '2026-10-05T06:00:00Z',
        started_at: null,
        completed_at: null,
        ...overrides,
    }
}

describe('DownloadsView: portada defensiva en las filas de la cola', () => {
    beforeEach(() => {
        resetMocks()
        document.body.innerHTML = ''
    })

    it('pinta la portada cuando get_queue trae cover_art_url', async () => {
        mockInvoke((command) => {
            if (command === 'get_queue') {
                return [
                    queueItem({ id: 1, cover_art_url: 'https://cdn.test/cover-1.jpg' }),
                    queueItem({ id: 2, track_id: 102, target_title: 'Covered Two', cover_art_url: 'https://cdn.test/cover-2.jpg' }),
                ]
            }
            if (command === 'get_queue_stats') return { total: 2, queued: 2, downloading: 0, completed: 0, failed: 0, paused: 0 }
            if (command === 'get_worker_status') return { running: true, paused: false, active_downloads: 0, max_concurrent: 3 }
            return null
        })

        const wrapper = mount(DownloadsView)
        await flushPromises()

        const covers = wrapper.findAll('[data-testid="track-cover"]')
        expect(covers.length).toBeGreaterThanOrEqual(2)
        const srcs = covers.map(c => c.find('[data-testid="track-cover-img"]').attributes('src'))
        expect(srcs).toContain('https://cdn.test/cover-1.jpg')
        expect(srcs).toContain('https://cdn.test/cover-2.jpg')
    })

    it('cae al placeholder cuando cover_art_url no llega (aún) del backend', async () => {
        mockInvoke((command) => {
            if (command === 'get_queue') {
                return [
                    queueItem({ id: 1, cover_art_url: null }),
                    // El campo directamente ausente tampoco debe romper nada
                    queueItem({ id: 2, track_id: 102, target_title: 'No Cover Field' }),
                ]
            }
            if (command === 'get_queue_stats') return { total: 2, queued: 2, downloading: 0, completed: 0, failed: 0, paused: 0 }
            if (command === 'get_worker_status') return { running: true, paused: false, active_downloads: 0, max_concurrent: 3 }
            return null
        })

        const wrapper = mount(DownloadsView)
        await flushPromises()

        const covers = wrapper.findAll('[data-testid="track-cover"]')
        expect(covers.length).toBeGreaterThanOrEqual(2)
        // Sin URL no hay <img>: TrackCover pinta su gradiente de reserva
        for (const cover of covers) {
            expect(cover.find('[data-testid="track-cover-img"]').exists()).toBe(false)
        }
    })
})

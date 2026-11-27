/**
 * usePlayerProvider.spec.ts — P10b (#409): entradas efímeras desde catálogo.
 *
 * Reproducir un resultado del proveedor encola una entrada efímera y la carga
 * por `entry_id`; el motor resuelve la identidad del proveedor sin crear
 * `Track`. Encolar mientras suena otra pista no interrumpe la reproducción.
 */
import { beforeEach, describe, expect, it, vi } from 'vitest'
import { flushPromises } from '@vue/test-utils'

vi.mock('../../api/tauri', () => ({
    invokeCommand: vi.fn(),
}))

vi.mock('../../api/player', () => ({
    playerLoadTrack: vi.fn(),
    playerLoadQueueEntry: vi.fn(),
    playerPause: vi.fn(),
    playerResume: vi.fn(),
    playerStop: vi.fn(),
    playerSeek: vi.fn(),
    playerSetVolume: vi.fn(),
    playerSetRate: vi.fn(),
    onPlayerTick: vi.fn(),
    onPlayerStreamState: vi.fn(),
    onPlayerEnded: vi.fn(),
    onPlayerAdvanced: vi.fn(),
    onPlayerQueueEntryChanged: vi.fn(),
    onPlayerModesChanged: vi.fn(() => Promise.resolve()),
}))

import { invokeCommand } from '../../api/tauri'
import * as playerApi from '../../api/player'
import { usePlayer } from '../../composables/usePlayer'

const mockInvoke = vi.mocked(invokeCommand)
const engine = vi.mocked(playerApi)

function configureEngineMocks(): void {
    let seq = 0
    engine.playerLoadTrack.mockReset().mockImplementation(() =>
        Promise.resolve({ seq: ++seq, durationSec: 200, stream: null }))
    engine.playerLoadQueueEntry.mockReset().mockImplementation(() =>
        Promise.resolve({ seq: ++seq, durationSec: 180, stream: null }))
    engine.playerPause.mockReset().mockResolvedValue(undefined)
    engine.playerResume.mockReset().mockResolvedValue(undefined)
    engine.playerStop.mockReset().mockResolvedValue(undefined)
    engine.playerSeek.mockReset().mockResolvedValue(undefined)
    engine.playerSetVolume.mockReset().mockResolvedValue(undefined)
    engine.playerSetRate.mockReset().mockResolvedValue(undefined)
    engine.onPlayerTick.mockReset().mockResolvedValue(() => {})
    engine.onPlayerStreamState.mockReset().mockResolvedValue(() => {})
    engine.onPlayerEnded.mockReset().mockResolvedValue(() => {})
    engine.onPlayerAdvanced.mockReset().mockResolvedValue(() => {})
    engine.onPlayerQueueEntryChanged.mockReset().mockResolvedValue(() => {})
}

const providerItem = {
    serviceId: 2,
    serviceTrackId: 'qobuz-track-9',
    title: 'One More Time',
    artist: 'Daft Punk',
    album: 'Discovery',
    coverUrl: 'https://example.invalid/discovery.jpg',
}

describe('usePlayer — entradas efímeras de proveedor (P10b #409)', () => {
    beforeEach(() => {
        mockInvoke.mockReset()
        configureEngineMocks()
        usePlayer().stop()
        mockInvoke.mockClear()
    })

    it('reproduce un ítem del proveedor por entry_id sin crear pista de biblioteca', async () => {
        mockInvoke.mockImplementation((cmd: string) => {
            if (cmd === 'playback_queue_enqueue_provider_item') return Promise.resolve('entry-p')
            return Promise.resolve(undefined)
        })
        const player = usePlayer()

        await player.playProviderItem(providerItem)
        await flushPromises()

        expect(mockInvoke).toHaveBeenCalledWith('playback_queue_enqueue_provider_item', {
            serviceId: 2,
            serviceTrackId: 'qobuz-track-9',
            atFront: false,
        })
        expect(engine.playerLoadQueueEntry).toHaveBeenCalledWith('entry-p', true, 1)
        expect(engine.playerLoadTrack).not.toHaveBeenCalled()
        expect(player.current.value?.id).toBe(0)
        expect(player.current.value?.title).toBe('One More Time')
        expect(mockInvoke).toHaveBeenCalledWith('playback_queue_set_current_entry', {
            entryId: 'entry-p',
        })
    })

    it('encola sin interrumpir la pista actual cuando ya suena algo', async () => {
        mockInvoke.mockImplementation((cmd: string) => {
            if (cmd === 'playback_queue_enqueue_provider_item') return Promise.resolve('entry-p')
            return Promise.resolve(undefined)
        })
        const player = usePlayer()
        await player.play({ id: 1, title: 'Library Song', artist: 'X' })
        mockInvoke.mockClear()
        engine.playerLoadQueueEntry.mockClear()

        const outcome = await player.enqueueProvider(providerItem)

        expect(outcome).toBe('queued')
        expect(player.current.value?.id).toBe(1)
        expect(player.upNext.value).toHaveLength(1)
        expect(player.upNext.value[0].id).toBe(0)
        expect(player.upNext.value[0].title).toBe('One More Time')
        expect(engine.playerLoadQueueEntry).not.toHaveBeenCalled()
    })

    it('sin nada cargado, encolar arranca la reproducción como en la biblioteca', async () => {
        mockInvoke.mockImplementation((cmd: string) => {
            if (cmd === 'playback_queue_enqueue_provider_item') return Promise.resolve('entry-p')
            return Promise.resolve(undefined)
        })
        const player = usePlayer()

        const outcome = await player.enqueueProvider(providerItem)
        await flushPromises()

        expect(outcome).toBe('started')
        expect(engine.playerLoadQueueEntry).toHaveBeenCalledWith('entry-p', true, 1)
        expect(player.current.value?.id).toBe(0)
    })
})

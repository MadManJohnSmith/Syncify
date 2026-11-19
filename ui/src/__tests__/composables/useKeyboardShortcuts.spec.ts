/**
 * P04 (#135) — window playback shortcuts registered through
 * `useKeyboardShortcuts`.
 *
 * The actions are a simulated queue with its own index, position, volume,
 * mute, shuffle and repeat state, so the test checks the contract the real
 * wiring fulfils with `usePlayer` (whose mutations go through the backend
 * queue commands). The registry's single document listener dispatches the
 * bindings; nothing here mounts a component.
 */
import { describe, it, expect, beforeEach, afterEach, vi } from 'vitest'
import {
    PLAYBACK_SEEK_STEP_SEC,
    PLAYBACK_SHORTCUT_KEYS,
    PLAYBACK_SHORTCUTS_BLOCK_SELECTOR,
    PLAYBACK_VOLUME_STEP,
    getRegisteredShortcuts,
    registerPlaybackShortcuts,
    registerShortcut,
    type PlaybackShortcutActions,
} from '@/composables/useKeyboardShortcuts'

type RepeatMode = 'off' | 'all' | 'one'

interface SimulatedQueue {
    queue: number[]
    index: number
    playing: boolean
    positionSec: number
    volume: number
    muted: boolean
    shuffle: boolean
    repeat: RepeatMode
}

function createSimulatedPlayer(): { state: SimulatedQueue; actions: PlaybackShortcutActions } {
    const state: SimulatedQueue = {
        queue: [10, 20, 30],
        index: 0,
        playing: false,
        positionSec: 30,
        volume: 0.5,
        muted: false,
        shuffle: false,
        repeat: 'off',
    }

    const actions: PlaybackShortcutActions = {
        toggle: vi.fn(() => { state.playing = !state.playing }),
        seekBy: vi.fn((seconds: number) => {
            state.positionSec = Math.max(0, state.positionSec + seconds)
        }),
        next: vi.fn(() => {
            state.index = Math.min(state.queue.length - 1, state.index + 1)
        }),
        previous: vi.fn(() => {
            state.index = Math.max(0, state.index - 1)
        }),
        changeVolume: vi.fn((delta: number) => {
            state.volume = Math.min(1, Math.max(0, state.volume + delta))
        }),
        toggleMute: vi.fn(() => { state.muted = !state.muted }),
        toggleShuffle: vi.fn(() => { state.shuffle = !state.shuffle }),
        cycleRepeat: vi.fn(() => {
            state.repeat = state.repeat === 'off' ? 'all' : state.repeat === 'all' ? 'one' : 'off'
        }),
    }

    return { state, actions }
}

function press(
    key: string,
    init: KeyboardEventInit = {},
    target: EventTarget = document,
): KeyboardEvent {
    const event = new KeyboardEvent('keydown', {
        key,
        bubbles: true,
        cancelable: true,
        ...init,
    })
    target.dispatchEvent(event)
    return event
}

describe('useKeyboardShortcuts — playback shortcuts (P04, #135)', () => {
    let state: SimulatedQueue
    let actions: PlaybackShortcutActions
    let unregister: (() => void) | null

    beforeEach(() => {
        document.body.innerHTML = ''
        const player = createSimulatedPlayer()
        state = player.state
        actions = player.actions
        unregister = registerPlaybackShortcuts(actions)
    })

    afterEach(() => {
        unregister?.()
        unregister = null
        document.body.innerHTML = ''
        vi.clearAllMocks()
    })

    it('Space toggles play/pause and claims the key', () => {
        const first = press(' ')

        expect(actions.toggle).toHaveBeenCalledTimes(1)
        expect(first.defaultPrevented).toBe(true)
        expect(state.playing).toBe(true)

        const second = press(' ', { code: 'Space' })

        expect(actions.toggle).toHaveBeenCalledTimes(2)
        expect(second.defaultPrevented).toBe(true)
        expect(state.playing).toBe(false)
    })

    it('left and right arrows seek by the configured step', () => {
        const back = press('ArrowLeft')

        expect(actions.seekBy).toHaveBeenNthCalledWith(1, -PLAYBACK_SEEK_STEP_SEC)
        expect(state.positionSec).toBe(25)
        expect(back.defaultPrevented).toBe(true)

        const forward = press('ArrowRight')

        expect(actions.seekBy).toHaveBeenNthCalledWith(2, PLAYBACK_SEEK_STEP_SEC)
        expect(state.positionSec).toBe(30)
        expect(forward.defaultPrevented).toBe(true)
        // Seeking is not a track change.
        expect(actions.previous).not.toHaveBeenCalled()
        expect(actions.next).not.toHaveBeenCalled()
    })

    it('Ctrl+arrows change track and plain arrows do not', () => {
        press('ArrowRight')
        expect(actions.next).not.toHaveBeenCalled()

        const nextEvent = press('ArrowRight', { ctrlKey: true })
        expect(actions.next).toHaveBeenCalledTimes(1)
        expect(state.index).toBe(1)
        expect(nextEvent.defaultPrevented).toBe(true)

        const previousEvent = press('ArrowLeft', { ctrlKey: true })
        expect(actions.previous).toHaveBeenCalledTimes(1)
        expect(state.index).toBe(0)
        expect(previousEvent.defaultPrevented).toBe(true)

        // Ctrl+arrow is prev/next only: it never seeks as well.
        expect(actions.seekBy).toHaveBeenCalledTimes(1)
    })

    it('up and down arrows change the volume step', () => {
        const up = press('ArrowUp')

        expect(actions.changeVolume).toHaveBeenNthCalledWith(1, PLAYBACK_VOLUME_STEP)
        expect(state.volume).toBeCloseTo(0.55)
        expect(up.defaultPrevented).toBe(true)

        const down = press('ArrowDown')

        expect(actions.changeVolume).toHaveBeenNthCalledWith(2, -PLAYBACK_VOLUME_STEP)
        expect(state.volume).toBeCloseTo(0.5)
        expect(down.defaultPrevented).toBe(true)
    })

    it('M, S and R mute, shuffle and cycle repeat', () => {
        const muteEvent = press('m')
        expect(actions.toggleMute).toHaveBeenCalledTimes(1)
        expect(state.muted).toBe(true)
        expect(muteEvent.defaultPrevented).toBe(true)

        press('s')
        expect(actions.toggleShuffle).toHaveBeenCalledTimes(1)
        expect(state.shuffle).toBe(true)

        press('r')
        expect(actions.cycleRepeat).toHaveBeenCalledTimes(1)
        expect(state.repeat).toBe('all')
        press('r')
        expect(state.repeat).toBe('one')
        press('r')
        expect(state.repeat).toBe('off')
    })

    it('does not fire when an extra modifier changes the combination', () => {
        press('r', { ctrlKey: true })
        expect(actions.cycleRepeat).not.toHaveBeenCalled()

        press('ArrowLeft', { shiftKey: true })
        expect(actions.seekBy).not.toHaveBeenCalled()

        press('m', { altKey: true })
        expect(actions.toggleMute).not.toHaveBeenCalled()

        press('ArrowRight', { metaKey: true })
        expect(actions.next).not.toHaveBeenCalled()
        expect(actions.seekBy).not.toHaveBeenCalled()
    })

    it('ignores every shortcut while typing in a text field', () => {
        const targets: Array<{ element: HTMLElement; label: string }> = []

        const input = document.createElement('input')
        const textarea = document.createElement('textarea')
        const select = document.createElement('select')
        const editable = document.createElement('div')
        editable.setAttribute('contenteditable', '')
        const nested = document.createElement('span')
        editable.appendChild(nested)

        // `nested` stays inside the contenteditable element: it is the case
        // `isContentEditable` may not answer and the ancestor lookup must.
        editable.appendChild(nested)
        document.body.append(input, textarea, select, editable)

        targets.push(
            { element: input, label: 'input' },
            { element: textarea, label: 'textarea' },
            { element: select, label: 'select' },
            { element: editable, label: 'contenteditable' },
            { element: nested, label: 'contenteditable child' },
        )

        for (const { element, label } of targets) {
            const space = press(' ', {}, element)
            const left = press('ArrowLeft', {}, element)
            const prev = press('ArrowLeft', { ctrlKey: true }, element)
            const mute = press('m', {}, element)

            expect(space.defaultPrevented, label).toBe(false)
            expect(left.defaultPrevented, label).toBe(false)
            // Ctrl+arrow must stay word navigation inside a field.
            expect(prev.defaultPrevented, label).toBe(false)
            expect(mute.defaultPrevented, label).toBe(false)
        }

        expect(actions.toggle).not.toHaveBeenCalled()
        expect(actions.seekBy).not.toHaveBeenCalled()
        expect(actions.previous).not.toHaveBeenCalled()
        expect(actions.toggleMute).not.toHaveBeenCalled()
    })

    it('leaves the existing view shortcuts (D, ?) intact', () => {
        const download = vi.fn()
        const help = vi.fn()
        const offDownload = registerShortcut('D', download)
        const offHelp = registerShortcut('?', help)

        press('d')
        press('?', { shiftKey: true })

        expect(download).toHaveBeenCalledTimes(1)
        expect(help).toHaveBeenCalledTimes(1)
        expect(actions.toggle).not.toHaveBeenCalled()
        expect(actions.seekBy).not.toHaveBeenCalled()
        expect(PLAYBACK_SHORTCUT_KEYS).not.toContain('D')
        expect(PLAYBACK_SHORTCUT_KEYS).not.toContain('?')

        offDownload()
        offHelp()
    })

    it('steps aside while an editor that owns the keyboard is open', () => {
        const blocker = document.createElement('div')
        blocker.setAttribute('data-playback-shortcuts', 'off')
        document.body.appendChild(blocker)

        const blocked = press(' ')
        expect(blocked.defaultPrevented).toBe(false)
        expect(actions.toggle).not.toHaveBeenCalled()

        blocker.remove()

        const allowed = press(' ')
        expect(allowed.defaultPrevented).toBe(true)
        expect(actions.toggle).toHaveBeenCalledTimes(1)
        expect(document.querySelector(PLAYBACK_SHORTCUTS_BLOCK_SELECTOR)).toBeNull()
    })

    it('unregistering removes every playback binding', () => {
        unregister?.()
        unregister = null

        press(' ')
        press('ArrowRight')
        press('ArrowRight', { ctrlKey: true })
        press('m')
        press('s')
        press('r')

        expect(actions.toggle).not.toHaveBeenCalled()
        expect(actions.seekBy).not.toHaveBeenCalled()
        expect(actions.next).not.toHaveBeenCalled()
        expect(actions.toggleMute).not.toHaveBeenCalled()
        expect(actions.toggleShuffle).not.toHaveBeenCalled()
        expect(actions.cycleRepeat).not.toHaveBeenCalled()

        const registered = new Set(getRegisteredShortcuts().map(binding => binding.keys))
        for (const keys of PLAYBACK_SHORTCUT_KEYS) {
            expect(registered.has(keys), keys).toBe(false)
        }
    })
})

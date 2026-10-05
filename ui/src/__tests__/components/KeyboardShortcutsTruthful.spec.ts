/**
 * Audit 8 regression: the cheat sheet used to advertise ~30 combinations that
 * had no handler anywhere in the app (Undo, Play/Pause, Ctrl+A/D/Q/N, Delete,
 * L, P, R, Ctrl+S/E/B/L, ...). The sheet is now derived from the bindings that
 * are actually registered, so a row without a handler is a test failure.
 */
import { describe, it, expect, beforeEach, afterEach, vi } from 'vitest'
import { mount, flushPromises } from '@vue/test-utils'
import { createRouter, createMemoryHistory } from 'vue-router'
import KeyboardShortcuts from '@/components/KeyboardShortcuts.vue'
import {
    getRegisteredShortcuts,
    useShortcut,
    openShortcutsHelp,
    closeShortcutsHelp
} from '@/composables/useKeyboardShortcuts'

// Ctrl+K is owned by App.vue (App.vue:861-866): it is listed but registered
// there, on purpose.
const HANDLED_ELSEWHERE = ['Ctrl+K']

function buildRouter() {
    return createRouter({
        history: createMemoryHistory(),
        routes: [
            { path: '/', component: { template: '<div/>' } },
            { path: '/library', component: { template: '<div/>' } },
            { path: '/downloads', component: { template: '<div/>' } },
            { path: '/metadata', component: { template: '<div/>' } },
            { path: '/lyrics', component: { template: '<div/>' } },
            { path: '/accounts', component: { template: '<div/>' } },
            { path: '/migration', component: { template: '<div/>' } },
            { path: '/queue', component: { template: '<div/>' } },
            { path: '/settings', component: { template: '<div/>' } },
        ]
    })
}

function listedCombos(): string[] {
    return Array.from(document.querySelectorAll('.shortcut-row')).map(row => {
        const keys = Array.from(row.querySelectorAll('kbd')).map(k => k.textContent?.trim() || '')
        return keys.join('+')
    })
}

describe('KeyboardShortcuts — la hoja solo anuncia atajos que existen', () => {
    let router: ReturnType<typeof buildRouter>

    beforeEach(() => {
        closeShortcutsHelp()
        router = buildRouter()
        document.body.innerHTML = ''
    })

    afterEach(() => {
        closeShortcutsHelp()
        document.body.innerHTML = ''
        vi.clearAllMocks()
    })

    it('every row in the sheet maps to a registered binding', async () => {
        const wrapper = mount(KeyboardShortcuts, {
            attachTo: document.body,
            global: { plugins: [router] }
        })
        await flushPromises()

        openShortcutsHelp()
        await flushPromises()

        const registered = new Set(getRegisteredShortcuts().map(s => s.keys))
        const listed = listedCombos()

        expect(listed.length).toBeGreaterThan(0)
        for (const combo of listed) {
            const live = registered.has(combo) || HANDLED_ELSEWHERE.includes(combo)
            expect(live, `"${combo}" appears in the sheet but nothing handles it`).toBe(true)
        }

        wrapper.unmount()
    })

    it('does not advertise the shortcuts it dropped as unimplemented', async () => {
        const wrapper = mount(KeyboardShortcuts, {
            attachTo: document.body,
            global: { plugins: [router] }
        })
        await flushPromises()

        openShortcutsHelp()
        await flushPromises()

        const sheet = document.querySelector('.shortcuts-modal')?.textContent || ''
        for (const gone of ['Undo', 'Redo', 'Play / Pause', 'Select all', 'New playlist', 'Fetch from MusicBrainz']) {
            expect(sheet).not.toContain(gone)
        }

        wrapper.unmount()
    })

    it('labels Ctrl+H for what it really does', async () => {
        const wrapper = mount(KeyboardShortcuts, {
            attachTo: document.body,
            global: { plugins: [router] }
        })
        await flushPromises()

        openShortcutsHelp()
        await flushPromises()

        const sheet = document.querySelector('.shortcuts-modal')?.textContent || ''
        // Ctrl+H toggles this sheet; it never touched HelpPanel.
        expect(sheet).not.toContain('Toggle help panel')
        expect(sheet).toContain('Toggle this shortcuts sheet')

        wrapper.unmount()
    })
})

describe('useShortcut — un único registro compartido', () => {
    afterEach(() => {
        vi.clearAllMocks()
    })

    it('registers into the global map even outside a component instance', () => {
        const handler = vi.fn()
        const unregister = useShortcut('Ctrl+Alt+Z', handler, { description: 'Undo stack' })

        const found = getRegisteredShortcuts().find(s => s.keys === 'Ctrl+Alt+Z')
        expect(found).toBeDefined()
        expect(found?.description).toBe('Undo stack')

        unregister()
        expect(getRegisteredShortcuts().some(s => s.keys === 'Ctrl+Alt+Z')).toBe(false)
    })
})
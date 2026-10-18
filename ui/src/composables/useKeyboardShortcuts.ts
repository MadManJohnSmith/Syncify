import { ref, onMounted, onUnmounted, getCurrentInstance, type Ref } from 'vue'

export type ShortcutHandler = (event: KeyboardEvent) => void

export interface ShortcutBinding {
    keys: string
    handler: ShortcutHandler
    description?: string
    category?: string
    when?: () => boolean
}

export interface ShortcutOptions {
    description?: string
    category?: string
    global?: boolean
    when?: () => boolean
}

// Parse key combo string like "Ctrl+K" or "Shift+Enter"
export function parseKeys(keys: string): { ctrl: boolean; shift: boolean; alt: boolean; meta: boolean; key: string } {
    const parts = keys.toLowerCase().split('+').map(p => p.trim())
    return {
        ctrl: parts.includes('ctrl') || parts.includes('control'),
        shift: parts.includes('shift'),
        alt: parts.includes('alt'),
        meta: parts.includes('meta') || parts.includes('cmd') || parts.includes('command'),
        key: parts.filter(p => !['ctrl', 'control', 'shift', 'alt', 'meta', 'cmd', 'command'].includes(p))[0] || ''
    }
}

// Check if event matches key combo
export function matchesKeys(event: KeyboardEvent, keys: string): boolean {
    const parsed = parseKeys(keys)

    if (event.ctrlKey !== parsed.ctrl) return false
    if (event.altKey !== parsed.alt) return false
    if (event.metaKey !== parsed.meta) return false

    // Normalize key names
    const eventKey = event.key.toLowerCase()
    const targetKey = parsed.key.toLowerCase()

    // Handle shift key requirement:
    // If target key is a shifted symbol like '?' (which naturally requires Shift on most keyboards)
    // and shift wasn't explicitly demanded in the shortcut string (e.g. "?" instead of "Shift+?"),
    // allow event.shiftKey to be true when event.key matches targetKey.
    const isShiftedSymbol = parsed.key === '?' || (parsed.key.length === 1 && !/[a-z0-9]/i.test(parsed.key))
    if (!parsed.shift && event.shiftKey && !isShiftedSymbol) return false
    if (parsed.shift && !event.shiftKey) return false

    // Handle special keys
    const keyMap: Record<string, string[]> = {
        'escape': ['escape', 'esc'],
        'enter': ['enter', 'return'],
        'space': [' ', 'space', 'spacebar'],
        'arrowup': ['arrowup', '↑', 'up'],
        'arrowdown': ['arrowdown', '↓', 'down'],
        'arrowleft': ['arrowleft', '←', 'left'],
        'arrowright': ['arrowright', '→', 'right'],
        'delete': ['delete', 'del'],
        'backspace': ['backspace'],
    }

    for (const [standard, aliases] of Object.entries(keyMap)) {
        if (aliases.includes(targetKey) && eventKey === standard) return true
        if (aliases.includes(eventKey) && targetKey === standard) return true
    }

    return eventKey === targetKey
}

// Global shortcut registry
const globalBindings = ref<Map<string, ShortcutBinding>>(new Map())

// Shared reactive state for shortcuts help modal
export const showShortcutsHelp: Ref<boolean> = ref(false)

export function openShortcutsHelp(): void {
    showShortcutsHelp.value = true
}

export function closeShortcutsHelp(): void {
    showShortcutsHelp.value = false
}

export function toggleShortcutsHelp(): void {
    showShortcutsHelp.value = !showShortcutsHelp.value
}

// Global standalone shortcut registration
export function registerShortcut(
    keys: string,
    handler: ShortcutHandler,
    options?: ShortcutOptions
): () => void {
    const binding: ShortcutBinding = {
        keys,
        handler,
        description: options?.description,
        category: options?.category,
        when: options?.when
    }
    globalBindings.value.set(keys, binding)
    return () => {
        globalBindings.value.delete(keys)
    }
}

// Global getter for registered shortcuts
export function getRegisteredShortcuts(): ShortcutBinding[] {
    return Array.from(globalBindings.value.values())
}

/**
 * Searches `bindings` for the first combo that matches the event and runs it.
 * Returns whether it fired anything.
 */
function dispatchBinding(
    event: KeyboardEvent,
    bindings: Map<string, ShortcutBinding>
): boolean {
    if (event.defaultPrevented) return false

    const target = event.target as HTMLElement | null
    const isInput = Boolean(
        target && (
            target.tagName === 'INPUT' ||
            target.tagName === 'TEXTAREA' ||
            target.isContentEditable
        )
    )

    for (const [keys, binding] of bindings) {
        if (!matchesKeys(event, keys)) continue
        if (binding.when && !binding.when()) continue
        if (isInput && !keys.toLowerCase().includes('ctrl') && !keys.toLowerCase().includes('escape')) continue

        event.preventDefault()
        binding.handler(event)
        return true
    }
    return false
}

function handleGlobalKeydown(event: KeyboardEvent) {
    dispatchBinding(event, globalBindings.value)
}

// A single global listener for the whole registry: useShortcut() and
// registerShortcut() work without each component adding its own.
if (typeof document !== 'undefined') {
    document.addEventListener('keydown', handleGlobalKeydown)
}

// Format keys utility
export function formatKeys(keys: string): string[] {
    return keys.split('+').map(k => {
        const key = k.trim()
        const displayMap: Record<string, string> = {
            'ctrl': 'Ctrl',
            'control': 'Ctrl',
            'shift': 'Shift',
            'alt': 'Alt',
            'meta': '⌘',
            'cmd': '⌘',
            'command': '⌘',
            'escape': 'Esc',
            'enter': 'Enter',
            'space': 'Space',
            'arrowup': '↑',
            'arrowdown': '↓',
            'arrowleft': '←',
            'arrowright': '→',
            'delete': 'Del',
            'backspace': '⌫',
        }
        return displayMap[key.toLowerCase()] || key.charAt(0).toUpperCase() + key.slice(1)
    })
}

export function useKeyboardShortcuts() {
    const localBindings = ref<Map<string, ShortcutBinding>>(new Map())

    // Register a shortcut (supports local and global)
    function register(
        keys: string,
        handler: ShortcutHandler,
        options?: ShortcutOptions
    ): () => void {
        const binding: ShortcutBinding = {
            keys,
            handler,
            description: options?.description,
            category: options?.category,
            when: options?.when
        }

        if (options?.global) {
            globalBindings.value.set(keys, binding)
        } else {
            localBindings.value.set(keys, binding)
        }

        return () => unregister(keys, options?.global)
    }

    // Alias for register to fulfill registerShortcut(...)
    const registerShortcutFn = register

    // Unregister a shortcut
    function unregister(keys: string, global?: boolean): void {
        if (global) {
            globalBindings.value.delete(keys)
        } else {
            localBindings.value.delete(keys)
        }
    }

    // Get registered shortcuts (both local and global)
    function getRegisteredShortcutsFn(): ShortcutBinding[] {
        const map = new Map<string, ShortcutBinding>()
        for (const [k, v] of globalBindings.value) {
            map.set(k, v)
        }
        for (const [k, v] of localBindings.value) {
            map.set(k, v)
        }
        return Array.from(map.values())
    }

    // Handle keydown event. Only the local bindings of this instance: global
// ones are handled by the module's single listener, and dispatchBinding would
// run again if checked here (preventDefault() does not mark the event when it
// is not cancelable, which is how tests dispatch it).
    function handleKeydown(event: KeyboardEvent) {
        dispatchBinding(event, localBindings.value)
    }

    const cleanup = () => {
        if (typeof document !== 'undefined') {
            document.removeEventListener('keydown', handleKeydown)
        }
        localBindings.value.clear()
    }

    if (getCurrentInstance()) {
        onMounted(() => {
            document.addEventListener('keydown', handleKeydown)
        })

        onUnmounted(() => {
            cleanup()
        })
    }

    return {
        showShortcutsHelp,
        openShortcutsHelp,
        closeShortcutsHelp,
        toggleShortcutsHelp,
        register,
        registerShortcut: registerShortcutFn,
        unregister,
        getRegisteredShortcuts: getRegisteredShortcutsFn,
        formatKeys,
        handleKeydown,
        cleanup
    }
}

// Utility hook for single shortcut
export function useShortcut(
    keys: string,
    handler: ShortcutHandler,
    options?: ShortcutOptions
): () => void {
    // Register in the global map so a single document listener handles all
    // shortcuts. It used to create its own instance of
    // useKeyboardShortcuts() for each component that used it, and each one
    // added its own keyboard listener to `document`.
    const unregister = registerShortcut(keys, handler, {
        description: options?.description,
        category: options?.category,
        when: options?.when
    })

    if (getCurrentInstance()) {
        onUnmounted(unregister)
    }

    return unregister
}

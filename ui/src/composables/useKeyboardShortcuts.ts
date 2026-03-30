import { ref, onMounted, onUnmounted } from 'vue'

type ShortcutHandler = (event: KeyboardEvent) => void

interface ShortcutBinding {
    keys: string
    handler: ShortcutHandler
    description?: string
    when?: () => boolean
}

// Parse key combo string like "Ctrl+K" or "Shift+Enter"
function parseKeys(keys: string): { ctrl: boolean; shift: boolean; alt: boolean; meta: boolean; key: string } {
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
function matchesKeys(event: KeyboardEvent, keys: string): boolean {
    const parsed = parseKeys(keys)

    if (event.ctrlKey !== parsed.ctrl) return false
    if (event.shiftKey !== parsed.shift) return false
    if (event.altKey !== parsed.alt) return false
    if (event.metaKey !== parsed.meta) return false

    // Normalize key names
    const eventKey = event.key.toLowerCase()
    const targetKey = parsed.key.toLowerCase()

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

export function useKeyboardShortcuts() {
    const localBindings = ref<Map<string, ShortcutBinding>>(new Map())

    // Register a shortcut
    function register(keys: string, handler: ShortcutHandler, options?: { description?: string; global?: boolean; when?: () => boolean }) {
        const binding: ShortcutBinding = {
            keys,
            handler,
            description: options?.description,
            when: options?.when
        }

        if (options?.global) {
            globalBindings.value.set(keys, binding)
        } else {
            localBindings.value.set(keys, binding)
        }

        return () => unregister(keys, options?.global)
    }

    // Unregister a shortcut
    function unregister(keys: string, global?: boolean) {
        if (global) {
            globalBindings.value.delete(keys)
        } else {
            localBindings.value.delete(keys)
        }
    }

    // Handle keydown event
    function handleKeydown(event: KeyboardEvent) {
        // Skip if in input/textarea (unless explicitly allowed)
        const target = event.target as HTMLElement
        const isInput = target.tagName === 'INPUT' || target.tagName === 'TEXTAREA' || target.isContentEditable

        // Check local bindings first
        for (const [keys, binding] of localBindings.value) {
            if (matchesKeys(event, keys)) {
                if (binding.when && !binding.when()) continue
                if (isInput && !keys.toLowerCase().includes('ctrl') && !keys.toLowerCase().includes('escape')) continue

                event.preventDefault()
                binding.handler(event)
                return
            }
        }

        // Then check global bindings
        for (const [keys, binding] of globalBindings.value) {
            if (matchesKeys(event, keys)) {
                if (binding.when && !binding.when()) continue
                if (isInput && !keys.toLowerCase().includes('ctrl') && !keys.toLowerCase().includes('escape')) continue

                event.preventDefault()
                binding.handler(event)
                return
            }
        }
    }

    onMounted(() => {
        document.addEventListener('keydown', handleKeydown)
    })

    onUnmounted(() => {
        document.removeEventListener('keydown', handleKeydown)
        // Clean up local bindings
        localBindings.value.clear()
    })

    return {
        register,
        unregister,
        // Utility to format keys for display
        formatKeys: (keys: string): string[] => {
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
    }
}

// Utility hook for single shortcut
export function useShortcut(keys: string, handler: ShortcutHandler, options?: { global?: boolean; when?: () => boolean }) {
    const { register } = useKeyboardShortcuts()

    onMounted(() => {
        register(keys, handler, options)
    })
}

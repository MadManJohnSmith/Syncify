import { describe, it, expect, beforeEach, afterEach, vi } from 'vitest'
import { mount, flushPromises } from '@vue/test-utils'
import { createRouter, createMemoryHistory } from 'vue-router'
import KeyboardShortcuts from '@/components/KeyboardShortcuts.vue'
import {
  useKeyboardShortcuts,
  showShortcutsHelp,
  openShortcutsHelp,
  closeShortcutsHelp,
  toggleShortcutsHelp,
  registerShortcut,
  getRegisteredShortcuts,
  formatKeys,
} from '@/composables/useKeyboardShortcuts'

describe('KeyboardShortcuts Consolidated Composable (TASK-59)', () => {
  let router: any

  beforeEach(() => {
    closeShortcutsHelp()
    router = createRouter({
      history: createMemoryHistory(),
      routes: [
        { path: '/', component: { template: '<div>Home</div>' } },
        { path: '/library', component: { template: '<div>Library</div>' } },
        { path: '/downloads', component: { template: '<div>Downloads</div>' } },
        { path: '/metadata', component: { template: '<div>Metadata</div>' } },
        { path: '/lyrics', component: { template: '<div>Lyrics</div>' } },
        { path: '/accounts', component: { template: '<div>Accounts</div>' } },
        { path: '/migration', component: { template: '<div>Migration</div>' } },
        { path: '/settings', component: { template: '<div>Settings</div>' } },
      ],
    })
    document.body.innerHTML = ''
  })

  afterEach(() => {
    closeShortcutsHelp()
    document.body.innerHTML = ''
  })

  it('shares reactive modal visibility state between composable and component', async () => {
    const wrapper = mount(KeyboardShortcuts, {
      attachTo: document.body,
      global: {
        plugins: [router],
      },
    })
    await flushPromises()

    expect(showShortcutsHelp.value).toBe(false)
    expect((wrapper.vm as any).showHelpModal).toBe(false)
    expect(document.querySelector('.shortcuts-modal')).toBeNull()

    // Open via composable helper
    openShortcutsHelp()
    await flushPromises()

    expect(showShortcutsHelp.value).toBe(true)
    expect((wrapper.vm as any).showHelpModal).toBe(true)
    expect(document.querySelector('.shortcuts-modal')).not.toBeNull()

    // Close via composable helper
    closeShortcutsHelp()
    await flushPromises()

    expect(showShortcutsHelp.value).toBe(false)
    expect((wrapper.vm as any).showHelpModal).toBe(false)
    expect(document.querySelector('.shortcuts-modal')).toBeNull()

    // Toggle via composable helper
    toggleShortcutsHelp()
    await flushPromises()

    expect(showShortcutsHelp.value).toBe(true)
    expect((wrapper.vm as any).showHelpModal).toBe(true)

    // Close via component exposed method
    ;(wrapper.vm as any).hide()
    await flushPromises()

    expect(showShortcutsHelp.value).toBe(false)
    expect((wrapper.vm as any).showHelpModal).toBe(false)

    // Open via component exposed method
    ;(wrapper.vm as any).show()
    await flushPromises()

    expect(showShortcutsHelp.value).toBe(true)
    expect((wrapper.vm as any).showHelpModal).toBe(true)

    // Toggle via component exposed method
    ;(wrapper.vm as any).toggle()
    await flushPromises()

    expect(showShortcutsHelp.value).toBe(false)
    expect((wrapper.vm as any).showHelpModal).toBe(false)

    wrapper.unmount()
  })

  it('opens shortcuts help modal when "?" key is pressed outside inputs', async () => {
    const wrapper = mount(KeyboardShortcuts, {
      attachTo: document.body,
      global: {
        plugins: [router],
      },
    })
    await flushPromises()

    expect(showShortcutsHelp.value).toBe(false)

    // Dispatch "?" keydown event
    const event = new KeyboardEvent('keydown', {
      key: '?',
      bubbles: true,
    })
    document.dispatchEvent(event)
    await flushPromises()

    expect(showShortcutsHelp.value).toBe(true)
    expect(document.querySelector('.shortcuts-modal')).not.toBeNull()

    wrapper.unmount()
  })

  it('does NOT open shortcuts help modal when "?" is typed inside an input', async () => {
    const wrapper = mount(KeyboardShortcuts, {
      attachTo: document.body,
      global: {
        plugins: [router],
      },
    })
    await flushPromises()

    const input = document.createElement('input')
    document.body.appendChild(input)
    input.focus()

    const event = new KeyboardEvent('keydown', {
      key: '?',
      bubbles: true,
    })
    input.dispatchEvent(event)
    await flushPromises()

    expect(showShortcutsHelp.value).toBe(false)
    expect(document.querySelector('.shortcuts-modal')).toBeNull()

    input.remove()
    wrapper.unmount()
  })

  it('toggles shortcuts help modal with Ctrl+/', async () => {
    const wrapper = mount(KeyboardShortcuts, {
      attachTo: document.body,
      global: {
        plugins: [router],
      },
    })
    await flushPromises()

    expect(showShortcutsHelp.value).toBe(false)

    // Open with Ctrl+/
    document.dispatchEvent(new KeyboardEvent('keydown', {
      key: '/',
      ctrlKey: true,
      bubbles: true,
    }))
    await flushPromises()

    expect(showShortcutsHelp.value).toBe(true)

    // Close with Ctrl+/
    document.dispatchEvent(new KeyboardEvent('keydown', {
      key: '/',
      ctrlKey: true,
      bubbles: true,
    }))
    await flushPromises()

    expect(showShortcutsHelp.value).toBe(false)

    wrapper.unmount()
  })

  it('closes shortcuts help modal with Escape when modal is open', async () => {
    const wrapper = mount(KeyboardShortcuts, {
      attachTo: document.body,
      global: {
        plugins: [router],
      },
    })
    await flushPromises()

    openShortcutsHelp()
    await flushPromises()
    expect(showShortcutsHelp.value).toBe(true)

    // Press Escape
    document.dispatchEvent(new KeyboardEvent('keydown', {
      key: 'Escape',
      bubbles: true,
    }))
    await flushPromises()

    expect(showShortcutsHelp.value).toBe(false)

    wrapper.unmount()
  })

  it('emits commands for Ctrl+K, Ctrl+F, and Ctrl+R', async () => {
    const wrapper = mount(KeyboardShortcuts, {
      attachTo: document.body,
      global: {
        plugins: [router],
      },
    })
    await flushPromises()

    // Ctrl+K -> command-palette
    document.dispatchEvent(new KeyboardEvent('keydown', {
      key: 'k',
      ctrlKey: true,
      bubbles: true,
    }))
    await flushPromises()
    expect(wrapper.emitted('command-palette')).toHaveLength(1)

    // Ctrl+F -> search
    document.dispatchEvent(new KeyboardEvent('keydown', {
      key: 'f',
      ctrlKey: true,
      bubbles: true,
    }))
    await flushPromises()
    expect(wrapper.emitted('search')).toHaveLength(1)

    // Ctrl+R -> refresh
    document.dispatchEvent(new KeyboardEvent('keydown', {
      key: 'r',
      ctrlKey: true,
      bubbles: true,
    }))
    await flushPromises()
    expect(wrapper.emitted('refresh')).toHaveLength(1)

    wrapper.unmount()
  })

  it('navigates to /settings on Ctrl+,', async () => {
    const wrapper = mount(KeyboardShortcuts, {
      attachTo: document.body,
      global: {
        plugins: [router],
      },
    })
    await flushPromises()

    document.dispatchEvent(new KeyboardEvent('keydown', {
      key: ',',
      ctrlKey: true,
      bubbles: true,
    }))
    await flushPromises()

    expect(router.currentRoute.value.path).toBe('/settings')

    wrapper.unmount()
  })

  it('navigates to tabs with Ctrl+1 through Ctrl+6', async () => {
    const wrapper = mount(KeyboardShortcuts, {
      attachTo: document.body,
      global: {
        plugins: [router],
      },
    })
    await flushPromises()

    const tabExpectations = [
      { num: '1', path: '/library' },
      { num: '2', path: '/downloads' },
      { num: '3', path: '/metadata' },
      { num: '4', path: '/lyrics' },
      { num: '5', path: '/accounts' },
      { num: '6', path: '/migration' },
    ]

    for (const { num, path } of tabExpectations) {
      document.dispatchEvent(new KeyboardEvent('keydown', {
        key: num,
        ctrlKey: true,
        bubbles: true,
      }))
      await flushPromises()
      expect(router.currentRoute.value.path).toBe(path)
    }

    wrapper.unmount()
  })

  it('provides getRegisteredShortcuts and formatKeys utilities', () => {
    const formatted = formatKeys('ctrl+shift+k')
    expect(formatted).toEqual(['Ctrl', 'Shift', 'K'])

    const unregister = registerShortcut('Ctrl+Alt+T', () => {}, {
      description: 'Test shortcut',
      category: 'Testing'
    })

    const shortcuts = getRegisteredShortcuts()
    const found = shortcuts.find(s => s.keys === 'Ctrl+Alt+T')
    expect(found).toBeDefined()
    expect(found?.description).toBe('Test shortcut')

    unregister()
    const shortcutsAfter = getRegisteredShortcuts()
    expect(shortcutsAfter.find(s => s.keys === 'Ctrl+Alt+T')).toBeUndefined()
  })
})

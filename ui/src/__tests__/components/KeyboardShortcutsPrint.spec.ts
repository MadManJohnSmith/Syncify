/**
 * KeyboardShortcutsPrint.spec.ts
 * FE-12: "Print Cheat Sheet" must call window.print (with the print stylesheet
 * present), and the dead "Customize Shortcuts" button must stay gone.
 */
import { describe, it, expect, beforeEach, afterEach, vi } from 'vitest'
import { mount, flushPromises } from '@vue/test-utils'
import { createRouter, createMemoryHistory } from 'vue-router'
import { readFileSync } from 'node:fs'
import path from 'node:path'
import KeyboardShortcuts from '@/components/KeyboardShortcuts.vue'
import { closeShortcutsHelp, openShortcutsHelp } from '@/composables/useKeyboardShortcuts'

const componentPath = path.resolve(process.cwd(), 'src/components/KeyboardShortcuts.vue')

describe('KeyboardShortcuts cheat sheet printing (FE-12)', () => {
  let router: any
  let printSpy: ReturnType<typeof vi.fn>

  beforeEach(() => {
    closeShortcutsHelp()
    router = createRouter({
      history: createMemoryHistory(),
      routes: [{ path: '/', component: { template: '<div>Home</div>' } }],
    })
    document.body.innerHTML = ''
    printSpy = vi.fn()
    ;(window as any).print = printSpy
  })

  afterEach(() => {
    closeShortcutsHelp()
    document.body.innerHTML = ''
    vi.restoreAllMocks()
  })

  async function openModal() {
    const wrapper = mount(KeyboardShortcuts, {
      attachTo: document.body,
      global: { plugins: [router] },
    })
    await flushPromises()
    openShortcutsHelp()
    await flushPromises()
    return wrapper
  }

  it('"Print Cheat Sheet" triggers window.print', async () => {
    const wrapper = await openModal()

    // The modal is teleported to <body>, so query the document, not the wrapper.
    const buttons = Array.from(document.querySelectorAll('.shortcuts-modal button'))
    const printButton = buttons.find(b => b.textContent?.includes('Print Cheat Sheet'))
    expect(printButton).toBeDefined()

    printButton!.dispatchEvent(new MouseEvent('click', { bubbles: true }))
    await flushPromises()

    expect(printSpy).toHaveBeenCalledTimes(1)

    wrapper.unmount()
  })

  it('ships a print stylesheet that hides chrome and forces a light palette', () => {
    const source = readFileSync(componentPath, 'utf-8')

    expect(source).toContain('@media print')
    expect(source).toContain('print-hidden')
    // Chrome that must not reach the paper.
    expect(source).toContain('visibility: hidden !important')
    expect(source).toContain('display: none !important')
    expect(source).toContain('color: #111 !important')
  })

  it('does not render the dead "Customize Shortcuts" button', async () => {
    const wrapper = await openModal()

    expect(wrapper.text()).not.toContain('Customize Shortcuts')

    wrapper.unmount()
  })
})

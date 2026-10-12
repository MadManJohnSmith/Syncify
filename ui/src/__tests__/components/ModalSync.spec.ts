/**
 * ModalSync.spec.ts
 * Tests for TASK-19: CommandPalette v-model synchronization,
 * visibility rendering in DOM, and close event propagation.
 */
import { describe, it, expect, beforeEach, afterEach, vi } from 'vitest'
import { mount, flushPromises } from '@vue/test-utils'
import { ref, defineComponent, h } from 'vue'
import { createRouter, createMemoryHistory } from 'vue-router'
import CommandPalette from '@/components/CommandPalette.vue'

describe('TASK-19: ModalSync - CommandPalette v-model Synchronization', () => {
  let router: any

  beforeEach(() => {
    router = createRouter({
      history: createMemoryHistory(),
      routes: [
        { path: '/', component: { template: '<div>Dashboard</div>' } },
        { path: '/library', component: { template: '<div>Library</div>' } },
        { path: '/settings', component: { template: '<div>Settings</div>' } },
      ],
    })
    document.body.innerHTML = ''
    vi.clearAllMocks()
  })

  afterEach(() => {
    document.body.innerHTML = ''
  })

  describe('CommandPalette.vue', () => {
    it('does not render inside DOM when modelValue is false', async () => {
      const wrapper = mount(CommandPalette, {
        attachTo: document.body,
        props: { modelValue: false },
        global: { plugins: [router] },
      })
      await flushPromises()

      expect(document.body.querySelector('.command-palette')).toBeNull()
      wrapper.unmount()
    })

    it('renders inside DOM when modelValue changes to true from parent', async () => {
      const wrapper = mount(CommandPalette, {
        attachTo: document.body,
        props: { modelValue: false },
        global: { plugins: [router] },
      })
      await flushPromises()

      expect(document.body.querySelector('.command-palette')).toBeNull()

      // Parent updates modelValue to true
      await wrapper.setProps({ modelValue: true })
      await flushPromises()

      const paletteEl = document.body.querySelector('.command-palette')
      expect(paletteEl).not.toBeNull()
      const input = document.body.querySelector('.palette-input input')
      expect(input).not.toBeNull()

      wrapper.unmount()
    })

    it('emits update:modelValue(false) and close when clicking backdrop', async () => {
      const wrapper = mount(CommandPalette, {
        attachTo: document.body,
        props: { modelValue: true },
        global: { plugins: [router] },
      })
      await flushPromises()

      const paletteEl = document.body.querySelector('.command-palette') as HTMLElement
      expect(paletteEl).not.toBeNull()

      // Click on backdrop itself (@click.self="close")
      paletteEl.dispatchEvent(new MouseEvent('click', { bubbles: true }))
      await flushPromises()

      expect(wrapper.emitted('update:modelValue')).toBeTruthy()
      expect(wrapper.emitted('update:modelValue')![0]).toEqual([false])
      expect(wrapper.emitted('close')).toBeTruthy()

      wrapper.unmount()
    })

    it('emits update:modelValue(false) and close when pressing Escape', async () => {
      const wrapper = mount(CommandPalette, {
        attachTo: document.body,
        props: { modelValue: true },
        global: { plugins: [router] },
      })
      await flushPromises()

      const input = document.body.querySelector('.palette-input input') as HTMLInputElement
      expect(input).not.toBeNull()

      input.dispatchEvent(new KeyboardEvent('keydown', { key: 'Escape', bubbles: true }))
      await flushPromises()

      expect(wrapper.emitted('update:modelValue')).toBeTruthy()
      expect(wrapper.emitted('update:modelValue')![0]).toEqual([false])
      expect(wrapper.emitted('close')).toBeTruthy()

      wrapper.unmount()
    })

    it('emits update:modelValue(false) and close when calling close() method', async () => {
      const wrapper = mount(CommandPalette, {
        attachTo: document.body,
        props: { modelValue: true },
        global: { plugins: [router] },
      })
      await flushPromises()

      wrapper.vm.close()
      await flushPromises()

      expect(wrapper.emitted('update:modelValue')).toBeTruthy()
      expect(wrapper.emitted('update:modelValue')![0]).toEqual([false])
      expect(wrapper.emitted('close')).toBeTruthy()

      wrapper.unmount()
    })

    it('emits update:modelValue(true) when calling open() method', async () => {
      const wrapper = mount(CommandPalette, {
        attachTo: document.body,
        props: { modelValue: false },
        global: { plugins: [router] },
      })
      await flushPromises()

      wrapper.vm.open()
      await flushPromises()

      expect(wrapper.emitted('update:modelValue')).toBeTruthy()
      expect(wrapper.emitted('update:modelValue')![0]).toEqual([true])

      wrapper.unmount()
    })

    it('synchronizes bidirectionally with parent reactive v-model state', async () => {
      const ParentComponent = defineComponent({
        components: { CommandPalette },
        setup() {
          const showPalette = ref(false)
          const closedCount = ref(0)
          const handleClose = () => {
            closedCount.value++
          }
          return { showPalette, closedCount, handleClose }
        },
        template: `
          <div>
            <button id="open-btn" @click="showPalette = true">Open</button>
            <CommandPalette v-model="showPalette" @close="handleClose" />
          </div>
        `,
      })

      const wrapper = mount(ParentComponent, {
        attachTo: document.body,
        global: { plugins: [router] },
      })
      await flushPromises()

      // Initially closed
      expect(document.body.querySelector('.command-palette')).toBeNull()
      expect(wrapper.vm.showPalette).toBe(false)

      // Open from parent button
      await wrapper.find('#open-btn').trigger('click')
      await flushPromises()

      expect(wrapper.vm.showPalette).toBe(true)
      expect(document.body.querySelector('.command-palette')).not.toBeNull()

      // Close from component backdrop click
      const paletteEl = document.body.querySelector('.command-palette') as HTMLElement
      paletteEl.dispatchEvent(new MouseEvent('click', { bubbles: true }))
      await flushPromises()

      expect(wrapper.vm.showPalette).toBe(false)
      expect(wrapper.vm.closedCount).toBe(1)

      wrapper.unmount()
    })
  })

})

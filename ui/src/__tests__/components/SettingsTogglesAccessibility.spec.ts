/**
 * BaseToggle.vue accessibility regression (audit item 7).
 *
 * The switch used to be a click-only <div> wrapping an unlabelled checkbox, so
 * it could not be reached or operated with the keyboard and announced nothing
 * useful to a screen reader.
 */
import { describe, it, expect } from 'vitest'
import { mount } from '@vue/test-utils'
import BaseToggle from '@/components/settings/BaseToggle.vue'

describe('BaseToggle — keyboard and screen-reader access', () => {
  it('exposes a switch role with its state', () => {
    const wrapper = mount(BaseToggle, {
      props: { title: 'Close to tray', subtitle: 'Keep running in the background', checked: true },
    })

    const control = wrapper.get('button[role="switch"]')
    expect(control.attributes('aria-checked')).toBe('true')
    expect(control.attributes('type')).toBe('button')
  })

  it('reports aria-checked=false when off', () => {
    const wrapper = mount(BaseToggle, {
      props: { title: 'Start on system boot', checked: false },
    })

    expect(wrapper.get('button[role="switch"]').attributes('aria-checked')).toBe('false')
  })

  it('folds title and subtitle into the accessible name', () => {
    const wrapper = mount(BaseToggle, {
      props: { title: 'Close to tray', subtitle: 'Keep running in the background', checked: true },
    })

    expect(wrapper.get('button[role="switch"]').attributes('aria-label')).toBe(
      'Close to tray. Keep running in the background'
    )
  })

  it('emits on activation so Enter and Space work without a custom key handler', async () => {
    const wrapper = mount(BaseToggle, {
      props: { title: 'Close to tray', checked: false },
    })

    // A real <button> turns Enter and Space into a click event; jsdom's
    // trigger() is the same path the browser takes.
    await wrapper.get('button[role="switch"]').trigger('click')

    expect(wrapper.emitted('click')).toHaveLength(1)
  })

  it('does not emit while disabled', async () => {
    const wrapper = mount(BaseToggle, {
      props: { title: 'Close to tray', checked: false, disabled: true },
    })

    const control = wrapper.get('button[role="switch"]')
    expect(control.attributes('disabled')).toBeDefined()
    await control.trigger('click')

    expect(wrapper.emitted('click')).toBeUndefined()
  })

  it('never renders a nested interactive checkbox', () => {
    const wrapper = mount(BaseToggle, {
      props: { title: 'Close to tray', checked: true },
    })

    expect(wrapper.find('input').exists()).toBe(false)
  })
})
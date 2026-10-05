/**
 * DraggableItem.vue regression (R9).
 *
 * The row advertised "Drag to reorder" in Settings but only emitted move-up /
 * move-down from arrow buttons, so nothing could be dragged.
 */
import { describe, it, expect } from 'vitest'
import { mount } from '@vue/test-utils'
import DraggableItem from '@/components/settings/DraggableItem.vue'

/** jsdom does not implement DataTransfer, so the minimal shape is stubbed here. */
function dataTransferStub() {
  return { setData: () => {}, effectAllowed: '' }
}

function dragEvent(type: string) {
  return { dataTransfer: dataTransferStub(), type }
}

describe('DraggableItem — real drag and drop reordering', () => {
  it('is draggable', () => {
    const wrapper = mount(DraggableItem, {
      props: { text: 'Spotify', index: 1, total: 3 },
    })

    expect(wrapper.attributes('draggable')).toBe('true')
  })

  it('reports where the drag started, zero-based', async () => {
    const wrapper = mount(DraggableItem, {
      props: { text: 'Tidal', index: 2, total: 3 },
    })

    await wrapper.trigger('dragstart', dragEvent('dragstart'))

    expect(wrapper.emitted('drag-start')).toEqual([[1]])
  })

  it('reports the drop target index, zero-based', async () => {
    const wrapper = mount(DraggableItem, {
      props: { text: 'Tidal', index: 2, total: 3 },
    })

    await wrapper.trigger('dragover', dragEvent('dragover'))
    await wrapper.trigger('drop', dragEvent('drop'))

    expect(wrapper.emitted('reorder')).toEqual([[1]])
  })

  it('accepts a drop because dragover is prevented', async () => {
    const wrapper = mount(DraggableItem, {
      props: { text: 'Tidal', index: 2, total: 3 },
      attachTo: document.body,
    })

    const dragover = new Event('dragover', { bubbles: true, cancelable: true })
    wrapper.element.dispatchEvent(dragover)

    expect(dragover.defaultPrevented).toBe(true)
  })

  it('marks the row while it is being dragged and clears it afterwards', async () => {
    const wrapper = mount(DraggableItem, {
      props: { text: 'Tidal', index: 2, total: 3 },
    })

    await wrapper.trigger('dragstart', dragEvent('dragstart'))
    expect(wrapper.classes()).toContain('opacity-40')

    await wrapper.trigger('dragend', dragEvent('dragend'))
    expect(wrapper.classes()).not.toContain('opacity-40')
  })

  it('keeps arrow reordering available with accessible names', async () => {
    const wrapper = mount(DraggableItem, {
      props: { text: 'Tidal', index: 2, total: 3 },
    })

    const up = wrapper.get('button[aria-label="Move Tidal up"]')
    const down = wrapper.get('button[aria-label="Move Tidal down"]')

    await up.trigger('click')
    await down.trigger('click')

    expect(wrapper.emitted('move-up')).toHaveLength(1)
    expect(wrapper.emitted('move-down')).toHaveLength(1)
  })

  it('disables moving past the ends of the list', () => {
    const first = mount(DraggableItem, { props: { text: 'Spotify', index: 1, total: 3 } })
    const last = mount(DraggableItem, { props: { text: 'Deezer', index: 3, total: 3 } })

    expect(first.get('button[aria-label="Move Spotify up"]').attributes('disabled')).toBeDefined()
    expect(last.get('button[aria-label="Move Deezer down"]').attributes('disabled')).toBeDefined()
  })

  it('keeps the auto-import checkbox out of the drag path', async () => {
    const wrapper = mount(DraggableItem, {
      props: { text: 'Spotify', index: 1, total: 2, autoImport: false },
    })

    const checkbox = wrapper.get('input[type="checkbox"]')
    expect(checkbox.attributes('aria-label')).toBe('Auto-import for Spotify')

    await checkbox.trigger('change')
    expect(wrapper.emitted('toggle-auto-import')).toHaveLength(1)
    expect(wrapper.emitted('reorder')).toBeUndefined()
  })

  it('renders an optional subtitle for rows that carry one', () => {
    const wrapper = mount(DraggableItem, {
      props: { text: 'LRCLIB', index: 1, total: 2, subtitle: 'line-level sync' },
    })

    expect(wrapper.text()).toContain('line-level sync')
  })
})
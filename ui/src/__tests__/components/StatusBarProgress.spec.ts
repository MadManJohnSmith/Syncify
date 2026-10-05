/**
 * R18: la barra de progreso global refleja el avance real de todas las tareas
 * activas, con estado indeterminado cuando ninguna declara total.
 */
import { describe, it, expect, vi, beforeEach, afterEach } from 'vitest'
import { mount, flushPromises } from '@vue/test-utils'
import StatusBar from '@/components/StatusBar.vue'
import { useGlobalTasks, resetGlobalTasks } from '@/composables/useGlobalTasks'
import { useEventBus } from '@/composables/useEventBus'
import { resetMocks } from '../setup'

vi.mock('vue-router', () => ({
  useRouter: () => ({ push: vi.fn() }),
  useRoute: () => ({ query: {}, params: {} }),
}))

async function mountBar() {
  const wrapper = mount(StatusBar)
  await flushPromises()
  return wrapper
}

describe('StatusBar: global progress bar (R18)', () => {
  beforeEach(() => {
    resetMocks()
    resetGlobalTasks()
    useEventBus().offAll()
  })

  afterEach(() => {
    resetGlobalTasks()
    useEventBus().offAll()
  })

  it('shows a bar with the weighted progress of every active task', async () => {
    const { addTask } = useGlobalTasks()
    addTask({
      id: 'download-1',
      type: 'download',
      name: 'Downloading album',
      status: 'running',
      progress: 100,
      current: 10,
      total: 10,
    })
    addTask({
      id: 'download-2',
      type: 'download',
      name: 'Downloading singles',
      status: 'running',
      progress: 10,
      current: 100,
      total: 1000,
    })

    const wrapper = await mountBar()

    const bar = wrapper.find('[role="progressbar"]')
    expect(bar.exists()).toBe(true)
    expect(bar.attributes('aria-valuenow')).toBe('11')
    expect(bar.find('div').attributes('style')).toContain('width: 11%')
    expect(wrapper.text()).toContain('2 tasks')
    expect(wrapper.text()).toContain('11%')
  })

  it('uses an indeterminate state when no task declares a total', async () => {
    const { addTask } = useGlobalTasks()
    addTask({
      id: 'metadata-1',
      type: 'metadata',
      name: 'MusicBrainz',
      status: 'running',
      progress: 35,
    })

    const wrapper = await mountBar()

    const bar = wrapper.find('[role="progressbar"]')
    expect(bar.exists()).toBe(true)
    expect(bar.attributes('aria-valuenow')).toBeUndefined()
    expect(bar.find('div').classes()).toContain('animate-pulse')
    // With no total, no frozen percentage is invented.
    expect(wrapper.text()).not.toContain('0%')
  })

  it('per-task detail stops summing every task together', async () => {
    const { addTask, updateTaskProgress } = useGlobalTasks()
    const id = addTask({
      id: 'sync-tidal',
      type: 'sync',
      name: 'Syncing Tidal',
      status: 'running',
      progress: 30,
      current: 30,
      total: 100,
      service: 'Tidal',
    })
    updateTaskProgress(id, 60, 60, 100)

    const wrapper = await mountBar()
    await wrapper.find('.sync-status').trigger('click')
    await flushPromises()

    const popover = wrapper.find('.status-popover')
    expect(popover.exists()).toBe(true)
    expect(popover.text()).toContain('60 / 100')
    expect(popover.text()).toContain('60%')
    // A 100-item sync total is no longer mixed with a multi-thousand-file scan.
    expect(popover.text()).not.toContain('120 / 200')
  })
})
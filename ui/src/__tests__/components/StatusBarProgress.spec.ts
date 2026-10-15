/**
 * R18 + BP-02/BP-03: el panel inferior muestra progreso individual solo de
 * procesos en segundo plano. La tercera barra de progreso ya no existe.
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

describe('StatusBar: background process detail (R18 + BP-02/BP-03)', () => {
  beforeEach(() => {
    resetMocks()
    resetGlobalTasks()
    useEventBus().offAll()
  })

  afterEach(() => {
    resetGlobalTasks()
    useEventBus().offAll()
  })

  it('excludes downloads from the background panel and removes the third global bar', async () => {
    const { addTask } = useGlobalTasks()
    addTask({
      id: 'download-1',
      type: 'download',
      name: 'Downloading album',
      status: 'running',
      progress: 80,
      current: 8,
      total: 10,
    })
    addTask({
      id: 'sync-2',
      type: 'sync',
      name: 'Syncing Tidal',
      status: 'running',
      progress: 10,
      current: 10,
      total: 100,
    })

    const wrapper = await mountBar()
    expect(wrapper.find('[role="progressbar"]').exists()).toBe(false)
    expect(wrapper.find('.sync-status').text()).toContain('10%')
    expect(wrapper.find('.sync-status').text()).not.toContain('80%')
    await wrapper.find('.sync-status').trigger('click')
    await flushPromises()
    const popover = wrapper.find('.status-popover')
    expect(wrapper.find('.status-bar').classes()).toContain('z-[160]')
    await wrapper.setProps({ overlayActive: true })
    expect(wrapper.find('.status-bar').classes()).toContain('z-0')
    expect(wrapper.find('.status-bar').classes()).not.toContain('z-[160]')
    expect(popover.text()).toContain('Syncing Tidal')
    expect(popover.text()).not.toContain('Downloading album')
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

    expect(wrapper.find('[role="progressbar"]').exists()).toBe(false)
    expect(wrapper.find('.sync-status').text()).not.toContain('0%')
    await wrapper.find('.sync-status').trigger('click')
    await flushPromises()
    const popover = wrapper.find('.status-popover')
    expect(popover.text()).toContain('MusicBrainz')
    expect(popover.find('.animate-pulse').exists()).toBe(true)
    // With no total, no frozen percentage is invented.
    expect(popover.text()).not.toContain('0%')
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
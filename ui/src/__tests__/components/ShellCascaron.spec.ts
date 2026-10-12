// Ola 1 shell: regressions for drawer, full-width header/progress, background-only
// progress, no Help/FAB/pill/collapse, and player-aware Settings in the footer.
import { describe, it, expect, beforeEach, afterEach, vi } from 'vitest'
import { mount, flushPromises } from '@vue/test-utils'
import App from '@/App.vue'
import StatusBar from '@/components/StatusBar.vue'
import { useGlobalTasks, resetGlobalTasks } from '@/composables/useGlobalTasks'
import { usePlayer } from '@/composables/usePlayer'
import { mockInvoke, resetMocks } from '../setup'

const track = { id: 42, title: 'Nocturne', artist: 'Chopin', album: 'Piano Works', coverUrl: null }

function stubBackend() {
  mockInvoke((cmd: string) => {
    if (cmd === 'get_worker_status') return { running: true, paused: false, active_downloads: 0, max_concurrent: 3 }
    if (cmd === 'get_queue_stats') return { total: 0, queued: 0, downloading: 0, completed: 0, failed: 0, paused: 0 }
    if (cmd === 'get_storage_stats') return { used_bytes: 0, total_bytes: 0, free_bytes: 0, breakdown: [] }
    if (cmd === 'get_service_statuses' || cmd === 'get_accounts') return []
    return []
  })
}

function mountApp() {
  return mount(App, { global: { stubs: {
    RouterView: true,
    RouterLink: {
      name: 'RouterLinkStub',
      // UI-CORR-4: `$attrs` se reenvía a mano y `to` se re-expone como
      // atributo, para que los data-testid y el destino de cada enlace lleguen
      // al DOM; sin ellos no se puede distinguir qué enlace es cuál (tests del
      // lateral colapsado). `inheritAttrs: false` evita que Vue intente además
      // aplicarlos al componente raíz del stub.
      inheritAttrs: false,
      props: ['to', 'title'],
      template: '<a :to="to" :title="title" class="router-link-stub" v-bind="$attrs"><slot /></a>',
    },
    StatusBar: true,
    ToastNotifications: true, CommandPalette: true, KeyboardShortcuts: true,
    OnboardingWizard: true,
  } } })
}

describe('Ola 1 — cascarón', () => {
  beforeEach(() => {
    resetMocks()
    resetGlobalTasks()
    usePlayer().stop()
    localStorage.clear()
    stubBackend()
  })
  afterEach(() => {
    resetGlobalTasks()
    usePlayer().stop()
  })

  it('keeps a classic always-visible sidebar below the full-width header, with no dark backdrop', async () => {
    const wrapper = mountApp()
    await flushPromises()
    const header = wrapper.find('header')
    expect(header.exists()).toBe(true)
    expect(header.classes()).toContain('w-full')
    expect(header.element.parentElement).toBe(wrapper.find('.h-screen').element)
    expect(wrapper.find('main').classes()).toContain('flex-1')
    expect(wrapper.find('main').classes()).toContain('min-w-0')
    expect(header.find('[data-testid="global-progress-indeterminate"]').exists()).toBe(false)
    expect(wrapper.find('[data-testid="view-scroll-host"]').classes()).toContain('overflow-hidden')

    // PL-01: el sidebar es hermano flex, NO un modal: existe sin pulsar nada
    // y no existe backdrop que oscurezca la app al navegar.
    const sidebar = wrapper.find('[data-testid="nav-drawer"]')
    expect(sidebar.exists()).toBe(true)
    expect(sidebar.classes()).toContain('w-64')
    expect(wrapper.find('[data-testid="nav-drawer-backdrop"]').exists()).toBe(false)

    // UI-CORR-3: el logo Syncify aparece UNA sola vez, en la cabecera de
    // arriba. El lateral ya no lo repite.
    expect(header.text()).toContain('Syncify')
    expect(sidebar.text()).not.toContain('Syncify')
    expect(sidebar.findAll('span').filter(s => s.text() === 'all_inclusive').length).toBe(0)
    // 12-oct-2026: la franja superior del lateral (el div con el botón de
    // colapsar) se eliminó a petición del usuario. La navegación arranca
    // arriba del todo: el primer hijo del lateral ya no es un div, es el <nav>.
    expect(sidebar.element.firstElementChild?.tagName.toLowerCase()).toBe('nav')
    expect(sidebar.find('[data-testid="nav-sidebar-collapse"]').exists()).toBe(false)
    // El único acceso para colapsar/expandir es la hamburguesa de la cabecera.
    const hamburger = header.find('[data-testid="nav-drawer-toggle"]')
    expect(hamburger.exists()).toBe(true)
    expect(hamburger.attributes('aria-controls')).toBe('app-nav-drawer')
    // El sidebar queda fijo bajo la cabecera (h-16 = 64px).
    expect(sidebar.classes().join(' ')).toContain('top-16')

    // UI-CORR-4: Settings volvió al lateral, con texto cuando está expandida.
    expect(sidebar.text()).toContain('Settings')
    expect(sidebar.find('[to="/settings"]').exists()).toBe(true)
    expect(sidebar.find('[data-testid="nav-sidebar-settings"]').exists()).toBe(true)
    expect(sidebar.text()).toContain('Library')
    expect(sidebar.find('[data-testid="nav-fullscreen-player"]').exists()).toBe(true)
    wrapper.unmount()
  })

  it('collapses the sidebar to icons, persists the choice and drops labels', async () => {
    const wrapper = mountApp()
    await flushPromises()
    const sidebar = wrapper.find('[data-testid="nav-drawer"]')
    expect(sidebar.classes()).toContain('w-64')
    expect(sidebar.text()).toContain('Library')

    await wrapper.find('[data-testid="nav-drawer-toggle"]').trigger('click')
    await flushPromises()

    const collapsed = wrapper.find('[data-testid="nav-drawer"]')
    expect(collapsed.classes()).toContain('w-16')
    // Solo iconos: las etiquetas desaparecen, la navegacion sigue ahi.
    expect(collapsed.text()).not.toContain('Library')
    expect(collapsed.find('[title="Library"]').exists()).toBe(true)
    expect(localStorage.getItem('syncify.sidebarCollapsed.v1')).toBe('true')

    // El toggle del header vuelve a expandir.
    const toggle = wrapper.find('[data-testid="nav-drawer-toggle"]')
    expect(toggle.attributes('aria-expanded')).toBe('false')
    await toggle.trigger('click')
    await flushPromises()
    expect(wrapper.find('[data-testid="nav-drawer"]').classes()).toContain('w-64')
    expect(localStorage.getItem('syncify.sidebarCollapsed.v1')).toBe('false')
    wrapper.unmount()
  })

  it('restores the collapsed sidebar from localStorage on mount', async () => {
    localStorage.setItem('syncify.sidebarCollapsed.v1', 'true')
    const wrapper = mountApp()
    await flushPromises()
    const collapsed = wrapper.find('[data-testid="nav-drawer"]')
    expect(collapsed.exists()).toBe(true)
    expect(collapsed.classes()).toContain('w-16')
    expect(collapsed.text()).not.toContain('Library')
    wrapper.unmount()
  })

  it('keeps Settings reachable as an icon + title when the sidebar is collapsed (UI-CORR-4)', async () => {
    const wrapper = mountApp()
    await flushPromises()
    await wrapper.find('[data-testid="nav-drawer-toggle"]').trigger('click')
    await flushPromises()

    const collapsed = wrapper.find('[data-testid="nav-drawer"]')
    expect(collapsed.classes()).toContain('w-16')
    // Sin texto (no caben etiquetas en 64px), pero con el title accesible: el
    // usuario puede seguir abriendo Ajustes desde el lateral.
    expect(collapsed.text()).not.toContain('Settings')
    expect(collapsed.find('[data-testid="nav-sidebar-settings"]').exists()).toBe(true)
    expect(collapsed.find('[title="Settings"]').exists()).toBe(true)

    // Y el estado se refleja en la hamburguesa: con el lateral colapsado,
    // "expandir" es la accion disponible.
    const toggle = wrapper.find('[data-testid="nav-drawer-toggle"]')
    expect(toggle.attributes('aria-expanded')).toBe('false')
    expect(toggle.attributes('aria-label')).toBe('Expandir menú')
    wrapper.unmount()
  })

  it('starts the sidebar straight at navigation, with no top row (UI-CORR-3)', async () => {
    const wrapper = mountApp()
    await flushPromises()

    // 12-oct-2026: la franja superior del lateral se eliminó. Ya no hay logo
    // duplicado ni botón de colapsar ahí; el lateral arranca en el <nav> y el
    // único acceso al colapso es la hamburguesa de la cabecera.
    const sidebar = wrapper.find('[data-testid="nav-drawer"]')
    expect(sidebar.element.firstElementChild?.tagName.toLowerCase()).toBe('nav')
    expect(sidebar.findAll('span').filter(s => s.text() === 'Syncify').length).toBe(0)
    expect(sidebar.findAll('[data-testid="nav-sidebar-expand"]').length).toBe(0)
    expect(sidebar.find('[data-testid="nav-sidebar-collapse"]').exists()).toBe(false)
    expect(wrapper.find('[data-testid="nav-drawer-toggle"]').exists()).toBe(true)
    wrapper.unmount()
  })

  it('offers the fullscreen player entry from the sidebar', async () => {
    const wrapper = mountApp()
    await flushPromises()
    const listener = vi.fn()
    window.addEventListener('syncify:open-fullscreen-player', listener)
    await wrapper.find('[data-testid="nav-fullscreen-player"]').trigger('click')
    expect(listener).toHaveBeenCalledOnce()
    // El sidebar sigue visible: navegar ya no lo cierra.
    expect(wrapper.find('[data-testid="nav-drawer"]').exists()).toBe(true)
    window.removeEventListener('syncify:open-fullscreen-player', listener)
    wrapper.unmount()
  })

  it('shows progress only for background tasks and never renders the pill, Help or FAB', async () => {
    const { addTask } = useGlobalTasks()
    addTask({ id: 'download-1', type: 'download', name: 'Downloading album', status: 'running', progress: 40, total: 100 })
    const wrapper = mountApp()
    await flushPromises()
    expect(wrapper.find('[data-testid="global-progress-bar"]').exists()).toBe(false)
    expect(wrapper.find('[data-testid="global-progress-indeterminate"]').exists()).toBe(false)
    expect(wrapper.find('[data-testid="global-progress-summary"]').exists()).toBe(false)
    expect(wrapper.find('.quick-actions-fab').exists()).toBe(false)
    expect(wrapper.find('header .tasks-dropdown').exists()).toBe(false)
    addTask({ id: 'metadata-1', type: 'metadata', name: 'MusicBrainz', status: 'running', progress: 25, total: 100 })
    await flushPromises()
    expect(wrapper.find('[data-testid="global-progress-bar"]').attributes('style')).toContain('width: 25%')
    expect(wrapper.find('header').text()).not.toContain('MusicBrainz')
    expect(wrapper.find('[data-testid="global-progress-summary"]').exists()).toBe(false)
    wrapper.unmount()
  })

  it('keeps Settings OUT of the footer: it moved back to the sidebar (UI-CORR-4)', async () => {
    localStorage.setItem('syncify_statusbar_collapsed', 'true')
    const wrapper = mount(StatusBar)
    await flushPromises()
    // La barra inferior sigue existiendo y sin colapsar...
    expect(wrapper.find('.status-bar').exists()).toBe(true)
    expect(wrapper.find('.status-collapsed').exists()).toBe(false)
    expect(wrapper.find('[title="Collapse status bar"]').exists()).toBe(false)
    // ...pero ya no tiene enlace a Ajustes: compite con Background Processes,
    // Online y Storage, y con el mini-reproductor abierto ni se leía.
    expect(wrapper.find('[data-testid="status-bar-settings"]').exists()).toBe(false)
    expect(wrapper.text()).not.toContain('Settings')
    // Storage sigue en su extremo derecho, intacto.
    expect(wrapper.find('.storage-info').exists()).toBe(true)
    wrapper.unmount()
  })
})

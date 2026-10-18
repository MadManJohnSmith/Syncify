/**
 * LayoutSpacing.spec.ts
 *
 * TASK-18: Tests validating that App reserves only the 32px StatusBar
 * footer while its mini player remains available. PL-07 removes the FAB.
 */
import { describe, it, expect, beforeEach, beforeAll, vi } from 'vitest'
import { mount, flushPromises } from '@vue/test-utils'
import App from '@/App.vue'
import StatusBar from '@/components/StatusBar.vue'
import { usePlayer, type PlayerTrack } from '@/composables/usePlayer'
import { mockInvoke } from '../setup'

// Stub audio element methods
beforeAll(() => {
  Object.defineProperty(window.HTMLMediaElement.prototype, 'play', {
    configurable: true,
    value: vi.fn(() => Promise.resolve()),
  })
  Object.defineProperty(window.HTMLMediaElement.prototype, 'pause', {
    configurable: true,
    value: vi.fn(),
  })
  Object.defineProperty(window.HTMLMediaElement.prototype, 'load', {
    configurable: true,
    value: vi.fn(),
  })
})

const sampleTrack: PlayerTrack = {
  id: 42,
  title: 'Nocturne in C-sharp minor',
  artist: 'Frédéric Chopin',
  album: 'Chopin: Piano Works',
  coverUrl: null,
}

describe('TASK-18: Footer Layout Collision and Dynamic Spacing', () => {
  const player = usePlayer()

  beforeEach(() => {
    player.stop()
    mockInvoke((cmd: string) => {
      if (cmd === 'get_worker_status') return { running: true, paused: false, active_downloads: 0, max_concurrent: 3 }
      if (cmd === 'get_queue_stats') return { total: 0, queued: 0, downloading: 0, completed: 0, failed: 0, paused: 0 }
      if (cmd === 'get_storage_stats') return { used_bytes: 0, total_bytes: 0, free_bytes: 0, breakdown: [] }
      if (cmd === 'get_service_statuses') return []
      return []
    })
  })

  describe('sidebar bottom controls above StatusBar', () => {
    it('keeps Now Playing reachable when the fixed status bar grows', async () => {
      const wrapper = mount(App, {
        global: {
          stubs: { RouterView: true, RouterLink: true },
        },
      })
      await flushPromises()

      const sidebar = wrapper.find('[data-testid="nav-drawer"]')
      const nav = sidebar.find('nav[aria-label="Views"]')
      const openPlayer = sidebar.find('[data-testid="nav-fullscreen-player"]')
      expect(openPlayer.exists()).toBe(true)
      expect(nav.exists()).toBe(true)
      // The bar publishes its measured height as --status-bar-h; the sidebar
      // scroller reserves that same height plus the 8px visual gutter. This
      // keeps its final button scrollable above the bar even when it wraps.
      expect(nav.attributes('style')).toContain('var(--status-bar-h, 48px) + 8px')
      expect(nav.classes()).toContain('overflow-y-auto')
      wrapper.unmount()
    })
  })

  describe('App.vue content bottom padding', () => {
    it('reserves the REAL height of the StatusBar footer, not a fixed pb-8 (UI-CORR-1)', async () => {
      const wrapper = mount(App, {
        global: {
          stubs: {
            RouterView: true,
            RouterLink: true,
          },
        },
      })
      await flushPromises()

      const pageContent = wrapper.find('#page-content')
      expect(pageContent.exists()).toBe(true)

      // UI-CORR-1: la barra inferior mide ahora su alto REAL (StatusBar publica
      // `--status-bar-h` en el documentElement al montar y en cada resize) y
      // `#page-content` reserva exactamente eso + colchón. El reserva ya NO es
      // una clase utilitaria fija (`pb-8` = 32px, `pb-24` = 96px): es la
      // variable CSS medida, así que se comprueba el valor inline y que no
      // queda ninguna clase que afirme un alto distinto.
      expect(pageContent.attributes('style')).toBe('padding-bottom: calc(var(--status-bar-h, 48px) + 8px);')
      expect(pageContent.classes()).not.toContain('pb-8')
      expect(pageContent.classes()).not.toContain('pb-24')

      // Y el valor que la barra publica es el alto real mínimo (48px), con el
      // que se suma el colchón de 8px → 56px reservados. Sin pista activa, el
      // hueco es solo la barra.
      expect(document.documentElement.style.getPropertyValue('--status-bar-h')).toBe('48px')

      // Activate player track: el mini-reproductor vive DENTRO de StatusBar, no
      // debajo de ella, así que el hueco reservado no crece ni se duplica.
      player.current.value = sampleTrack
      await flushPromises()

      expect(pageContent.attributes('style')).toBe('padding-bottom: calc(var(--status-bar-h, 48px) + 8px);')
      expect(pageContent.classes()).not.toContain('pb-8')
      expect(pageContent.classes()).not.toContain('pb-24')

      // Reset / stop playback: el hueco se mantiene estable.
      player.stop()
      await flushPromises()

      expect(pageContent.attributes('style')).toBe('padding-bottom: calc(var(--status-bar-h, 48px) + 8px);')

      wrapper.unmount()
    })

    it('renders exactly one footer player, inside StatusBar', async () => {
      const wrapper = mount(App, {
        global: {
          stubs: {
            RouterView: true,
            RouterLink: true,
            StatusBar: true,
          },
        },
      })
      player.current.value = sampleTrack
      await flushPromises()

      // PL-08: the duplicate NowPlayingBar is gone; StatusBar is the sole
      // holder of the mini player (mounted here as a stub wrapper element).
      const statusBars = wrapper.findAllComponents(StatusBar)
      expect(statusBars.length).toBe(1)
      expect(wrapper.text()).not.toContain('NowPlayingBar')

      await wrapper.unmount()
    })
  })

  // PL-08: NowPlayingBar.vue se borró por decisión explícita del usuario — su
  // reproductor ya vivía duplicado dentro de StatusBar. Este spec sigue
  // cubriendo el padding del shell y el mini reproductor de StatusBar.

  describe('StatusBar.vue mini player', () => {
    it('renders the track and the transport controls while a track is active', async () => {
      const wrapper = mount(StatusBar)
      await flushPromises()

      // Without a track there is no player at all
      expect(wrapper.find('[data-testid="status-bar-player"]').exists()).toBe(false)

      player.current.value = sampleTrack
      await flushPromises()

      const player_ = wrapper.find('[data-testid="status-bar-player"]')
      expect(player_.exists()).toBe(true)
      expect(wrapper.text()).toContain('Nocturne in C-sharp minor')

      for (const testId of ['status-bar-prev', 'status-bar-toggle', 'status-bar-next', 'status-bar-track-cover']) {
        expect(wrapper.find(`[data-testid="${testId}"]`).exists()).toBe(true)
      }

      // Transport still drives the shared player composable
      player.stop()
      player.current.value = sampleTrack
      player.closeFullscreen()
      await flushPromises()
      await wrapper.find('[data-testid="status-bar-track-cover"]').trigger('click')
      expect(player.isFullscreenOpen.value).toBe(true)

      wrapper.unmount()
    })
  })

  describe('StatusBar.vue popover elevation and positioning', () => {
    it('elevates popovers to z-[160] and shifts them between bottom-10 and bottom-26 based on player state', async () => {
      const wrapper = mount(StatusBar)
      await flushPromises()

      // Open sync popover
      const syncStatusBtn = wrapper.find('.sync-status')
      await syncStatusBtn.trigger('click')
      await flushPromises()

      const popover = wrapper.find('.status-popover')
      expect(popover.exists()).toBe(true)
      expect(popover.classes()).toContain('z-[160]')
      // When player is idle, popover sits at bottom-10
      expect(popover.classes()).toContain('bottom-10')
      expect(popover.classes()).not.toContain('bottom-26')

      // When player becomes active, popover lifts to bottom-26
      player.current.value = sampleTrack
      await flushPromises()

      expect(popover.classes()).toContain('bottom-26')
      expect(popover.classes()).not.toContain('bottom-10')
      expect(popover.classes()).toContain('z-[160]')
    })

    // UI-CORR-2 (10-oct-2026): el botón "Online" está a la izquierda, pero su
    // popover se centraba en la barra (`left-1/2 -translate-x-1/2` sobre el
    // contenedor de la barra) y abría a cientos de píxeles del texto pulsado.
    // Ahora se ancla al propio botón, desde un contenedor `relative`, así que
    // se centra justo ENCIMA de él.
    it('anchors the Network popover to the Online button instead of centering it on the bar (UI-CORR-2)', async () => {
      const wrapper = mount(StatusBar)
      await flushPromises()

      const onlineBtn = wrapper.find('[data-testid="status-bar-network"]')
      expect(onlineBtn.exists()).toBe(true)

      // El botón vive dentro de un contenedor `relative`: ése es el ancla de
      // posicionamiento del popover. Antes el popover era hermano suelto y se
      // centraba en la barra entera.
      const anchor = onlineBtn.element.parentElement as HTMLElement
      expect(anchor.classList.contains('relative')).toBe(true)

      // No está centrado en la barra: el popover es hijo del ancla.
      expect(anchor.querySelector('.status-popover')).toBe(null) // aún cerrado
      await onlineBtn.trigger('click')
      await flushPromises()

      const networkPopover = anchor.querySelector('.status-popover') as HTMLElement | null
      expect(networkPopover).not.toBe(null)
      expect(networkPopover!.classList.contains('z-[160]')).toBe(true)

      // Se posiciona sobre el botón: `bottom: 100%` (encima) y centrado en él.
      // En jsdom no hay layout real (todos los `getBoundingClientRect()` son 0),
      // así que el clamp de ventana estrecha cae en su rama alineada al ancla;
      // eso es justo lo que garantiza que el panel nazca del botón y no del
      // centro de la barra. El contrato que aquí se verifica es el ancla: el
      // popover es hijo del contenedor del botón y se sitúa sobre él.
      const style = networkPopover!.getAttribute('style') ?? ''
      expect(style).toContain('position: absolute')
      expect(style).toContain('bottom: 100%')
      expect(style).toMatch(/left: (50%|0px)/)
      expect(style).toMatch(/translateX\(-50%\)|transform: none/)

      // El hueco de separación visual respecto al botón existe en ambas ramas.
      expect(style).toContain('margin-bottom: 0.5rem')

      // Y el eje Z sigue intacto: la barra sube su stacking al abrir un panel,
      // el botón no crea un contexto propio nuevo.
      const statusBar = wrapper.find('.status-bar')
      expect(statusBar.classes()).toContain('z-[160]')

      wrapper.unmount()
    })

    // La prueba anterior sólo puede comprobar el ancla: sin layout real, todas
    // las cajas miden 0. Aquí se inyecta geometría real para ejercitar las dos
    // ramas del clamp de `networkPopoverStyle` y demostrar que el panel queda
    // centrado sobre el botón cuando hay sitio, y alineado al borde cuando no.
    it('centres the Network popover on the button and clamps it to the viewport on narrow screens (UI-CORR-2)', async () => {
      const wrapper = mount(StatusBar)
      await flushPromises()
      await wrapper.find('[data-testid="status-bar-network"]').trigger('click')
      await flushPromises()

      const bar = wrapper.find('.status-bar').element as HTMLElement
      const button = wrapper.find('.network-status').element as HTMLElement

      const stubRects = (
        barLeft: number, barRight: number,
        btnLeft: number, btnRight: number,
      ) => {
        bar.getBoundingClientRect = () =>
          ({ left: barLeft, right: barRight, width: barRight - barLeft }) as DOMRect
        button.getBoundingClientRect = () =>
          ({ left: btnLeft, right: btnRight, width: btnRight - btnLeft }) as DOMRect
      }

      const popoverStyle = () =>
        wrapper.find('.status-popover').element.getAttribute('style') ?? ''

      /** La geometría no es reactiva: hay que forzar un repintado para que la
       *  función se re-evalúe con las cajas recién inyectadas. */
      const rerender = async () => {
        await wrapper.find('[data-testid="status-bar-network"]').trigger('click')
        await wrapper.find('[data-testid="status-bar-network"]').trigger('click')
        await flushPromises()
      }

      // Ventana ancha (1200px): el botón está en x=560..660 y caben 144px a
      // cada lado del panel de 288px → rama centrada, justo sobre el botón.
      stubRects(0, 1200, 560, 660)
      await rerender()
      expect(popoverStyle()).toContain('left: 50%')
      expect(popoverStyle()).toContain('translateX(-50%)')

      // Ventana estrecha (320px), botón pegado a la izquierda: no caben 144px a
      // la izquierda → se alinea al borde izquierdo en vez de salirse.
      stubRects(0, 320, 80, 160)
      await rerender()
      expect(popoverStyle()).toContain('left: 0px')
      expect(popoverStyle()).toContain('transform: none')

      // Ventana estrecha, botón pegado a la derecha: se alinea al borde derecho.
      stubRects(0, 320, 160, 260)
      await rerender()
      expect(popoverStyle()).toContain('right: 0px')
      expect(popoverStyle()).toContain('transform: none')

      // En ningún caso el panel se sale de la barra: `bottom: 100%` se mantiene,
      // así que siempre queda anclado al botón que lo abrió.
      expect(popoverStyle()).toContain('bottom: 100%')

      wrapper.unmount()
    })
  })

  // UI-CORR-1 (10-oct-2026): el reproductor de pantalla completa se renderiza
  // con `<Teleport to="body">`, para salir del stacking context del footer y
  // de su `overflow`. Dentro del footer, un `z-[200]` nunca podía cubrir al
  // resto de la app y quedaba recortado.
  describe('FullscreenPlayer.vue stacking and teleport', () => {
    it('renders exactly one player, outside the StatusBar stacking context, above every popover (UI-CORR-1)', async () => {
      const wrapper = mount(App, {
        global: {
          stubs: {
            RouterView: true,
            RouterLink: true,
          },
        },
      })
      player.current.value = sampleTrack
      player.openFullscreen()
      await flushPromises()

      // Un único reproductor en pantalla completa.
      const players = document.querySelectorAll('[data-testid="fullscreen-player"]')
      expect(players.length).toBe(1)

      // Está fuera del `wrapper` (fuera del footer y de su stacking context).
      expect(wrapper.element.contains(players[0])).toBe(false)

      // Y por encima de los popovers de la barra (z-[160]).
      expect(players[0].classList.contains('z-[200]')).toBe(true)

      // El cierre real (no solo ocultar el estado) retira el nodo del DOM.
      player.closeFullscreen()
      await flushPromises()
      expect(document.querySelectorAll('[data-testid="fullscreen-player"]').length).toBe(0)

      wrapper.unmount()
    })
  })
})

import { describe, it, expect, vi, beforeEach, beforeAll } from 'vitest'
import { mount, flushPromises } from '@vue/test-utils'
import { nextTick } from 'vue'
import StatusBar from '@/components/StatusBar.vue'
import FullscreenPlayer from '@/components/player/FullscreenPlayer.vue'
import { usePlayer } from '@/composables/usePlayer'
import { mockInvoke } from '../setup'

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

const sampleTrack = {
  id: 101,
  title: 'Starboy',
  artist: 'The Weeknd',
  album: 'Starboy',
  coverUrl: 'https://example.com/cover.jpg',
}

/**
 * UI-CORR-1 (10-oct-2026): el reproductor de pantalla completa se renderiza
 * ahora con `<Teleport to="body">`, para salir del stacking context del footer
 * `StatusBar` y de su `overflow` (que era lo que lo recortaba). Consecuencia
 * para las pruebas: su DOM vive en `document.body`, NO dentro del `wrapper`,
 * así que `wrapper.find()` no lo ve. Estas consultas al documento son la forma
 * de testearlo.
 */
function docQuery(selector: string): HTMLElement | null {
  return document.querySelector<HTMLElement>(selector)
}

function docQueryAll(selector: string): HTMLElement[] {
  return Array.from(document.querySelectorAll<HTMLElement>(selector))
}

/** Espera a que Vue pinte el siguiente tick (Teleport incluido). */
async function paint() {
  await flushPromises()
  await nextTick()
}

describe('PL-08, PL-09, PL-10, PL-04, PL-05: Reproductor Completo & StatusBar Center', () => {
  const player = usePlayer()

  beforeEach(() => {
    player.stop()
    player.closeFullscreen()
    mockInvoke((cmd: string, args: any) => {
      if (cmd === 'get_worker_status') return { running: true, paused: false, active_downloads: 0, max_concurrent: 3 }
      if (cmd === 'get_queue_stats') return { total: 0, queued: 0, downloading: 0, completed: 0, failed: 0, paused: 0 }
      if (cmd === 'get_storage_stats') return { used_bytes: 0, total_bytes: 0, free_bytes: 0, breakdown: [] }
      if (cmd === 'get_service_statuses') return []
      if (cmd === 'get_track_metadata') {
        return {
          track_id: 101,
          title: 'Starboy',
          artist_name: 'The Weeknd',
          album_name: 'Starboy',
          album_id: 55,
          duration_ms: 230000,
          track_number: 1,
          disc_number: 1,
          isrc: 'USUM71607007',
          explicit: true,
          genre: 'R&B',
          bpm: 186,
          musical_key: 'G Minor',
          release_year: 2016,
          musicbrainz_id: 'mb-uuid-1234',
          cover_art_url: 'https://example.com/cover.jpg',
          file_path: '/music/The Weeknd/Starboy/01 - Starboy.flac',
          sources: [
            {
              id: 1,
              trackId: 101,
              serviceId: 1,
              format: 'FLAC',
              bitDepth: 24,
              sampleRate: 96000,
              qualityScore: 98,
              serviceName: 'qobuz',
              available: 1,
              availabilityStatus: 'confirmed',
              availabilityReason: null,
              lastChecked: null,
            },
          ],
        }
      }
      if (cmd === 'get_lyrics') {
        return {
          id: 50,
          track_id: 101,
          format: 'lrc',
          sync_level: 'line',
          source: 'lrclib',
          content: "[00:01.00] I'm tryna put you in the worst mood, ah\n[00:04.50] P1 cleaner than your church shoes, ah\n[00:10.00] Milli point two just to hurt you, ah",
          language: 'en',
        }
      }
      return null
    })
  })

  describe('PL-08: Reproductor en el centro de StatusBar.vue', () => {
    it('no muestra el reproductor central cuando no hay pista activa', async () => {
      const wrapper = mount(StatusBar)
      await flushPromises()

      expect(wrapper.find('[data-testid="status-bar-player"]').exists()).toBe(false)
      wrapper.unmount()
    })

    it('renderiza la pista en curso en el centro de la barra con controles y título', async () => {
      const wrapper = mount(StatusBar)
      player.current.value = sampleTrack
      player.durationSec.value = 230
      player.positionSec.value = 45
      await flushPromises()

      const centerPlayer = wrapper.find('[data-testid="status-bar-player"]')
      expect(centerPlayer.exists()).toBe(true)
      expect(centerPlayer.classes()).toContain('status-center-player')
      expect(centerPlayer.classes()).toContain('justify-center')

      // Muestra título y artista
      const info = wrapper.find('[data-testid="status-bar-track-info"]')
      expect(info.text()).toContain('Starboy')
      expect(info.text()).toContain('The Weeknd')

      // Controles de transporte
      expect(wrapper.find('[data-testid="status-bar-prev"]').exists()).toBe(true)
      expect(wrapper.find('[data-testid="status-bar-toggle"]').exists()).toBe(true)
      expect(wrapper.find('[data-testid="status-bar-next"]').exists()).toBe(true)

      // Botón expandir a pantalla completa
      expect(wrapper.find('[data-testid="status-bar-expand-btn"]').exists()).toBe(true)

      wrapper.unmount()
    })
  })

  describe('PL-09: Señal de hover en la portada y click para expandir reproductor', () => {
    it('la portada en StatusBar señala que se puede expandir y abre el reproductor al hacer click', async () => {
      const wrapper = mount(StatusBar)
      player.current.value = sampleTrack
      await flushPromises()

      const coverBtn = wrapper.find('[data-testid="status-bar-track-cover"]')
      expect(coverBtn.exists()).toBe(true)

      // Señal visual de hover (overlay con icono)
      const hint = wrapper.find('[data-testid="cover-expand-hint"]')
      expect(hint.exists()).toBe(true)
      expect(hint.classes()).toContain('group-hover:opacity-100')
      expect(coverBtn.attributes('title')).toContain('Expand player (Fullscreen)')

      // Click abre el reproductor
      expect(player.isFullscreenOpen.value).toBe(false)
      await coverBtn.trigger('click')
      expect(player.isFullscreenOpen.value).toBe(true)

      wrapper.unmount()
    })

  })

  describe('PL-10: Acceso al reproductor grande desde evento de ventana (menú hamburguesa)', () => {
    it('abre el reproductor grande al recibir syncify:open-fullscreen-player', async () => {
      const wrapper = mount(FullscreenPlayer)
      await paint()

      expect(player.isFullscreenOpen.value).toBe(false)
      expect(docQuery('[data-testid="fullscreen-player"]')).toBe(null)

      // Simular evento emitido por App.vue al pulsar "Now Playing" en el drawer
      window.dispatchEvent(new CustomEvent('syncify:open-fullscreen-player'))
      await paint()

      expect(player.isFullscreenOpen.value).toBe(true)
      // UI-CORR-1: va al body (hijo directo), no dentro del wrapper. VTU
      // sustituye <Transition> por su stub, así que el contenedor inmediato es
      // ese stub; lo que importa es que el nodo ya no esté bajo el `wrapper`.
      const teleported = docQuery('[data-testid="fullscreen-player"]')!
      expect(teleported).not.toBe(null)
      expect(wrapper.element.contains(teleported)).toBe(false)
      expect(wrapper.find('[data-testid="fullscreen-player"]').exists()).toBe(false)

      // Cerrar mediante botón cerrar
      const closeBtn = docQuery('[data-testid="close-fullscreen-player"]')!
      closeBtn.click()
      await paint()
      expect(player.isFullscreenOpen.value).toBe(false)
      expect(docQuery('[data-testid="fullscreen-player"]')).toBe(null)

      wrapper.unmount()
    })

    it('cierra el reproductor grande al presionar Escape', async () => {
      const wrapper = mount(FullscreenPlayer)
      player.openFullscreen()
      await paint()

      expect(player.isFullscreenOpen.value).toBe(true)
      window.dispatchEvent(new KeyboardEvent('keydown', { key: 'Escape' }))
      await paint()

      expect(player.isFullscreenOpen.value).toBe(false)
      wrapper.unmount()
    })
  })

  describe('PL-04: Letras sincronizadas, portada animada/HD y metadatos completos', () => {
    it('muestra y resalta letras sincronizadas según la posición y permite buscar con click', async () => {
      const wrapper = mount(FullscreenPlayer)
      player.current.value = sampleTrack
      player.positionSec.value = 5.0
      player.openFullscreen()
      await paint()

      // Pestaña letras activa por defecto
      expect(docQuery('[data-testid="panel-lyrics"]')).not.toBe(null)

      // Las 3 líneas parseadas
      const line0 = docQuery('[data-testid="lyric-line-0"]')
      const line1 = docQuery('[data-testid="lyric-line-1"]')
      const line2 = docQuery('[data-testid="lyric-line-2"]')

      expect(line0).not.toBe(null)
      expect(line1).not.toBe(null)
      expect(line2).not.toBe(null)

      // En pos 5.0, la línea activa es la 1 (starts at 00:04.50)
      expect(line1!.classList.contains('text-accent')).toBe(true)
      expect(line1!.classList.contains('font-bold')).toBe(true)

      // Click en línea 2 para seek
      line2!.click()
      await paint()
      expect(player.positionSec.value).toBe(10)

      wrapper.unmount()
    })

    it('muestra panel de metadatos completos con calidad Hi-Res, ISRC, BPM y tonalidad', async () => {
      const wrapper = mount(FullscreenPlayer)
      player.current.value = sampleTrack
      player.openFullscreen()
      await paint()

      // Cambiar a pestaña metadatos
      docQuery('[data-testid="tab-metadata"]')!.click()
      await paint()

      const metaPanel = docQuery('[data-testid="panel-metadata"]')!
      expect(metaPanel).not.toBe(null)
      expect(metaPanel.textContent).toContain('USUM71607007') // ISRC
      expect(metaPanel.textContent).toContain('186') // BPM
      expect(metaPanel.textContent).toContain('G Minor') // Musical key
      expect(metaPanel.textContent).toContain('24-bit')
      expect(metaPanel.textContent).toContain('96.0 kHz')
      expect(metaPanel.textContent).toContain('FLAC')

      // Badge de calidad en el contenedor de carátula
      const qualityBadge = docQuery('[data-testid="audio-quality-badge"]')!
      expect(qualityBadge).not.toBe(null)
      expect(qualityBadge.textContent).toContain('FLAC')

      wrapper.unmount()
    })

    it('permite alternar entre portada animada y HD cuando existe portada animada', async () => {
      const wrapper = mount(FullscreenPlayer)
      player.current.value = sampleTrack
      player.openFullscreen()
      await paint()

      // Por defecto muestra toggle de animada si existe o HD
      expect(docQuery('[data-testid="fullscreen-cover-container"]')).not.toBe(null)

      wrapper.unmount()
    })
  })

  describe('PL-05: Visualizador de Booklets digitales (.pdf)', () => {
    it('muestra la pestaña de booklet con mensaje informativo si el álbum no tiene booklet', async () => {
      const wrapper = mount(FullscreenPlayer)
      player.current.value = sampleTrack
      player.openFullscreen()
      await flushPromises()

      // Ir a pestaña Booklet
      docQuery('[data-testid="tab-booklet"]')!.click()
      await paint()

      const bookletPanel = docQuery('[data-testid="panel-booklet"]')!
      expect(bookletPanel).not.toBe(null)
      expect(bookletPanel.textContent).toContain('No Digital Booklet')
      expect(bookletPanel.textContent).toContain('booklet.pdf')

      wrapper.unmount()
    })
  })

  /**
   * UI-CORR-2 (18-oct-2026): los seis defectos del reproductor grande.
   * El backend resuelve los sidecars contra el sistema de ficheros real
   * (`fetch_track_metadata`) y el webview sólo compone. Estas pruebas fijan el
   * contrato observado: booklet visible, metadata completa, carátula +50% y
   * libre, letras limpias por línea y carátulas animadas (WebP y MP4).
   */
  describe('UI-CORR-2: correcciones del reproductor grande', () => {
    const richMetadata = {
      track_id: 101,
      title: 'Starboy',
      artist_name: 'The Weeknd',
      album_artist: 'The Weeknd',
      album_name: 'Starboy',
      album_id: 55,
      duration_ms: 230000,
      track_number: 1,
      total_tracks: 18,
      disc_number: 1,
      total_discs: 2,
      isrc: 'USUM71607007',
      explicit: true,
      genre: 'R&B',
      bpm: 186,
      musical_key: 'G Minor',
      release_year: 2016,
      release_date: '2016-11-25',
      label: 'XO',
      upc: '0602557234567',
      musicbrainz_id: 'mb-uuid-1234',
      album_musicbrainz_id: 'mb-album-uuid',
      musicbrainz_release_id: 'mb-release-uuid',
      cover_art_url: 'https://example.com/cover.jpg',
      file_path: '/music/The Weeknd/Starboy/01 - Starboy.flac',
      file_format: 'FLAC',
      file_size_bytes: 52_428_800,
      bit_depth: 24,
      sample_rate: 96000,
      downloaded_at: '2026-10-18 10:00:00',
      imported_from: 'tidal',
      downloaded_from: 'qobuz',
      tempo_source: 'analysis',
      tempo_confidence: 0.93,
      file_disambiguator: 'remaster-2016',
      booklet_path: '/music/The Weeknd/Starboy/booklet.pdf',
      animated_cover_path: '/music/The Weeknd/Starboy/cover.animated.webp',
      hd_cover_path: '/music/The Weeknd/Starboy/cover.jpg',
      sources: [
        {
          id: 1,
          trackId: 101,
          serviceId: 1,
          format: 'FLAC',
          bitDepth: 24,
          sampleRate: 96000,
          qualityScore: 98,
          serviceName: 'qobuz',
          available: 1,
          availabilityStatus: 'confirmed',
          availabilityReason: null,
          lastChecked: null,
        },
      ],
    }

    function useRichMetadata(overrides: Record<string, unknown> = {}) {
      mockInvoke((cmd: string) => {
        if (cmd === 'get_track_metadata') return { ...richMetadata, ...overrides }
        if (cmd === 'get_lyrics') {
          return {
            id: 50,
            track_id: 101,
            format: 'lrc',
            sync_level: 'word',
            source: 'lrclib',
            content:
              '[00:01.00]<00:01.00>I <00:01.30>looked <00:01.60>at <00:02.00>you\n[00:04.50]<00:04.50>Same <00:04.90>as <00:05.10>before',
            language: 'en',
          }
        }
        if (cmd === 'read_track_tags') {
          return {
            track_id: 101,
            file_path: richMetadata.file_path,
            file_format: 'FLAC',
            all_tags: {
              ALBUMARTIST: ['The Weeknd'],
              COMPOSER: ['Doc McKinney'],
              REPLAYGAIN_TRACK_GAIN: ['-7.20 dB'],
            },
            has_cover: true,
            cover_mime: 'image/jpeg',
          }
        }
        return null
      })
    }

    it('muestra el booklet resuelto por el backend en el iframe (asset protocol)', async () => {
      useRichMetadata()
      const wrapper = mount(FullscreenPlayer)
      player.current.value = sampleTrack
      player.openFullscreen()
      await paint()

      docQuery('[data-testid="tab-booklet"]')!.click()
      await paint()

      const iframe = docQuery('[data-testid="booklet-iframe"]')
      expect(iframe).not.toBe(null)
      expect(iframe!.getAttribute('src')).toContain(
        encodeURIComponent('/music/The Weeknd/Starboy/booklet.pdf'),
      )
      expect(docQuery('[data-testid="panel-booklet"]')!.textContent).toContain('Available')

      wrapper.unmount()
    })

    it('renderiza la carátula animada WebP y la variante MP4 como vídeo', async () => {
      useRichMetadata()
      const wrapper = mount(FullscreenPlayer)
      player.current.value = sampleTrack
      player.openFullscreen()
      await paint()

      const animatedImg = docQuery('[data-testid="animated-cover-img"]')
      expect(animatedImg).not.toBe(null)
      expect(animatedImg!.getAttribute('src')).toContain(
        encodeURIComponent('/music/The Weeknd/Starboy/cover.animated.webp'),
      )
      // El conmutador aparece porque hay portada animada.
      expect(docQuery('[data-testid="toggle-cover-hd"]')).not.toBe(null)
      wrapper.unmount()

      // Variante MP4: video, nunca <img> (un <img> no reproduce vídeo).
      useRichMetadata({ animated_cover_path: '/music/The Weeknd/Starboy/animated_cover.mp4' })
      const wrapper2 = mount(FullscreenPlayer)
      player.current.value = sampleTrack
      player.openFullscreen()
      await paint()

      const video = docQuery('[data-testid="animated-cover-video"]')
      expect(video).not.toBe(null)
      expect(video!.getAttribute('src')).toContain(
        encodeURIComponent('/music/The Weeknd/Starboy/animated_cover.mp4'),
      )
      expect(docQuery('[data-testid="animated-cover-img"]')).toBe(null)
      wrapper2.unmount()
    })

    it('deja la etiqueta de calidad fuera de la carátula y agranda el arte un 50%', async () => {
      useRichMetadata()
      const wrapper = mount(FullscreenPlayer)
      player.current.value = sampleTrack
      player.openFullscreen()
      await paint()

      const cover = docQuery('[data-testid="fullscreen-cover-container"]')!
      const badge = docQuery('[data-testid="audio-quality-badge"]')!

      expect(badge).not.toBe(null)
      // Nada tapa la carátula: la etiqueta vive fuera del contenedor del arte.
      expect(cover.contains(badge)).toBe(false)
      expect(badge.textContent).toContain('FLAC')
      expect(badge.textContent).toContain('24-bit')
      // 384 / 480 / 576 px = +50% sobre los 256 / 320 / 384 anteriores.
      expect(cover.className).toContain('max-w-[24rem]')
      expect(cover.className).toContain('sm:max-w-[30rem]')
      expect(cover.className).toContain('md:max-w-[36rem]')

      wrapper.unmount()
    })

    it('etiqueta la identidad cuando la fuente viene de la cascada (ISRC o metadatos)', async () => {
      useRichMetadata()
      // La pista ya sonaba desde Tidal lossy y la cascada encontró la misma
      // grabación en Qobuz por metadatos: el badge debe decirlo, no ocultarlo.
      player.streamQuality.value = {
        serviceName: 'qobuz',
        qualityLabel: 'FLAC 16 bits / 44.1 kHz',
        qualityTier: 'lossless',
        matchMethod: 'metadata',
      }

      const wrapper = mount(FullscreenPlayer)
      player.current.value = sampleTrack
      player.openFullscreen()
      await paint()

      const badge = docQuery('[data-testid="audio-quality-badge"]')!
      expect(badge.textContent).toContain('FLAC 16 bits / 44.1 kHz')
      const note = docQuery('[data-testid="audio-quality-match"]')
      expect(note).not.toBe(null)
      expect(note!.textContent).toContain('por metadatos')
      expect(badge.getAttribute('title')).toContain('misma grabación')

      // Sin cascada (fuente directa) no se añade ruido al badge.
      player.streamQuality.value = {
        serviceName: 'qobuz',
        qualityLabel: 'FLAC 24 bits / 96 kHz',
        qualityTier: 'hires',
        matchMethod: 'direct',
      }
      await paint()
      expect(docQuery('[data-testid="audio-quality-match"]')).toBe(null)

      player.streamQuality.value = null
      wrapper.unmount()
    })

    it('lista la metadata ampliada: album artist, sello, UPC, fichero y tags del archivo', async () => {
      useRichMetadata()
      const wrapper = mount(FullscreenPlayer)
      player.current.value = sampleTrack
      player.openFullscreen()
      await paint()

      docQuery('[data-testid="tab-metadata"]')!.click()
      await paint()

      const panel = docQuery('[data-testid="panel-metadata"]')!
      const text = panel.textContent ?? ''
      expect(text).toContain('Album Artist')
      expect(text).toContain('The Weeknd')
      expect(text).toContain('XO') // sello
      expect(text).toContain('0602557234567') // UPC
      expect(text).toContain('2016-11-25') // fecha de lanzamiento
      expect(text).toContain('#1') // número de pista
      expect(text).toContain('/ 18') // total de pistas
      expect(text).toContain('50.0 MB') // tamaño de fichero
      expect(text).toContain('remaster-2016') // disambiguador
      // Tags reales del contenedor (no sólo las columnas SQL).
      expect(text).toContain('COMPOSER')
      expect(text).toContain('Doc McKinney')
      expect(text).toContain('REPLAYGAIN_TRACK_GAIN')

      wrapper.unmount()
    })

    it('renderiza las letras por línea con time tags, sin etiquetas de palabra', async () => {
      useRichMetadata()
      const wrapper = mount(FullscreenPlayer)
      player.current.value = sampleTrack
      player.positionSec.value = 4.6
      player.openFullscreen()
      await paint()

      const line0 = docQuery('[data-testid="lyric-line-0"]')!
      const line1 = docQuery('[data-testid="lyric-line-1"]')!
      expect(line0.textContent).toBe('I looked at you')
      expect(line1.textContent).toBe('Same as before')
      // Ni un `<00:01.00>` visible en el texto.
      expect(line0.textContent).not.toContain('<')
      // La línea activa sigue siendo la 1 (empieza en 00:04.50).
      expect(line1.classList.contains('text-accent')).toBe(true)

      wrapper.unmount()
    })

    it('abre el booklet externamente con la ruta real (no una URL asset)', async () => {
      useRichMetadata()
      const invoked: Array<{ cmd: string; args?: Record<string, unknown> }> = []
      mockInvoke((cmd: string, args?: Record<string, unknown>) => {
        if (cmd === 'get_track_metadata') return richMetadata
        if (cmd === 'open_local_sidecar') {
          invoked.push({ cmd, args })
          return null
        }
        return null
      })

      const wrapper = mount(FullscreenPlayer)
      player.current.value = sampleTrack
      player.openFullscreen()
      await paint()

      docQuery('[data-testid="tab-booklet"]')!.click()
      await paint()

      docQuery('[data-testid="booklet-open-external"]')!.click()
      await paint()

      expect(invoked).toHaveLength(1)
      expect(invoked[0].args).toEqual({ path: '/music/The Weeknd/Starboy/booklet.pdf' })

      wrapper.unmount()
    })
  })
})

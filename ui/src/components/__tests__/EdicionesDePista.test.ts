import { describe, it, expect, vi, beforeEach } from 'vitest'
import { mount, flushPromises } from '@vue/test-utils'
import EdicionesDePista from '../EdicionesDePista.vue'

vi.mock('../../api/editions', () => ({
  getEdicionesDePista: vi.fn(),
  resolverAparicion: vi.fn(),
}))

import { getEdicionesDePista, resolverAparicion } from '../../api/editions'

const listar = vi.mocked(getEdicionesDePista)
const resolver = vi.mocked(resolverAparicion)

const aparicion = (extra: Record<string, unknown> = {}) => ({
  item_id: 11,
  service: 'qobuz',
  service_track_id: '434547369',
  album: 'Greatest Hits',
  validation_status: 'needs_review',
  ...extra,
})

describe('EdicionesDePista', () => {
  beforeEach(() => {
    listar.mockReset()
    resolver.mockReset()
    resolver.mockResolvedValue(undefined)
  })

  it('esta plegada y resume cuantos albumes hay sin listar nada', async () => {
    listar.mockResolvedValue([
      aparicion(),
      aparicion({ item_id: 12, service: 'tidal', album: 'Greatest Hits' }),
      aparicion({ item_id: 13, service: 'spotify', album: 'The Singles Collection' }),
    ])
    const wrapper = mount(EdicionesDePista, { props: { trackId: 7 } })
    await flushPromises()

    expect(wrapper.find('[data-testid="ediciones-resumen"]').text()).toBe(
      'También aparece en 2 álbumes · 3 por revisar',
    )
    expect(wrapper.find('[data-testid="ediciones-lista"]').exists()).toBe(false)
  })

  it('al desplegar muestra album legible, servicio y estado', async () => {
    listar.mockResolvedValue([aparicion()])
    const wrapper = mount(EdicionesDePista, { props: { trackId: 7 } })
    await flushPromises()

    await wrapper.find('[data-testid="ediciones-toggle"]').trigger('click')

    const fila = wrapper.find('[data-testid="edicion"]')
    expect(fila.find('[data-testid="edicion-album"]').text()).toBe('Greatest Hits')
    expect(fila.text()).toContain('Qobuz')
    expect(fila.find('[data-testid="edicion-estado-needs_review"]').text()).toBe('Por revisar')
    // El id del proveedor no ensucia la fila: queda en el tooltip.
    expect(fila.text()).not.toContain('434547369')
  })

  it('una aparicion antigua sin album lo dice en vez de dejarlo vacio', async () => {
    listar.mockResolvedValue([aparicion({ album: null })])
    const wrapper = mount(EdicionesDePista, { props: { trackId: 7 } })
    await flushPromises()

    await wrapper.find('[data-testid="ediciones-toggle"]').trigger('click')

    expect(wrapper.find('[data-testid="edicion-album"]').text()).toBe(
      'Sin datos de álbum (aparición antigua)',
    )
  })

  it('una aparicion comprobada no se puede volver a confirmar', async () => {
    listar.mockResolvedValue([aparicion({ validation_status: 'verified' })])
    const wrapper = mount(EdicionesDePista, { props: { trackId: 7 } })
    await flushPromises()

    await wrapper.find('[data-testid="ediciones-toggle"]').trigger('click')

    expect(wrapper.find('[data-testid="edicion-estado-verified"]').text()).toBe('Comprobada')
    expect(wrapper.find('[data-testid="edicion-confirmar"]').exists()).toBe(false)
    expect(wrapper.find('[data-testid="edicion-descartar"]').exists()).toBe(true)
  })

  it('dice que no hay otras apariciones en vez de mostrar una lista vacia', async () => {
    listar.mockResolvedValue([])
    const wrapper = mount(EdicionesDePista, { props: { trackId: 7 } })
    await flushPromises()

    expect(wrapper.find('[data-testid="ediciones-resumen"]').text()).toBe('Sin otras apariciones')
  })

  it('confirmar conserva la aparicion y recarga', async () => {
    listar.mockResolvedValue([aparicion()])
    const wrapper = mount(EdicionesDePista, { props: { trackId: 7 } })
    await flushPromises()
    await wrapper.find('[data-testid="ediciones-toggle"]').trigger('click')

    await wrapper.find('[data-testid="edicion-confirmar"]').trigger('click')
    await flushPromises()

    expect(resolver).toHaveBeenCalledWith(11, 7, true)
    expect(listar).toHaveBeenCalledTimes(2)
  })

  it('descartar borra la aparicion y recarga', async () => {
    listar.mockResolvedValue([aparicion()])
    const wrapper = mount(EdicionesDePista, { props: { trackId: 7 } })
    await flushPromises()
    await wrapper.find('[data-testid="ediciones-toggle"]').trigger('click')

    await wrapper.find('[data-testid="edicion-descartar"]').trigger('click')
    await flushPromises()

    expect(resolver).toHaveBeenCalledWith(11, 7, false)
    expect(listar).toHaveBeenCalledTimes(2)
  })

  it('no consulta si no hay pista seleccionada', async () => {
    mount(EdicionesDePista, { props: { trackId: null } })
    await flushPromises()

    expect(listar).not.toHaveBeenCalled()
  })
})

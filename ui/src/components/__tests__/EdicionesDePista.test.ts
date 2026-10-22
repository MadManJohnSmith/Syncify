import { describe, it, expect, vi, beforeEach } from 'vitest'
import { mount, flushPromises } from '@vue/test-utils'
import EdicionesDePista from '../EdicionesDePista.vue'

vi.mock('../../api/editions', () => ({
  getEdicionesDePista: vi.fn(),
}))

import { getEdicionesDePista } from '../../api/editions'

const listar = vi.mocked(getEdicionesDePista)

describe('EdicionesDePista', () => {
  beforeEach(() => {
    listar.mockReset()
  })

  it('muestra el álbum de llegada y el servicio de cada aparición', async () => {
    listar.mockResolvedValue([
      {
        service: 'qobuz',
        service_track_id: '434547369',
        album: 'Lucky (Apple Original Series Soundtrack)',
        validation_status: 'needs_review',
      },
    ])
    const wrapper = mount(EdicionesDePista, { props: { trackId: 7 } })
    await flushPromises()

    expect(listar).toHaveBeenCalledWith(7)
    const filas = wrapper.findAll('[data-testid="edicion"]')
    expect(filas).toHaveLength(1)
    expect(filas[0].text()).toContain('Lucky (Apple Original Series Soundtrack)')
    expect(filas[0].text()).toContain('qobuz')
  })

  it('dice que no hay apariciones en vez de mostrar una lista vacía', async () => {
    listar.mockResolvedValue([])
    const wrapper = mount(EdicionesDePista, { props: { trackId: 7 } })
    await flushPromises()

    expect(wrapper.find('[data-testid="ediciones-vacio"]').exists()).toBe(true)
  })

  it('no consulta si no hay pista seleccionada', async () => {
    mount(EdicionesDePista, { props: { trackId: null } })
    await flushPromises()

    expect(listar).not.toHaveBeenCalled()
  })
})

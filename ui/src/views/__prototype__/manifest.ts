import type { ProtoVariant } from './types'
import { VARIANTS as accounts } from './accountsVariants'

/**
 * Índice de los prototipos presentes en esta copia. Lee las variantes del tab
 * en vez de repetirlas: si el tab añade una, aparece aquí sola.
 *
 * Solo el tab de accounts viaja en dev; el resto de prototipos vive sin
 * commitear en el árbol de trabajo.
 */
export interface ProtoTab {
  id: string
  label: string
  blurb: string
  route: string
  variants: readonly ProtoVariant[]
}

export const PROTO_TABS: ProtoTab[] = [
  { id: 'accounts', label: 'Accounts', blurb: 'Conexiones por servicio (sin auditar)', route: '/__prototype/accounts', variants: accounts },
]

export const TOTAL_VARIANTS = PROTO_TABS.reduce((sum, tab) => sum + tab.variants.length, 0)

import type { ProtoVariant } from './types'

/** Variantes de este tab. Vive aquí para que el índice general las lea sin duplicarlas. */
export const VARIANTS: ProtoVariant[] = [
  { id: 'actual', label: 'Hoy', hint: 'La pantalla actual, como referencia' },
  { id: 'cobertura', label: 'A · Cobertura', hint: 'Tarjetas con cobertura por cuenta y acciones alineadas' },
  { id: 'libro', label: 'B · Libro mayor', hint: 'Una tabla: cada cuenta es una fila auditable' },
  { id: 'salud', label: 'C · Salud', hint: 'Primero lo que requiere acción, con reglas visibles' },
  { id: 'detalle', label: 'D · Detalle', hint: 'Lista de conexiones + panel de una sola conexión' },
]

/** PROTOTIPO — tipos compartidos por el anfitrión y los tabs. */
export interface ProtoVariant {
  id: string
  label: string
  hint: string
}

export interface ProtoNote {
  label: string
  value: string
  formula: string
  caveat: string
}

/** Segmento de la barra de estado compartida. */
export interface BarSegment {
  label: string
  value: number
  tone: string
  hint?: string
}

/** Línea de letra con marca de tiempo. */
export interface StageLine {
  atMs: number
  text: string
}

/** Fila de la tabla «actual → propuesto». */
export interface DiffRow {
  field: string
  current: string | null
  proposed: string | null
  source: string
  confidence: number
}

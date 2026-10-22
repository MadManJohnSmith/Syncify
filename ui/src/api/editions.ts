import { invokeCommand } from './tauri'

/**
 * Una aparición: el ítem del proveedor se vio y quedó ligado a esta pista,
 * sin mover su álbum canónico ni contar como fuente (ED-04b).
 */
export interface EdicionDePista {
  service: string
  service_track_id: string
  album: string | null
  validation_status: string
}

export async function getEdicionesDePista(trackId: number): Promise<EdicionDePista[]> {
  return invokeCommand<EdicionDePista[]>('ediciones_de_pista', { trackId })
}

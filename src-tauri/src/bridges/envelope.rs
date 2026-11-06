use serde::{Deserialize, Serialize};

/// Envelope común que todo puente Python escribe por stdout.
///
/// `data` y `error` son opcionales: un fallo de dominio llega con
/// `success = false` y `error` relleno, igual que antes del contrato único.
/// El shape serializado no cambia, así que el contrato IPC con la UI se
/// mantiene byte a byte.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BridgeEnvelope<T> {
    pub success: bool,
    pub data: Option<T>,
    pub error: Option<String>,
}

//! Runtime de la máquina de estados de un job de descarga (ADR-0022).
//!
//! La política pura vive en `syncify_core_domain::job_state`; aquí está el
//! pegamento con el proceso: el registro de tokens de cancelación para que
//! `cancel_queue_item` aborte una transferencia en vuelo y la recuperación
//! única de filas interrumpidas.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use tokio_util::sync::CancellationToken;

use crate::db::DbPool;

/// Q4: espera añadida por puesto al reencolar jobs interrumpidos. El primer
/// puesto (rank 0) queda reclamable al momento; cada siguiente espera 2 s más.
const RECOVERY_STAGGER_STEP_SEC: i64 = 2;
/// Techo del escalonado individual: a partir de aquí los puestos restantes
/// comparten espera y el goteo corre a ritmo de reclamación del worker.
const RECOVERY_STAGGER_MAX_SEC: i64 = 60;

/// Tokens de cancelación de los jobs que el worker tiene reclamados.
///
/// Una entrada existe solo mientras el job está en vuelo: el worker la crea al
/// reclamarlo y la borra al terminar (incluido el error) mediante
/// [`CancellationGuard`]. Un `cancel` sin token devuelve `false` porque no hay
/// transferencia que abortar.
#[derive(Clone, Default)]
pub struct CancellationRegistry {
    tokens: Arc<Mutex<HashMap<i64, CancellationToken>>>,
}

impl CancellationRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, HashMap<i64, CancellationToken>> {
        // Un pánico con el lock tomado no debe envenenar la cancelación: el mapa
        // no tiene invariantes que un pánico pueda romper a medias.
        self.tokens
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    /// Registra (o reemplaza) el token de un job reclamado.
    pub fn register(&self, job_id: i64) -> CancellationToken {
        let token = CancellationToken::new();
        self.lock().insert(job_id, token.clone());
        token
    }

    /// Cancela el job si está en vuelo. `true` si había una transferencia que abortar.
    pub fn cancel(&self, job_id: i64) -> bool {
        match self.lock().get(&job_id) {
            Some(token) => {
                token.cancel();
                true
            }
            None => false,
        }
    }

    /// Olvida el token de un job terminado.
    pub fn release(&self, job_id: i64) {
        self.lock().remove(&job_id);
    }
}

/// Libera el token aunque el procesamiento termine por error o por pánico.
pub struct CancellationGuard {
    registry: CancellationRegistry,
    job_id: i64,
}

impl CancellationGuard {
    pub fn new(registry: CancellationRegistry, job_id: i64) -> Self {
        Self { registry, job_id }
    }
}

impl Drop for CancellationGuard {
    fn drop(&mut self) {
        self.registry.release(self.job_id);
    }
}

/// Recuperación de arranque: toda fila `downloading` vuelve `queued`.
///
/// Es la única transición `downloading → queued` del árbol (ADR-0022). No toca
/// `failed` ni `cancelled`: un fallo programado conserva su `next_attempt_at` y
/// una cancelación del usuario sigue cancelada.
///
/// Q4: la fila reencolada no queda reclamable de inmediato sino escalonada
/// (`next_attempt_at` creciente por puesto). Tras un cierre o crash durante una
/// tormenta, reencolar sin espera lanza la cola entera contra el proveedor en
/// el primer segundo; el escalonado convierte esa andanada en un goteo y deja
/// que el jitter y el breaker por proveedor se ocupen de los reintentos.
pub async fn recover_interrupted_jobs(pool: &DbPool) -> u64 {
    let ids: Vec<i64> = match sqlx::query_scalar(
        "SELECT id FROM download_queue WHERE status = 'downloading' ORDER BY id",
    )
    .fetch_all(pool)
    .await
    {
        Ok(ids) => ids,
        Err(error) => {
            tracing::warn!(%error, "No se pudieron listar las descargas interrumpidas");
            return bulk_recover_interrupted_jobs(pool).await;
        }
    };
    if ids.is_empty() {
        return 0;
    }

    let mut tx = match pool.begin().await {
        Ok(tx) => tx,
        Err(error) => {
            tracing::warn!(%error, "No se pudo abrir la transacción de recuperación");
            return bulk_recover_interrupted_jobs(pool).await;
        }
    };

    let mut recovered = 0u64;
    for (rank, id) in ids.iter().enumerate() {
        let delay = ((rank as i64) * RECOVERY_STAGGER_STEP_SEC).min(RECOVERY_STAGGER_MAX_SEC);
        match sqlx::query(
            "UPDATE download_queue SET status = 'queued', started_at = NULL, next_attempt_at = datetime('now', ?) WHERE id = ? AND status = 'downloading'",
        )
        .bind(format!("+{delay} seconds"))
        .bind(id)
        .execute(&mut *tx)
        .await
        {
            Ok(result) => recovered += result.rows_affected(),
            Err(error) => {
                tracing::warn!(%error, "No se pudieron reencolar las descargas interrumpidas");
                // El drop del rollback devuelve las filas a `downloading`; el
                // respaldo bulk rehace la transición sin escalonado.
                return bulk_recover_interrupted_jobs(pool).await;
            }
        }
    }

    match tx.commit().await {
        Ok(()) => recovered,
        Err(error) => {
            tracing::warn!(%error, "No se pudo confirmar la recuperación escalonada; fallback sin escalonado");
            bulk_recover_interrupted_jobs(pool).await
        }
    }
}

/// Recuperación original, una sola UPDATE sin escalonado: el respaldo cuando el
/// camino escalonado no puede completarse. Deja las filas reclamables de
/// inmediato, el comportamiento que la puerta S222 caracterizó.
async fn bulk_recover_interrupted_jobs(pool: &DbPool) -> u64 {
    match sqlx::query(
        "UPDATE download_queue SET status = 'queued', started_at = NULL WHERE status = 'downloading'",
    )
    .execute(pool)
    .await
    {
        Ok(result) => result.rows_affected(),
        Err(error) => {
            tracing::warn!(%error, "No se pudieron reencolar las descargas interrumpidas");
            0
        }
    }
}

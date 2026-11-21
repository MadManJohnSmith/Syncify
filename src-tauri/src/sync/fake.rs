//! Proveedor falso en proceso (seam 1 del spec #64): permite probar el job de
//! `SyncRun` sin red. Molde del proveedor simulado del PR #298, pero sin
//! HTTP: las páginas viven en memoria y los fallos se programan por adelantado.
//!
//! Los ítems son JSON con la forma que el importador del servicio espera; en
//! los tests de este ticket, la forma de track de Qobuz del fixture de
//! caracterización. Así el job corre el importador real de punta a punta.

use super::source::{FavoritesPage, LibrarySource, SourceError};
use std::sync::Mutex;

/// Página servida por el proveedor falso; el fallo programado se consume una
/// sola vez (un `Retryable` reintentado deja de fallar a la segunda).
#[allow(dead_code)] // Cubierta por `tests/sync_run_job_fake_source_test.rs` y `tests/sync_run_qobuz_parity_test.rs` (seam 1 del spec: pruebas del job sin red).
struct FakeState {
    pages_served: usize,
    /// Fallo programado por índice de petición absoluta: `(n, error)` falla la
    /// petición número `n` (contando desde 0, reintentos incluidos) una vez.
    failures: Vec<(usize, SourceError)>,
    offsets_seen: Vec<i64>,
}

#[allow(dead_code)] // Cubierta por `tests/sync_run_job_fake_source_test.rs` y `tests/sync_run_qobuz_parity_test.rs` (seam 1 del spec: pruebas del job sin red).
pub struct FakeLibrarySource {
    service: String,
    pages: Vec<FavoritesPage>,
    reported_total: Option<i64>,
    state: Mutex<FakeState>,
    /// Gancho invocado tras servir la página `n`: el test simula la
    /// cancelación externa (decisión de la regla derivada: detener en el
    /// límite de página) sin acoplar el proveedor al job.
    on_page_served: Option<std::sync::Arc<dyn Fn(usize) + Send + Sync>>,
}

#[allow(dead_code)] // Cubierta por `tests/sync_run_job_fake_source_test.rs` y `tests/sync_run_qobuz_parity_test.rs`.
impl FakeLibrarySource {
    /// Proveedor falso con las páginas dadas en orden; el total reportado por
    /// defecto es la suma de ítems (el contrato exige asignarlo una vez, no
    /// acumularlo por página).
    pub fn with_pages(pages: Vec<Vec<serde_json::Value>>) -> Self {
        let total = pages.iter().map(|p| p.len() as i64).sum();
        Self {
            service: "fake".to_string(),
            pages: pages
                .into_iter()
                .map(|items| FavoritesPage {
                    items,
                    reported_total: Some(total),
                })
                .collect(),
            reported_total: Some(total),
            state: Mutex::new(FakeState {
                pages_served: 0,
                failures: Vec::new(),
                offsets_seen: Vec::new(),
            }),
            on_page_served: None,
        }
    }

    pub fn with_service_name(mut self, service: impl Into<String>) -> Self {
        self.service = service.into();
        self
    }

    /// Sobrescribe el total reportado (p. ej. `None` para probar la cobertura
    /// «desconocida», o un valor que no cuadra con los ítems).
    pub fn with_reported_total(mut self, total: Option<i64>) -> Self {
        self.reported_total = total;
        for page in &mut self.pages {
            page.reported_total = total;
        }
        self
    }

    /// Programa un fallo para la primera petición (se consume una vez).
    /// Con `SourceErrorKind::Retryable` el reintento del job lo supera.
    pub fn with_failure(self, error: SourceError) -> Self {
        self.with_failure_at(0, error)
    }

    /// Programa un fallo para la petición número `fetch_index` (0 = primera;
    /// un reintento cuenta como petición nueva).
    pub fn with_failure_at(self, fetch_index: usize, error: SourceError) -> Self {
        self.state
            .lock()
            .expect("fake state")
            .failures
            .push((fetch_index, error));
        self
    }

    pub fn on_page_served(mut self, hook: impl Fn(usize) + Send + Sync + 'static) -> Self {
        self.on_page_served = Some(std::sync::Arc::new(hook));
        self
    }

    /// Los offsets que el job pidió: la forma del recorrido queda fijada.
    pub fn offsets_seen(&self) -> Vec<i64> {
        self.state.lock().expect("fake state").offsets_seen.clone()
    }
}

impl LibrarySource for FakeLibrarySource {
    fn service_name(&self) -> &str {
        &self.service
    }

    async fn fetch_favorites_page(&self, offset: i64) -> Result<FavoritesPage, SourceError> {
        let mut state = self.state.lock().expect("fake state");
        let fetch_index = state.offsets_seen.len();
        state.offsets_seen.push(offset);
        let scheduled = state.failures.iter().position(|(at, _)| *at == fetch_index);
        if let Some(pos) = scheduled {
            let (_, error) = state.failures.remove(pos);
            drop(state);
            return Err(error);
        }
        let index = state.pages_served;
        let page = self.pages.get(index).cloned();
        state.pages_served += 1;
        drop(state);

        match page {
            Some(mut page) => {
                page.reported_total = self.reported_total;
                if let Some(hook) = &self.on_page_served {
                    hook(index);
                }
                Ok(page)
            }
            None => Ok(FavoritesPage {
                items: Vec::new(),
                reported_total: self.reported_total,
            }),
        }
    }
}

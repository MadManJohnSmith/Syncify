//! Infraestructura inyectada por el núcleo (`src-tauri`): limitador global de
//! peticiones y token de Apple Music compartido con la caché de sesión. El
//! crate no depende de Tauri ni de la BD: la fachada conecta sus hooks aquí.

use futures::future::BoxFuture;
use reqwest::Client;

/// Puntos de enganche de infraestructura del motor de letras.
pub trait EngineHooks: Send + Sync {
    /// Espera el turno del limitador global antes de golpear un servicio
    /// (en el núcleo: `LRCLIB_LIMITER.wait("lrclib")`).
    fn wait_rate_limit<'a>(&'a self, service: &'a str) -> BoxFuture<'a, ()>;

    /// Token web de Apple Music desde la caché de sesión de la app; `None` si
    /// no está disponible.
    fn apple_music_token<'a>(&'a self, client: &'a Client) -> BoxFuture<'a, Option<String>>;
}

/// Hooks sin efecto para pruebas offline; no contacta red.
#[derive(Debug, Default, Clone, Copy)]
pub struct NoopHooks;

impl EngineHooks for NoopHooks {
    fn wait_rate_limit<'a>(&'a self, _service: &'a str) -> BoxFuture<'a, ()> {
        Box::pin(async move {})
    }

    fn apple_music_token<'a>(&'a self, _client: &'a Client) -> BoxFuture<'a, Option<String>> {
        Box::pin(async move { None })
    }
}

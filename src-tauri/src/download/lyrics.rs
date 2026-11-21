// Fachada del motor de letras (ADR-0017): delega en `syncify-lyrics-engine`
// y conecta la infraestructura del núcleo que el crate no ve — cliente HTTP
// central, limitador global de peticiones, token de Apple Music (caché de
// sesión compartida con la portada animada) y configuración persistida en BD.

use futures::future::BoxFuture;
use syncify_lyrics_engine::EngineHooks;

// La superficie pública del motor viaja intacta para los consumidores del núcleo.
#[allow(unused_imports)]
pub use syncify_lyrics_engine::{
    clear_lyrics_cache, generate_sidecar_lrc, is_valid_lyrics, set_cached_lyrics,
    validate_and_embed_flac_lyrics, LRCLibResponse, LyricsClient, LyricsEngineConfig, LyricsLine,
    LyricsPipelineService, LyricsResponse, LyricsSourceSetting, LYRICS_SOURCE_IDS,
};

// El contrato de dominio sigue expuesto por la misma ruta que antes de la extracción.
#[allow(unused_imports)]
pub use syncify_lyrics_domain::{
    calculate_confidence_score, deduplicate_lines, detect_language_heuristic, detect_sync_type,
    ms_to_lrc_timestamp, parse_lrc_line, parse_time_str_to_ms, parse_ttml_to_elrc,
    parse_ultrastar_to_elrc, preserve_word_timestamps_exact, simplify_track_name,
    strip_lrc_timestamps, validate_lyrics_timestamps, LyricsLineDomain, LyricsResolution,
    LyricsSyncType, LyricsTagContract, ResolutionStatus,
};

/// Conecta el motor con la infraestructura del núcleo: el limitador global de
/// peticiones y el token de Apple Music compartido con la caché de sesión.
struct CoreHooks;

impl EngineHooks for CoreHooks {
    fn wait_rate_limit<'a>(&'a self, service: &'a str) -> BoxFuture<'a, ()> {
        Box::pin(async move {
            crate::download::http_client::LRCLIB_LIMITER
                .wait(service)
                .await
        })
    }

    fn apple_music_token<'a>(
        &'a self,
        client: &'a reqwest::Client,
    ) -> BoxFuture<'a, Option<String>> {
        Box::pin(
            async move { crate::services::animated_cover::extract_apple_music_token(client).await },
        )
    }
}

/// Construye el motor con el cliente HTTP central y los hooks del núcleo.
pub fn new_lyrics_client() -> LyricsClient {
    LyricsClient::with_parts(
        crate::download::http_client::create_http_client(),
        std::sync::Arc::new(CoreHooks),
    )
}

/// Motor con la configuración persistida (comandos y búsqueda interactiva).
pub fn lyrics_client_with_config(config: LyricsEngineConfig) -> LyricsClient {
    LyricsClient::with_parts_and_config(
        crate::download::http_client::create_http_client(),
        std::sync::Arc::new(CoreHooks),
        config,
    )
}

/// Servicio de integración para pipelines de descarga con configuración por defecto.
pub fn pipeline_default() -> LyricsPipelineService {
    LyricsPipelineService::with_client(new_lyrics_client())
}

/// Download callers must use this constructor to consume the persisted source policy.
pub async fn pipeline_from_db(db: &crate::DbPool) -> LyricsPipelineService {
    let config = crate::commands::lyrics::load_lyrics_engine_config(db).await;
    LyricsPipelineService::with_client(lyrics_client_with_config(config))
}

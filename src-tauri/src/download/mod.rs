// Download service module for Syncify
// Implements credential-free downloads from Qobuz, Tidal, Amazon Music

pub mod amazon;
pub mod audio_inspector;
pub mod http_client;
pub mod identity;
pub mod job;
pub mod jobs;
pub mod lyrics;
pub mod orchestrator;
pub mod progress;
pub mod providers;
pub mod quality_reconcile;
// Provider gateway (D01/D02): el orquestador despacha por este registro toda
// descarga de proveedor; la provenance del sitio viaja en `ProviderRouting`.
pub mod provider_gateway;
pub mod qobuz;
// ED discovery/selection is an offline-tested policy API not yet wired into
// the desktop binary; the binary compiles these modules separately from lib.rs.
// Keep it compiled (and tested) without treating this staged API as production use.
#[allow(dead_code)]
pub mod resolver;
pub mod songlink;
pub mod tidal;

#[allow(unused_imports)]
pub use audio_inspector::{
    classify_physical_audio_quality, enforce_post_download_quality_gate,
    inspect_physical_audio_file, populate_flac_streaminfo_md5, verify_flac_stream_integrity,
    PhysicalAudioMetadata,
};

#[allow(unused_imports)]
pub use job::{recover_interrupted_jobs, CancellationGuard, CancellationRegistry};
#[allow(unused_imports)]
pub use lyrics::LyricsClient;
#[allow(unused_imports)]
pub use orchestrator::{DownloadOrchestrator, SongLinkEngineTarget};
pub use progress::*;
#[allow(unused_imports)]
pub use provider_gateway::{
    AmazonProvider, DownloadProvider, ProviderCapabilities, ProviderDelivery, ProviderError,
    ProviderRequest, ProviderRouting, QobuzProvider, TidalProvider,
};
pub use resolver::{
    classify_attempt_failure, exclusions_for_failure, expand_attempt_sequence,
    resolve_download_plan, resolve_locked_track_source_id, DownloadIntent, DownloadPlan,
    ReResolutionTrigger, ResolutionExclusions,
};
#[allow(unused_imports)]
pub use resolver::{
    AttemptStep, CandidateAccount, CandidateIdentity, PlanCandidate, ResolutionError,
    ResolutionLog, ResolvedQuality, MAX_PLAN_CANDIDATES,
};
#[allow(unused_imports)]
pub use songlink::{SongLinkAvailability, SongLinkClient, TrackAvailability};

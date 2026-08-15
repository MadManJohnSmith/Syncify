// Download service module for Syncify
// Implements credential-free downloads from Qobuz, Tidal, Amazon Music

pub mod amazon;
pub mod http_client;
pub mod lyrics;
pub mod orchestrator;
pub mod progress;
pub mod qobuz;
pub mod songlink;
pub mod tidal;

pub use lyrics::LyricsClient;
pub use orchestrator::DownloadOrchestrator;
pub use progress::*;

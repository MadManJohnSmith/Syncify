// Download service module for Syncify
// Implements credential-free downloads from Qobuz, Tidal, Amazon Music

mod amazon;
mod http_client;
pub mod lyrics;
mod orchestrator;
mod progress;
mod qobuz;
pub mod songlink;
mod tidal;

pub use lyrics::LyricsClient;
pub use orchestrator::DownloadOrchestrator;
pub use progress::*;

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

#[allow(unused_imports)]
pub use lyrics::LyricsClient;
#[allow(unused_imports)]
pub use orchestrator::{DownloadOrchestrator, SongLinkEngineTarget};
#[allow(unused_imports)]
pub use songlink::{SongLinkAvailability, SongLinkClient, TrackAvailability};
pub use progress::*;

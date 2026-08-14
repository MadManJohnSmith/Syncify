pub mod animated_cover;
pub mod artist_info;
pub mod bandcamp;
pub mod favorites;
pub mod goodies;
pub mod http_client;
pub mod layout;
pub mod lyrics;
pub mod playlist_resolver;
pub mod qobuz_downloader;
pub mod rescue;
pub mod songlink;
pub mod soulseek;
pub mod staging;
pub mod tidal;

pub use animated_cover::{download_animated_cover, resolve_and_download_animated_cover, validate_animated_webp_bytes, AnimatedCoverStatus};
pub use artist_info::{download_artist_info, download_artist_info_with_url};
pub use favorites::{FavoriteItem, FavoritesBatchSummary, QobuzFavoritesClient, TrackManifestEntry};
pub use goodies::download_goodies_booklet;
pub use layout::LibraryLayout;
pub use lyrics::{LyricsClient, LyricsLine, LyricsResponse};
pub use playlist_resolver::PlaylistResolver;
pub use qobuz_downloader::{
    build_flac_metadata, build_output_path, build_request_signature, map_quality_to_allowed_format_ids,
    map_quality_to_allowed_format_ids_with_lossy_fallback, map_quality_to_format_id,
    sanitize_path_component, sign_api_request, DownloadRequest, DownloadResult,
    QobuzAuthStatus, QobuzDownloader, QobuzTrack, StreamResolution, StreamUrlSource,
};
pub use rescue::{fetch_expected_release_tracklist, rescue_missing_track, MissingTrackInfo};
pub use tidal::{
    StreamSourceType, TidalAuthResolution, TidalAuthStatus, TidalDownloader, TidalGuiCredentials,
    TidalGuiSessionExt, TidalStreamResolution, TidalTrack,
};




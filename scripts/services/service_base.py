"""
Abstract base class for music service integrations.
All service implementations (Qobuz, Tidal, Deezer, etc.) must inherit from this.
"""

from abc import ABC, abstractmethod
from dataclasses import dataclass
from typing import List, Optional, Dict, Any, Callable, TYPE_CHECKING
from enum import Enum

if TYPE_CHECKING:
    from core_logic.migration_engine import AudioQualityConfig, MetadataTagConfig


class ServiceType(Enum):
    """Supported music services."""
    SPOTIFY = "spotify"
    QOBUZ = "qobuz"
    TIDAL = "tidal"
    DEEZER = "deezer"
    SOUNDCLOUD = "soundcloud"


class DownloadQuality(Enum):
    """Quality levels across services (normalized)."""
    LOSSY_LOW = "lossy_low"              # ~128kbps
    LOSSY_STANDARD = "lossy_standard"    # ~320kbps
    LOSSLESS_CD = "lossless_cd"          # 16bit/44.1kHz (FLAC)
    LOSSLESS_HIRES_96 = "lossless_hires_96"  # 24bit/96kHz (FLAC) - Qobuz format_id=7
    LOSSLESS_HIRES = "lossless_hires"    # 24bit/192kHz (FLAC) - Qobuz format_id=27


class DownloadStatus(Enum):
    """Download status tracking."""
    QUEUED = "queued"
    DOWNLOADING = "downloading"
    COMPLETED = "completed"
    FAILED = "failed"
    PAUSED = "paused"
    CANCELLED = "cancelled"


@dataclass
class ServiceCredentials:
    """Authentication credentials for a music service."""
    service_type: ServiceType
    username: Optional[str] = None
    password: Optional[str] = None
    token: Optional[str] = None
    refresh_token: Optional[str] = None
    client_id: Optional[str] = None
    client_secret: Optional[str] = None
    extra: Optional[Dict[str, Any]] = None


@dataclass
class TrackMetadata:
    """Unified track metadata across services."""
    # Core identifiers
    service_id: str                  # Service-specific track ID
    service_type: ServiceType
    
    # Basic info
    title: str
    artists: List[str]
    album: str
    album_artist: Optional[str] = None
    
    # Track details
    track_number: Optional[int] = None
    disc_number: Optional[int] = None
    duration_ms: Optional[int] = None
    
    # Release info
    release_date: Optional[str] = None
    year: Optional[int] = None
    label: Optional[str] = None
    
    # Classification
    genres: Optional[List[str]] = None
    release_type: Optional[str] = None  # "album", "single", "compilation", "live", etc.
    
    # Quality
    quality: Optional[DownloadQuality] = None
    sample_rate: Optional[int] = None
    bit_depth: Optional[int] = None
    
    # Media
    artwork_url: Optional[str] = None
    lyrics: Optional[str] = None
    
    # Identifiers
    isrc: Optional[str] = None
    upc: Optional[str] = None
    
    # Custom tags
    custom_tags: Optional[Dict[str, str]] = None
    
    def __post_init__(self):
        """Initialize empty lists if None."""
        if self.genres is None:
            self.genres = []
        if self.custom_tags is None:
            self.custom_tags = {}

    @property
    def id(self) -> str:
        return self.service_id

    @property
    def artist(self) -> str:
        return self.artists[0] if self.artists else ""

    @property
    def duration(self) -> int:
        return (self.duration_ms or 0) // 1000


@dataclass
class AlbumMetadata:
    """Unified album metadata."""
    service_id: str
    service_type: ServiceType
    title: str
    artist: str
    artists: List[str]
    release_date: Optional[str] = None
    year: Optional[int] = None
    label: Optional[str] = None
    genres: Optional[List[str]] = None
    track_count: Optional[int] = None
    artwork_url: Optional[str] = None
    upc: Optional[str] = None


@dataclass
class PlaylistMetadata:
    """Unified playlist metadata."""
    service_id: str
    service_type: ServiceType
    name: str
    description: Optional[str] = None
    owner: Optional[str] = None
    track_count: Optional[int] = None
    is_public: bool = False
    artwork_url: Optional[str] = None


@dataclass
class SearchResult:
    """Unified search result."""
    result_type: str  # "track", "album", "artist", "playlist"
    service_id: str
    service_type: ServiceType
    title: str
    artist: Optional[str] = None
    album: Optional[str] = None
    year: Optional[int] = None
    duration_ms: Optional[int] = None
    artwork_url: Optional[str] = None
    quality: Optional[DownloadQuality] = None

    @property
    def id(self) -> str:
        return self.service_id

    @property
    def duration(self) -> int:
        return (self.duration_ms or 0) // 1000


@dataclass
class DownloadResult:
    """Result of a download operation."""
    success: bool
    track_metadata: Optional[TrackMetadata] = None
    filepath: Optional[str] = None
    file_size_bytes: Optional[int] = None
    download_duration_seconds: Optional[float] = None
    status: DownloadStatus = DownloadStatus.COMPLETED
    error_message: Optional[str] = None


class MusicService(ABC):
    """
    Abstract base class for music service integrations.
    
    Each service implementation must provide:
    1. Authentication
    2. Search functionality
    3. Download capabilities
    4. Metadata retrieval
    """
    
    def __init__(self, credentials: ServiceCredentials, verbose: bool = False):
        """
        Initialize the service.
        
        Args:
            credentials: Service authentication credentials
            verbose: Enable verbose logging
        """
        self.credentials = credentials
        self.verbose = verbose
        self._authenticated = False
    
    @abstractmethod
    async def authenticate(self) -> bool:
        """
        Authenticate with the service.
        
        Returns:
            True if authentication successful, False otherwise
        """
        pass
    
    @abstractmethod
    async def is_authenticated(self) -> bool:
        """
        Check if currently authenticated.
        
        Returns:
            True if authenticated and token valid
        """
        pass
    
    @abstractmethod
    async def search(self, query: str, result_type: str = "track", limit: int = 10) -> List[SearchResult]:
        """
        Search the service catalog.
        
        Args:
            query: Search query string
            result_type: Type of results ("track", "album", "artist", "playlist")
            limit: Maximum number of results
            
        Returns:
            List of search results
        """
        pass
    
    @abstractmethod
    async def get_track_metadata(self, track_id: str) -> Optional[TrackMetadata]:
        """
        Retrieve detailed metadata for a track.
        
        Args:
            track_id: Service-specific track identifier
            
        Returns:
            TrackMetadata object or None if not found
        """
        pass
    
    @abstractmethod
    async def get_album_metadata(self, album_id: str) -> Optional[AlbumMetadata]:
        """
        Retrieve album metadata.
        
        Args:
            album_id: Service-specific album identifier
            
        Returns:
            AlbumMetadata object or None if not found
        """
        pass
    
    @abstractmethod
    async def get_album_tracks(self, album_id: str) -> List[TrackMetadata]:
        """
        Get all tracks in an album.
        
        Args:
            album_id: Service-specific album identifier
            
        Returns:
            List of TrackMetadata objects
        """
        pass
    
    @abstractmethod
    async def get_playlist_metadata(self, playlist_id: str) -> Optional[PlaylistMetadata]:
        """
        Retrieve playlist metadata.
        
        Args:
            playlist_id: Service-specific playlist identifier
            
        Returns:
            PlaylistMetadata object or None if not found
        """
        pass
    
    @abstractmethod
    async def get_playlist_tracks(self, playlist_id: str) -> List[TrackMetadata]:
        """
        Get all tracks in a playlist.
        
        Args:
            playlist_id: Service-specific playlist identifier
            
        Returns:
            List of TrackMetadata objects
        """
        pass
    
    @abstractmethod
    async def download_track(
        self, 
        track_id: str, 
        output_path: str,
        quality: DownloadQuality = DownloadQuality.LOSSLESS_CD,
        audio_config: Optional['AudioQualityConfig'] = None,
        metadata_config: Optional['MetadataTagConfig'] = None,
        progress_callback: Optional[Callable[[int, int], None]] = None
    ) -> DownloadResult:
        """
        Download a track to disk.
        
        Args:
            track_id: Service-specific track identifier
            output_path: Destination file path
            quality: Desired download quality
            audio_config: Optional audio quality configuration
            metadata_config: Optional metadata tag configuration
            progress_callback: Optional callback(bytes_downloaded, total_bytes)
            
        Returns:
            DownloadResult with status and metadata
        """
        pass
    
    @abstractmethod
    async def get_available_qualities(self, track_id: str) -> List[DownloadQuality]:
        """
        Get available quality options for a track.
        
        Args:
            track_id: Service-specific track identifier
            
        Returns:
            List of available quality levels
        """
        pass
    
    @property
    @abstractmethod
    def service_name(self) -> str:
        """Human-readable service name."""
        pass
    
    @property
    @abstractmethod
    def service_type(self) -> ServiceType:
        """Service type enum."""
        pass
    
    @property
    @abstractmethod
    def supports_lossless(self) -> bool:
        """Whether service supports lossless audio."""
        pass
    
    def _log(self, message: str, level: str = "info"):
        """Internal logging helper."""
        if self.verbose or level == "error":
            prefix = f"[{self.service_name}]"
            print(f"{prefix} {message}")

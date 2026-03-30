"""
Settings Manager for Syncify
Handles loading and applying user preferences for audio quality and metadata tags.
"""

import json
from pathlib import Path
from typing import Dict, Any, Optional


class SettingsManager:
    """Manages application settings for download quality and metadata preferences."""
    
    def __init__(self, config_path: Optional[str] = None):
        """
        Initialize settings manager.
        
        Args:
            config_path: Path to config.json file. If None, uses default location.
        """
        if config_path is None:
            # Default to config.json in project root
            self.config_path = Path(__file__).parent.parent / "config.json"
        else:
            self.config_path = Path(config_path)
        
        self.settings = self._load_settings()
    
    def _load_settings(self) -> Dict[str, Any]:
        """Load settings from config.json file."""
        try:
            with open(self.config_path, 'r', encoding='utf-8') as f:
                config = json.load(f)
                return config.get('migration_settings', {})
        except Exception as e:
            print(f"Warning: Could not load settings from {self.config_path}: {e}")
            return self._get_default_settings()
    
    def _get_default_settings(self) -> Dict[str, Any]:
        """Return default settings if config file is not available."""
        return {
            "audio_quality": {
                "format": "flac",
                "sample_rate": 96000,
                "bit_depth": 24,
                "bitrate": 320,
                "album_art_resolution": "1400"
            },
            "metadata_tags": {
                "core": {
                    "title": True,
                    "artist": True,
                    "album": True,
                    "album_artist": True,
                    "track_number": True,
                    "disc_number": True,
                    "date": True,
                    "genre": True
                },
                "extended": {
                    "musicbrainz_ids": True,
                    "isrc": True,
                    "upc": True,
                    "label": True,
                    "composer": True,
                    "producer": False,
                    "compilation": True,
                    "mediatype": True,
                    "albumversion": False,
                    "originaldate": False
                },
                "enrichment": {
                    "bpm": True,
                    "mood": True,
                    "occasion": True,
                    "style": True,
                    "language": True,
                    "country": True
                },
                "classical": {
                    "work": False,
                    "movement": False,
                    "movementnumber": False
                },
                "credits": {
                    "personnel": True,
                    "copyright": False
                },
                "artwork": {
                    "embed_album_art": True
                }
            }
        }
    
    def save_settings(self, settings: Dict[str, Any]) -> bool:
        """
        Save settings to config.json file.
        
        Args:
            settings: Settings dictionary to save
            
        Returns:
            True if successful, False otherwise
        """
        try:
            # Load existing config
            with open(self.config_path, 'r', encoding='utf-8') as f:
                config = json.load(f)
            
            # Update migration_settings
            if 'migration_settings' not in config:
                config['migration_settings'] = {}
            
            config['migration_settings'].update(settings)
            
            # Save back to file
            with open(self.config_path, 'w', encoding='utf-8') as f:
                json.dump(config, f, indent=2, ensure_ascii=False)
            
            self.settings = config['migration_settings']
            return True
        except Exception as e:
            print(f"Error saving settings: {e}")
            return False
    
    # Audio Quality Getters
    
    def get_audio_format(self) -> str:
        """Get preferred audio format (flac, mp3, etc.)."""
        return self.settings.get('audio_quality', {}).get('format', 'flac')
    
    def get_sample_rate(self) -> int:
        """Get preferred sample rate in Hz."""
        return self.settings.get('audio_quality', {}).get('sample_rate', 96000)
    
    def get_bit_depth(self) -> int:
        """Get preferred bit depth."""
        return self.settings.get('audio_quality', {}).get('bit_depth', 24)
    
    def get_bitrate(self) -> int:
        """Get preferred bitrate for lossy formats."""
        return self.settings.get('audio_quality', {}).get('bitrate', 320)
    
    def get_album_art_resolution(self) -> str:
        """Get preferred album art resolution."""
        return self.settings.get('audio_quality', {}).get('album_art_resolution', '1400')
    
    # Metadata Tag Getters
    
    def should_include_tag(self, category: str, tag: str) -> bool:
        """
        Check if a specific metadata tag should be included.
        
        Args:
            category: Tag category (core, extended, enrichment, classical, credits, artwork)
            tag: Tag name
            
        Returns:
            True if tag should be included, False otherwise
        """
        metadata_tags = self.settings.get('metadata_tags', {})
        category_tags = metadata_tags.get(category, {})
        
        # Core tags are always included
        if category == 'core':
            return True
        
        return category_tags.get(tag, False)
    
    def get_enabled_tags(self) -> Dict[str, bool]:
        """
        Get flat dictionary of all enabled tags.
        
        Returns:
            Dictionary mapping tag names to enabled status
        """
        enabled = {}
        metadata_tags = self.settings.get('metadata_tags', {})
        
        for category, tags in metadata_tags.items():
            if isinstance(tags, dict):
                for tag, enabled_status in tags.items():
                    enabled[tag] = enabled_status
        
        return enabled
    
    # Specific Tag Checks
    
    def should_include_musicbrainz(self) -> bool:
        """Check if MusicBrainz IDs should be included."""
        return self.should_include_tag('extended', 'musicbrainz_ids')
    
    def should_include_bpm(self) -> bool:
        """Check if BPM should be detected and included."""
        return self.should_include_tag('enrichment', 'bpm')
    
    def should_include_mood(self) -> bool:
        """Check if mood tags should be included."""
        return self.should_include_tag('enrichment', 'mood')
    
    def should_include_occasion(self) -> bool:
        """Check if occasion tags should be included."""
        return self.should_include_tag('enrichment', 'occasion')
    
    def should_include_style(self) -> bool:
        """Check if style tags should be included."""
        return self.should_include_tag('enrichment', 'style')
    
    def should_include_personnel(self) -> bool:
        """Check if personnel/credits should be included in COMMENT."""
        return self.should_include_tag('credits', 'personnel')
    
    def should_include_classical(self) -> bool:
        """Check if any classical music tags are enabled."""
        classical = self.settings.get('metadata_tags', {}).get('classical', {})
        return any(classical.values())
    
    def should_embed_album_art(self) -> bool:
        """Check if album art should be embedded."""
        return self.should_include_tag('artwork', 'embed_album_art')
    
    # Utility Methods
    
    def get_quality_preset_name(self) -> str:
        """Get a human-readable name for current quality settings."""
        format_name = self.get_audio_format().upper()
        sample_rate = self.get_sample_rate() / 1000  # Convert to kHz
        bit_depth = self.get_bit_depth()
        
        if format_name == 'FLAC':
            return f"{format_name} {sample_rate:.1f}kHz/{bit_depth}bit"
        else:
            return f"{format_name} {self.get_bitrate()}kbps"
    
    def get_summary(self) -> Dict[str, Any]:
        """
        Get summary of current settings.
        
        Returns:
            Dictionary with settings summary
        """
        enabled_tags = self.get_enabled_tags()
        enabled_count = sum(1 for v in enabled_tags.values() if v)
        
        return {
            "audio_quality": {
                "format": self.get_audio_format(),
                "preset": self.get_quality_preset_name(),
                "sample_rate": self.get_sample_rate(),
                "bit_depth": self.get_bit_depth(),
                "album_art": self.get_album_art_resolution()
            },
            "metadata": {
                "total_tags": len(enabled_tags),
                "enabled_tags": enabled_count,
                "musicbrainz": self.should_include_musicbrainz(),
                "bpm_detection": self.should_include_bpm(),
                "enrichment": any([
                    self.should_include_mood(),
                    self.should_include_occasion(),
                    self.should_include_style()
                ]),
                "classical": self.should_include_classical()
            }
        }


# Global instance for easy access
_settings_manager = None

def get_settings_manager() -> SettingsManager:
    """Get global settings manager instance."""
    global _settings_manager
    if _settings_manager is None:
        _settings_manager = SettingsManager()
    return _settings_manager

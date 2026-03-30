"""
Audio format conversion utilities using FFmpeg.
"""

import asyncio
import subprocess
import logging
from pathlib import Path
from typing import Optional
from enum import Enum


class AudioFormat(Enum):
    """Supported audio formats."""
    FLAC = "flac"
    ALAC = "alac"
    WAV = "wav"
    MP3 = "mp3"
    AAC = "aac"
    OGG = "ogg"
    OPUS = "opus"


class AudioConverter:
    """Convert audio files using FFmpeg."""
    
    def __init__(self, logger: Optional[logging.Logger] = None):
        self.logger = logger or logging.getLogger(__name__)
        self._ffmpeg_available: Optional[bool] = None
    
    def is_ffmpeg_available(self) -> bool:
        """Check if FFmpeg is installed and accessible."""
        if self._ffmpeg_available is not None:
            return self._ffmpeg_available
        
        try:
            result = subprocess.run(
                ["ffmpeg", "-version"], 
                capture_output=True, 
                timeout=5
            )
            self._ffmpeg_available = result.returncode == 0
        except (FileNotFoundError, subprocess.TimeoutExpired):
            self._ffmpeg_available = False
        
        if not self._ffmpeg_available:
            self.logger.warning(
                "FFmpeg not found. Audio format conversion will not be available. "
                "Install FFmpeg to enable format conversion: https://ffmpeg.org/download.html"
            )
        
        return self._ffmpeg_available
    
    def get_output_extension(self, format: str) -> str:
        """Get the file extension for a given format."""
        extensions = {
            "flac": ".flac",
            "alac": ".m4a",
            "wav": ".wav",
            "mp3": ".mp3",
            "aac": ".m4a",
            "ogg": ".ogg",
            "opus": ".opus"
        }
        return extensions.get(format.lower(), ".flac")
    
    async def convert(
        self,
        input_path: Path,
        output_path: Optional[Path] = None,
        target_format: str = "flac",
        sample_rate: Optional[int] = None,
        bit_depth: Optional[int] = None,
        bitrate: int = 320
    ) -> Optional[Path]:
        """
        Convert audio file to target format.
        
        Args:
            input_path: Source audio file
            output_path: Destination path (auto-generated if None)
            target_format: Target format (flac, mp3, aac, ogg, wav, alac, opus)
            sample_rate: Target sample rate in Hz (None = keep original)
            bit_depth: Target bit depth (None = keep original, only for lossless)
            bitrate: Bitrate in kbps for lossy formats
            
        Returns:
            Path to converted file, or None if conversion failed
        """
        if not self.is_ffmpeg_available():
            self.logger.error("FFmpeg not available, cannot convert audio")
            return None
        
        input_path = Path(input_path)
        if not input_path.exists():
            self.logger.error(f"Input file not found: {input_path}")
            return None
        
        # Generate output path if not specified
        if output_path is None:
            ext = self.get_output_extension(target_format)
            output_path = input_path.with_suffix(ext)
        else:
            output_path = Path(output_path)
        
        # Build FFmpeg command
        cmd = ["ffmpeg", "-i", str(input_path), "-y"]  # -y to overwrite
        
        # Add sample rate conversion if specified
        if sample_rate:
            cmd.extend(["-ar", str(sample_rate)])
        
        # Format-specific encoding options
        target_format = target_format.lower()
        
        if target_format == "mp3":
            cmd.extend(["-codec:a", "libmp3lame", "-b:a", f"{bitrate}k"])
        
        elif target_format == "aac":
            cmd.extend(["-codec:a", "aac", "-b:a", f"{bitrate}k"])
        
        elif target_format == "ogg":
            cmd.extend(["-codec:a", "libvorbis", "-b:a", f"{bitrate}k"])
        
        elif target_format == "opus":
            cmd.extend(["-codec:a", "libopus", "-b:a", f"{bitrate}k"])
        
        elif target_format == "wav":
            if bit_depth == 16:
                cmd.extend(["-codec:a", "pcm_s16le"])
            elif bit_depth == 24:
                cmd.extend(["-codec:a", "pcm_s24le"])
            elif bit_depth == 32:
                cmd.extend(["-codec:a", "pcm_s32le"])
            else:
                cmd.extend(["-codec:a", "pcm_s24le"])  # Default to 24-bit
        
        elif target_format == "alac":
            cmd.extend(["-codec:a", "alac"])
        
        elif target_format == "flac":
            # FLAC options
            if bit_depth:
                cmd.extend(["-sample_fmt", f"s{bit_depth}"])
            cmd.extend(["-codec:a", "flac", "-compression_level", "8"])
        
        else:
            self.logger.error(f"Unsupported format: {target_format}")
            return None
        
        cmd.append(str(output_path))
        
        self.logger.info(f"Converting {input_path.name} to {target_format}...")
        self.logger.debug(f"FFmpeg command: {' '.join(cmd)}")
        
        try:
            process = await asyncio.create_subprocess_exec(
                *cmd,
                stdout=asyncio.subprocess.PIPE,
                stderr=asyncio.subprocess.PIPE
            )
            
            stdout, stderr = await process.communicate()
            
            if process.returncode == 0:
                self.logger.info(f"Successfully converted to: {output_path}")
                return output_path
            else:
                self.logger.error(f"FFmpeg conversion failed: {stderr.decode()}")
                return None
                
        except Exception as e:
            self.logger.error(f"Conversion error: {e}")
            return None
    
    async def convert_and_replace(
        self,
        file_path: Path,
        target_format: str,
        sample_rate: Optional[int] = None,
        bit_depth: Optional[int] = None,
        bitrate: int = 320
    ) -> Optional[Path]:
        """
        Convert file and replace original with converted version.
        
        Returns the new file path (which may have different extension).
        """
        file_path = Path(file_path)
        
        # Skip if already in target format
        current_ext = file_path.suffix.lower().lstrip('.')
        if current_ext == target_format.lower():
            self.logger.debug(f"File already in {target_format} format, skipping conversion")
            return file_path
        
        # Convert to temp file
        temp_output = file_path.with_suffix(f".converting{self.get_output_extension(target_format)}")
        
        result = await self.convert(
            input_path=file_path,
            output_path=temp_output,
            target_format=target_format,
            sample_rate=sample_rate,
            bit_depth=bit_depth,
            bitrate=bitrate
        )
        
        if result and result.exists():
            # Remove original
            try:
                file_path.unlink()
            except Exception as e:
                self.logger.warning(f"Could not remove original file: {e}")
            
            # Rename temp to final
            final_path = file_path.with_suffix(self.get_output_extension(target_format))
            try:
                result.rename(final_path)
                return final_path
            except Exception as e:
                self.logger.error(f"Could not rename converted file: {e}")
                return result
        
        return None


# Singleton instance for easy access
_converter: Optional[AudioConverter] = None

def get_audio_converter() -> AudioConverter:
    """Get the global AudioConverter instance."""
    global _converter
    if _converter is None:
        _converter = AudioConverter()
    return _converter

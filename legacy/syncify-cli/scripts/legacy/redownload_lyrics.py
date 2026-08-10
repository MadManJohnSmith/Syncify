"""Re-download lyrics for existing audio files using Apple Music."""
import asyncio
import sys
import os
from pathlib import Path
from mutagen import File as MutagenFile

sys.path.insert(0, 'c:/Users/madma/Documents/Syncify')
from services.lyrics_service import LyricsService

APPLE_TOKEN = os.getenv('APPLE_MUSIC_DEV_TOKEN', 'YOUR_APPLE_MUSIC_DEV_TOKEN')

DESKTOP = Path(os.environ['USERPROFILE']) / 'Desktop'

def get_track_info(audio_path: Path) -> tuple:
    """Extract track name and artist from audio file metadata."""
    try:
        audio = MutagenFile(audio_path, easy=True)
        if audio:
            title = audio.get('title', [audio_path.stem])[0]
            artist = audio.get('artist', ['Unknown'])[0]
            return title, artist
    except:
        pass
    # Fallback: parse filename
    name = audio_path.stem
    if ' - ' in name:
        parts = name.split(' - ', 1)
        return parts[1], parts[0]
    return name, 'Unknown'

async def redownload_lyrics():
    service = LyricsService(apple_music_token=APPLE_TOKEN, verbose=False)
    
    # Find all audio files
    audio_extensions = ['.flac', '.mp3', '.m4a', '.wav', '.ogg']
    audio_files = []
    for ext in audio_extensions:
        audio_files.extend(DESKTOP.rglob(f'*{ext}'))
    
    print(f'Found {len(audio_files)} audio files on Desktop')
    print('=' * 70)
    
    stats = {
        'total': 0,
        'word_synced': 0,
        'line_synced': 0,
        'skipped': 0,
        'failed': 0,
        'sources': {}
    }
    
    for audio_path in audio_files:
        stats['total'] += 1
        lrc_path = audio_path.with_suffix('.lrc')
        
        # Check if already word-synced
        if lrc_path.exists():
            content = lrc_path.read_text(encoding='utf-8', errors='ignore')
            if '<' in content and '>' in content:
                stats['skipped'] += 1
                stats['word_synced'] += 1
                print(f"SKIP (word) | {audio_path.name[:50]}")
                continue
        
        # Get track info
        title, artist = get_track_info(audio_path)
        
        # Fetch new lyrics
        result = await service.get_lyrics(title, artist)
        
        if result.synced_lyrics:
            # Save LRC
            lrc_path.write_text(result.synced_lyrics, encoding='utf-8')
            
            if result.word_synced:
                stats['word_synced'] += 1
                status = 'WORD'
            else:
                stats['line_synced'] += 1
                status = 'LINE'
            
            stats['sources'][result.source] = stats['sources'].get(result.source, 0) + 1
            print(f"{status:4} | {result.source:12} | {artist[:15]:15} - {title[:30]}")
        else:
            stats['failed'] += 1
            print(f"FAIL | {'':12} | {artist[:15]:15} - {title[:30]}")
    
    await service.close()
    
    print()
    print('=' * 70)
    print('SUMMARY')
    print('=' * 70)
    print(f"Total tracks: {stats['total']}")
    print(f"Word-synced: {stats['word_synced']} ({int(stats['word_synced']/stats['total']*100)}%)")
    print(f"Line-synced: {stats['line_synced']}")
    print(f"Skipped (already word): {stats['skipped']}")
    print(f"Failed: {stats['failed']}")
    print()
    print('Sources:')
    for source, count in sorted(stats['sources'].items(), key=lambda x: -x[1]):
        print(f"  {source}: {count}")

if __name__ == '__main__':
    asyncio.run(redownload_lyrics())

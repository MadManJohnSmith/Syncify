"""Force re-download ALL lyrics to apply word spacing fix."""
import asyncio
import sys
import os
from pathlib import Path
from mutagen import File as MutagenFile

sys.path.insert(0, 'c:/Users/madma/Documents/Syncify')
from services.lyrics_service import LyricsService

APPLE_TOKEN = os.getenv('APPLE_MUSIC_DEV_TOKEN', 'YOUR_APPLE_MUSIC_DEV_TOKEN')
DESKTOP = Path(os.environ['USERPROFILE']) / 'Desktop'

def get_track_info(audio_path):
    try:
        audio = MutagenFile(audio_path, easy=True)
        if audio:
            title = audio.get('title', [audio_path.stem])[0]
            artist = audio.get('artist', ['Unknown'])[0]
            return title, artist
    except:
        pass
    name = audio_path.stem
    if ' - ' in name:
        parts = name.split(' - ', 1)
        return parts[1], parts[0]
    return name, 'Unknown'

async def redownload_all():
    service = LyricsService(apple_music_token=APPLE_TOKEN, verbose=False)
    audio_files = list(DESKTOP.rglob('*.flac')) + list(DESKTOP.rglob('*.mp3')) + list(DESKTOP.rglob('*.m4a'))
    
    total = len(audio_files)
    print(f'Force re-downloading {total} files (overwriting existing)...')
    print('=' * 70)
    
    stats = {'word': 0, 'line': 0, 'fail': 0}
    
    for i, audio_path in enumerate(audio_files):
        title, artist = get_track_info(audio_path)
        result = await service.get_lyrics(title, artist)
        
        if result.synced_lyrics:
            lrc_path = audio_path.with_suffix('.lrc')
            lrc_path.write_text(result.synced_lyrics, encoding='utf-8')
            if result.word_synced:
                stats['word'] += 1
                status = 'WORD'
            else:
                stats['line'] += 1
                status = 'LINE'
            print(f"[{i+1}/{total}] {status} | {artist[:15]:15} - {title[:30]}")
        else:
            stats['fail'] += 1
            print(f"[{i+1}/{total}] FAIL | {artist[:15]:15} - {title[:30]}")
    
    print()
    print('=' * 70)
    print('SUMMARY')
    print('=' * 70)
    word_pct = int(stats['word'] / total * 100) if total > 0 else 0
    print(f"Word-synced: {stats['word']} ({word_pct}%)")
    print(f"Line-synced: {stats['line']}")
    print(f"Failed: {stats['fail']}")
    await service.close()

if __name__ == '__main__':
    asyncio.run(redownload_all())

"""Test script for multi-source lyrics service with Apple Music."""
import asyncio
import sys
sys.path.insert(0, '.')

from services.lyrics_service import LyricsService

APPLE_TOKEN = os.getenv('APPLE_MUSIC_DEV_TOKEN', 'YOUR_APPLE_MUSIC_DEV_TOKEN')

# 30 diverse test tracks
test_tracks = [
    ('Bohemian Rhapsody', 'Queen'),
    ('Blinding Lights', 'The Weeknd'),
    ('Smells Like Teen Spirit', 'Nirvana'),
    ('Bad Guy', 'Billie Eilish'),
    ('Shape of You', 'Ed Sheeran'),
    ('Uptown Funk', 'Mark Ronson'),
    ('Rolling in the Deep', 'Adele'),
    ('Levitating', 'Dua Lipa'),
    ('Somebody That I Used to Know', 'Gotye'),
    ('Get Lucky', 'Daft Punk'),
    ('Stronger', 'Kanye West'),
    ('Hotline Bling', 'Drake'),
    ('Lose Yourself', 'Eminem'),
    ('Humble', 'Kendrick Lamar'),
    ('Sicko Mode', 'Travis Scott'),
    ('Despacito', 'Luis Fonsi'),
    ('Gangnam Style', 'PSY'),
    ('Waka Waka', 'Shakira'),
    ('Con Calma', 'Daddy Yankee'),
    ('La Bamba', 'Ritchie Valens'),
    ('Stairway to Heaven', 'Led Zeppelin'),
    ('Hotel California', 'Eagles'),
    ('Sweet Child O Mine', 'Guns N Roses'),
    ('Comfortably Numb', 'Pink Floyd'),
    ('Radioactive', 'Imagine Dragons'),
    ('Mr. Brightside', 'The Killers'),
    ('Take Me Out', 'Franz Ferdinand'),
    ('Pumped Up Kicks', 'Foster the People'),
    ('Some Obscure Track', 'Unknown Artist'),
    ('Another Unknown Song', 'Fake Band'),
]

async def test_lyrics():
    service = LyricsService(apple_music_token=APPLE_TOKEN, verbose=False)
    
    results = {
        'total': 0, 'found': 0, 'synced': 0, 'plain': 0, 'word_synced': 0,
        'instrumental': 0, 'not_found': 0, 'sources': {}
    }
    
    print('Testing lyrics service with Apple Music priority (30 songs)...')
    print('Priority: Apple Music (word-synced) -> syncedlyrics (line-synced)')
    print('=' * 75)
    
    for track, artist in test_tracks:
        results['total'] += 1
        result = await service.get_lyrics(track, artist)
        
        if result.synced_lyrics:
            results['found'] += 1
            results['synced'] += 1
            if result.word_synced:
                results['word_synced'] += 1
            status = 'SYNCED'
        elif result.plain_lyrics:
            results['found'] += 1
            results['plain'] += 1
            status = 'PLAIN'
        elif result.instrumental:
            results['instrumental'] += 1
            status = 'INSTRUMENTAL'
        else:
            results['not_found'] += 1
            status = 'NOT FOUND'
        
        source = result.source
        results['sources'][source] = results['sources'].get(source, 0) + 1
        
        word_sync = ' [word]' if result.word_synced else ''
        print(f"{status:12} | {source:14} | {artist[:20]:20} - {track[:25]}{word_sync}")
    
    await service.close()
    
    pct = int(results['found']/results['total']*100) if results['total'] > 0 else 0
    print()
    print('=' * 75)
    print('SUMMARY')
    print('=' * 75)
    print(f"Total tracks tested: {results['total']}")
    print(f"Found lyrics: {results['found']} ({pct}%)")
    print(f"  - Synced (LRC): {results['synced']}")
    print(f"    - Word-synced: {results['word_synced']}")
    print(f"    - Line-synced: {results['synced'] - results['word_synced']}")
    print(f"  - Plain (TXT): {results['plain']}")
    print(f"Instrumental: {results['instrumental']}")
    print(f"Not found: {results['not_found']}")
    print()
    print('Sources breakdown:')
    for source, count in sorted(results['sources'].items(), key=lambda x: -x[1]):
        pct_src = int(count/results['total']*100)
        print(f"  {source}: {count} tracks ({pct_src}%)")

if __name__ == '__main__':
    asyncio.run(test_lyrics())

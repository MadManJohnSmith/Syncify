import sqlite3

conn = sqlite3.connect(r'src-tauri/data/syncify.db')
c = conn.cursor()

print("=== DETAILED ORPHAN ANALYSIS ===\n")

# Get sample orphan track
c.execute('''
SELECT t.id, t.title, t.isrc
FROM tracks t 
LEFT JOIN track_artists ta ON t.id = ta.track_id 
WHERE ta.track_id IS NULL 
LIMIT 5
''')
orphans = c.fetchall()

print("Sample orphan tracks:")
for o in orphans:
    print(f"  Track ID {o[0]}: {o[1]}")
    print(f"    ISRC: {o[2]}")
    
    # Check for track_sources
    c.execute('SELECT service_track_id FROM track_sources WHERE track_id = ?', (o[0],))
    src = c.fetchone()
    print(f"    Spotify ID: {src[0] if src else 'NONE'}")
    
    # Check if there are ANY existing track_artists for this track
    c.execute('SELECT COUNT(*) FROM track_artists WHERE track_id = ?', (o[0],))
    count = c.fetchone()[0]
    print(f"    Track-Artist links: {count}")
    print()

# Check for tracks that might have the same primary key collision
print("\n=== PRIMARY KEY ANALYSIS ===")
c.execute('''
SELECT track_id, artist_id, role, COUNT(*) 
FROM track_artists 
GROUP BY track_id, artist_id, role 
HAVING COUNT(*) > 1
''')
dupes = c.fetchall()
print(f"Duplicate PK entries: {len(dupes)}")

# Check how many artists have 0 links
c.execute('''
SELECT COUNT(*) 
FROM artists a 
LEFT JOIN track_artists ta ON a.id = ta.artist_id 
WHERE ta.artist_id IS NULL
''')
unlinked_artists = c.fetchone()[0]
print(f"Artists with 0 track links: {unlinked_artists}")

conn.close()

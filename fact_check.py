import sqlite3
import os

db_path = os.path.join("src-tauri", "data", "syncify.db")

def check_track(title_query):
    try:
        conn = sqlite3.connect(db_path)
        conn.row_factory = sqlite3.Row
        cursor = conn.cursor()
        
        # Select all relevant fields
        query = """
        SELECT 
            t.title, 
            t.release_year, 
            t.genre, 
            t.isrc, 
            t.musicbrainz_id,
            al.title as album_name,
            al.cover_art_url,
            (SELECT COUNT(*) FROM track_artists WHERE track_id = t.id) as artist_count
        FROM tracks t
        LEFT JOIN albums al ON t.album_id = al.id
        WHERE t.title LIKE ?
        """
        
        cursor.execute(query, (f"%{title_query}%",))
        rows = cursor.fetchall()
        
        print(f"--- Fact Check for '{title_query}' ---")
        for row in rows:
            print(f"Title: {row['title']}")
            print(f"Album: {row['album_name']}")
            print(f"ISRC: {row['isrc']}")
            print(f"MBID: {row['musicbrainz_id']}")
            print(f"Art: {row['cover_art_url']}")
            print(f"Year: {row['release_year']}")
            print(f"Genre: {row['genre']}")
            print(f"Artist Count: {row['artist_count']}")
            
            # Re-calculate Score (Backend Logic)
            score = 0
            score += 10 if row['title'] else 0
            score += 10 if row['artist_count'] > 0 else 0
            score += 10 if row['album_name'] else 0
            score += 20 if row['isrc'] else 0
            score += 20 if row['musicbrainz_id'] else 0
            score += 10 if row['cover_art_url'] else 0
            score += 10 if row['release_year'] and row['release_year'] > 0 else 0
            score += 10 if row['genre'] else 0
            
            print(f"Calculated Score: {score}%")
            
            # Re-calculate Issues (Frontend Logic)
            issues = 0
            if not row['album_name']: issues += 1
            if not row['isrc']: issues += 1
            if not row['musicbrainz_id']: issues += 1
            if not row['cover_art_url']: issues += 1
            if not row['release_year']: issues += 1
            if not row['genre']: issues += 1
            
            print(f"Calculated Issues: {issues}")
            print("-" * 30)
            
        conn.close()
    except Exception as e:
        print(f"Error: {e}")

check_track("#1 Crush")
check_track("Adia")
check_track("Heroes")

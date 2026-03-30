-- Migration 0019: Robust Library Search Index
-- Replaces limited tracks_fts with comprehensive library_fts (Title + Artist + Album)
-- Includes triggers for automatic synchronization

-- 1. Clean up old FTS tables and triggers
DROP TABLE IF EXISTS tracks_fts;
DROP TRIGGER IF EXISTS tracks_fts_insert;
DROP TRIGGER IF EXISTS tracks_fts_delete;
DROP TRIGGER IF EXISTS tracks_fts_update;

DROP TABLE IF EXISTS artists_fts;
DROP TRIGGER IF EXISTS artists_fts_insert;
DROP TRIGGER IF EXISTS artists_fts_delete;
DROP TRIGGER IF EXISTS artists_fts_update;

-- 2. Create new FTS5 table
-- We use a standard FTS table (managed by triggers) rather than content-less or external content
-- to ensure reliability and performance. Rowid matches tracks.id.
CREATE VIRTUAL TABLE library_fts USING fts5(
    title,
    artist,
    album,
    tokenize='porter unicode61'
);

-- 3. Populate initial data
INSERT INTO library_fts(rowid, title, artist, album)
SELECT 
    t.id, 
    t.title,
    COALESCE(
        (SELECT GROUP_CONCAT(a.name, ' ') 
         FROM track_artists ta 
         JOIN artists a ON ta.artist_id = a.id 
         WHERE ta.track_id = t.id), 
        ''
    ) as artist_names,
    COALESCE(al.title, '') as album_title
FROM tracks t
LEFT JOIN albums al ON t.album_id = al.id;

-- 4. Create Triggers for automatic maintenance

-- 4.1 Tracks Insert
CREATE TRIGGER library_fts_insert AFTER INSERT ON tracks BEGIN
    INSERT INTO library_fts(rowid, title, artist, album)
    VALUES (
        NEW.id,
        NEW.title,
        '', -- No artists linked yet on insert usually
        (SELECT title FROM albums WHERE id = NEW.album_id)
    );
END;

-- 4.2 Tracks Delete
CREATE TRIGGER library_fts_delete AFTER DELETE ON tracks BEGIN
    DELETE FROM library_fts WHERE rowid = OLD.id;
END;

-- 4.3 Tracks Update (Title or Album change)
CREATE TRIGGER library_fts_update AFTER UPDATE OF title, album_id ON tracks BEGIN
    UPDATE library_fts SET 
        title = NEW.title,
        album = (SELECT title FROM albums WHERE id = NEW.album_id)
    WHERE rowid = NEW.id;
END;

-- 4.4 Track Artists Changed (Link added) -> Update Artist String
CREATE TRIGGER library_fts_artist_link_insert AFTER INSERT ON track_artists BEGIN
    UPDATE library_fts SET 
        artist = (
            SELECT GROUP_CONCAT(a.name, ' ') 
            FROM track_artists ta 
            JOIN artists a ON ta.artist_id = a.id 
            WHERE ta.track_id = NEW.track_id
        )
    WHERE rowid = NEW.track_id;
END;

-- 4.5 Track Artists Changed (Link removed) -> Update Artist String
CREATE TRIGGER library_fts_artist_link_delete AFTER DELETE ON track_artists BEGIN
    UPDATE library_fts SET 
        artist = (
            SELECT GROUP_CONCAT(a.name, ' ') 
            FROM track_artists ta 
            JOIN artists a ON ta.artist_id = a.id 
            WHERE ta.track_id = OLD.track_id
        )
    WHERE rowid = OLD.track_id;
END;

-- 4.6 Album Title Updated -> Update all tracks in that album
CREATE TRIGGER library_fts_album_update AFTER UPDATE OF title ON albums BEGIN
    UPDATE library_fts SET album = NEW.title 
    WHERE rowid IN (SELECT id FROM tracks WHERE album_id = NEW.id);
END;

-- 4.7 Artist Name Updated -> Update all tracks by that artist
-- This is heavier but ensures consistency
CREATE TRIGGER library_fts_artist_update AFTER UPDATE OF name ON artists BEGIN
    UPDATE library_fts 
    SET artist = (
        SELECT GROUP_CONCAT(a.name, ' ') 
        FROM track_artists ta 
        JOIN artists a ON ta.artist_id = a.id 
        WHERE ta.track_id = library_fts.rowid
    )
    WHERE rowid IN (
        SELECT track_id FROM track_artists WHERE artist_id = NEW.id
    );
END;

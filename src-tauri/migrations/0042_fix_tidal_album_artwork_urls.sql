-- 0042_fix_tidal_album_artwork_urls.sql
UPDATE albums
SET cover_art_url = 
  'https://resources.tidal.com/images/' ||
  REPLACE(cover_art_url, '-', '/') ||
  '/320x320.jpg'
WHERE tidal_id IS NOT NULL
  AND cover_art_url IS NOT NULL
  AND cover_art_url NOT LIKE 'http%';
